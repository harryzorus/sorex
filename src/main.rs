use clap::Parser;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{Read, Write};

use serde::Deserialize;
use sieve::binary::{
    encode_docs_binary, BinaryLayer, DocMetaInput, PostingEntry, SieveFooter, SieveHeader, VERSION,
};
use sieve::fst_index::build_fst_index;
use sieve::levenshtein_dfa::ParametricDFA;
use sieve::{build::*, build_index, FieldBoundary, SearchDoc, SearchIndex};

mod cli;
use cli::{Cli, Commands};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    #[serde(default)]
    field_boundaries: Vec<FieldBoundary>,
}

/// Input for binary layer encoding
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LayerInput {
    /// Number of documents
    doc_count: usize,
    /// Terms with their posting lists
    terms: HashMap<String, Vec<u32>>,
}

fn main() {
    // Try to parse as clap CLI first
    let cli = Cli::try_parse();

    if let Ok(cli) = cli {
        match cli.command {
            Some(Commands::Build {
                input,
                output,
                indexes,
                emit_wasm,
            }) => {
                if let Err(e) = run_build(&input, &output, indexes, emit_wasm) {
                    eprintln!("❌ {}", e);
                    std::process::exit(1);
                }
                return;
            }
            Some(Commands::Inspect { file }) => {
                inspect_sieve_file(&file);
                return;
            }
            None => {}
        }
    }

    // Fall back to legacy arg parsing for backwards compatibility
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        match args[1].as_str() {
            "--binary" => {
                // New mode: build full index and output binary format
                build_binary_index();
                return;
            }
            "--binary-layer" => {
                // Layer mode: convert terms to binary layer
                encode_binary_layer();
                return;
            }
            "--inspect" => {
                // Inspect mode: display .sieve file structure diagram
                if args.len() < 3 {
                    eprintln!("Usage: indexer --inspect <file.sieve>");
                    std::process::exit(1);
                }
                inspect_sieve_file(&args[2]);
                return;
            }
            #[cfg(feature = "embed-wasm")]
            "--demo" => {
                // Demo mode: generate complete demo package with WASM, JS, HTML
                let output_dir = args.get(2).map(String::as_str).unwrap_or("sieve-demo");
                generate_demo_package(output_dir);
                return;
            }
            #[cfg(feature = "embed-wasm")]
            "--emit-wasm" => {
                // Emit WASM files to directory (for site builds)
                if args.len() < 3 {
                    eprintln!("Usage: sieve --emit-wasm <output-dir>");
                    std::process::exit(1);
                }
                emit_wasm_files(&args[2]);
                return;
            }
            "--help" | "-h" => {
                print_help();
                return;
            }
            other => {
                eprintln!("Unknown option: {}", other);
                print_help();
                std::process::exit(1);
            }
        }
    }

    // Default: original indexer mode (JSON output)
    build_search_index();
}

fn print_help() {
    eprintln!("SIEVE - Full-text search index builder");
    eprintln!();
    eprintln!("USAGE:");
    eprintln!("    sieve [OPTIONS]");
    eprintln!();
    eprintln!("OPTIONS:");
    eprintln!("    (no args)          Read JSON payload from stdin, output JSON index");
    eprintln!("    --binary           Read JSON payload from stdin, output binary .sieve format");
    eprintln!("    --binary-layer     Read layer input from stdin, output binary .sieve format");
    eprintln!("    --inspect <FILE>   Display structure diagram of a .sieve file");
    #[cfg(feature = "embed-wasm")]
    eprintln!("    --emit-wasm <DIR>  Emit embedded WASM/JS/TypeScript files to directory");
    #[cfg(feature = "embed-wasm")]
    eprintln!("    --demo [DIR]       Generate complete demo package (index + WASM + HTML)");
    eprintln!("    --help, -h         Show this help message");
    eprintln!();
    eprintln!("EXAMPLES:");
    eprintln!("    sieve --inspect index.sieve         # Inspect file structure");
    eprintln!("    cat payload.json | sieve --binary   # Build binary index");
    #[cfg(feature = "embed-wasm")]
    eprintln!("    sieve --emit-wasm ./src/wasm        # Extract WASM for site build");
    #[cfg(feature = "embed-wasm")]
    eprintln!("    cat payload.json | sieve --demo     # Generate demo package");
}

/// Original indexer mode: reads JSON, outputs JSON search index
fn build_search_index() {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read stdin");
    let payload: Payload = serde_json::from_str(&raw).expect("invalid payload");

    let SearchIndex {
        docs,
        texts,
        suffix_array,
        lcp,
        field_boundaries,
        version,
    } = build_index(payload.docs, payload.texts, payload.field_boundaries);

    let serialized = serde_json::to_string(&SearchIndex {
        docs,
        texts,
        suffix_array,
        lcp,
        field_boundaries,
        version,
    })
    .expect("serialize index");
    std::io::stdout()
        .write_all(serialized.as_bytes())
        .expect("write stdout");
}

/// Binary mode: reads Payload JSON, outputs binary .sieve format with FST.
///
/// Uses build_fst_index() for parallel construction of:
/// - Inverted index (term → postings)
/// - Vocabulary suffix array (for prefix search)
/// - FST (for zero-CPU fuzzy search with Levenshtein DFA)
fn build_binary_index() {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read stdin");
    let payload: Payload = serde_json::from_str(&raw).expect("invalid payload");

    // Build FstIndex with parallel construction
    let fst_index = build_fst_index(
        payload.docs.clone(),
        payload.texts,
        payload.field_boundaries,
    );

    // Extract vocabulary (already sorted)
    let vocabulary = fst_index.vocabulary;

    // Convert vocab_suffix_array to (u32, u32) format
    let suffix_array: Vec<(u32, u32)> = fst_index
        .vocab_suffix_array
        .iter()
        .map(|e| (e.term_idx as u32, e.offset as u32))
        .collect();

    // Build section_id table (deduplicated) and convert postings to PostingEntry
    let mut section_id_set: HashSet<String> = HashSet::new();
    for pl in fst_index.inverted_index.terms.values() {
        for posting in &pl.postings {
            if let Some(ref id) = posting.section_id {
                section_id_set.insert(id.clone());
            }
        }
    }
    let section_table: Vec<String> = section_id_set.into_iter().collect();

    // Create section_id -> index mapping (1-indexed, 0 = no section)
    let section_idx_map: HashMap<&str, u32> = section_table
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), (i + 1) as u32))
        .collect();

    // Convert inverted index to postings array with section_id indices (in vocabulary order)
    let postings: Vec<Vec<PostingEntry>> = vocabulary
        .iter()
        .map(|term| {
            fst_index
                .inverted_index
                .terms
                .get(term)
                .map(|pl| {
                    pl.postings
                        .iter()
                        .map(|p| {
                            let section_idx = p
                                .section_id
                                .as_ref()
                                .and_then(|id| section_idx_map.get(id.as_str()))
                                .copied()
                                .unwrap_or(0);
                            PostingEntry {
                                doc_id: p.doc_id as u32,
                                section_idx,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    // Build Levenshtein DFA (precomputed tables for fuzzy search)
    let lev_dfa = ParametricDFA::build(true); // k=2 with transpositions
    let lev_dfa_bytes = lev_dfa.to_bytes();

    // Encode docs as binary with section_id support
    let docs_input: Vec<DocMetaInput> = payload
        .docs
        .iter()
        .map(|d| DocMetaInput {
            title: d.title.clone(),
            excerpt: d.excerpt.clone(),
            href: d.href.clone(),
            doc_type: d.kind.clone(),
            section_id: None, // Section ID set by build pipeline per layer
        })
        .collect();
    let docs_bytes = encode_docs_binary(&docs_input);

    // Build binary layer with section_id support (v6)
    let layer = BinaryLayer::build_v6(
        &vocabulary,
        &suffix_array,
        &postings,
        &section_table,
        payload.docs.len(),
        lev_dfa_bytes,
        docs_bytes,
    )
    .expect("failed to build binary layer");

    let bytes = layer.to_bytes().expect("failed to serialize binary layer");

    // Write binary to stdout (single file contains everything including docs)
    std::io::stdout().write_all(&bytes).expect("write stdout");
}

/// Binary layer mode: reads LayerInput JSON, outputs binary .sieve format
fn encode_binary_layer() {
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read stdin");

    let input: LayerInput = serde_json::from_str(&raw).expect("invalid layer input");

    // Sort vocabulary lexicographically (for binary search and suffix array)
    let mut vocabulary: Vec<String> = input.terms.keys().cloned().collect();
    vocabulary.sort();

    // Build postings in vocabulary order
    let postings: Vec<Vec<u32>> = vocabulary
        .iter()
        .map(|term| input.terms.get(term).cloned().unwrap_or_default())
        .collect();

    // Build suffix array over vocabulary (for prefix search)
    // Each entry: (term_ord, char_offset) sorted by suffix
    let mut suffix_array: Vec<(u32, u32)> = Vec::new();
    for (term_ord, term) in vocabulary.iter().enumerate() {
        for offset in 0..term.len() {
            suffix_array.push((term_ord as u32, offset as u32));
        }
    }

    // Sort by suffix
    suffix_array.sort_by(|a, b| {
        let suffix_a = &vocabulary[a.0 as usize][a.1 as usize..];
        let suffix_b = &vocabulary[b.0 as usize][b.1 as usize..];
        suffix_a.cmp(suffix_b)
    });

    // Build Levenshtein DFA (precomputed tables for fuzzy search)
    let lev_dfa = ParametricDFA::build(true); // k=2 with transpositions
    let lev_dfa_bytes = lev_dfa.to_bytes();

    // Empty docs for layer-only mode (docs not provided in this mode)
    let docs_bytes = encode_docs_binary(&[]);

    // Build binary layer (vocabulary stored directly, no FST)
    let layer = BinaryLayer::build(
        &vocabulary,
        &suffix_array,
        &postings,
        input.doc_count,
        lev_dfa_bytes,
        docs_bytes,
    )
    .expect("failed to build binary layer");

    let bytes = layer.to_bytes().expect("failed to serialize binary layer");

    // Write binary to stdout
    std::io::stdout().write_all(&bytes).expect("write stdout");
}

/// Inspect a .sieve file and display its structure diagram
fn inspect_sieve_file(path: &str) {
    let bytes = fs::read(path).expect("failed to read file");
    let total_size = bytes.len();

    // Validate minimum size (use smaller v2 header size for compatibility)
    let min_header_size = 36; // v2 header size
    let min_size = min_header_size + SieveFooter::SIZE;
    if total_size < min_size {
        eprintln!(
            "Error: File too small ({} bytes, minimum {})",
            total_size, min_size
        );
        std::process::exit(1);
    }

    // Read header magic and version first
    let version = bytes[4];

    // Read header based on version
    let (header, header_size) = if version >= 3 {
        let header =
            SieveHeader::read(&mut std::io::Cursor::new(&bytes)).expect("failed to read header");
        (header, SieveHeader::SIZE)
    } else {
        // For v2 and earlier, construct header with lev_dfa_len = 0
        // v2 header is 36 bytes (no lev_dfa_len)
        let mut cursor = std::io::Cursor::new(&bytes);
        let mut magic = [0u8; 4];
        std::io::Read::read_exact(&mut cursor, &mut magic).expect("failed to read magic");
        if magic != [0x53, 0x49, 0x46, 0x54] {
            eprintln!("Error: Invalid magic bytes (expected SIEVE)");
            std::process::exit(1);
        }

        let mut buf = [0u8; 32]; // 36 - 4 (magic) = 32 for v2
        std::io::Read::read_exact(&mut cursor, &mut buf).expect("failed to read header");

        let header = SieveHeader {
            version: buf[0],
            flags: sieve::binary::FormatFlags::default(),
            doc_count: u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]),
            term_count: u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]),
            vocab_len: u32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]),
            sa_len: u32::from_le_bytes([buf[14], buf[15], buf[16], buf[17]]),
            postings_len: u32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]),
            skip_len: u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]),
            section_table_len: u32::from_le_bytes([buf[26], buf[27], buf[28], buf[29]]), // Was fst_len in v2/v3, section_table_len in v6
            lev_dfa_len: 0, // v2 doesn't have this field
            docs_len: 0,    // v2 doesn't have this field
        };
        (header, 36) // v2 header is 36 bytes
    };

    // Validate footer and CRC32
    let footer = SieveFooter::read(&bytes).expect("failed to read footer");
    let content = &bytes[..bytes.len() - SieveFooter::SIZE];
    let computed_crc = SieveFooter::compute_crc32(content);
    let crc_valid = footer.crc32 == computed_crc;

    // Calculate section offsets
    let header_end = header_size;
    let vocab_end = header_end + header.vocab_len as usize;
    let sa_end = vocab_end + header.sa_len as usize;
    let postings_end = sa_end + header.postings_len as usize;
    let skip_end = postings_end + header.skip_len as usize;
    let section_table_end = skip_end + header.section_table_len as usize; // Was fst_len in v2/v3, section_table_len in v6
    let lev_dfa_end = section_table_end + header.lev_dfa_len as usize;
    let docs_end = lev_dfa_end + header.docs_len as usize;

    // Section info for display
    struct Section {
        name: &'static str,
        size: usize,
        offset: usize,
    }

    let mut sections = vec![
        Section {
            name: "HEADER",
            size: header_size,
            offset: 0,
        },
        Section {
            name: "VOCABULARY",
            size: header.vocab_len as usize,
            offset: header_end,
        },
        Section {
            name: "SUFFIX ARRAY",
            size: header.sa_len as usize,
            offset: vocab_end,
        },
        Section {
            name: "POSTINGS",
            size: header.postings_len as usize,
            offset: sa_end,
        },
        Section {
            name: "SKIP LISTS",
            size: header.skip_len as usize,
            offset: postings_end,
        },
        Section {
            name: "SECTION TABLE",
            size: header.section_table_len as usize,
            offset: skip_end,
        },
    ];

    // Only add LEV DFA section for v3+
    if header.version >= 3 {
        sections.push(Section {
            name: "LEV DFA",
            size: header.lev_dfa_len as usize,
            offset: section_table_end,
        });
    }

    // Only add DOCS section for v5+
    if header.version >= 5 {
        sections.push(Section {
            name: "DOCS",
            size: header.docs_len as usize,
            offset: lev_dfa_end,
        });
        sections.push(Section {
            name: "FOOTER",
            size: SieveFooter::SIZE,
            offset: docs_end,
        });
    } else if header.version >= 3 {
        sections.push(Section {
            name: "FOOTER",
            size: SieveFooter::SIZE,
            offset: lev_dfa_end,
        });
    } else {
        sections.push(Section {
            name: "FOOTER",
            size: SieveFooter::SIZE,
            offset: section_table_end,
        });
    }

    // Box width constant (inner content width)
    const W: usize = 68;

    // Print header info
    println!();
    println!("╔{}╗", "═".repeat(W));
    println!("║{:^w$}║", "SIEVE FILE INSPECTOR", w = W);
    println!("╠{}╣", "═".repeat(W));
    println!("║  File:     {:<55}  ║", truncate_path(path, 55));
    println!("║  Size:     {:<55}  ║", format_size(total_size));
    println!(
        "║  Version:  {:<55}  ║",
        format!("{} (current: {})", header.version, VERSION)
    );
    println!("╚{}╝", "═".repeat(W));
    println!();

    // Print metadata
    println!("┌─ METADATA {}┐", "─".repeat(W - 12));
    println!(
        "│  Documents:      {:>10}{:>w$}│",
        header.doc_count,
        "",
        w = W - 30
    );
    println!(
        "│  Terms:          {:>10}{:>w$}│",
        header.term_count,
        "",
        w = W - 30
    );
    println!(
        "│  CRC32:          {:#010x} {:<7}{:>w$}│",
        footer.crc32,
        if crc_valid { "✓ valid" } else { "✗ BAD" },
        "",
        w = W - 38
    );
    println!(
        "│  Skip Lists:     {:>10}{:>w$}│",
        if header.flags.has_skip_lists() {
            "yes"
        } else {
            "no"
        },
        "",
        w = W - 30
    );
    println!("└{}┘", "─".repeat(W));
    println!();

    // Print structure diagram
    println!("┌─ BINARY STRUCTURE {}┐", "─".repeat(W - 19));
    println!("│{:>w$}│", "", w = W);

    // Find the maximum section size for bar scaling
    let max_size = sections.iter().map(|s| s.size).max().unwrap_or(1);
    let bar_width = 30;

    for section in &sections {
        let pct = (section.size as f64 / total_size as f64) * 100.0;
        let bar_len = if max_size > 0 && section.size > 0 {
            ((section.size as f64 / max_size as f64 * bar_width as f64) as usize).max(1)
        } else {
            0
        };

        let bar: String = "█".repeat(bar_len);
        let empty: String = "░".repeat(bar_width - bar_len);

        println!(
            "│  {:<12} │{}{}│ {:>8} {:>6.1}%  │",
            section.name,
            bar,
            empty,
            format_size(section.size),
            pct
        );
    }

    println!("│{:>w$}│", "", w = W);
    println!("├─ OFFSETS {}┤", "─".repeat(W - 11));
    println!("│{:>w$}│", "", w = W);
    println!(
        "│  {:<12} {:>10}  {:>10}  {:>10}{:>w$}│",
        "SECTION",
        "OFFSET",
        "LENGTH",
        "END",
        "",
        w = W - 50
    );
    println!(
        "│  {:<12} {:>10}  {:>10}  {:>10}{:>w$}│",
        "────────────",
        "──────────",
        "──────────",
        "──────────",
        "",
        w = W - 50
    );

    for section in &sections {
        let end = section.offset + section.size;
        println!(
            "│  {:<12} {:>10}  {:>10}  {:>10}{:>w$}│",
            section.name,
            format!("0x{:06X}", section.offset),
            format_size(section.size),
            format!("0x{:06X}", end),
            "",
            w = W - 50
        );
    }

    println!("│{:>w$}│", "", w = W);
    println!("└{}┘", "─".repeat(W));
    println!();

    // Print size breakdown
    let content_size = header.vocab_len
        + header.sa_len
        + header.postings_len
        + header.skip_len
        + header.section_table_len
        + header.lev_dfa_len
        + header.docs_len;
    let overhead = header_size + SieveFooter::SIZE;

    let largest_name = sections
        .iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER")
        .max_by_key(|s| s.size)
        .map(|s| s.name)
        .unwrap_or("N/A");
    let largest_size = sections
        .iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER")
        .map(|s| s.size)
        .max()
        .unwrap_or(0);
    let smallest_name = sections
        .iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER" && s.size > 0)
        .min_by_key(|s| s.size)
        .map(|s| s.name)
        .unwrap_or("N/A");
    let smallest_size = sections
        .iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER" && s.size > 0)
        .map(|s| s.size)
        .min()
        .unwrap_or(0);

    println!("┌─ SIZE BREAKDOWN {}┐", "─".repeat(W - 17));
    println!(
        "│  Content:   {:>10}  ({:.1}% of total){:>w$}│",
        format_size(content_size as usize),
        content_size as f64 / total_size as f64 * 100.0,
        "",
        w = W - 42
    );
    println!(
        "│  Overhead:  {:>10}  (header + footer){:>w$}│",
        format_size(overhead),
        "",
        w = W - 42
    );
    println!("│{:>w$}│", "", w = W);
    println!(
        "│  Largest:   {:>10}  {:<12}{:>w$}│",
        format_size(largest_size),
        largest_name,
        "",
        w = W - 40
    );
    println!(
        "│  Smallest:  {:>10}  {:<12}{:>w$}│",
        format_size(smallest_size),
        smallest_name,
        "",
        w = W - 40
    );
    println!("└{}┘", "─".repeat(W));
    println!();
}

/// Format bytes as human-readable size
fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

/// Truncate a path to fit in the given width
fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - max_len + 3..])
    }
}

// Embedded WASM and JS assets (built with wasm-pack)
// Only included when embed-wasm feature is enabled
#[cfg(feature = "embed-wasm")]
const SIEVE_WASM: &[u8] = include_bytes!("../pkg/sieve_bg.wasm");
#[cfg(feature = "embed-wasm")]
const SIEVE_JS: &str = include_str!("../pkg/sieve.js");
#[cfg(feature = "embed-wasm")]
const SIEVE_DTS: &str = include_str!("../pkg/sieve.d.ts");
#[cfg(feature = "embed-wasm")]
const SIEVE_WASM_DTS: &str = include_str!("../pkg/sieve_bg.wasm.d.ts");

/// Generate a complete demo package with index, WASM, JS, and HTML
#[cfg(feature = "embed-wasm")]
fn generate_demo_package(output_dir: &str) {
    use std::path::Path;

    // Create output directory
    fs::create_dir_all(output_dir).expect("failed to create output directory");

    // Read input from stdin
    let mut raw = String::new();
    std::io::stdin()
        .read_to_string(&mut raw)
        .expect("failed to read stdin");

    // Use sample data if stdin is empty
    let payload: Payload = if raw.trim().is_empty() {
        eprintln!("No input provided, using sample data...");
        create_sample_payload()
    } else {
        serde_json::from_str(&raw).expect("invalid payload JSON")
    };

    let doc_count = payload.docs.len();

    // Build binary index
    let fst_index = build_fst_index(
        payload.docs.clone(),
        payload.texts,
        payload.field_boundaries,
    );

    let vocabulary = fst_index.vocabulary;
    let suffix_array: Vec<(u32, u32)> = fst_index
        .vocab_suffix_array
        .iter()
        .map(|e| (e.term_idx as u32, e.offset as u32))
        .collect();

    // Build section_id table (deduplicated) for v6 format
    let mut section_id_set: HashSet<String> = HashSet::new();
    for pl in fst_index.inverted_index.terms.values() {
        for posting in &pl.postings {
            if let Some(ref id) = posting.section_id {
                section_id_set.insert(id.clone());
            }
        }
    }
    let section_table: Vec<String> = section_id_set.into_iter().collect();

    // Create section_id -> index mapping (1-indexed, 0 = no section)
    let section_idx_map: HashMap<&str, u32> = section_table
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), (i + 1) as u32))
        .collect();

    // Convert postings to v6 format with section_ids
    let postings: Vec<Vec<PostingEntry>> = vocabulary
        .iter()
        .map(|term| {
            fst_index
                .inverted_index
                .terms
                .get(term)
                .map(|pl| {
                    pl.postings
                        .iter()
                        .map(|p| {
                            let section_idx = p
                                .section_id
                                .as_ref()
                                .and_then(|id| section_idx_map.get(id.as_str()))
                                .copied()
                                .unwrap_or(0);
                            PostingEntry {
                                doc_id: p.doc_id as u32,
                                section_idx,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    let lev_dfa = ParametricDFA::build(true);
    let lev_dfa_bytes = lev_dfa.to_bytes();

    let docs_input: Vec<DocMetaInput> = payload
        .docs
        .iter()
        .map(|d| DocMetaInput {
            title: d.title.clone(),
            excerpt: d.excerpt.clone(),
            href: d.href.clone(),
            doc_type: d.kind.clone(),
            section_id: None, // Section ID set per-posting in v6 format
        })
        .collect();
    let docs_bytes = encode_docs_binary(&docs_input);

    let layer = BinaryLayer::build_v6(
        &vocabulary,
        &suffix_array,
        &postings,
        &section_table,
        doc_count,
        lev_dfa_bytes,
        docs_bytes,
    )
    .expect("failed to build binary layer");

    let index_bytes = layer.to_bytes().expect("failed to serialize index");

    // Write files to output directory
    let dir = Path::new(output_dir);

    // 1. Write .sieve index
    let index_path = dir.join("index.sieve");
    fs::write(&index_path, &index_bytes).expect("failed to write index.sieve");
    eprintln!(
        "  ✓ Created {} ({} bytes)",
        index_path.display(),
        index_bytes.len()
    );

    // 2. Write WASM file
    let wasm_path = dir.join("sieve_bg.wasm");
    fs::write(&wasm_path, SIEVE_WASM).expect("failed to write sieve_bg.wasm");
    eprintln!(
        "  ✓ Created {} ({} bytes)",
        wasm_path.display(),
        SIEVE_WASM.len()
    );

    // 3. Write JS file
    let js_path = dir.join("sieve.js");
    fs::write(&js_path, SIEVE_JS).expect("failed to write sieve.js");
    eprintln!(
        "  ✓ Created {} ({} bytes)",
        js_path.display(),
        SIEVE_JS.len()
    );

    // 4. Write demo HTML
    let html_path = dir.join("index.html");
    fs::write(&html_path, generate_demo_html(doc_count)).expect("failed to write index.html");
    eprintln!("  ✓ Created {}", html_path.display());

    eprintln!();
    eprintln!("✅ Demo package created in '{}'", output_dir);
    eprintln!();
    eprintln!("To test locally:");
    eprintln!("  cd {} && python3 -m http.server 8080", output_dir);
    eprintln!("  open http://localhost:8080");
}

/// Create sample payload for demo using European countries dataset
#[cfg(feature = "embed-wasm")]
fn create_sample_payload() -> Payload {
    let countries = vec![
        ("Austria", "A landlocked country in Central Europe", "Austria is a landlocked country in Central Europe. Vienna, the capital and largest city, is the cultural and political center of the country. Austria is known for its Alpine scenery, classical music heritage, and contributions to European culture. The country is located in the eastern Alps and has a population of about 9 million. Austria joined the European Union in 1995 and is part of the Eurozone since 1999. The Danube River flows through Austria and is an important waterway. Austria is famous for composers like Wolfgang Amadeus Mozart and Ludwig van Beethoven who spent significant time there."),
        ("Belgium", "A federal monarchy in Western Europe", "Belgium is a federal monarchy in Western Europe known for its beer, chocolate, and waffles. Brussels, the capital, is the de facto capital of the European Union. Belgium has a population of approximately 11.5 million and is divided into three regions: Flanders, Wallonia, and Brussels. The country is famous for its medieval architecture, including the Belfry of Bruges and the Grand Place in Brussels. Belgium shares borders with the Netherlands, Germany, Luxembourg, and France. The Belgian language situation is complex with Dutch, French, and German all being official languages in different regions."),
        ("Bulgaria", "A country on the Balkan Peninsula", "Bulgaria is a country on the Balkan Peninsula in Southeast Europe. Sofia, the capital and largest city, is located in the west-central part of the country. Bulgaria has a rich history spanning over 1,400 years as a nation state. The country is known for its natural beauty, including the Rila Mountains and the Black Sea coast. Bulgaria is famous for its rose oil production, Orthodox Christian heritage, and ancient Thracian culture. The Danube River forms Bulgaria's northern border with Romania. Bulgaria joined the European Union in 2007 and is part of the Schengen Area."),
        ("Croatia", "A Mediterranean country in Southeast Europe", "Croatia is a Mediterranean country in Southeast Europe with a Pannonian plain in the north. Zagreb is the capital and largest city, located in the northwestern part of the country. Croatia has a population of about 4 million and is known for its Adriatic coast and numerous islands. The country is famous for Dubrovnik, a UNESCO World Heritage Site often called the Pearl of the Adriatic. Croatia joined the European Union in 2013 and adopted the Euro in 2023. The country is known for its natural parks, including Plitvice Lakes and Krka National Park. Croatia has a strong tourism industry due to its beautiful coastline and historical sites."),
        ("Cyprus", "An island nation in the Mediterranean Sea", "Cyprus is an island nation in the eastern Mediterranean Sea. Nicosia, the capital, is the most populated city on the island. Cyprus has a population of about 1.2 million and is known for its Mediterranean beaches and year-round sunshine. The island has been inhabited since the Neolithic period and has a rich cultural heritage spanning Greek, Roman, Byzantine, and Ottoman periods. Cyprus joined the European Union in 2004 and adopted the Euro in 2008. The island is famous for its copper resources, wine production, and tourist beaches. Cyprus is divided between the Republic of Cyprus in the south and the Turkish Republic of Northern Cyprus in the north."),
        ("Czech Republic", "A country in Central Europe with a rich history", "The Czech Republic is a country in Central Europe with a rich medieval history. Prague, the capital, is known as the City of a Hundred Spires and is famous for its architecture and cultural heritage. The country has a population of about 10.5 million and is known for its beer, crystal glass, and automotive industry. Prague Castle is one of the largest castle complexes in the world. The Czech Republic joined the European Union in 2004 and the Schengen Area in 2007. The country is located in the Bohemian and Moravian plateaus. Charles Bridge in Prague is one of the oldest bridges in Europe and a major tourist attraction."),
        ("Denmark", "A Nordic country in Northern Europe", "Denmark is a Nordic country in Northern Europe consisting of the Jutland Peninsula and numerous islands. Copenhagen, the capital, is the largest city and is located on the eastern coast of Zealand. Denmark has a population of about 5.9 million and is known for its flat landscape, strong winds, and extensive coastline. The country is famous for its design, wind energy, and bicycle culture. Denmark is part of the Nordic countries and joined the European Union in 1973. The Little Mermaid statue in Copenhagen is a famous symbol of the city. Denmark has a strong welfare system and consistently ranks high in quality of life indices."),
        ("Estonia", "A Baltic country in Northern Europe", "Estonia is a Baltic country in Northern Europe on the eastern shore of the Baltic Sea. Tallinn, the capital, is known for its well-preserved Old Town, a UNESCO World Heritage Site. Estonia has a population of about 1.4 million and is known for its forests, lakes, and digital innovation. The country is famous for being a digital society with e-government, e-banking, and e-services. Estonia joined the European Union in 2004 and adopted the Euro in 2011. The country has a rich cultural heritage with many traditional songs and dances. Estonia is one of the most wired countries in the world with widespread internet access."),
        ("Finland", "A Nordic country known for lakes and forests", "Finland is a Nordic country in Northern Europe known for its thousands of lakes and vast forests. Helsinki, the capital, is located on the Gulf of Finland and is known for its modern architecture and design. Finland has a population of about 5.5 million and is famous for its education system, Nokia technology, and music industry. The country is known for the Northern Lights (Aurora Borealis) and winter sports. Finland joined the European Union in 1995 and adopted the Euro in 1999. Finland recently joined NATO in 2023. The country is known for its sauna culture with more saunas than any other country in the world."),
        ("France", "A major Western European nation", "France is a major Western European nation and one of the world's most influential countries. Paris, the capital, is known as the City of Light and is famous for its art, fashion, and architecture. France has a population of about 68 million and is known for its wine, cheese, and culinary traditions. The Eiffel Tower, built for the 1889 World's Fair, is one of the most recognizable landmarks in the world. France joined the European Union (as the European Economic Community) in 1957 and is a founding member. The country is known for its contributions to art, literature, philosophy, and science. France has the largest economy in Europe and is a permanent member of the United Nations Security Council."),
        ("Germany", "A major central European power", "Germany is a major central European country and one of the world's largest economies. Berlin, the capital, is known for its history, art scene, and nightlife. Germany has a population of about 83 million and is known for its engineering, automotive industry, and beer. The Brandenburg Gate is a famous symbol of German history and reunification. Germany joined the European Union in 1993 (with reunification in 1990) and adopted the Euro in 2002. The country is known for its contributions to philosophy, music (Bach, Beethoven), and science. Germany is a federal republic with a strong welfare system and is Europe's largest economy."),
        ("Greece", "A Mediterranean country with ancient history", "Greece is a Mediterranean country with an ancient history spanning thousands of years. Athens, the capital, is known as the birthplace of Western democracy and philosophy. Greece has a population of about 10.5 million and is famous for its islands, beaches, and classical ruins. The Acropolis in Athens is one of the most important landmarks of ancient Greek civilization. Greece joined the European Union in 1981 and adopted the Euro in 2001. The country consists of the mainland and over 6,000 islands, of which about 227 are inhabited. Greece is known for its Mediterranean cuisine, Orthodox Christianity, and contributions to Western civilization."),
        ("Hungary", "A country in Central Europe on the Danube", "Hungary is a country in Central Europe situated along the Danube River. Budapest, the capital, is known as the Pearl of the Danube and is famous for its thermal baths and architecture. Hungary has a population of about 9.7 million and is known for its music, wine, and paprika spice. The Parliament Building in Budapest is one of the largest buildings in Europe and a UNESCO World Heritage Site. Hungary joined the European Union in 2004. The country has a rich musical heritage with famous composers like Ferenc Liszt and Béla Bartók. Lake Balaton is the largest lake in Central Europe and a popular tourist destination."),
        ("Iceland", "An island nation in the North Atlantic", "Iceland is an island nation in the North Atlantic Ocean, located midway between the United States and mainland Europe. Reykjavik, the capital, is one of the world's northernmost capital cities of a sovereign state. Iceland has a population of about 380,000 and is known for its dramatic landscapes, waterfalls, and geysers. The Golden Circle is a popular tourist route featuring geysers, hot springs, and waterfalls. Iceland is famous for its Viking heritage, sagas, and literary culture. The country is powered almost entirely by renewable energy, mostly geothermal and hydroelectric power. Iceland is not a member of the European Union but is part of the European Economic Area."),
        ("Ireland", "An island nation in Western Europe", "Ireland is an island nation in Western Europe, consisting of the Irish Free State (independent) and the island of Ireland. Dublin, the capital, is known for its Georgian architecture, literary heritage, and vibrant culture. Ireland has a population of about 5.2 million and is known for its literature, music, and pubs. The Cliffs of Moher are one of Ireland's most visited natural attractions. Ireland joined the European Union in 1973 and adopted the Euro in 2002. The country is known for its contributions to literature with famous writers like James Joyce and Samuel Beckett. Ireland is known for its strong tradition of storytelling, music, and traditional Irish dancing."),
        ("Italy", "A Mediterranean country with a vast cultural heritage", "Italy is a Mediterranean country in Southern Europe with a vast cultural heritage and history. Rome, the capital, is known as the Eternal City and was the capital of the Roman Empire. Italy has a population of about 57 million and is famous for its art, architecture, and cuisine. The Colosseum in Rome is one of the most recognizable landmarks of ancient Roman civilization. Italy joined the European Union in 1957 (as the European Economic Community) and adopted the Euro in 2002. Italy is known for its fashion industry, fine wines, and iconic cars like Ferrari and Lamborghini. The country consists of the mainland and numerous islands including Sicily and Sardinia."),
    ];

    let mut docs = Vec::new();
    let mut texts = Vec::new();

    for (idx, (name, excerpt, text)) in countries.iter().enumerate() {
        docs.push(SearchDoc {
            id: idx,
            title: name.to_string(),
            excerpt: excerpt.to_string(),
            href: format!("/countries/{}", name.to_lowercase().replace(' ', "-")),
            kind: "page".to_string(),
        });
        texts.push(text.to_string());
    }

    Payload {
        docs,
        texts,
        field_boundaries: vec![],
    }
}

/// Generate demo HTML page with European countries dataset
#[cfg(feature = "embed-wasm")]
fn generate_demo_html(doc_count: usize) -> String {
    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sieve Search Demo - European Countries</title>
    <style>
        * {{ box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 900px;
            margin: 0 auto;
            padding: 2rem;
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            min-height: 100vh;
        }}
        .container {{
            background: white;
            border-radius: 12px;
            padding: 2rem;
            box-shadow: 0 20px 60px rgba(0,0,0,0.3);
        }}
        h1 {{ 
            color: #333;
            margin: 0 0 0.5rem 0;
            font-size: 2.5rem;
        }}
        .subtitle {{
            color: #666;
            margin-bottom: 2rem;
            font-size: 1.1rem;
        }}
        .search-box {{
            display: flex;
            gap: 0.5rem;
            margin: 2rem 0;
        }}
        input[type="text"] {{
            flex: 1;
            padding: 1rem;
            font-size: 1.1rem;
            border: 2px solid #ddd;
            border-radius: 8px;
            outline: none;
            transition: all 0.3s;
        }}
        input[type="text"]:focus {{
            border-color: #667eea;
            box-shadow: 0 0 0 3px rgba(102, 126, 234, 0.1);
        }}
        .results {{
            background: white;
            border-radius: 8px;
            overflow: hidden;
        }}
        .result {{
            padding: 1.5rem;
            border-bottom: 1px solid #eee;
            transition: background 0.2s;
        }}
        .result:hover {{
            background: #f9f9f9;
        }}
        .result:last-child {{ border-bottom: none; }}
        .result h3 {{
            margin: 0 0 0.5rem 0;
            color: #667eea;
            font-size: 1.3rem;
        }}
        .result h3 a {{
            text-decoration: none;
            color: inherit;
        }}
        .result h3 a:hover {{
            text-decoration: underline;
        }}
        .result p {{
            margin: 0 0 0.5rem 0;
            color: #555;
            line-height: 1.6;
        }}
        .result .meta {{
            font-size: 0.85rem;
            color: #999;
            display: flex;
            gap: 1rem;
        }}
        .empty {{
            padding: 3rem 2rem;
            text-align: center;
            color: #999;
        }}
        .stats {{
            font-size: 0.95rem;
            color: #666;
            margin-bottom: 1.5rem;
            padding: 1rem;
            background: #f0f4ff;
            border-radius: 8px;
            border-left: 4px solid #667eea;
        }}
        .suggestions {{
            margin-top: 2rem;
            padding-top: 2rem;
            border-top: 1px solid #eee;
        }}
        .suggestions h3 {{
            color: #333;
            font-size: 0.9rem;
            margin-bottom: 0.5rem;
        }}
        .tags {{
            display: flex;
            flex-wrap: wrap;
            gap: 0.5rem;
        }}
        .tag {{
            display: inline-block;
            padding: 0.5rem 1rem;
            background: #f0f4ff;
            color: #667eea;
            border-radius: 20px;
            cursor: pointer;
            font-size: 0.85rem;
            transition: all 0.2s;
        }}
        .tag:hover {{
            background: #667eea;
            color: white;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🌍 European Countries Search</h1>
        <p class="subtitle">Formally verified full-text search with {} documents</p>

        <div class="search-box">
            <input type="text" id="query" placeholder="Search countries... (try: Alps, Mediterranean, culture, capital)" autofocus>
        </div>

        <div class="stats" id="stats"></div>
        <div class="results" id="results">
            <div class="empty">Start typing to search for European countries...</div>
        </div>

        <div class="suggestions">
            <h3>Try searching for:</h3>
            <div class="tags">
                <span class="tag" onclick="document.getElementById('query').value='Alps'; search()">Alps</span>
                <span class="tag" onclick="document.getElementById('query').value='Mediterranean'; search()">Mediterranean</span>
                <span class="tag" onclick="document.getElementById('query').value='EU'; search()">European Union</span>
                <span class="tag" onclick="document.getElementById('query').value='culture'; search()">Culture</span>
                <span class="tag" onclick="document.getElementById('query').value='capital'; search()">Capital Cities</span>
                <span class="tag" onclick="document.getElementById('query').value='history'; search()">History</span>
                <span class="tag" onclick="document.getElementById('query').value='mountains'; search()">Mountains</span>
                <span class="tag" onclick="document.getElementById('query').value='island'; search()">Islands</span>
            </div>
        </div>
    </div>

    <script type="module">
        import init, {{ SieveIndex }} from './sieve.js';

        let index = null;

        async function setup() {{
            // Initialize WASM
            await init();

            // Load the search index
            const response = await fetch('./index.sieve');
            const bytes = new Uint8Array(await response.arrayBuffer());
            index = SieveIndex.from_bytes(bytes);

            document.getElementById('query').addEventListener('input', debounce(search, 150));
            console.log('Sieve initialized with', index.doc_count(), 'European country documents');
        }}

        function debounce(fn, ms) {{
            let timeout;
            return (...args) => {{
                clearTimeout(timeout);
                timeout = setTimeout(() => fn(...args), ms);
            }};
        }}

        window.search = function search(e) {{
            const query = e?.target?.value?.trim?.() || document.getElementById('query').value?.trim?.() || '';
            const resultsDiv = document.getElementById('results');
            const statsDiv = document.getElementById('stats');

            if (!query) {{
                resultsDiv.innerHTML = '<div class="empty">Start typing to search for European countries...</div>';
                statsDiv.textContent = '';
                return;
            }}

            const start = performance.now();
            const results = index.search(query, 10);
            const elapsed = (performance.now() - start).toFixed(2);

            statsDiv.textContent = `Found ${{results.length}} result${{results.length !== 1 ? 's' : ''}} in ${{elapsed}}ms`;

            if (results.length === 0) {{
                resultsDiv.innerHTML = '<div class="empty">No countries found matching "${{escapeHtml(query)}}"</div>';
                return;
            }}

            resultsDiv.innerHTML = results.map(r => `
                <div class="result">
                    <h3><a href="${{r.href}}">${{escapeHtml(r.title)}}</a></h3>
                    <p>${{escapeHtml(r.excerpt)}}</p>
                    <div class="meta">
                        <span>Relevance: ${{r.score.toFixed(1)}}</span>
                        <span>Type: ${{r.kind}}</span>
                    </div>
                </div>
            `).join('');
        }};

        function escapeHtml(text) {{
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }}

        setup().catch(console.error);
    </script>
</body>
</html>
"#,
        doc_count
    )
}

/// Emit embedded WASM files to a directory.
///
/// Writes all files needed for WASM integration:
/// - sieve_bg.wasm - the WebAssembly binary
/// - sieve.js - JavaScript bindings
/// - sieve.d.ts - TypeScript type declarations
/// - sieve_bg.wasm.d.ts - WASM export declarations
#[cfg(feature = "embed-wasm")]
fn emit_wasm_files(output_dir: &str) {
    use std::path::Path;

    // Create output directory
    fs::create_dir_all(output_dir).expect("failed to create output directory");

    let dir = Path::new(output_dir);

    // Write WASM binary
    let wasm_path = dir.join("sieve_bg.wasm");
    fs::write(&wasm_path, SIEVE_WASM).expect("failed to write sieve_bg.wasm");
    eprintln!("  ✓ {}", wasm_path.display());

    // Write JS bindings
    let js_path = dir.join("sieve.js");
    fs::write(&js_path, SIEVE_JS).expect("failed to write sieve.js");
    eprintln!("  ✓ {}", js_path.display());

    // Write TypeScript declarations
    let dts_path = dir.join("sieve.d.ts");
    fs::write(&dts_path, SIEVE_DTS).expect("failed to write sieve.d.ts");
    eprintln!("  ✓ {}", dts_path.display());

    // Write WASM TypeScript declarations
    let wasm_dts_path = dir.join("sieve_bg.wasm.d.ts");
    fs::write(&wasm_dts_path, SIEVE_WASM_DTS).expect("failed to write sieve_bg.wasm.d.ts");
    eprintln!("  ✓ {}", wasm_dts_path.display());

    eprintln!();
    eprintln!("✅ WASM files emitted to '{}'", output_dir);
}
