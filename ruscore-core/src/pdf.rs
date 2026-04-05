//! SVG→PDF conversion using `usvg` + `svg2pdf` + `pdf-writer`.
//!
//! Builds a single multi-page PDF. Each SVG chunk is renumbered to avoid ref collisions.

use anyhow::{Context, Result};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref};
use std::path::Path;
use tracing::{debug, info};

use crate::ScorePages;

/// Refs per SVG chunk — must be large enough to cover any single SVG's objects.
const REFS_PER_CHUNK: i32 = 5000;

/// Convert scored SVG pages to a single multi-page PDF file.
pub fn generate(pages: &ScorePages, output: &Path) -> Result<()> {
    info!("Converting {} SVGs to PDF...", pages.len());

    let options = usvg::Options::default();
    let mut pdf = Pdf::new();

    // Reserve ref space: catalog + page tree + (page + content) per page + chunk space
    let catalog_ref = Ref::new(1);
    let page_tree_ref = Ref::new(2);
    // Each page needs: page_ref, content_ref, then REFS_PER_CHUNK for the svg chunk
    let page_base = 3;

    let mut page_refs = Vec::with_capacity(pages.len());
    let mut page_data = Vec::new();

    for (i, (&idx, svg_bytes)) in pages.iter().enumerate() {
        debug!("Parsing page {idx}...");
        let tree = usvg::Tree::from_data(svg_bytes, &options)
            .with_context(|| format!("failed to parse SVG for page {idx}"))?;

        let (chunk, x_ref) = svg2pdf::to_chunk(&tree, svg2pdf::ConversionOptions::default())
            .map_err(|e| anyhow::anyhow!("svg2pdf failed for page {idx}: {e:?}"))?;

        // Renumber chunk refs to avoid collisions: offset by chunk_base
        let chunk_base = page_base + (i as i32) * (REFS_PER_CHUNK + 2);
        let page_ref = Ref::new(chunk_base);
        let content_ref = Ref::new(chunk_base + 1);
        let chunk_offset = chunk_base + 2; // chunk refs start here

        let renumbered = chunk.renumber(|old| Ref::new(old.get() + chunk_offset - 1));
        let new_x_ref = Ref::new(x_ref.get() + chunk_offset - 1);

        let size = tree.size();
        page_refs.push(page_ref);
        page_data.push((
            idx,
            renumbered,
            new_x_ref,
            page_ref,
            content_ref,
            size.width(),
            size.height(),
        ));
    }

    // Catalog
    pdf.catalog(catalog_ref).pages(page_tree_ref);

    // Page tree
    pdf.pages(page_tree_ref).kids(page_refs).finish();

    // Write each page
    for (idx, chunk, x_ref, page_ref, content_ref, w, h) in &page_data {
        debug!("Writing page {idx} to PDF...");

        pdf.extend(chunk);

        let x_name = Name(b"S0");
        let mut content = Content::new();
        content.save_state();
        content.transform([*w, 0.0, 0.0, *h, 0.0, 0.0]);
        content.x_object(x_name);
        content.restore_state();
        pdf.stream(*content_ref, &content.finish());

        let mut page = pdf.page(*page_ref);
        page.parent(page_tree_ref)
            .media_box(Rect::new(0.0, 0.0, *w, *h))
            .contents(*content_ref);
        page.resources().x_objects().pair(x_name, *x_ref);
        page.finish();
    }

    let pdf_bytes = pdf.finish();
    std::fs::write(output, &pdf_bytes)
        .with_context(|| format!("failed to write {}", output.display()))?;

    info!("Wrote {} ({} pages)", output.display(), pages.len());
    Ok(())
}
