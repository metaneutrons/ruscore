//! CLI tool to scrape MuseScore sheet music and convert to PDF.

#![forbid(unsafe_code)]

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

/// Scrape MuseScore sheet music and convert to PDF.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// MuseScore score URL
    url: String,

    /// Output PDF path
    #[arg(default_value = "score.pdf")]
    output: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".parse().expect("valid filter")),
        )
        .init();

    let cli = Cli::parse();

    let mut chrome = ruscore_core::chrome::Chrome::start().await?;
    let (pages, metadata) = ruscore_core::scraper::scrape(&mut chrome.session, &cli.url).await?;

    tracing::info!(
        "Score: {} by {} ({} pages)",
        metadata.title,
        metadata.composer,
        metadata.pages
    );

    ruscore_core::pdf::generate(&pages, &cli.output)?;
    chrome.shutdown();
    Ok(())
}
