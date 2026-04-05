//! MuseScore page navigation, score viewer scrolling, and SVG capture.
//!
//! After scrolling triggers lazy loading, collects SVG URLs from the browser's
//! performance resource timing API and fetches them from cache.

use anyhow::{Context, Result, bail};
use chromiumoxide::Browser;
use regex::Regex;
use std::collections::BTreeMap;
use tokio::time::{Duration, sleep};
use tracing::{debug, info, warn};

/// Scraped score: page index → SVG bytes, sorted by page number.
pub type ScorePages = BTreeMap<usize, Vec<u8>>;

/// Navigate to a MuseScore URL, scroll the score viewer, and capture all SVG pages.
pub async fn scrape(browser: &Browser, url: &str) -> Result<ScorePages> {
    let page = browser
        .new_page("about:blank")
        .await
        .context("failed to create new page")?;

    info!("Navigating to {url}");
    page.goto(url).await.context("failed to navigate to URL")?;

    info!("Waiting for page to load (Cloudflare challenge may take a moment)...");
    sleep(Duration::from_secs(5)).await;

    info!("Waiting for score to appear...");
    wait_for_score(&page).await?;

    let total_pages = extract_page_count(&page).await?;
    info!("Score has {total_pages} pages.");

    info!("Scrolling score viewer...");
    scroll_score_viewer(&page, total_pages).await?;
    sleep(Duration::from_secs(3)).await;

    // Collect SVG URLs from performance resource timing + fetch from cache
    info!("Downloading SVGs...");
    let result: String = page
        .evaluate(format!(
            r#"(async () => {{
                const entries = performance.getEntriesByType('resource');
                const svgUrls = entries
                    .filter(e => /score_\d+\.svg/.test(e.name))
                    .map(e => e.name);

                const unique = [...new Set(svgUrls)];
                const results = {{}};

                for (const url of unique) {{
                    const m = url.match(/score_(\d+)\.svg/);
                    if (!m) continue;
                    const idx = parseInt(m[1]);
                    try {{
                        const r = await fetch(url);
                        if (r.ok) {{
                            results[idx] = await r.text();
                        }}
                    }} catch(e) {{}}
                }}

                return JSON.stringify({{
                    found: unique.length,
                    downloaded: Object.keys(results).length,
                    pages: results
                }});
            }})()"#
        ))
        .await
        .context("failed to collect SVGs")?
        .into_value()
        .unwrap_or_default();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap_or_default();

    let found = parsed.get("found").and_then(|v| v.as_u64()).unwrap_or(0);
    let downloaded = parsed
        .get("downloaded")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    info!("Found {found} SVG URLs, downloaded {downloaded}.");

    let mut pages_map = BTreeMap::new();
    if let Some(pages) = parsed.get("pages").and_then(|v| v.as_object()) {
        for (key, value) in pages {
            if let (Ok(idx), Some(svg)) = (key.parse::<usize>(), value.as_str()) {
                info!("  Page {}: {} bytes", idx + 1, svg.len());
                pages_map.insert(idx, svg.as_bytes().to_vec());
            }
        }
    }

    if pages_map.is_empty() {
        bail!("no SVGs downloaded — Cloudflare may have blocked the page");
    }

    info!("Captured {}/{total_pages} SVGs.", pages_map.len());
    Ok(pages_map)
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

/// Extract total page count from the score image alt text.
async fn extract_page_count(page: &chromiumoxide::Page) -> Result<usize> {
    let alt: String = page
        .evaluate("document.querySelector(\"img[src*='score_'][src*='.svg']\")?.alt || ''")
        .await?
        .into_value()
        .unwrap_or_default();

    let re = Regex::new(r"(\d+)\s+of\s+(\d+)\s+pages?")?;
    if let Some(caps) = re.captures(&alt) {
        return Ok(caps[2].parse::<usize>().unwrap_or(1));
    }
    warn!("Could not parse page count from alt: {alt:?}, defaulting to 1");
    Ok(1)
}

/// Scroll the score viewer container incrementally.
/// Uses scrollIntoView on each page placeholder to reliably trigger IntersectionObserver.
async fn scroll_score_viewer(page: &chromiumoxide::Page, total_pages: usize) -> Result<()> {
    // Scroll each page placeholder into view
    for i in 0..total_pages {
        let scrolled: bool = page
            .evaluate(format!(
                r#"(() => {{
                    const container = document.querySelector("img[src*='score_0.svg']");
                    if (!container) return false;
                    let scrollable = container;
                    while (scrollable && scrollable !== document.body) {{
                        if (scrollable.scrollHeight > scrollable.clientHeight + 10) break;
                        scrollable = scrollable.parentElement;
                    }}
                    if (!scrollable) return false;
                    const child = scrollable.children[{i}];
                    if (child) child.scrollIntoView({{ behavior: 'instant', block: 'center' }});
                    return !!child;
                }})()"#
            ))
            .await?
            .into_value()
            .unwrap_or(false);

        if !scrolled && i > 0 {
            debug!("  No child at index {i}");
        }
        sleep(Duration::from_millis(500)).await;
    }

    // Also scroll to the very end
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
