//! Redis PDF cache.

use anyhow::{Context, Result};
use fred::prelude::*;
use tracing::info;

/// TTL for cached PDFs (24 hours).
const PDF_TTL_SECS: i64 = 86400;

/// Redis-backed PDF blob cache.
pub struct PdfCache {
    client: Client,
}

impl PdfCache {
    /// Connect to Redis.
    pub async fn connect(url: &str) -> Result<Self> {
        let config = Config::from_url(url).context("invalid Redis URL")?;
        let client = Client::new(config, None, None, None);
        client.init().await.context("failed to connect to Redis")?;
        info!("Redis connected.");
        Ok(Self { client })
    }

    /// Get a cached PDF by URL hash. Returns None on miss.
    pub async fn get(&self, url_hash: &str) -> Result<Option<Vec<u8>>> {
        let key = format!("pdf:{url_hash}");
        let val: Option<Vec<u8>> = self.client.get(&key).await?;
        Ok(val)
    }

    /// Store a PDF with TTL.
    pub async fn set(&self, url_hash: &str, pdf_bytes: &[u8]) -> Result<()> {
        let key = format!("pdf:{url_hash}");
        self.client
            .set::<(), _, _>(
                &key,
                pdf_bytes.to_vec(),
                Some(Expiration::EX(PDF_TTL_SECS)),
                None,
                false,
            )
            .await?;
        Ok(())
    }

    /// Check if a PDF is cached.
    pub async fn exists(&self, url_hash: &str) -> Result<bool> {
        let key = format!("pdf:{url_hash}");
        let exists: bool = self.client.exists(&key).await?;
        Ok(exists)
    }
}
