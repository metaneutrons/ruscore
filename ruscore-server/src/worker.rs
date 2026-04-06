//! Background worker — processes queued jobs sequentially.

use crate::state::AppState;
use std::sync::Arc;
use tokio::sync::Notify;
use tracing::{error, info};

/// Run the background worker loop. Processes one job at a time.
pub async fn run(state: AppState, notify: Arc<Notify>) {
    info!("Worker started.");

    loop {
        tokio::select! {
            _ = notify.notified() => {},
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {},
        }

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

            match process_job(&job.url).await {
                Ok((metadata, pages, pdf_bytes)) => {
                    let meta_json = serde_json::to_value(&metadata).unwrap_or_default();
                    if let Err(e) = state
                        .db
                        .complete(job.id, &meta_json, pages as i64, &pdf_bytes)
                    {
                        error!("Failed to store result for job {}: {e}", job.id);
                    }
                    info!(
                        "Job {} completed: {} ({} pages, {} bytes)",
                        job.id,
                        metadata.title,
                        pages,
                        pdf_bytes.len()
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

/// Scrape SVGs, generate PDF, return (metadata, page_count, pdf_bytes).
async fn process_job(url: &str) -> anyhow::Result<(ruscore_core::ScoreMetadata, usize, Vec<u8>)> {
    let mut chrome = ruscore_core::chrome::Chrome::start().await?;
    let (pages, metadata) = ruscore_core::scraper::scrape(&mut chrome.session, url).await?;
    let page_count = pages.len();

    let tmp = tempfile::NamedTempFile::new()?;
    ruscore_core::pdf::generate(&pages, tmp.path())?;
    let pdf_bytes = std::fs::read(tmp.path())?;

    chrome.shutdown();
    Ok((metadata, page_count, pdf_bytes))
}
