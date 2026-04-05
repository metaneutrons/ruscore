//! MuseScore page navigation, score viewer scrolling, and SVG downloading.

use anyhow::{Context, Result, bail};
use chromiumoxide::Browser;
use regex::Regex;
use std::collections::BTreeMap;
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn};

/// Scraped score: page index → SVG bytes, sorted by page number.
pub type ScorePages = BTreeMap<usize, Vec<u8>>;

/// Navigate to a MuseScore URL, scroll the score viewer, and download all SVG pages.
pub async fn scrape(browser: &Browser, url: &str) -> Result<ScorePages> {
    let page = browser
        .new_page("about:blank")
        .await
        .context("failed to create new page")?;

    info!("Navigating to {url}");
    page.goto(url).await.context("failed to navigate to URL")?;

    // Wait for the page to actually load (Cloudflare challenge takes time)
    info!("Waiting for page to load (Cloudflare challenge may take a moment)...");
    sleep(Duration::from_secs(5)).await;

    // Check what URL we're actually on
    let current_url: String = page
        .evaluate("window.location.href")
        .await?
        .into_value()
        .unwrap_or_default();
    debug!("Current URL: {current_url}");

    // Wait for the score image to appear in the DOM
    info!("Waiting for score to appear...");
    wait_for_score(&page).await?;

    // Extract total page count from alt text
    let total_pages = extract_page_count(&page).await?;
    info!("Score has {total_pages} pages.");

    // Scroll the score viewer container to trigger lazy loading
    info!("Scrolling score viewer...");
    scroll_score_viewer(&page).await?;
    sleep(Duration::from_secs(2)).await;

    // Collect all score SVG URLs from the DOM (they're S3 presigned URLs, not predictable)
    info!("Collecting SVG URLs from page...");
    let svg_urls: Vec<String> = page
        .evaluate(
            r#"(() => {
                const urls = [];
                for (const img of document.querySelectorAll('img')) {
                    const src = img.src || img.dataset?.src || '';
                    if (/score_\d+\.svg/.test(src)) urls.push(src);
                }
                return JSON.stringify([...new Set(urls)]);
            })()"#,
        )
        .await
        .context("failed to collect SVG URLs")?
        .into_value::<String>()
        .map(|s| serde_json::from_str::<Vec<String>>(&s).unwrap_or_default())
        .unwrap_or_default();

    if svg_urls.is_empty() {
        bail!("no score SVG URLs found in page DOM");
    }

    info!("Found {} SVG URLs in DOM.", svg_urls.len());

    // Download each SVG using in-browser fetch (carries Cloudflare cookies + S3 auth)
    info!("Downloading {} SVGs...", svg_urls.len());
    let mut result = BTreeMap::new();
    let svg_pattern = Regex::new(r"score_(\d+)\.svg")?;

    for svg_url in &svg_urls {
        let idx = svg_pattern
            .captures(svg_url)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<usize>().ok())
            .unwrap_or(0);

        // Escape the URL for JS string embedding
        let escaped_url = svg_url.replace('\\', "\\\\").replace('"', "\\\"");
        let js = format!(
            r#"(async () => {{
                try {{
                    const r = await fetch("{escaped_url}");
                    if (!r.ok) return JSON.stringify({{error: r.status}});
                    return JSON.stringify({{data: await r.text()}});
                }} catch(e) {{
                    return JSON.stringify({{error: e.message}});
                }}
            }})()"#
        );

        let response: String = page
            .evaluate(js)
            .await
            .with_context(|| format!("JS fetch failed for page {idx}"))?
            .into_value()
            .unwrap_or_default();

        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap_or_default();

        if let Some(data) = parsed.get("data").and_then(|v| v.as_str()) {
            info!("  Page {}: {} bytes", idx + 1, data.len());
            result.insert(idx, data.as_bytes().to_vec());
        } else {
            let err = parsed
                .get("error")
                .map_or("unknown".into(), |v| v.to_string());
            warn!("  Page {} FAILED: {err}", idx + 1);
        }
    }

    if result.is_empty() {
        bail!("no SVGs downloaded — Cloudflare may have blocked requests");
    }

    info!("Downloaded {}/{} SVGs.", result.len(), svg_urls.len());
    Ok(result)
}

/// Poll until the score image element appears in the DOM.
async fn wait_for_score(page: &chromiumoxide::Page) -> Result<()> {
    for _ in 0..120 {
        let found: bool = page
            .evaluate("!!document.querySelector(\"img[src*='score_']\")")
            .await?
            .into_value()
            .unwrap_or(false);

        if found {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }
    bail!("score image not found after 60s — is this a valid MuseScore URL?")
}

/// Extract total page count from the score image alt text (e.g. "1 of 15 pages").
async fn extract_page_count(page: &chromiumoxide::Page) -> Result<usize> {
    let alt: String = page
        .evaluate("document.querySelector(\"img[src*='score_'][src*='.svg']\")?.alt || ''")
        .await?
        .into_value()
        .unwrap_or_default();

    let re = Regex::new(r"(\d+)\s+of\s+(\d+)\s+pages?")?;
    if let Some(caps) = re.captures(&alt) {
        let total = caps[2].parse::<usize>().unwrap_or(1);
        return Ok(total);
    }

    warn!("Could not parse page count from alt text: {alt:?}, defaulting to 1");
    Ok(1)
}

/// Scroll the score viewer container incrementally to trigger lazy loading.
async fn scroll_score_viewer(page: &chromiumoxide::Page) -> Result<()> {
    // First, find the scrollable container and get its scroll height
    let scroll_height: f64 = page
        .evaluate(
            r#"(() => {
                let el = document.querySelector("img[src*='score_0.svg']");
                while (el && el !== document.body) {
                    if (el.scrollHeight > el.clientHeight + 10) return el.scrollHeight;
                    el = el.parentElement;
                }
                return 0;
            })()"#,
        )
        .await?
        .into_value()
        .unwrap_or(0.0);

    if scroll_height < 1.0 {
        warn!("No scrollable score viewer container found");
        return Ok(());
    }

    debug!("Score viewer scroll height: {scroll_height}px");

    // Scroll in small increments with separate evaluate calls
    // to keep the WebSocket connection alive
    let step = 300;
    let total = scroll_height as i64;
    let mut pos = 0i64;

    while pos < total {
        page.evaluate(format!(
            r#"(() => {{
                let el = document.querySelector("img[src*='score_0.svg']");
                while (el && el !== document.body) {{
                    if (el.scrollHeight > el.clientHeight + 10) {{ el.scrollTop = {pos}; return true; }}
                    el = el.parentElement;
                }}
                return false;
            }})()"#
        ))
        .await?;

        pos += step;
        sleep(Duration::from_millis(250)).await;
    }

    // Scroll to the very end
    page.evaluate(
        r#"(() => {
            let el = document.querySelector("img[src*='score_0.svg']");
            while (el && el !== document.body) {
                if (el.scrollHeight > el.clientHeight + 10) { el.scrollTop = el.scrollHeight; return; }
                el = el.parentElement;
            }
        })()"#,
    )
    .await?;

    debug!("Scroll complete");
    Ok(())
}
