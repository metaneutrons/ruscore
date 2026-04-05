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

    info!("Waiting for page to load...");
    sleep(Duration::from_secs(5)).await;

    info!("Waiting for score to appear...");
    for _ in 0..120 {
        if session
            .evaluate_bool("!!document.querySelector(\"img[src*='score_']\")")
            .await?
        {
            break;
        }
        sleep(Duration::from_millis(500)).await;
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

    let step = 300;
    let mut pos = 0i64;
    let total = height as i64;

    while pos < total {
        session
            .evaluate(&format!(
                r#"(() => {{
                    let el = document.querySelector("img[src*='score_0.svg']");
                    while (el && el !== document.body) {{
                        if (el.scrollHeight > el.clientHeight + 10) {{ el.scrollTop = {pos}; return; }}
                        el = el.parentElement;
                    }}
                }})()"#
            ))
            .await?;
        pos += step;

        // Drain any events that arrived during this scroll step
        drain_svg_events(session, &svg_re, &mut svg_requests);
        sleep(Duration::from_millis(250)).await;
    }

    // Wait for remaining SVGs to load
    for _ in 0..20 {
        sleep(Duration::from_millis(500)).await;
        drain_svg_events(session, &svg_re, &mut svg_requests);
        if svg_requests.len() >= total_pages {
            break;
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
    while let Ok((method, params)) = session.events.try_recv() {
        if method != "Network.responseReceived" {
            continue;
        }
        let Some(url) = params
            .get("response")
            .and_then(|r| r.get("url"))
            .and_then(|u| u.as_str())
        else {
            continue;
        };
        let Some(caps) = svg_re.captures(url) else {
            continue;
        };
        let Some(idx) = caps.get(1).and_then(|m| m.as_str().parse::<usize>().ok()) else {
            continue;
        };
        let Some(req_id) = params.get("requestId").and_then(|v| v.as_str()) else {
            continue;
        };
        debug!("  Captured score_{idx}.svg (request {req_id})");
        svg_requests.insert(idx, req_id.to_string());
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
