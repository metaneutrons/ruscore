//! Scrape MuseScore sheet music SVGs and convert to PDF.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::redundant_closure)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::uninlined_format_args)]

mod chrome;
mod pdf;
mod scraper;

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
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    let mut chrome = chrome::Chrome::start().await?;
    let pages = scraper::scrape(&chrome.browser, &cli.url).await?;
    pdf::generate(&pages, &cli.output)?;

    chrome.shutdown();
    Ok(())
}
