pub mod document;
pub mod manifest;
pub mod parallel;

use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[cfg(feature = "parallel")]
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};

use crate::binary::SieveFooter;

pub use document::*;
pub use manifest::*;
pub use parallel::*;

/// Normalized index definition with include filter
#[derive(Clone, Debug)]
pub struct NormalizedIndexDefinition {
    pub include: IncludeFilter,
    pub fields: Option<Vec<String>>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct OutputManifest {
    pub version: u32,
    pub indexes: HashMap<String, OutputIndexInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wasm: Option<WasmInfo>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct OutputIndexInfo {
    pub file: String,
    #[serde(rename = "docCount")]
    pub doc_count: usize,
    #[serde(rename = "termCount")]
    pub term_count: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct WasmInfo {
    pub js: String,
    pub wasm: String,
    pub types: String,
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

/// Create a spinner style for indeterminate progress
#[cfg(all(feature = "parallel", feature = "embed-wasm"))]
fn create_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.cyan} {prefix:<12} {msg}")
        .unwrap()
        .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
}

pub fn run_build(
    input_dir: &str,
    output_dir: &str,
    indexes: Option<Vec<String>>,
    emit_wasm: bool,
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

    // 3. Determine which indexes to build
    let index_defs: Vec<(String, NormalizedIndexDefinition)> = if let Some(names) = indexes {
        names
            .iter()
            .filter_map(|name| {
                manifest
                    .indexes
                    .get(name)
                    .map(|def| {
                        (
                            name.clone(),
                            NormalizedIndexDefinition {
                                include: IncludeFilter::from(def.include.clone()),
                                fields: def.fields.clone(),
                            },
                        )
                    })
                    .or_else(|| {
                        eprintln!("Warning: Index '{}' not defined in manifest", name);
                        None
                    })
            })
            .collect()
    } else if manifest.indexes.is_empty() {
        // Default: single "index" with all documents
        vec![(
            "index".to_string(),
            NormalizedIndexDefinition {
                include: IncludeFilter::All,
                fields: None,
            },
        )]
    } else {
        manifest
            .indexes
            .iter()
            .map(|(k, v)| {
                (
                    k.clone(),
                    NormalizedIndexDefinition {
                        include: IncludeFilter::from(v.include.clone()),
                        fields: v.fields.clone(),
                    },
                )
            })
            .collect()
    };

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

    let mut output_manifest = OutputManifest {
        version: 1,
        indexes: HashMap::new(),
        wasm: None,
    };

    for index in &built_indexes {
        // Compute content hash for filename
        let hash = format!("{:08x}", SieveFooter::compute_crc32(&index.bytes));
        let filename = if index_defs.len() == 1 && index.name == "index" {
            // Single index: use "index-{hash}.sieve"
            format!("index-{}.sieve", hash)
        } else {
            // Multiple indexes: use "{name}-{hash}.sieve"
            format!("{}-{}.sieve", index.name, hash)
        };
        let path = output_path.join(&filename);

        fs::write(&path, &index.bytes)
            .map_err(|e| format!("Failed to write {}: {}", path.display(), e))?;

        #[cfg(feature = "parallel")]
        write_pb.set_message(format!("{} ({} docs)", filename, index.doc_count));
        #[cfg(feature = "parallel")]
        write_pb.inc(1);

        output_manifest.indexes.insert(
            index.name.clone(),
            OutputIndexInfo {
                file: filename,
                doc_count: index.doc_count,
                term_count: index.term_count,
            },
        );
    }

    #[cfg(feature = "parallel")]
    write_pb.finish_with_message("done");

    // 7. Emit WASM if requested
    #[cfg(feature = "embed-wasm")]
    if emit_wasm {
        #[cfg(all(feature = "parallel", feature = "embed-wasm"))]
        let wasm_pb = multi.add(ProgressBar::new_spinner());
        #[cfg(all(feature = "parallel", feature = "embed-wasm"))]
        wasm_pb.set_style(create_spinner_style());
        #[cfg(all(feature = "parallel", feature = "embed-wasm"))]
        wasm_pb.set_prefix("WASM");
        #[cfg(all(feature = "parallel", feature = "embed-wasm"))]
        wasm_pb.set_message("emitting files...");
        #[cfg(all(feature = "parallel", feature = "embed-wasm"))]
        wasm_pb.enable_steady_tick(std::time::Duration::from_millis(80));

        emit_wasm_to_dir(output_path)?;
        output_manifest.wasm = Some(WasmInfo {
            js: "sieve.js".to_string(),
            wasm: "sieve_bg.wasm".to_string(),
            types: "sieve.d.ts".to_string(),
        });

        #[cfg(all(feature = "parallel", feature = "embed-wasm"))]
        wasm_pb.finish_with_message("emitted");
    }

    if emit_wasm && cfg!(not(feature = "embed-wasm")) {
        return Err("--emit-wasm requires 'embed-wasm' feature. Build with: cargo build --features embed-wasm".to_string());
    }

    // 8. Write output manifest
    let manifest_json = serde_json::to_string_pretty(&output_manifest)
        .map_err(|e| format!("Failed to serialize manifest: {}", e))?;
    fs::write(output_path.join("manifest.json"), manifest_json)
        .map_err(|e| format!("Failed to write manifest: {}", e))?;

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

/// Emit embedded WASM files to a directory.
#[cfg(feature = "embed-wasm")]
fn emit_wasm_to_dir(output_path: &Path) -> Result<(), String> {
    const SIEVE_WASM: &[u8] = include_bytes!("../../pkg/sieve_bg.wasm");
    const SIEVE_JS: &str = include_str!("../../pkg/sieve.js");
    const SIEVE_DTS: &str = include_str!("../../pkg/sieve.d.ts");
    const SIEVE_WASM_DTS: &str = include_str!("../../pkg/sieve_bg.wasm.d.ts");

    // Write WASM binary
    let wasm_path = output_path.join("sieve_bg.wasm");
    fs::write(&wasm_path, SIEVE_WASM)
        .map_err(|e| format!("Failed to write sieve_bg.wasm: {}", e))?;
    eprintln!("  ✓ {}", wasm_path.display());

    // Write JS bindings
    let js_path = output_path.join("sieve.js");
    fs::write(&js_path, SIEVE_JS).map_err(|e| format!("Failed to write sieve.js: {}", e))?;
    eprintln!("  ✓ {}", js_path.display());

    // Write TypeScript declarations
    let dts_path = output_path.join("sieve.d.ts");
    fs::write(&dts_path, SIEVE_DTS).map_err(|e| format!("Failed to write sieve.d.ts: {}", e))?;
    eprintln!("  ✓ {}", dts_path.display());

    // Write WASM TypeScript declarations
    let wasm_dts_path = output_path.join("sieve_bg.wasm.d.ts");
    fs::write(&wasm_dts_path, SIEVE_WASM_DTS)
        .map_err(|e| format!("Failed to write sieve_bg.wasm.d.ts: {}", e))?;
    eprintln!("  ✓ {}", wasm_dts_path.display());

    Ok(())
}
