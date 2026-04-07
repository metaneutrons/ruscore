//! MuseScore scraper — navigate, scroll, capture SVGs via CDP network interception.

use anyhow::{Result, bail};
use regex::Regex;
use std::collections::BTreeMap;
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn};

use crate::cdp::CdpSession;
use crate::{ScoreMetadata, ScorePages};

/// Scrape all score SVG pages and metadata from a MuseScore URL.
pub async fn scrape(session: &mut CdpSession, url: &str) -> Result<(ScorePages, ScoreMetadata)> {
    let svg_re = Regex::new(r"score_(\d+)\.svg")?;

    info!("Navigating to {url}");
    session.navigate(url).await?;

    // Wait for the score viewer to fully render (React hydration + lazy init)
    // Wait for the score viewer to fully render
    info!("Waiting for score viewer to load...");
    let mut viewer_ready = false;
    for i in 0..60 {
        sleep(Duration::from_secs(1)).await;

        // Check if we're stuck on Cloudflare challenge
        let page_url: String = session
            .evaluate_string("window.location.href")
            .await
            .unwrap_or_default();
        let title: String = session
            .evaluate_string("document.title")
            .await
            .unwrap_or_default();

        if title.contains("Just a moment") || title.contains("Attention Required") {
            if i > 45 {
                bail!("Cloudflare challenge not solved after 45s — page is blocked");
            }
            // Try to auto-click Turnstile checkbox
            if i % 3 == 2 {
                try_solve_turnstile(session).await;
            }
            continue;
        }

        // Check for 404 / error pages
        if title.contains("404") || title.contains("Page not found") {
            bail!("Score page not found (404): {page_url}");
        }

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
            viewer_ready = true;
            break;
        }
    }

    if !viewer_ready {
        bail!("Score viewer did not load after 60s — the page may not contain a playable score");
    }

    let total_pages = extract_page_count(session).await?;
    info!("Score has {total_pages} pages.");

    // Extract metadata from JSON-LD and DOM
    let mut metadata = extract_metadata(session, total_pages).await?;
    info!("Title: {}", metadata.title);

    // Scroll page-by-page, waiting for each SVG to load before advancing
    info!("Scrolling score viewer and capturing SVGs...");
    let mut svg_requests: BTreeMap<usize, String> = BTreeMap::new();

    // Drain initial events (score_0.svg loads with the page)
    sleep(Duration::from_secs(1)).await;
    drain_svg_events(session, &svg_re, &mut svg_requests);

    // Fire-and-forget continuous scroll inside the browser
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

    // Wait for each page's SVG to arrive, with per-page timeout
    for page_idx in 0..total_pages {
        if svg_requests.contains_key(&page_idx) {
            info!("  Page {} ✓ (already loaded)", page_idx + 1);
            continue;
        }

        let mut found = false;
        for _ in 0..20 {
            sleep(Duration::from_millis(500)).await;
            drain_svg_events(session, &svg_re, &mut svg_requests);
            if svg_requests.contains_key(&page_idx) {
                found = true;
                break;
            }
        }

        if found {
            info!("  Page {} ✓", page_idx + 1);
        } else {
            warn!("  Page {} timed out, continuing", page_idx + 1);
        }
    }

    // Final drain for any stragglers
    sleep(Duration::from_secs(2)).await;
    drain_svg_events(session, &svg_re, &mut svg_requests);

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
        bail!(
            "No SVG pages could be downloaded — the score may require a MuseScore subscription or the page structure has changed"
        );
    }

    if result.len() < total_pages {
        let msg = format!(
            "Only {}/{} pages captured. This score likely requires a MuseScore Pro+ subscription to view the full score.",
            result.len(),
            total_pages
        );
        warn!("{msg}");
        metadata.warnings.push(msg);
    }

    info!("Captured {}/{total_pages} SVGs.", result.len());
    Ok((result, metadata))
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

/// Extract score metadata from JSON-LD (MusicComposition) and DOM.
async fn extract_metadata(session: &CdpSession, total_pages: usize) -> Result<ScoreMetadata> {
    let json_str = session
        .evaluate_string(
            r#"(() => {
                const scripts = document.querySelectorAll('script[type="application/ld+json"]');
                const all = [];
                for (const s of scripts) {
                    try { all.push(JSON.parse(s.textContent)); } catch {}
                }
                return JSON.stringify(all);
            })()"#,
        )
        .await?;

    let ld_array: Vec<serde_json::Value> = serde_json::from_str(&json_str).unwrap_or_default();

    // Helper: find first value for a key across all JSON-LD blocks
    let find_str = |key: &str| -> String {
        for ld in &ld_array {
            if let Some(v) = ld.get(key).and_then(|v| v.as_str()) {
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
        String::new()
    };

    let title = find_str("name");
    let d = find_str("text");
    let description = if d.is_empty() {
        find_str("description")
    } else {
        d
    };
    let thumbnail_url = find_str("thumbnailUrl");

    // Composer: check MusicComposition.composer or MusicRecording.byArtist
    let composer = ld_array
        .iter()
        .find_map(|ld| {
            ld.get("composer")
                .and_then(|v| v.get("name").or(Some(v)))
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
        })
        .or_else(|| {
            ld_array.iter().find_map(|ld| {
                ld.get("byArtist")
                    .and_then(|v| v.get("name").or(Some(v)))
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(|s| s.to_string())
            })
        })
        .unwrap_or_default();

    // Arranger + instruments from alt text: "... arranged by X ... for Organ, Trumpet ..."
    let alt = session
        .evaluate_string("document.querySelector(\"img[src*='score_'][src*='.svg']\")?.alt || ''")
        .await?;

    let arranger = Regex::new(r"arranged by ([^.]+?)(?:\s+for\s)")
        .ok()
        .and_then(|re| re.captures(&alt))
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().trim().to_string())
        .unwrap_or_default();

    let instruments = Regex::new(r"\bfor\s+(.+?)(?:\s*[–\-]\s*\d+\s+of)")
        .ok()
        .and_then(|re| re.captures(&alt))
        .and_then(|c| c.get(1))
        .map(|m| {
            m.as_str()
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default();

    // If title is still empty, try parsing from alt: "Title by Artist Sheet Music..."
    let title = if title.is_empty() {
        Regex::new(r"^(.+?)\s+by\s+")
            .ok()
            .and_then(|re| re.captures(&alt))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    } else {
        title
    };

    // If composer is still empty, try from alt: "... by Artist Sheet Music..."
    let composer = if composer.is_empty() {
        Regex::new(r"\bby\s+(.+?)\s+Sheet Music")
            .ok()
            .and_then(|re| re.captures(&alt))
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default()
    } else {
        composer
    };

    // Strip common prefixes from composer (case-insensitive)
    let composer = {
        let lower = composer.to_lowercase();
        let prefixes = ["written by ", "words & music by ", "composed by "];
        let stripped = prefixes
            .iter()
            .find(|p| lower.starts_with(*p))
            .map(|p| composer[p.len()..].to_string());
        stripped.unwrap_or(composer)
    };

    Ok(ScoreMetadata {
        title,
        composer,
        arranger,
        instruments,
        pages: total_pages,
        description,
        thumbnail_url,
        warnings: Vec::new(),
    })
}

/// Attempt to auto-solve Cloudflare Turnstile challenge by clicking the checkbox.
///
/// Turnstile renders inside an iframe. We find the iframe's position on the page
/// and dispatch a mouse click at the checkbox location via CDP Input.dispatchMouseEvent.
async fn try_solve_turnstile(session: &CdpSession) {
    // Find the Turnstile iframe bounding box
    let coords = session
        .evaluate(
            r#"(() => {
                // Turnstile iframe: look for cf-turnstile or challenge iframe
                const selectors = [
                    'iframe[src*="challenges.cloudflare.com"]',
                    'iframe[src*="turnstile"]',
                    '#cf-turnstile iframe',
                    '.cf-turnstile iframe',
                    'iframe[title*="challenge"]',
                ];
                for (const sel of selectors) {
                    const iframe = document.querySelector(sel);
                    if (iframe) {
                        const rect = iframe.getBoundingClientRect();
                        // Checkbox is typically at ~30px from left, centered vertically
                        return [rect.x + 30, rect.y + rect.height / 2];
                    }
                }
                // Fallback: look for any visible challenge container
                const container = document.querySelector('#challenge-stage, #turnstile-wrapper, .cf-turnstile');
                if (container) {
                    const rect = container.getBoundingClientRect();
                    return [rect.x + 30, rect.y + rect.height / 2];
                }
                return null;
            })()"#,
        )
        .await;

    let coords = match coords {
        Ok(v) => v,
        Err(_) => return,
    };

    let arr = match coords.as_array() {
        Some(a) if a.len() == 2 => a,
        _ => return,
    };

    let x = arr[0].as_f64().unwrap_or(0.0);
    let y = arr[1].as_f64().unwrap_or(0.0);

    if x < 1.0 || y < 1.0 {
        return;
    }

    debug!("Attempting Turnstile auto-click at ({x}, {y})");

    // Mouse move → click sequence (mimics real user interaction)
    let _ = session
        .send(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mouseMoved",
                "x": x,
                "y": y,
            }),
        )
        .await;

    tokio::time::sleep(Duration::from_millis(100)).await;

    let _ = session
        .send(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mousePressed",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 1,
            }),
        )
        .await;

    let _ = session
        .send(
            "Input.dispatchMouseEvent",
            serde_json::json!({
                "type": "mouseReleased",
                "x": x,
                "y": y,
                "button": "left",
                "clickCount": 1,
            }),
        )
        .await;

    debug!("Turnstile click dispatched");
}
