//! SVG→PDF conversion using `usvg` + `svg2pdf` + `lopdf` for merging.

use anyhow::{Context, Result};
use lopdf::{Document, Object, ObjectId};
use std::collections::BTreeMap;
use std::path::Path;
use tracing::{debug, info};

use crate::ScorePages;

/// Convert scored SVG pages to a single merged PDF file.
pub fn generate(pages: &ScorePages, output: &Path) -> Result<()> {
    info!("Converting {} SVGs to PDF...", pages.len());

    let options = usvg::Options::default();
    let mut pdf_docs: Vec<Vec<u8>> = Vec::with_capacity(pages.len());

    for (&idx, svg_bytes) in pages {
        debug!("Converting page {idx}...");
        let tree = usvg::Tree::from_data(svg_bytes, &options)
            .with_context(|| format!("failed to parse SVG for page {idx}"))?;

        let pdf_bytes = svg2pdf::to_pdf(
            &tree,
            svg2pdf::ConversionOptions::default(),
            svg2pdf::PageOptions::default(),
        )
        .map_err(|e| anyhow::anyhow!("svg2pdf failed for page {idx}: {e:?}"))?;

        pdf_docs.push(pdf_bytes);
    }

    if pdf_docs.len() == 1 {
        std::fs::write(output, &pdf_docs[0])
            .with_context(|| format!("failed to write {}", output.display()))?;
    } else {
        info!("Merging {} pages...", pdf_docs.len());
        let mut merged = merge_pdfs(&pdf_docs)?;
        merged
            .save(output)
            .with_context(|| format!("failed to write {}", output.display()))?;
    }

    info!("Wrote {} ({} pages)", output.display(), pages.len());
    Ok(())
}

/// Merge multiple single-page PDFs into one document.
fn merge_pdfs(pdf_bytes_list: &[Vec<u8>]) -> Result<Document> {
    let mut merged = Document::with_version("1.7");
    let mut next_id: u32 = 1;
    let mut page_refs: Vec<ObjectId> = Vec::new();

    for (i, pdf_bytes) in pdf_bytes_list.iter().enumerate() {
        let doc = Document::load_mem(pdf_bytes)
            .with_context(|| format!("failed to parse PDF for page {i}"))?;

        // Build ID mapping: old → new (offset all IDs)
        let id_offset = next_id;
        let max_id = doc.max_id;
        let mut id_map = BTreeMap::new();
        for &old_id in doc.objects.keys() {
            let new_id = (old_id.0 + id_offset, old_id.1);
            id_map.insert(old_id, new_id);
        }
        next_id = id_offset + max_id + 1;

        // Copy all objects with remapped IDs
        for (old_id, object) in &doc.objects {
            let new_id = id_map[old_id];
            let new_obj = remap_object(object, &id_map);
            merged.objects.insert(new_id, new_obj);
        }

        // Find the page object(s) in this document
        let doc_pages = doc.get_pages();
        for &page_id in doc_pages.values() {
            page_refs.push(id_map[&page_id]);
        }
    }

    // Build page tree
    let pages_id = (next_id, 0);
    next_id += 1;
    let kids: Vec<Object> = page_refs.iter().map(|&id| Object::Reference(id)).collect();
    let mut pages_dict = lopdf::Dictionary::new();
    pages_dict.set("Type", Object::Name(b"Pages".to_vec()));
    pages_dict.set("Kids", Object::Array(kids));
    pages_dict.set("Count", Object::Integer(page_refs.len() as i64));
    merged
        .objects
        .insert(pages_id, Object::Dictionary(pages_dict));

    // Update each page's Parent
    for &page_id in &page_refs {
        if let Some(Object::Dictionary(dict)) = merged.objects.get_mut(&page_id) {
            dict.set("Parent", Object::Reference(pages_id));
        }
    }

    // Build catalog
    let catalog_id = (next_id, 0);
    let mut catalog = lopdf::Dictionary::new();
    catalog.set("Type", Object::Name(b"Catalog".to_vec()));
    catalog.set("Pages", Object::Reference(pages_id));
    merged
        .objects
        .insert(catalog_id, Object::Dictionary(catalog));

    merged.trailer.set("Root", Object::Reference(catalog_id));
    merged.max_id = next_id;

    Ok(merged)
}

/// Recursively remap all object references in a PDF object.
fn remap_object(obj: &Object, id_map: &BTreeMap<ObjectId, ObjectId>) -> Object {
    match obj {
        Object::Reference(id) => Object::Reference(*id_map.get(id).unwrap_or(id)),
        Object::Dictionary(dict) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, val) in dict.iter() {
                new_dict.set(key.clone(), remap_object(val, id_map));
            }
            Object::Dictionary(new_dict)
        }
        Object::Array(arr) => Object::Array(arr.iter().map(|v| remap_object(v, id_map)).collect()),
        Object::Stream(stream) => {
            let mut new_dict = lopdf::Dictionary::new();
            for (key, val) in stream.dict.iter() {
                new_dict.set(key.clone(), remap_object(val, id_map));
            }
            Object::Stream(lopdf::Stream::new(new_dict, stream.content.clone()))
        }
        other => other.clone(),
    }
}
