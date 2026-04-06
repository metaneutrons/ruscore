//! Background worker — processes queued jobs sequentially.

use crate::state::AppState;
use anyhow::Result;
use std::path::PathBuf;
use tokio::sync::Notify;
use tracing::{error, info};

/// Run the background worker loop. Processes one job at a time.
pub async fn run(state: AppState, data_dir: PathBuf, notify: std::sync::Arc<Notify>) {
    info!("Worker started.");

    loop {
        // Wait for notification of new work (or check periodically)
        tokio::select! {
            _ = notify.notified() => {},
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
        }

        // Process all queued jobs
        loop {
            let job = match state.db.claim_next() {
                Ok(Some(job)) => job,
                Ok(None) => break,
                Err(e) => {
                    error!("Failed to claim job: {e}");
                    break;
                }
            };

            info!("Processing job {} ({})", job.id, job.url);

            match process_job(&state, &job.url, &job.url_hash, &data_dir).await {
                Ok((metadata, pages)) => {
                    let meta_json = serde_json::to_value(&metadata).unwrap_or_default();
                    if let Err(e) = state.db.complete(job.id, &meta_json, pages as i64) {
                        error!("Failed to mark job {} complete: {e}", job.id);
                    }
                    info!(
                        "Job {} completed: {} ({} pages)",
                        job.id, metadata.title, pages
                    );
                }
                Err(e) => {
                    let msg = format!("{e:#}");
                    error!("Job {} failed: {msg}", job.id);
                    let _ = state.db.fail(job.id, &msg);
                }
            }
        }
    }
}

/// Process a single job: scrape SVGs, generate PDF, cache in Redis.
async fn process_job(
    state: &AppState,
    url: &str,
    url_hash: &str,
    _data_dir: &PathBuf,
) -> Result<(ruscore_core::ScoreMetadata, usize)> {
    // Check Redis cache first
    if state.cache.exists(url_hash).await? {
        info!("  Cache hit for {url_hash}, skipping scrape");
        // Still need metadata — re-scrape is the only way to get it
        // For a cache hit we'd need to store metadata separately
        // For now, proceed with scrape anyway (metadata is cheap)
    }

    let mut chrome = ruscore_core::chrome::Chrome::start().await?;
    let (pages, metadata) = ruscore_core::scraper::scrape(&mut chrome.session, url).await?;
    let page_count = pages.len();

    // Generate PDF in memory
    let tmp = tempfile::NamedTempFile::new()?;
    ruscore_core::pdf::generate(&pages, tmp.path())?;
    let pdf_bytes = std::fs::read(tmp.path())?;

    // Cache in Redis
    state.cache.set(url_hash, &pdf_bytes).await?;
    info!("  Cached PDF ({} bytes) as pdf:{url_hash}", pdf_bytes.len());

    chrome.shutdown();
    Ok((metadata, page_count))
}
