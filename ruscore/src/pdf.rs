//! SVG→PDF conversion using `usvg` + `svg2pdf` + `pdf-writer`.
//!
//! Builds a single multi-page PDF directly — no intermediate files or merging.

use anyhow::{Context, Result};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use std::path::Path;
use tracing::{debug, info};

use crate::scraper::ScorePages;

/// Convert scored SVG pages to a single multi-page PDF file.
pub fn generate(pages: &ScorePages, output: &Path) -> Result<()> {
    info!("Converting {} SVGs to PDF...", pages.len());

    // Parse all SVGs first to get chunks and determine ref ranges
    let options = usvg::Options::default();
    let mut svg_data = Vec::with_capacity(pages.len());

    // Start ref allocator high enough to not conflict with svg2pdf chunks
    // Each SVG chunk can use many refs; 10000 per page is safe headroom
    let base_ref = pages.len() as i32 * 10000 + 1;
    let mut alloc = Ref::new(base_ref);

    let catalog_ref = alloc.bump();
    let page_tree_ref = alloc.bump();

    for (&idx, svg_bytes) in pages {
        debug!("Parsing page {idx}...");
        let tree = usvg::Tree::from_data(svg_bytes, &options)
            .with_context(|| format!("failed to parse SVG for page {idx}"))?;

        let (chunk, x_ref) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default())
            .map_err(|e| anyhow::anyhow!("svg2pdf failed for page {idx}: {e:?}"))?;

        let size = tree.size();
        svg_data.push((idx, chunk, x_ref, size.width(), size.height()));
    }

    let mut pdf = Pdf::new();

    // Pre-allocate page + content refs
    let page_entries: Vec<_> = svg_data
        .iter()
        .map(|_| {
            let page_ref = alloc.bump();
            let content_ref = alloc.bump();
            (page_ref, content_ref)
        })
        .collect();

    // Catalog
    pdf.catalog(catalog_ref).pages(page_tree_ref);

    // Page tree
    let page_refs: Vec<Ref> = page_entries.iter().map(|e| e.0).collect();
    pdf.pages(page_tree_ref).kids(page_refs).finish();

    // Write each page
    for (i, (idx, chunk, x_ref, w, h)) in svg_data.iter().enumerate() {
        debug!("Writing page {idx} to PDF...");
        let (page_ref, content_ref) = page_entries[i];

        // Embed the SVG chunk objects
        pdf.extend(chunk);

        // Content stream: draw the XObject scaled to page size
        let x_name = Name(b"S0");
        let mut content = Content::new();
        content.save_state();
        content.transform([*w, 0.0, 0.0, *h, 0.0, 0.0]);
        content.x_object(x_name);
        content.restore_state();
        pdf.stream(content_ref, &content.finish());

        // Page object
        let mut page = pdf.page(page_ref);
        page.parent(page_tree_ref)
            .media_box(Rect::new(0.0, 0.0, *w, *h))
            .contents(content_ref);
        page.resources().x_objects().pair(x_name, *x_ref);
        page.finish();
    }

    let pdf_bytes = pdf.finish();
    std::fs::write(output, &pdf_bytes)
        .with_context(|| format!("failed to write {}", output.display()))?;

    info!("Wrote {} ({} pages)", output.display(), pages.len());
    Ok(())
}
