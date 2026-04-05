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

    // Wait for the score image to appear
    info!("Waiting for score to load...");
    wait_for_score(&page).await?;

    // Extract total page count from alt text
    let total_pages = extract_page_count(&page).await?;
    info!("Score has {total_pages} pages.");

    // Scroll the score viewer container to trigger lazy loading
    info!("Scrolling score viewer...");
    scroll_score_viewer(&page).await?;
    sleep(Duration::from_secs(2)).await;

    // Get the first SVG URL to derive the pattern
    let first_src: String = page
        .evaluate("document.querySelector(\"img[src*='score_'][src*='.svg']\")?.src || ''")
        .await
        .context("failed to query first SVG src")?
        .into_value()
        .unwrap_or_default();

    if first_src.is_empty() {
        bail!("could not find score SVG URL in page");
    }
    debug!("First SVG URL: {first_src}");

    let (prefix, suffix) = parse_svg_url_parts(&first_src)?;

    // Download each SVG using in-browser fetch (carries Cloudflare cookies)
    info!("Downloading {total_pages} SVGs...");
    let mut result = BTreeMap::new();

    for i in 0..total_pages {
        let svg_url = format!("{prefix}score_{i}.svg{suffix}");

        let js = format!(
            r#"(async () => {{
                try {{
                    const r = await fetch("{svg_url}");
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
            .with_context(|| format!("JS fetch failed for page {i}"))?
            .into_value()
            .unwrap_or_default();

        let parsed: serde_json::Value = serde_json::from_str(&response).unwrap_or_default();

        if let Some(data) = parsed.get("data").and_then(|v| v.as_str()) {
            info!("  Page {}: {} bytes", i + 1, data.len());
            result.insert(i, data.as_bytes().to_vec());
        } else {
            let err = parsed
                .get("error")
                .map_or("unknown".into(), |v| v.to_string());
            warn!("  Page {} FAILED: {err}", i + 1);
        }
    }

    if result.is_empty() {
        bail!("no SVGs downloaded — Cloudflare may have blocked requests");
    }

    info!("Downloaded {}/{total_pages} SVGs.", result.len());
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
    let result: String = page
        .evaluate(
            r#"(async () => {
                let el = document.querySelector("img[src*='score_0.svg']");
                let scrollable = null;
                while (el && el !== document.body) {
                    if (el.scrollHeight > el.clientHeight + 10) {
                        scrollable = el;
                        break;
                    }
                    el = el.parentElement;
                }
                if (!scrollable) return "no scrollable container found";

                for (let pos = 0; pos < scrollable.scrollHeight; pos += 300) {
                    scrollable.scrollTop = pos;
                    await new Promise(r => setTimeout(r, 300));
                }
                scrollable.scrollTop = scrollable.scrollHeight;
                return "scrolled " + scrollable.scrollHeight + "px";
            })()"#,
        )
        .await?
        .into_value()
        .unwrap_or_default();

    debug!("Scroll result: {result}");
    Ok(())
}

/// Parse the first SVG URL into prefix and suffix for constructing other page URLs.
///
/// Input:  `https://example.com/path/score_0.svg?no-cache=123`
/// Output: `("https://example.com/path/", "?no-cache=123")`
fn parse_svg_url_parts(url: &str) -> Result<(String, String)> {
    let re = Regex::new(r"^(.*/)score_0\.svg(.*)$")?;
    let caps = re
        .captures(url)
        .context("first SVG URL doesn't match expected pattern")?;

    Ok((caps[1].to_string(), caps[2].to_string()))
}
