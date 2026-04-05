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

    let mut pdf = Pdf::new();
    let mut alloc = Ref::new(1);

    let catalog_ref = alloc.bump();
    let page_tree_ref = alloc.bump();

    // Pre-allocate refs for each page and its content stream
    let page_entries: Vec<_> = pages
        .iter()
        .map(|(&idx, svg_bytes)| {
            let page_ref = alloc.bump();
            let content_ref = alloc.bump();
            (idx, svg_bytes, page_ref, content_ref)
        })
        .collect();

    // Write catalog
    pdf.catalog(catalog_ref).pages(page_tree_ref);

    // Write page tree with all page refs
    let page_refs: Vec<Ref> = page_entries.iter().map(|e| e.2).collect();
    pdf.pages(page_tree_ref).kids(page_refs).finish();

    // Write each page
    for &(idx, svg_bytes, page_ref, content_ref) in &page_entries {
        debug!("Converting page {idx}...");

        let options = usvg::Options::default();
        let tree = usvg::Tree::from_data(svg_bytes, &options)
            .with_context(|| format!("failed to parse SVG for page {idx}"))?;

        let size = tree.size();
        let w = size.width();
        let h = size.height();

        // Convert SVG to a PDF chunk (XObject form)
        let (chunk, x_ref) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default())
            .map_err(|e| anyhow::anyhow!("svg2pdf conversion failed for page {idx}: {e:?}"))?;

        // Extend PDF with the chunk's objects
        pdf.extend(&chunk);

        // Build content stream that paints the XObject
        let x_name = Name(b"S0");
        let mut content = Content::new();
        content.save_state();
        content.transform([w, 0.0, 0.0, h, 0.0, 0.0]);
        content.x_object(x_name);
        content.restore_state();
        let content_data = content.finish();

        // Write content stream
        pdf.stream(content_ref, &content_data);

        // Write page object
        let mut page = pdf.page(page_ref);
        page.parent(page_tree_ref)
            .media_box(Rect::new(0.0, 0.0, w, h))
            .contents(content_ref);
        page.resources().x_objects().pair(x_name, x_ref);
        page.finish();
    }

    let pdf_bytes = pdf.finish();
    std::fs::write(output, &pdf_bytes)
        .with_context(|| format!("failed to write {}", output.display()))?;

    info!("Wrote {} ({} pages)", output.display(), pages.len());
    Ok(())
}
