//! Background worker — processes queued jobs with persistent Chrome, retry, and recycling.

use crate::state::AppState;
use ruscore_core::chrome::Chrome;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::time::{Duration, sleep, timeout};
use tracing::{error, info, warn};

const MAX_RETRIES: usize = 3;
const RETRY_BACKOFF_SECS: u64 = 5;
const JOB_TIMEOUT_SECS: u64 = 300; // 5 minutes per job
const CHROME_RECYCLE_AFTER: usize = 50; // Restart Chrome every N jobs

/// Run the background worker loop. Maintains a persistent Chrome instance.
pub async fn run(state: AppState, notify: Arc<Notify>) {
    info!("Worker started.");

    let mut chrome: Option<Chrome> = None;
    let mut jobs_since_recycle: usize = 0;

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

            // Proactive Chrome recycling to prevent memory leaks
            if jobs_since_recycle >= CHROME_RECYCLE_AFTER {
                info!("Recycling Chrome after {jobs_since_recycle} jobs.");
                if let Some(mut c) = chrome.take() {
                    c.shutdown();
                }
                jobs_since_recycle = 0;
            }

            let mut last_err = String::new();
            let mut succeeded = false;

            for attempt in 1..=MAX_RETRIES {
                // Ensure Chrome is running
                if chrome.is_none() {
                    match Chrome::start().await {
                        Ok(c) => {
                            chrome = Some(c);
                            jobs_since_recycle = 0;
                        }
                        Err(e) => {
                            last_err = format!("Chrome failed to start: {e:#}");
                            error!("{last_err}");
                            sleep(Duration::from_secs(RETRY_BACKOFF_SECS)).await;
                            continue;
                        }
                    }
                }

                let session = &mut chrome.as_mut().unwrap().session;

                // Per-job timeout to prevent hung jobs from blocking the queue
                match timeout(
                    Duration::from_secs(JOB_TIMEOUT_SECS),
                    process_job(session, &job.url),
                )
                .await
                {
                    Ok(Ok((metadata, pages, pdf_bytes))) => {
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
                        jobs_since_recycle += 1;
                        break;
                    }
                    Ok(Err(e)) => {
                        last_err = format!("{e:#}");
                        if should_retry(&last_err) {
                            warn!(
                                job_id = %job.id,
                                attempt,
                                error = %last_err,
                                "Retryable failure, restarting Chrome..."
                            );
                            if let Some(mut c) = chrome.take() {
                                c.shutdown();
                            }
                            sleep(Duration::from_secs(RETRY_BACKOFF_SECS * attempt as u64)).await;
                            continue;
                        }
                        break; // Non-retryable
                    }
                    Err(_) => {
                        last_err = format!(
                            "Job timed out after {JOB_TIMEOUT_SECS}s — Chrome may be stuck"
                        );
                        warn!("Job {} {last_err}, killing Chrome...", job.id);
                        if let Some(mut c) = chrome.take() {
                            c.shutdown();
                        }
                        if attempt < MAX_RETRIES {
                            sleep(Duration::from_secs(RETRY_BACKOFF_SECS * attempt as u64)).await;
                            continue;
                        }
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

/// Determine if an error is retryable (Cloudflare, Chrome crash, network).
fn should_retry(err: &str) -> bool {
    err.contains("Cloudflare")
        || err.contains("did not load")
        || err.contains("WS send failed")
        || err.contains("CDP")
        || err.contains("timed out")
        || err.contains("connection")
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
