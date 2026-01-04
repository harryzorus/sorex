use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::io::{Read, Write};

use sieve::binary::{encode_docs_binary, BinaryLayer, DocMetaInput, PostingEntry, SieveHeader, SieveFooter, VERSION};
use sieve::fst_index::build_fst_index;
use sieve::levenshtein_dfa::ParametricDFA;
use sieve::{build_index, FieldBoundary, SearchDoc, SearchIndex};
use serde::Deserialize;

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
    let suffix_array: Vec<(u32, u32)> = fst_index.vocab_suffix_array
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
            fst_index.inverted_index.terms
                .get(term)
                .map(|pl| {
                    pl.postings.iter().map(|p| {
                        let section_idx = p.section_id
                            .as_ref()
                            .and_then(|id| section_idx_map.get(id.as_str()))
                            .copied()
                            .unwrap_or(0);
                        PostingEntry {
                            doc_id: p.doc_id as u32,
                            section_idx,
                        }
                    }).collect()
                })
                .unwrap_or_default()
        })
        .collect();

    // Build Levenshtein DFA (precomputed tables for fuzzy search)
    let lev_dfa = ParametricDFA::build(true); // k=2 with transpositions
    let lev_dfa_bytes = lev_dfa.to_bytes();

    // Encode docs as binary with section_id support
    let docs_input: Vec<DocMetaInput> = payload.docs
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
    std::io::stdout()
        .write_all(&bytes)
        .expect("write stdout");
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
    let layer = BinaryLayer::build(&vocabulary, &suffix_array, &postings, input.doc_count, lev_dfa_bytes, docs_bytes)
        .expect("failed to build binary layer");

    let bytes = layer.to_bytes().expect("failed to serialize binary layer");

    // Write binary to stdout
    std::io::stdout()
        .write_all(&bytes)
        .expect("write stdout");
}

/// Inspect a .sieve file and display its structure diagram
fn inspect_sieve_file(path: &str) {
    let bytes = fs::read(path).expect("failed to read file");
    let total_size = bytes.len();

    // Validate minimum size (use smaller v2 header size for compatibility)
    let min_header_size = 36; // v2 header size
    let min_size = min_header_size + SieveFooter::SIZE;
    if total_size < min_size {
        eprintln!("Error: File too small ({} bytes, minimum {})", total_size, min_size);
        std::process::exit(1);
    }

    // Read header magic and version first
    let version = bytes[4];

    // Read header based on version
    let (header, header_size) = if version >= 3 {
        let header = SieveHeader::read(&mut std::io::Cursor::new(&bytes))
            .expect("failed to read header");
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
        Section { name: "HEADER", size: header_size, offset: 0 },
        Section { name: "VOCABULARY", size: header.vocab_len as usize, offset: header_end },
        Section { name: "SUFFIX ARRAY", size: header.sa_len as usize, offset: vocab_end },
        Section { name: "POSTINGS", size: header.postings_len as usize, offset: sa_end },
        Section { name: "SKIP LISTS", size: header.skip_len as usize, offset: postings_end },
        Section { name: "SECTION TABLE", size: header.section_table_len as usize, offset: skip_end },
    ];

    // Only add LEV DFA section for v3+
    if header.version >= 3 {
        sections.push(Section { name: "LEV DFA", size: header.lev_dfa_len as usize, offset: section_table_end });
    }

    // Only add DOCS section for v5+
    if header.version >= 5 {
        sections.push(Section { name: "DOCS", size: header.docs_len as usize, offset: lev_dfa_end });
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: docs_end });
    } else if header.version >= 3 {
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: lev_dfa_end });
    } else {
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: section_table_end });
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
    println!("║  Version:  {:<55}  ║", format!("{} (current: {})", header.version, VERSION));
    println!("╚{}╝", "═".repeat(W));
    println!();

    // Print metadata
    println!("┌─ METADATA {}┐", "─".repeat(W - 12));
    println!("│  Documents:      {:>10}{:>w$}│", header.doc_count, "", w = W - 30);
    println!("│  Terms:          {:>10}{:>w$}│", header.term_count, "", w = W - 30);
    println!("│  CRC32:          {:#010x} {:<7}{:>w$}│",
        footer.crc32,
        if crc_valid { "✓ valid" } else { "✗ BAD" },
        "", w = W - 38);
    println!("│  Skip Lists:     {:>10}{:>w$}│",
        if header.flags.has_skip_lists() { "yes" } else { "no" },
        "", w = W - 30);
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

        println!("│  {:<12} │{}{}│ {:>8} {:>6.1}%  │",
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
    println!("│  {:<12} {:>10}  {:>10}  {:>10}{:>w$}│",
        "SECTION", "OFFSET", "LENGTH", "END", "", w = W - 50);
    println!("│  {:<12} {:>10}  {:>10}  {:>10}{:>w$}│",
        "────────────", "──────────", "──────────", "──────────", "", w = W - 50);

    for section in &sections {
        let end = section.offset + section.size;
        println!("│  {:<12} {:>10}  {:>10}  {:>10}{:>w$}│",
            section.name,
            format!("0x{:06X}", section.offset),
            format_size(section.size),
            format!("0x{:06X}", end),
            "", w = W - 50
        );
    }

    println!("│{:>w$}│", "", w = W);
    println!("└{}┘", "─".repeat(W));
    println!();

    // Print size breakdown
    let content_size = header.vocab_len + header.sa_len + header.postings_len +
                       header.skip_len + header.section_table_len + header.lev_dfa_len + header.docs_len;
    let overhead = header_size + SieveFooter::SIZE;

    let largest_name = sections.iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER")
        .max_by_key(|s| s.size)
        .map(|s| s.name)
        .unwrap_or("N/A");
    let largest_size = sections.iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER")
        .map(|s| s.size)
        .max()
        .unwrap_or(0);
    let smallest_name = sections.iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER" && s.size > 0)
        .min_by_key(|s| s.size)
        .map(|s| s.name)
        .unwrap_or("N/A");
    let smallest_size = sections.iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER" && s.size > 0)
        .map(|s| s.size)
        .min()
        .unwrap_or(0);

    println!("┌─ SIZE BREAKDOWN {}┐", "─".repeat(W - 17));
    println!("│  Content:   {:>10}  ({:.1}% of total){:>w$}│",
        format_size(content_size as usize),
        content_size as f64 / total_size as f64 * 100.0,
        "", w = W - 42);
    println!("│  Overhead:  {:>10}  (header + footer){:>w$}│",
        format_size(overhead),
        "", w = W - 42);
    println!("│{:>w$}│", "", w = W);
    println!("│  Largest:   {:>10}  {:<12}{:>w$}│",
        format_size(largest_size), largest_name, "", w = W - 40);
    println!("│  Smallest:  {:>10}  {:<12}{:>w$}│",
        format_size(smallest_size), smallest_name, "", w = W - 40);
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
    let suffix_array: Vec<(u32, u32)> = fst_index.vocab_suffix_array
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
            fst_index.inverted_index.terms
                .get(term)
                .map(|pl| {
                    pl.postings.iter().map(|p| {
                        let section_idx = p.section_id
                            .as_ref()
                            .and_then(|id| section_idx_map.get(id.as_str()))
                            .copied()
                            .unwrap_or(0);
                        PostingEntry {
                            doc_id: p.doc_id as u32,
                            section_idx,
                        }
                    }).collect()
                })
                .unwrap_or_default()
        })
        .collect();

    let lev_dfa = ParametricDFA::build(true);
    let lev_dfa_bytes = lev_dfa.to_bytes();

    let docs_input: Vec<DocMetaInput> = payload.docs
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
    ).expect("failed to build binary layer");

    let index_bytes = layer.to_bytes().expect("failed to serialize index");

    // Write files to output directory
    let dir = Path::new(output_dir);

    // 1. Write .sieve index
    let index_path = dir.join("index.sieve");
    fs::write(&index_path, &index_bytes).expect("failed to write index.sieve");
    eprintln!("  ✓ Created {} ({} bytes)", index_path.display(), index_bytes.len());

    // 2. Write WASM file
    let wasm_path = dir.join("sieve_bg.wasm");
    fs::write(&wasm_path, SIEVE_WASM).expect("failed to write sieve_bg.wasm");
    eprintln!("  ✓ Created {} ({} bytes)", wasm_path.display(), SIEVE_WASM.len());

    // 3. Write JS file
    let js_path = dir.join("sieve.js");
    fs::write(&js_path, SIEVE_JS).expect("failed to write sieve.js");
    eprintln!("  ✓ Created {} ({} bytes)", js_path.display(), SIEVE_JS.len());

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

/// Create sample payload for demo when no input is provided
#[cfg(feature = "embed-wasm")]
fn create_sample_payload() -> Payload {
    Payload {
        docs: vec![
            SearchDoc {
                id: 0,
                title: "Getting Started with Sieve".to_string(),
                excerpt: "Learn how to use Sieve for fast full-text search.".to_string(),
                href: "/docs/getting-started".to_string(),
                kind: "post".to_string(),
            },
            SearchDoc {
                id: 1,
                title: "Suffix Arrays Explained".to_string(),
                excerpt: "Deep dive into suffix array data structures.".to_string(),
                href: "/docs/suffix-arrays".to_string(),
                kind: "post".to_string(),
            },
            SearchDoc {
                id: 2,
                title: "Fuzzy Search with Levenshtein".to_string(),
                excerpt: "How Sieve handles typos using Levenshtein automata.".to_string(),
                href: "/docs/fuzzy-search".to_string(),
                kind: "post".to_string(),
            },
        ],
        texts: vec![
            "Getting Started with Sieve Learn how to use Sieve for fast full-text search".to_string(),
            "Suffix Arrays Explained Deep dive into suffix array data structures".to_string(),
            "Fuzzy Search with Levenshtein How Sieve handles typos using Levenshtein automata".to_string(),
        ],
        field_boundaries: vec![],
    }
}

/// Generate demo HTML page
#[cfg(feature = "embed-wasm")]
fn generate_demo_html(doc_count: usize) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sieve Search Demo</title>
    <style>
        * {{ box-sizing: border-box; }}
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 800px;
            margin: 0 auto;
            padding: 2rem;
            background: #f5f5f5;
        }}
        h1 {{ color: #333; }}
        .search-box {{
            display: flex;
            gap: 0.5rem;
            margin: 1rem 0;
        }}
        input[type="text"] {{
            flex: 1;
            padding: 0.75rem 1rem;
            font-size: 1rem;
            border: 2px solid #ddd;
            border-radius: 8px;
            outline: none;
        }}
        input[type="text"]:focus {{
            border-color: #0066cc;
        }}
        .results {{
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 8px rgba(0,0,0,0.1);
            overflow: hidden;
        }}
        .result {{
            padding: 1rem;
            border-bottom: 1px solid #eee;
        }}
        .result:last-child {{ border-bottom: none; }}
        .result h3 {{
            margin: 0 0 0.5rem 0;
            color: #0066cc;
        }}
        .result p {{
            margin: 0;
            color: #666;
        }}
        .result .meta {{
            font-size: 0.85rem;
            color: #999;
            margin-top: 0.5rem;
        }}
        .empty {{
            padding: 2rem;
            text-align: center;
            color: #999;
        }}
        .stats {{
            font-size: 0.9rem;
            color: #666;
            margin-bottom: 1rem;
        }}
        .loading {{ opacity: 0.5; }}
    </style>
</head>
<body>
    <h1>🔍 Sieve Search Demo</h1>
    <p>Formally verified full-text search with {} documents.</p>

    <div class="search-box">
        <input type="text" id="query" placeholder="Type to search..." autofocus>
    </div>

    <div class="stats" id="stats"></div>
    <div class="results" id="results">
        <div class="empty">Start typing to search...</div>
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
            console.log('Sieve initialized with', index.doc_count(), 'documents');
        }}

        function debounce(fn, ms) {{
            let timeout;
            return (...args) => {{
                clearTimeout(timeout);
                timeout = setTimeout(() => fn(...args), ms);
            }};
        }}

        function search(e) {{
            const query = e.target.value.trim();
            const resultsDiv = document.getElementById('results');
            const statsDiv = document.getElementById('stats');

            if (!query) {{
                resultsDiv.innerHTML = '<div class="empty">Start typing to search...</div>';
                statsDiv.textContent = '';
                return;
            }}

            const start = performance.now();
            const results = index.search(query, 10);
            const elapsed = (performance.now() - start).toFixed(2);

            statsDiv.textContent = `Found ${{results.length}} results in ${{elapsed}}ms`;

            if (results.length === 0) {{
                resultsDiv.innerHTML = '<div class="empty">No results found</div>';
                return;
            }}

            resultsDiv.innerHTML = results.map(r => `
                <div class="result">
                    <h3><a href="${{r.href}}">${{escapeHtml(r.title)}}</a></h3>
                    <p>${{escapeHtml(r.excerpt)}}</p>
                    <div class="meta">Score: ${{r.score.toFixed(1)}} · Type: ${{r.kind}}</div>
                </div>
            `).join('');
        }}

        function escapeHtml(text) {{
            const div = document.createElement('div');
            div.textContent = text;
            return div.innerHTML;
        }}

        setup().catch(console.error);
    </script>
</body>
</html>
"#, doc_count)
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
