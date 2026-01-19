// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Build pipeline for Sorex search indexes.
//!
//! Reads a directory of per-document JSON files (defined by a manifest), builds
//! search indexes in parallel using rayon, and emits `.sorex` binary files with
//! optional embedded WASM for self-contained browser deployment.
//!
//! The typical flow is: `manifest.json` lists documents, we load them in parallel,
//! build indexes for each filter criterion, and write everything to disk with
//! progress bars so you know we haven't hung.

pub mod document;
pub mod manifest;
pub mod parallel;

use std::fs;
use std::path::Path;

#[cfg(feature = "parallel")]
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

pub use document::*;
pub use manifest::*;
pub use parallel::*;

/// Normalized index definition with include filter
#[derive(Clone, Debug)]
pub struct NormalizedIndexDefinition {
    pub include: IncludeFilter,
    pub fields: Option<Vec<String>>,
}

/// Create a progress style for the main progress bars
#[cfg(feature = "parallel")]
fn create_progress_style() -> ProgressStyle {
    ProgressStyle::with_template(
        "{spinner:.cyan} {prefix:<12} [{bar:40.cyan/dim}] {pos}/{len} {msg}",
    )
    .unwrap()
    .progress_chars("━━╸")
}

pub fn run_build(
    input_dir: &str,
    output_dir: &str,
    emit_demo: bool,
    ranking_path: Option<&str>,
    ranking_batch_size: Option<usize>,
) -> Result<(), String> {
    let input_path = Path::new(input_dir);
    let output_path = Path::new(output_dir);

    // Set up multi-progress display
    #[cfg(feature = "parallel")]
    let multi = MultiProgress::new();

    // 1. Read manifest
    let manifest_path = input_path.join("manifest.json");
    let manifest_content = fs::read_to_string(&manifest_path)
        .map_err(|e| format!("Failed to read manifest: {}", e))?;
    let manifest: InputManifest = serde_json::from_str(&manifest_content)
        .map_err(|e| format!("Invalid manifest JSON: {}", e))?;

    // 2. Load documents in parallel with progress bar
    #[cfg(feature = "parallel")]
    let load_pb = multi.add(ProgressBar::new(manifest.documents.len() as u64));
    #[cfg(feature = "parallel")]
    load_pb.set_style(create_progress_style());
    #[cfg(feature = "parallel")]
    load_pb.set_prefix("Loading");
    #[cfg(feature = "parallel")]
    load_pb.set_message("documents...");

    let documents = parallel::load_documents_with_progress(
        input_path,
        &manifest,
        #[cfg(feature = "parallel")]
        &load_pb,
    )?;

    #[cfg(feature = "parallel")]
    load_pb.finish_with_message(format!("loaded {} documents", documents.len()));

    if documents.is_empty() {
        eprintln!("⚠️  No documents loaded; skipping build");
        return Ok(());
    }

    // 3. Build a single index with all documents
    let index_defs: Vec<(String, NormalizedIndexDefinition)> = vec![(
        "index".to_string(),
        NormalizedIndexDefinition {
            include: IncludeFilter::All,
            fields: None,
        },
    )];

    // 4. Build indexes in parallel with progress bar
    #[cfg(feature = "parallel")]
    let build_pb = multi.add(ProgressBar::new(index_defs.len() as u64));
    #[cfg(feature = "parallel")]
    build_pb.set_style(create_progress_style());
    #[cfg(feature = "parallel")]
    build_pb.set_prefix("Building");
    #[cfg(feature = "parallel")]
    build_pb.set_message("indexes...");

    let built_indexes = parallel::build_indexes_with_progress(
        &documents,
        &index_defs,
        ranking_path,
        ranking_batch_size,
        #[cfg(feature = "parallel")]
        &build_pb,
    );

    #[cfg(feature = "parallel")]
    build_pb.finish_with_message(format!("built {} indexes", built_indexes.len()));

    // 5. Create output directory
    fs::create_dir_all(output_path).map_err(|e| format!("Failed to create output dir: {}", e))?;

    // 6. Write index files with progress
    #[cfg(feature = "parallel")]
    let write_pb = multi.add(ProgressBar::new(built_indexes.len() as u64));
    #[cfg(feature = "parallel")]
    write_pb.set_style(create_progress_style());
    #[cfg(feature = "parallel")]
    write_pb.set_prefix("Writing");
    #[cfg(feature = "parallel")]
    write_pb.set_message("files...");

    let mut first_index_file: Option<String> = None;

    for index in &built_indexes {
        // Simple filename: index.sorex or {name}.sorex for multiple indexes
        let filename = if index_defs.len() == 1 && index.name == "index" {
            "index.sorex".to_string()
        } else {
            format!("{}.sorex", index.name)
        };
        let path = output_path.join(&filename);

        fs::write(&path, &index.bytes)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

        if first_index_file.is_none() {
            first_index_file = Some(filename.clone());
        }

        #[cfg(feature = "parallel")]
        write_pb.set_message(format!("{} ({} docs)", filename, index.doc_count));
        #[cfg(feature = "parallel")]
        write_pb.inc(1);
    }

    #[cfg(feature = "parallel")]
    write_pb.finish_with_message("done");

    // 7. Emit JS loader (only when embed-wasm feature is enabled)
    #[cfg(feature = "embed-wasm")]
    emit_js_loader(output_path)?;

    // 8. Emit demo HTML if requested
    if emit_demo {
        let index_file = first_index_file.as_deref().unwrap_or("index.sorex");
        emit_demo_html(output_path, index_file)?;
    }

    // Final summary
    #[cfg(feature = "parallel")]
    {
        let total_docs: usize = built_indexes.iter().map(|i| i.doc_count).sum();
        let total_terms: usize = built_indexes.iter().map(|i| i.term_count).sum();
        let total_bytes: usize = built_indexes.iter().map(|i| i.bytes.len()).sum();
        eprintln!();
        eprintln!("✅ Build complete");
        eprintln!(
            "   {} indexes │ {} documents │ {} terms │ {} bytes",
            built_indexes.len(),
            total_docs,
            total_terms,
            format_bytes(total_bytes)
        );
    }

    #[cfg(not(feature = "parallel"))]
    eprintln!("✅ Build complete");

    Ok(())
}

#[cfg(feature = "parallel")]
fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Emit the JS loader that extracts WASM from .sorex files.
/// Only compiled when embed-wasm feature is enabled.
#[cfg(feature = "embed-wasm")]
fn emit_js_loader(output_path: &Path) -> Result<(), String> {
    // Generated by build.rs: wasm-pack + bun
    const LOADER_JS: &str = include_str!(concat!(env!("SOREX_OUT_DIR"), "/sorex.js"));

    let loader_path = output_path.join("sorex.js");
    fs::write(&loader_path, LOADER_JS).map_err(|e| format!("Failed to write sorex.js: {}", e))?;
    eprintln!("  ✓ {}", loader_path.display());

    Ok(())
}

/// Emit demo HTML page.
fn emit_demo_html(output_path: &Path, index_file: &str) -> Result<(), String> {
    const DEMO_HTML: &str = include_str!("../../data/demo.html");

    let demo_html = DEMO_HTML.replace("{{INDEX_FILE}}", index_file);
    let demo_path = output_path.join("demo.html");
    fs::write(&demo_path, demo_html).map_err(|e| format!("Failed to write demo.html: {}", e))?;
    eprintln!("  ✓ {}", demo_path.display());

    Ok(())
}
