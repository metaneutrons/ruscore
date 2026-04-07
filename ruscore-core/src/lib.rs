//! Core library for MuseScore score scraping and PDF generation.
//!
//! Provides Chrome CDP management, MuseScore page scraping with metadata
//! extraction, and SVG-to-PDF conversion.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::redundant_closure)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::uninlined_format_args)]

pub mod cdp;
pub mod chrome;
pub mod pdf;
pub mod scraper;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Scraped score: page index → SVG bytes, sorted by page number.
pub type ScorePages = BTreeMap<usize, Vec<u8>>;

/// Metadata extracted from a MuseScore score page.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScoreMetadata {
    /// Score title.
    pub title: String,
    /// Original composer.
    pub composer: String,
    /// Arranger name.
    pub arranger: String,
    /// Instruments in the score.
    pub instruments: Vec<String>,
    /// Total number of pages.
    pub pages: usize,
    /// Score description.
    pub description: String,
    /// Thumbnail URL (first page SVG).
    pub thumbnail_url: String,
    /// Warnings (e.g. partial capture due to PRO+ paywall).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
