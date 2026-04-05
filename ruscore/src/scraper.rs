//! MuseScore scraper — navigate, scroll, capture SVGs via CDP network interception.

use anyhow::{Result, bail};
use regex::Regex;
use std::collections::BTreeMap;
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn};

use crate::cdp::CdpSession;

/// Scraped score: page index → SVG bytes, sorted by page number.
pub type ScorePages = BTreeMap<usize, Vec<u8>>;

/// Scrape all score SVG pages from a MuseScore URL.
pub async fn scrape(session: &mut CdpSession, url: &str) -> Result<ScorePages> {
    let svg_re = Regex::new(r"score_(\d+)\.svg")?;

    info!("Navigating to {url}");
    session.navigate(url).await?;

    // Wait for the score viewer to fully render (React hydration + lazy init)
    info!("Waiting for score viewer to load...");
    for i in 0..60 {
        sleep(Duration::from_secs(1)).await;
        // Check if the scrollable container has children (score pages)
        let children = session
            .evaluate_f64(
                r#"(() => {
                    const img = document.querySelector("img[src*='score_']");
                    if (!img) return 0;
                    let el = img;
                    while (el && el !== document.body) {
                        if (el.scrollHeight > el.clientHeight + 10) return el.children.length;
                        el = el.parentElement;
                    }
                    return 0;
                })()"#,
            )
            .await
            .unwrap_or(0.0);

        if children > 1.0 {
            debug!(
                "Score viewer ready after {}s ({} children)",
                i + 1,
                children as usize
            );
            break;
        }
    }

    let total_pages = extract_page_count(session).await?;
    info!("Score has {total_pages} pages.");

    // Scroll and collect SVG network responses simultaneously
    info!("Scrolling score viewer and capturing SVGs...");
    let mut svg_requests: BTreeMap<usize, String> = BTreeMap::new();

    let height = session
        .evaluate_f64(
            r#"(() => {
                let el = document.querySelector("img[src*='score_0.svg']");
                while (el && el !== document.body) {
                    if (el.scrollHeight > el.clientHeight + 10) return el.scrollHeight;
                    el = el.parentElement;
                }
                return 0;
            })()"#,
        )
        .await?;

    debug!("Scroll height: {height}px");

    // Fire-and-forget: start the scroll loop inside the browser
    // Don't await the promise — it may not resolve if React re-renders
    session
        .send(
            "Runtime.evaluate",
            serde_json::json!({
                "expression": r#"(async () => {
                    let el = document.querySelector("img[src*='score_0.svg']");
                    while (el && el !== document.body) {
                        if (el.scrollHeight > el.clientHeight + 10) break;
                        el = el.parentElement;
                    }
                    if (!el) return;
                    for (let pos = 0; pos < el.scrollHeight; pos += 300) {
                        el.scrollTop = pos;
                        await new Promise(r => setTimeout(r, 300));
                    }
                    el.scrollTop = el.scrollHeight;
                })()"#,
                "returnByValue": true,
                "awaitPromise": false
            }),
        )
        .await?;

    // Wait for the scroll to complete and SVGs to load, draining events
    let scroll_time = (height / 300.0 * 0.3) as u64 + 10;
    info!("Waiting ~{scroll_time}s for scroll + lazy loading...");

    // Wait for remaining SVGs to load
    for i in 0..60 {
        sleep(Duration::from_secs(1)).await;
        drain_svg_events(session, &svg_re, &mut svg_requests);
        if svg_requests.len() >= total_pages {
            info!("All {total_pages} pages captured!");
            break;
        }
        if i % 5 == 4 {
            debug!(
                "  {}/{total_pages} captured, waiting...",
                svg_requests.len()
            );
        }
    }

    info!(
        "Found {} SVG responses. Fetching bodies...",
        svg_requests.len()
    );

    let mut result = BTreeMap::new();
    for (&idx, req_id) in &svg_requests {
        match session.get_response_body(req_id).await {
            Ok(bytes) if !bytes.is_empty() => {
                info!("  Page {}: {} bytes", idx + 1, bytes.len());
                result.insert(idx, bytes);
            }
            Ok(_) => warn!("  Page {}: empty body", idx + 1),
            Err(e) => warn!("  Page {}: {e}", idx + 1),
        }
    }

    if result.is_empty() {
        bail!("no SVGs captured");
    }

    info!("Captured {}/{total_pages} SVGs.", result.len());
    Ok(result)
}

/// Drain pending events and extract SVG response request IDs.
fn drain_svg_events(
    session: &mut CdpSession,
    svg_re: &Regex,
    svg_requests: &mut BTreeMap<usize, String>,
) {
    let mut event_count = 0;
    let mut network_count = 0;
    while let Ok((method, params)) = session.events.try_recv() {
        event_count += 1;
        if method == "Network.responseReceived" {
            network_count += 1;
            let Some(url) = params
                .get("response")
                .and_then(|r| r.get("url"))
                .and_then(|u| u.as_str())
            else {
                continue;
            };
            if url.contains("score_") {
                debug!(
                    "  Network response: ...{}",
                    &url[url.len().saturating_sub(60)..]
                );
            }
            let Some(caps) = svg_re.captures(url) else {
                continue;
            };
            let Some(idx) = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok()) else {
                continue;
            };
            let Some(req_id) = params.get("requestId").and_then(|v| v.as_str()) else {
                continue;
            };
            info!("  Captured score_{idx}.svg (request {req_id})");
            svg_requests.insert(idx, req_id.to_string());
        }
    }
    if event_count > 0 {
        debug!("  Drained {event_count} events ({network_count} network responses)");
    }
}

async fn extract_page_count(session: &CdpSession) -> Result<usize> {
    let alt = session
        .evaluate_string("document.querySelector(\"img[src*='score_'][src*='.svg']\")?.alt || ''")
        .await?;

    let re = Regex::new(r"(\d+)\s+of\s+(\d+)\s+pages?")?;
    if let Some(caps) = re.captures(&alt) {
        return Ok(caps[2].parse::<usize>().unwrap_or(1));
    }
    warn!("Could not parse page count from: {alt:?}");
    Ok(1)
}
