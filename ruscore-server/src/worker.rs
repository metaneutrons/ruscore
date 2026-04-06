//! Background worker — processes queued jobs sequentially with persistent Chrome.

use crate::state::AppState;
use ruscore_core::chrome::Chrome;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, sleep};
use tracing::{error, info, warn};

const MAX_RETRIES: usize = 3;
const RETRY_BACKOFF_SECS: u64 = 5;

/// Run the background worker loop. Maintains a persistent Chrome instance.
pub async fn run(state: AppState, notify: Arc<Notify>) {
    info!("Worker started.");

    let mut chrome: Option<Chrome> = None;

    loop {
        tokio::select! {
            _ = notify.notified() => {},
            _ = sleep(Duration::from_secs(5)) => {},
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

            let mut last_err = String::new();
            let mut succeeded = false;

            for attempt in 1..=MAX_RETRIES {
                // Ensure Chrome is running
                if chrome.is_none() {
                    match Chrome::start().await {
                        Ok(c) => chrome = Some(c),
                        Err(e) => {
                            last_err = format!("Chrome failed to start: {e:#}");
                            error!("{last_err}");
                            sleep(Duration::from_secs(RETRY_BACKOFF_SECS)).await;
                            continue;
                        }
                    }
                }

                let session = &mut chrome.as_mut().unwrap().session;

                match process_job(session, &job.url).await {
                    Ok((metadata, pages, pdf_bytes)) => {
                        let meta_json = serde_json::to_value(&metadata).unwrap_or_default();
                        if let Err(e) =
                            state
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
                        succeeded = true;
                        break;
                    }
                    Err(e) => {
                        last_err = format!("{e:#}");
                        let is_cloudflare =
                            last_err.contains("Cloudflare") || last_err.contains("did not load");
                        let is_chrome_dead = last_err.contains("WS send failed")
                            || last_err.contains("CDP")
                            || last_err.contains("Chrome");

                        if is_cloudflare || is_chrome_dead {
                            warn!(
                                "Job {} attempt {attempt}/{MAX_RETRIES} failed ({}), restarting Chrome...",
                                job.id,
                                if is_cloudflare {
                                    "Cloudflare"
                                } else {
                                    "Chrome crashed"
                                }
                            );
                            // Kill Chrome and retry with fresh session
                            if let Some(mut c) = chrome.take() {
                                c.shutdown();
                            }
                            sleep(Duration::from_secs(RETRY_BACKOFF_SECS * attempt as u64)).await;
                            continue;
                        }

                        // Non-retryable error (404, no SVGs, etc.)
                        break;
                    }
                }
            }

            if !succeeded {
                error!("Job {} failed: {last_err}", job.id);
                let _ = state.db.fail(job.id, &last_err);
            }
        }
    }
}

/// Process a single job using an existing Chrome session.
async fn process_job(
    session: &mut ruscore_core::cdp::CdpSession,
    url: &str,
) -> anyhow::Result<(ruscore_core::ScoreMetadata, usize, Vec<u8>)> {
    let (pages, metadata) = ruscore_core::scraper::scrape(session, url).await?;
    let page_count = pages.len();

    let tmp = tempfile::NamedTempFile::new()?;
    ruscore_core::pdf::generate(&pages, tmp.path())?;
    let pdf_bytes = std::fs::read(tmp.path())?;

    Ok((metadata, page_count, pdf_bytes))
}
