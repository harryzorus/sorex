// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Sorex CLI: build, inspect, and search `.sorex` indexes.
//!
//! ```bash
//! # Build an index from JSON documents
//! sorex index --input ./docs --output ./search
//!
//! # Inspect the binary structure
//! sorex inspect ./search/index-*.sorex
//!
//! # Search with tiered results (exact → prefix → fuzzy)
//! sorex search ./search/index-*.sorex "query" --limit 10
//! ```
//!
//! The CLI also supports benchmarking with `--bench` for statistical analysis
//! and `--wasm` for WASM/Deno parity testing.

use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::fs;
use std::io::Write;
use std::time::Instant;

use sorex::binary::{LoadedLayer, SorexFooter, SorexHeader, VERSION};
use sorex::build::run_build;
use sorex::tiered_search::{SearchResult, TierSearcher};

mod cli;
use cli::display::{
    double_divider, double_footer, double_header, format_size, match_type_label, pad_left,
    pad_right, row, row_double, savings_colored, score_value, section_bot, section_mid,
    section_top, styled, technique_badge, themed, tier_label, timing_ms, timing_us, title,
    truncate_path, visible_len, BOLD, DIM, BRIGHT_CYAN, BRIGHT_GREEN, CYAN, GRAY, GREEN, RED,
    WHITE, YELLOW,
};
use cli::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Index {
            input,
            output,
            demo,
        } => {
            if let Err(e) = run_build(&input, &output, demo) {
                eprintln!("❌ {}", e);
                std::process::exit(1);
            }
        }
        Commands::Inspect { file } => {
            inspect_sorex_file(&file);
        }
        Commands::Search {
            file,
            query,
            limit,
            wasm,
            bench,
            confidence,
        } => {
            if bench {
                benchmark_search(&file, &query, limit, wasm, confidence);
            } else if wasm {
                search_sorex_file_wasm(&file, &query, limit);
            } else {
                search_sorex_file(&file, &query, limit);
            }
        }
    }
}

/// Inspect a .sorex file and display its structure diagram
fn inspect_sorex_file(path: &str) {
    // Set up progress bars
    let mp = MultiProgress::new();
    let style = ProgressStyle::default_spinner()
        .template("{spinner:.cyan} {msg}")
        .unwrap();

    let pb_read = mp.add(ProgressBar::new_spinner());
    pb_read.set_style(style.clone());
    pb_read.set_message("Reading file...");
    pb_read.enable_steady_tick(std::time::Duration::from_millis(80));

    let bytes = fs::read(path).expect("failed to read file");
    let total_size = bytes.len();
    pb_read.finish_with_message(format!("Read {} bytes", format_size(total_size)));

    // Validate minimum size
    let min_header_size = 36;
    let min_size = min_header_size + SorexFooter::SIZE;
    if total_size < min_size {
        eprintln!(
            "Error: File too small ({} bytes, minimum {})",
            total_size, min_size
        );
        std::process::exit(1);
    }

    let pb_parse = mp.add(ProgressBar::new_spinner());
    pb_parse.set_style(style.clone());
    pb_parse.set_message("Parsing header...");
    pb_parse.enable_steady_tick(std::time::Duration::from_millis(80));

    // Read header
    let version = bytes[4];
    let (hdr, header_size) = if version >= 3 {
        let h =
            SorexHeader::read(&mut std::io::Cursor::new(&bytes)).expect("failed to read header");
        (h, SorexHeader::SIZE)
    } else {
        let mut cursor = std::io::Cursor::new(&bytes);
        let mut magic = [0u8; 4];
        std::io::Read::read_exact(&mut cursor, &mut magic).expect("failed to read magic");
        if magic != [0x53, 0x4F, 0x52, 0x58] {
            eprintln!("Error: Invalid magic bytes");
            std::process::exit(1);
        }
        let mut buf = [0u8; 32];
        std::io::Read::read_exact(&mut cursor, &mut buf).expect("failed to read header");
        let h = SorexHeader {
            version: buf[0],
            flags: sorex::binary::FormatFlags::default(),
            doc_count: u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]),
            term_count: u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]),
            vocab_len: u32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]),
            sa_len: u32::from_le_bytes([buf[14], buf[15], buf[16], buf[17]]),
            postings_len: u32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]),
            skip_len: u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]),
            section_table_len: u32::from_le_bytes([buf[26], buf[27], buf[28], buf[29]]),
            lev_dfa_len: 0,
            docs_len: 0,
            wasm_len: 0,
            dict_table_len: 0,
        };
        (h, 36)
    };

    // Validate footer and CRC32
    let ftr = SorexFooter::read(&bytes).expect("failed to read footer");
    let content = &bytes[..bytes.len() - SorexFooter::SIZE];
    let computed_crc = SorexFooter::compute_crc32(content);
    let crc_valid = ftr.crc32 == computed_crc;
    pb_parse.finish_with_message(format!(
        "Parsed v{} header, CRC32 {}",
        hdr.version,
        if crc_valid { "valid" } else { "INVALID" }
    ));

    // Start parallel tasks: Brotli compression and index loading
    let pb_brotli = mp.add(ProgressBar::new_spinner());
    pb_brotli.set_style(style.clone());
    pb_brotli.set_message("Computing Brotli size...");
    pb_brotli.enable_steady_tick(std::time::Duration::from_millis(80));

    let pb_decode = mp.add(ProgressBar::new_spinner());
    pb_decode.set_style(style.clone());
    pb_decode.set_message("Decoding index...");
    pb_decode.enable_steady_tick(std::time::Duration::from_millis(80));

    // Start Brotli compression in background thread (quality 11 = max compression)
    let bytes_clone = bytes.clone();
    let brotli_handle = std::thread::spawn(move || {
        let mut encoder = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
        encoder
            .write_all(&bytes_clone)
            .expect("brotli compression failed");
        encoder.into_inner().len()
    });

    // Load full index for dynamic stats (section_table, skip_lists, dict_tables)
    // This runs in parallel with Brotli compression
    let layer = LoadedLayer::from_bytes(&bytes).ok();
    pb_decode.finish_with_message("Decoded index");

    // Compute raw (uncompressed) sizes in parallel using rayon::join
    let pb_raw = mp.add(ProgressBar::new_spinner());
    pb_raw.set_style(style.clone());
    pb_raw.set_message("Computing raw sizes...");
    pb_raw.enable_steady_tick(std::time::Duration::from_millis(80));

    // Raw size = what the data would be without any compression
    let (vocab_raw, sa_raw, postings_raw, section_table_raw, docs_raw) = if let Some(ref l) = layer
    {
        #[cfg(feature = "rayon")]
        {
            use rayon::prelude::*;
            // Use nested rayon::join to run all computations in parallel
            let ((vocab_raw, sa_raw), (postings_raw, (section_table_raw, docs_raw))) = rayon::join(
                || {
                    rayon::join(
                        // VOCABULARY: raw = sum of all string bytes (no length prefixes or front compression)
                        || l.vocabulary.par_iter().map(|s| s.len()).sum::<usize>(),
                        // SUFFIX ARRAY: raw = entries * 8 bytes (4 bytes term_ord + 4 bytes offset)
                        || l.suffix_array.len() * 8,
                    )
                },
                || {
                    rayon::join(
                        // POSTINGS: raw = total entries * 9 bytes (4 doc_id + 4 section_idx + 1 heading_level)
                        || l.postings.par_iter().map(|pl| pl.len() * 9).sum::<usize>(),
                        || {
                            rayon::join(
                                // SECTION TABLE: raw = sum of all section ID string bytes
                                || l.section_table.par_iter().map(|s| s.len()).sum::<usize>(),
                                // DOCS: raw = sum of all doc metadata fields as strings
                                || {
                                    l.docs
                                        .par_iter()
                                        .map(|d| {
                                            d.title.len()
                                                + d.excerpt.len()
                                                + d.href.len()
                                                + d.doc_type.len()
                                                + d.category.as_ref().map_or(0, |s| s.len())
                                                + d.author.as_ref().map_or(0, |s| s.len())
                                                + d.tags.iter().map(|t: &String| t.len()).sum::<usize>()
                                        })
                                        .sum::<usize>()
                                },
                            )
                        },
                    )
                },
            );
            (vocab_raw, sa_raw, postings_raw, section_table_raw, docs_raw)
        }

        #[cfg(not(feature = "rayon"))]
        {
            // Sequential fallback when rayon is not available
            let vocab_raw = l.vocabulary.iter().map(|s| s.len()).sum::<usize>();
            let sa_raw = l.suffix_array.len() * 8;
            let postings_raw = l.postings.iter().map(|pl| pl.len() * 9).sum::<usize>();
            let section_table_raw = l.section_table.iter().map(|s| s.len()).sum::<usize>();
            let docs_raw = l.docs
                .iter()
                .map(|d| {
                    d.title.len()
                        + d.excerpt.len()
                        + d.href.len()
                        + d.doc_type.len()
                        + d.category.as_ref().map_or(0, |s| s.len())
                        + d.author.as_ref().map_or(0, |s| s.len())
                        + d.tags.iter().map(|t| t.len()).sum::<usize>()
                })
                .sum::<usize>();
            (vocab_raw, sa_raw, postings_raw, section_table_raw, docs_raw)
        }
    } else {
        (0, 0, 0, 0, 0)
    };
    pb_raw.finish_with_message("Computed raw sizes");

    // Build sections list in v12 FILE ORDER (matches actual binary layout)
    // See src/binary/header.rs SectionOffsets for single source of truth
    struct Section {
        name: &'static str,
        size: usize,     // Compressed size in file
        raw_size: usize, // Uncompressed/raw size
        technique: &'static str,
    }

    // v12 layout order (dependency-optimized for streaming decode):
    // 1. HEADER, 2. WASM, 3. VOCABULARY, 4. DICT_TABLES, 5. POSTINGS,
    // 6. SUFFIX_ARRAY, 7. DOCS, 8. SECTION_TABLE, 9. SKIP_LISTS, 10. LEV_DFA, 11. FOOTER
    let mut sections = vec![Section {
        name: "HEADER",
        size: header_size,
        raw_size: header_size,
        technique: "",
    }];

    // v7+: WASM comes first for async streaming compilation
    if hdr.version >= 7 && hdr.wasm_len > 0 {
        sections.push(Section {
            name: "WASM",
            size: hdr.wasm_len as usize,
            raw_size: hdr.wasm_len as usize,
            technique: "RAW",
        });
    }

    // VOCABULARY (needed by SUFFIX_ARRAY)
    sections.push(Section {
        name: "VOCABULARY",
        size: hdr.vocab_len as usize,
        raw_size: vocab_raw,
        technique: "FC",
    });

    // v7+: DICT_TABLES (needed by DOCS)
    if hdr.version >= 7 && hdr.dict_table_len > 0 {
        sections.push(Section {
            name: "DICT TABLES",
            size: hdr.dict_table_len as usize,
            raw_size: hdr.dict_table_len as usize,
            technique: "DICT",
        });
    }

    // POSTINGS (independent, largest section)
    sections.push(Section {
        name: "POSTINGS",
        size: hdr.postings_len as usize,
        raw_size: postings_raw,
        technique: "DELTA",
    });

    // SUFFIX_ARRAY (after VOCABULARY)
    sections.push(Section {
        name: "SUFFIX ARRAY",
        size: hdr.sa_len as usize,
        raw_size: sa_raw,
        technique: "STRM",
    });

    // v5+: DOCS (after DICT_TABLES)
    if hdr.version >= 5 {
        sections.push(Section {
            name: "DOCS",
            size: hdr.docs_len as usize,
            raw_size: docs_raw,
            technique: "BIN",
        });
    }

    // SECTION_TABLE (for deep links)
    sections.push(Section {
        name: "SECTION TABLE",
        size: hdr.section_table_len as usize,
        raw_size: section_table_raw,
        technique: "DEDUP",
    });

    // SKIP_LISTS
    sections.push(Section {
        name: "SKIP LISTS",
        size: hdr.skip_len as usize,
        raw_size: hdr.skip_len as usize,
        technique: "SKIP",
    });

    // v3+: LEV_DFA (only for T3 fuzzy, last before footer)
    if hdr.version >= 3 {
        sections.push(Section {
            name: "LEV DFA",
            size: hdr.lev_dfa_len as usize,
            raw_size: hdr.lev_dfa_len as usize,
            technique: "DFA",
        });
    }

    // FOOTER (always last)
    sections.push(Section {
        name: "FOOTER",
        size: SorexFooter::SIZE,
        raw_size: SorexFooter::SIZE,
        technique: "CRC",
    });

    // ═══════════════════════════════════════════════════════════════════
    // HEADER BOX
    // Wait for Brotli compression to complete
    let brotli_size = brotli_handle.join().expect("brotli thread panicked");
    pb_brotli.finish_with_message(format!(
        "Brotli: {} ({:.0}% smaller)",
        format_size(brotli_size),
        (1.0 - brotli_size as f64 / total_size as f64) * 100.0
    ));

    // Clear progress output before displaying results
    mp.clear().unwrap();
    println!();

    // ═══════════════════════════════════════════════════════════════════
    // HEADER BOX
    // ═══════════════════════════════════════════════════════════════════
    // Extract filename for title
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path);

    double_header();
    title(filename);
    double_divider();

    let compression_ratio = (1.0 - brotli_size as f64 / total_size as f64) * 100.0;

    // Row 1: Size info with brotli savings
    let savings_label = if compression_ratio > 0.0 {
        themed(GREEN, &[BOLD], &format!("{:.0}% smaller", compression_ratio))
    } else {
        themed(RED, &[BOLD], &format!("{:.0}% larger", -compression_ratio))
    };
    let size_info = format!(
        "{} {} {} {} {} {}",
        themed(WHITE, &[], "size"),
        styled(&[BOLD], &format_size(total_size)),
        styled(&[DIM], "→"),
        format_size(brotli_size),
        themed(WHITE, &[], "(brotli)"),
        savings_label
    );

    // Row 1: Size + Version on same line
    let version_color = if hdr.version == VERSION {
        BRIGHT_GREEN
    } else {
        YELLOW
    };
    let version_info = if hdr.version == VERSION {
        format!(
            "{} {}",
            themed(WHITE, &[], "version"),
            themed(version_color, &[BOLD], &hdr.version.to_string())
        )
    } else {
        format!(
            "{} {} {}",
            themed(WHITE, &[], "version"),
            themed(version_color, &[BOLD], &hdr.version.to_string()),
            themed(WHITE, &[], &format!("(current: {})", VERSION))
        )
    };

    row_double(&format!(
        "  {}  {}{}{}",
        size_info,
        " ".repeat(34usize.saturating_sub(visible_len(&size_info))),
        styled(&[DIM], "│ "),
        version_info
    ));

    // Row 2: Docs, Terms, Skips, CRC
    let docs_info = format!(
        "{}{}",
        themed(WHITE, &[], "docs "),
        themed(WHITE, &[BOLD], &hdr.doc_count.to_string())
    );
    let terms_info = format!(
        "{}{}",
        themed(WHITE, &[], "terms "),
        themed(WHITE, &[BOLD], &hdr.term_count.to_string())
    );
    let skip_status = if hdr.flags.has_skip_lists() {
        themed(GREEN, &[], "✓")
    } else {
        styled(&[DIM], "✗")
    };
    let skip_info = format!("{} {}", themed(WHITE, &[], "skips"), skip_status);
    let crc_status = if crc_valid {
        themed(GREEN, &[BOLD], "✓")
    } else {
        themed(RED, &[BOLD], "✗")
    };
    let crc_info = format!(
        "{}{} {}",
        themed(WHITE, &[], "crc "),
        themed(CYAN, &[], &format!("{:08x}", ftr.crc32)),
        crc_status
    );

    row_double(&format!(
        "  {}    {}    {}    {}",
        pad_right(&docs_info, 12),
        pad_right(&terms_info, 14),
        pad_right(&skip_info, 10),
        crc_info
    ));

    double_footer();
    println!();

    // ═══════════════════════════════════════════════════════════════════
    // BREAKDOWN BOX
    // ═══════════════════════════════════════════════════════════════════
    section_top("BREAKDOWN");
    row("");

    // Header row with styled columns
    row(&format!(
        "  {}  {}  {}  {}  {}",
        pad_right(&themed(WHITE, &[BOLD], "SECTION"), 13),
        pad_left(&themed(WHITE, &[], "RAW"), 9),
        pad_left(&themed(WHITE, &[], "FILE"), 9),
        pad_left(&themed(WHITE, &[], "SAVED"), 6),
        themed(WHITE, &[], "FORMAT")
    ));
    row(&format!(
        "  {}  {}  {}  {}  {}",
        styled(&[DIM], &"─".repeat(13)),
        styled(&[DIM], &"─".repeat(9)),
        styled(&[DIM], &"─".repeat(9)),
        styled(&[DIM], &"─".repeat(6)),
        styled(&[DIM], &"─".repeat(7))
    ));

    for section in &sections {
        let savings = savings_colored(section.raw_size, section.size);
        let tech_label = if section.technique.is_empty() {
            "".to_string()
        } else {
            technique_badge(section.technique)
        };
        row(&format!(
            "  {}  {}  {}  {}  {}",
            pad_right(&themed(WHITE, &[], section.name), 13),
            pad_left(&themed(GRAY, &[], &format_size(section.raw_size)), 9),
            pad_left(&format_size(section.size), 9),
            pad_left(&savings, 6),
            tech_label
        ));
    }

    // Show totals with emphasis
    let total_raw: usize = sections.iter().map(|s| s.raw_size).sum();
    let total_file: usize = sections.iter().map(|s| s.size).sum();
    let total_savings_pct = if total_raw > 0 {
        (1.0 - total_file as f64 / total_raw as f64) * 100.0
    } else {
        0.0
    };
    row(&format!(
        "  {}  {}  {}  {}",
        styled(&[DIM], &"─".repeat(13)),
        styled(&[DIM], &"─".repeat(9)),
        styled(&[DIM], &"─".repeat(9)),
        styled(&[DIM], &"─".repeat(6))
    ));

    let total_savings_colored = if total_savings_pct > 0.0 {
        themed(GREEN, &[BOLD], &format!("{:>5.0}%", total_savings_pct))
    } else {
        themed(RED, &[BOLD], &format!("{:>+5.0}%", total_savings_pct))
    };
    row(&format!(
        "  {}  {}  {}  {}",
        pad_right(&themed(BRIGHT_CYAN, &[BOLD], "TOTAL"), 13),
        pad_left(&styled(&[BOLD], &format_size(total_raw)), 9),
        pad_left(&styled(&[BOLD], &format_size(total_file)), 9),
        pad_left(&total_savings_colored, 6)
    ));

    row("");

    // ═══════════════════════════════════════════════════════════════════
    // FORMATS
    // ═══════════════════════════════════════════════════════════════════
    section_mid("FORMATS");
    row("");

    // Get dynamic stats from loaded index
    let section_table_count = layer.as_ref().map(|l| l.section_table.len()).unwrap_or(0);
    let (cat_count, auth_count, tag_count) = layer
        .as_ref()
        .map(|l| {
            (
                l.dict_tables.category.len(),
                l.dict_tables.author.len(),
                l.dict_tables.tags.len(),
            )
        })
        .unwrap_or((0, 0, 0));

    row(&format!(
        "  {}  {} shared prefix elision for sorted vocabulary",
        technique_badge("FC"),
        styled(&[DIM], "Front compression:")
    ));
    row(&format!(
        "  {}  {} term_ords (16-bit) + offsets (32-bit varints)",
        technique_badge("STRM"),
        styled(&[DIM], "Separated streams:")
    ));
    row(&format!(
        "  {} {} delta doc_ids, varint section_idx, u8 level",
        technique_badge("DELTA"),
        styled(&[DIM], "Delta+Varint:")
    ));
    row(&format!(
        "          {} ({} unique sections)",
        styled(&[DIM], "Sorted by doc_id"),
        themed(CYAN, &[], &section_table_count.to_string())
    ));
    row(&format!(
        "  {} {} ({} unique)",
        technique_badge("DEDUP"),
        styled(&[DIM], "Deduplicated length-prefixed strings"),
        themed(CYAN, &[], &section_table_count.to_string())
    ));
    row(&format!(
        "  {}   {} stored as-is without special encoding",
        technique_badge("RAW"),
        styled(&[DIM], "Uncompressed:")
    ));
    if hdr.dict_table_len > 0 {
        row(&format!(
            "  {}  {} ({} cats, {} auth, {} tags)",
            technique_badge("DICT"),
            styled(&[DIM], "Parquet-style dictionaries"),
            themed(BRIGHT_GREEN, &[], &cat_count.to_string()),
            themed(BRIGHT_GREEN, &[], &auth_count.to_string()),
            themed(BRIGHT_GREEN, &[], &tag_count.to_string())
        ));
    }

    row("");
    section_bot();
    println!();
}

/// Search a .sorex file and display results
fn search_sorex_file(path: &str, query: &str, limit: usize) {
    use sorex::compare_results;
    use std::collections::HashSet;

    // Load index
    let load_start = Instant::now();
    let bytes = fs::read(path).expect("failed to read file");
    let layer = LoadedLayer::from_bytes(&bytes).expect("failed to load index");
    let searcher = TierSearcher::from_layer(layer).expect("failed to build searcher");
    let load_time = load_start.elapsed();

    // Warm up all tiers (prime caches and branch predictor)
    for _ in 0..10 {
        let _ = searcher.search_tier1_exact(query, limit);
        let _ = searcher.search_tier2_prefix(query, &HashSet::new(), limit);
        let _ = searcher.search_tier3_fuzzy(query, &HashSet::new(), limit);
    }

    // Tier 1: Exact match (now with hot cache)
    let t1_start = Instant::now();
    let t1_results = searcher.search_tier1_exact(query, limit);
    let t1_time = t1_start.elapsed();
    let t1_count = t1_results.len();

    // Tier 2: Prefix match (exclude T1 results)
    let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();
    let t2_start = Instant::now();
    let t2_results = searcher.search_tier2_prefix(query, &t1_ids, limit);
    let t2_time = t2_start.elapsed();
    let t2_count = t2_results.len();

    // Tier 3: Fuzzy match (exclude T1 and T2 results)
    let mut exclude_ids = t1_ids.clone();
    exclude_ids.extend(t2_results.iter().map(|r| r.doc_id));
    let t3_start = Instant::now();
    let t3_results = searcher.search_tier3_fuzzy(query, &exclude_ids, limit);
    let t3_time = t3_start.elapsed();
    let t3_count = t3_results.len();

    let total_search_time = t1_time + t2_time + t3_time;

    // Merge and sort results
    let mut results: Vec<_> = t1_results
        .into_iter()
        .chain(t2_results)
        .chain(t3_results)
        .collect();
    results.sort_by(|a, b| compare_results(a, b, searcher.docs()));
    results.truncate(limit);

    // Display header
    println!();
    double_header();
    title("SOREX SEARCH");
    double_divider();
    row_double(&format!("  File:   {}", truncate_path(path, 57)));
    row_double(&format!("  Query:  \"{}\"", query));
    row_double(&format!("  Limit:  {}", limit));
    double_footer();
    println!();

    // Display timing with per-tier breakdown
    section_top("PERFORMANCE");
    row(&format!(
        "  Index load:    {} ms",
        timing_ms(load_time.as_secs_f64() * 1000.0)
    ));
    row("");
    row(&format!(
        "  {} Exact:    {} µs  ({:>2} hits)",
        tier_label(1),
        timing_us(t1_time.as_secs_f64() * 1_000_000.0),
        t1_count
    ));
    row(&format!(
        "  {} Prefix:   {} µs  ({:>2} hits)",
        tier_label(2),
        timing_us(t2_time.as_secs_f64() * 1_000_000.0),
        t2_count
    ));
    row(&format!(
        "  {} Fuzzy:    {} µs  ({:>2} hits)",
        tier_label(3),
        timing_us(t3_time.as_secs_f64() * 1_000_000.0),
        t3_count
    ));
    row("");
    row(&format!(
        "  Search total:  {} µs",
        timing_us(total_search_time.as_secs_f64() * 1_000_000.0)
    ));
    row(&format!(
        "  Total:         {} ms",
        timing_ms((load_time + total_search_time).as_secs_f64() * 1000.0)
    ));
    section_bot();
    println!();

    // Display results
    if results.is_empty() {
        section_top("RESULTS (0)");
        row("  No results found.");
        section_bot();
    } else {
        section_top(&format!("RESULTS ({})", results.len()));
        row("");
        row(&format!(
            "  {:<3} {:<6} {:<12} {:>7}  {}",
            "#", "TIER", "MATCH TYPE", "SCORE", "TITLE"
        ));
        row(&format!(
            "  {:<3} {:<6} {:<12} {:>7}  {}",
            "─".repeat(3),
            "─".repeat(6),
            "─".repeat(12),
            "─".repeat(7),
            "─".repeat(35)
        ));

        for (i, r) in results.iter().enumerate() {
            let doc_title = searcher
                .docs()
                .get(r.doc_id)
                .map(|d| d.title.as_str())
                .unwrap_or("unknown");
            let tier = tier_label(r.tier);
            let match_type = match_type_label(&format!("{:?}", r.match_type));
            let score = score_value(r.score);
            let truncated_title = if doc_title.len() > 35 {
                format!("{}...", &doc_title[..32])
            } else {
                doc_title.to_string()
            };

            // Pad colored strings to fixed visible width
            let tier_padded = format!("{}{}", tier, " ".repeat(6 - visible_len(&tier)));
            let match_padded = format!("{}{}", match_type, " ".repeat(12 - visible_len(&match_type)));

            row(&format!(
                "  {:<3} {} {} {}  {}",
                i + 1,
                tier_padded,
                match_padded,
                score,
                truncated_title
            ));

            // Show section_id if present (resolve from section_idx)
            if r.section_idx > 0 {
                if let Some(section_id) = searcher.section_table().get((r.section_idx - 1) as usize) {
                    row(&format!("      └─ #{}", section_id));
                }
            }
        }

        row("");
        section_bot();
    }
    println!();
}

/// Search using WASM via Deno runtime (for parity testing)
#[allow(unused_variables)]
fn search_sorex_file_wasm(path: &str, query: &str, limit: usize) {
    #[cfg(feature = "deno-runtime")]
    use sorex::deno_runtime::DenoSearchContext;

    // Load index
    let load_start = Instant::now();
    let bytes = fs::read(path).expect("failed to read file");
    let _layer = LoadedLayer::from_bytes(&bytes).expect("failed to load index");
    let load_time = load_start.elapsed();

    // Get loader JS from the same directory as the index file
    let loader_js: String = {
        let index_dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
        let loader_path = index_dir.join("sorex.js");
        if loader_path.exists() {
            fs::read_to_string(&loader_path)
                .expect("failed to read sorex.js from index directory")
        } else {
            eprintln!("Error: sorex.js not found in index directory");
            eprintln!("  Expected: {}", loader_path.display());
            eprintln!("  The loader must be generated alongside the index file.");
            std::process::exit(1);
        }
    };

    // Display header
    println!();
    double_header();
    title("SOREX SEARCH (WASM/DENO)");
    double_divider();
    row_double(&format!("  File:   {}", truncate_path(path, 57)));
    row_double(&format!("  Query:  \"{}\"", query));
    row_double(&format!("  Limit:  {}", limit));
    double_footer();
    println!();

    // Use DenoSearchContext for warm measurements (WASM initialized once)
    #[cfg(feature = "deno-runtime")]
    {
        // Initialize WASM (this includes compilation overhead)
        let init_start = Instant::now();
        let ctx_result = DenoSearchContext::new(&bytes, &loader_js);
        let init_time = init_start.elapsed();

        match ctx_result {
            Ok(mut ctx) => {
                // Warm-up searches to trigger V8 TurboFan optimization
                // V8 uses tiered compilation: Liftoff (baseline) -> TurboFan (optimizing)
                // TurboFan requires ~100+ iterations to trigger for WASM hot functions
                ctx.warmup_turbofan(query, limit);

                // Timed search with per-tier breakdown
                let timing_result = ctx.search_with_tier_timing(query, limit)
                    .expect("search with tier timing failed");

                let total_search_time_us = timing_result.t1_time_us + timing_result.t2_time_us + timing_result.t3_time_us;

                section_top("PERFORMANCE");
                row(&format!(
                    "  Index load:    {} ms",
                    timing_ms(load_time.as_secs_f64() * 1000.0)
                ));
                row(&format!(
                    "  WASM init:     {} ms  (cold, includes compilation)",
                    timing_ms(init_time.as_secs_f64() * 1000.0)
                ));
                row("");
                row(&format!(
                    "  {} Exact:    {} µs  ({:>2} hits)",
                    tier_label(1),
                    timing_us(timing_result.t1_time_us),
                    timing_result.t1_count
                ));
                row(&format!(
                    "  {} Prefix:   {} µs  ({:>2} hits)",
                    tier_label(2),
                    timing_us(timing_result.t2_time_us),
                    timing_result.t2_count
                ));
                row(&format!(
                    "  {} Fuzzy:    {} µs  ({:>2} hits)",
                    tier_label(3),
                    timing_us(timing_result.t3_time_us),
                    timing_result.t3_count
                ));
                row("");
                row(&format!(
                    "  Search total:  {} µs",
                    timing_us(total_search_time_us)
                ));
                row(&format!(
                    "  Total:         {} ms",
                    timing_ms(load_time.as_secs_f64() * 1000.0 + init_time.as_secs_f64() * 1000.0 + total_search_time_us / 1000.0)
                ));
                section_bot();
                println!();

                display_wasm_results(&timing_result.results);
            }
            Err(e) => {
                display_wasm_error(&e);
            }
        }
    }

    #[cfg(not(feature = "deno-runtime"))]
    {
        display_wasm_error("Deno runtime not enabled. Build with --features deno-runtime");
    }
}

#[cfg(feature = "deno-runtime")]
fn display_wasm_results(results: &[sorex::deno_runtime::WasmSearchResult]) {
    if results.is_empty() {
        section_top("RESULTS (0)");
        row("  No results found.");
        section_bot();
    } else {
        section_top(&format!("RESULTS ({})", results.len()));
        row("");
        row(&format!(
            "  {:<3} {:<6} {:<12}  {}",
            "#", "TIER", "MATCH TYPE", "TITLE"
        ));
        row(&format!(
            "  {:<3} {:<6} {:<12}  {}",
            "─".repeat(3),
            "─".repeat(6),
            "─".repeat(12),
            "─".repeat(43)
        ));

        for (i, r) in results.iter().enumerate() {
            let tier = tier_label(r.tier);
            let match_type_str = match r.match_type {
                0 => "Title",
                1 => "Section",
                2 => "Subheading",
                3 => "Subheading2",
                _ => "Content",
            };
            let match_type = match_type_label(match_type_str);
            let truncated_title = if r.title.len() > 43 {
                format!("{}...", &r.title[..40])
            } else {
                r.title.clone()
            };

            // Pad colored strings to fixed visible width
            let tier_padded = format!("{}{}", tier, " ".repeat(6 - visible_len(&tier)));
            let match_padded = format!("{}{}", match_type, " ".repeat(12 - visible_len(&match_type)));

            row(&format!(
                "  {:<3} {} {}  {}",
                i + 1,
                tier_padded,
                match_padded,
                truncated_title
            ));

            if let Some(ref section_id) = r.section_id {
                row(&format!("      └─ #{}", section_id));
            }
        }

        row("");
        section_bot();
    }
    println!();
}

fn display_wasm_error(e: &str) {
    section_top("ERROR");
    row(&format!("  {}", e));
    row("");
    row("  The Deno runtime requires the deno-runtime feature:");
    row("    cargo build --release --features deno-runtime");
    row("");
    row("  For testing, use the Node.js comparison script instead:");
    row("    node tests/compare-wasm-native.js");
    section_bot();
    println!();
}

// ============================================================================
// BENCHMARK MODE
// ============================================================================

/// Statistics for a sample of timing measurements
struct BenchStats {
    mean: f64,
    std_dev: f64,
    ci_lower: f64,
    ci_upper: f64,
    n: usize,
}

impl BenchStats {
    /// Calculate statistics with confidence interval
    fn from_samples(samples: &[f64], confidence: u8) -> Self {
        let n = samples.len();
        let mean = samples.iter().sum::<f64>() / n as f64;
        let variance = samples.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1) as f64;
        let std_dev = variance.sqrt();
        let std_error = std_dev / (n as f64).sqrt();

        // t-distribution critical values for common confidence levels
        // Using approximate values for large n (converges to z-score)
        let t_critical = match confidence {
            99 => 2.576,
            95 => 1.96,
            90 => 1.645,
            _ => 1.96, // default to 95%
        };

        let margin = t_critical * std_error;
        BenchStats {
            mean,
            std_dev,
            ci_lower: mean - margin,
            ci_upper: mean + margin,
            n,
        }
    }

    /// Check if confidence interval is tight enough (within 5% of mean)
    fn is_stable(&self) -> bool {
        if self.mean == 0.0 {
            return true;
        }
        let ci_width = self.ci_upper - self.ci_lower;
        let relative_width = ci_width / self.mean;
        relative_width < 0.10 // Within 10% of mean
    }
}

/// Benchmark search with statistical analysis
fn benchmark_search(path: &str, query: &str, limit: usize, wasm: bool, confidence: u8) {
    // Load index
    let load_start = Instant::now();
    let bytes = fs::read(path).expect("failed to read file");
    let layer = LoadedLayer::from_bytes(&bytes).expect("failed to load index");
    let searcher = TierSearcher::from_layer(layer).expect("failed to build searcher");
    let load_time = load_start.elapsed();

    // Display header
    println!();
    double_header();
    let mode = if wasm { "WASM" } else { "NATIVE" };
    title(&format!("SOREX BENCHMARK ({})", mode));
    double_divider();
    row_double(&format!("  File:       {}", truncate_path(path, 53)));
    row_double(&format!("  Query:      \"{}\"", query));
    row_double(&format!("  Limit:      {}", limit));
    row_double(&format!("  Confidence: {}%", confidence));
    double_footer();
    println!();

    if wasm {
        benchmark_wasm(path, &bytes, query, limit, confidence, load_time);
    } else {
        benchmark_native(&searcher, query, limit, confidence, load_time);
    }
}

/// Benchmark native Rust search
fn benchmark_native(
    searcher: &TierSearcher,
    query: &str,
    limit: usize,
    confidence: u8,
    load_time: std::time::Duration,
) {
    use sorex::compare_results;
    use std::collections::HashSet;

    const MIN_SAMPLES: usize = 30;
    const MAX_SAMPLES: usize = 1000;

    // Warm up (prime caches and branch predictor)
    for _ in 0..50 {
        let _ = searcher.search_tier1_exact(query, limit);
        let _ = searcher.search_tier2_prefix(query, &HashSet::new(), limit);
        let _ = searcher.search_tier3_fuzzy(query, &HashSet::new(), limit);
    }

    let mut t1_samples = Vec::with_capacity(MAX_SAMPLES);
    let mut t2_samples = Vec::with_capacity(MAX_SAMPLES);
    let mut t3_samples = Vec::with_capacity(MAX_SAMPLES);
    let mut total_samples = Vec::with_capacity(MAX_SAMPLES);

    let mut final_results = Vec::new();
    let mut t1_count;
    let mut t2_count;
    let mut t3_count;

    print!("  Sampling: ");
    std::io::stdout().flush().unwrap();

    // Collect samples until stable or max reached
    loop {
        // T1: Exact match
        let t1_start = Instant::now();
        let t1_results = searcher.search_tier1_exact(query, limit);
        let t1_time = t1_start.elapsed().as_secs_f64() * 1_000_000.0;
        t1_samples.push(t1_time);
        t1_count = t1_results.len();

        // T2: Prefix match
        let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();
        let t2_start = Instant::now();
        let t2_results = searcher.search_tier2_prefix(query, &t1_ids, limit);
        let t2_time = t2_start.elapsed().as_secs_f64() * 1_000_000.0;
        t2_samples.push(t2_time);
        t2_count = t2_results.len();

        // T3: Fuzzy match
        let mut exclude_ids = t1_ids.clone();
        exclude_ids.extend(t2_results.iter().map(|r| r.doc_id));
        let t3_start = Instant::now();
        let t3_results = searcher.search_tier3_fuzzy(query, &exclude_ids, limit);
        let t3_time = t3_start.elapsed().as_secs_f64() * 1_000_000.0;
        t3_samples.push(t3_time);
        t3_count = t3_results.len();

        total_samples.push(t1_time + t2_time + t3_time);

        // Store final results on first iteration
        if final_results.is_empty() {
            final_results = t1_results
                .into_iter()
                .chain(t2_results)
                .chain(t3_results)
                .collect();
            final_results.sort_by(|a, b| compare_results(a, b, searcher.docs()));
            final_results.truncate(limit);
        }

        let n = total_samples.len();
        if n % 50 == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }

        if n >= MIN_SAMPLES {
            let stats = BenchStats::from_samples(&total_samples, confidence);
            if stats.is_stable() || n >= MAX_SAMPLES {
                break;
            }
        }
    }
    println!(" done ({} samples)", total_samples.len());
    println!();

    // Calculate final statistics
    let t1_stats = BenchStats::from_samples(&t1_samples, confidence);
    let t2_stats = BenchStats::from_samples(&t2_samples, confidence);
    let t3_stats = BenchStats::from_samples(&t3_samples, confidence);
    let total_stats = BenchStats::from_samples(&total_samples, confidence);

    // Display results
    section_top(&format!("BENCHMARK RESULTS ({}% CI)", confidence));
    row(&format!(
        "  Index load:    {} ms",
        timing_ms(load_time.as_secs_f64() * 1000.0)
    ));
    row("");
    row(&format!(
        "  {} Exact:    {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]  ({:>2} hits)",
        tier_label(1), timing_us(t1_stats.mean), t1_stats.std_dev, t1_stats.ci_lower, t1_stats.ci_upper, t1_count
    ));
    row(&format!(
        "  {} Prefix:   {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]  ({:>2} hits)",
        tier_label(2), timing_us(t2_stats.mean), t2_stats.std_dev, t2_stats.ci_lower, t2_stats.ci_upper, t2_count
    ));
    row(&format!(
        "  {} Fuzzy:    {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]  ({:>2} hits)",
        tier_label(3), timing_us(t3_stats.mean), t3_stats.std_dev, t3_stats.ci_lower, t3_stats.ci_upper, t3_count
    ));
    row("");
    row(&format!(
        "  Search total:  {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]",
        timing_us(total_stats.mean), total_stats.std_dev, total_stats.ci_lower, total_stats.ci_upper
    ));
    row(&format!("  Samples:       {:>10}", total_stats.n));
    section_bot();
    println!();

    // Display results
    display_native_results(&final_results, searcher);
}

/// Display native search results
fn display_native_results(results: &[SearchResult], searcher: &TierSearcher) {
    use cli::display::visible_len;

    if results.is_empty() {
        section_top("RESULTS (0)");
        row("  No results found.");
        section_bot();
    } else {
        section_top(&format!("RESULTS ({})", results.len()));
        row("");
        row(&format!(
            "  {:<3} {:<6} {:<12} {:>7}  {}",
            "#", "TIER", "MATCH TYPE", "SCORE", "TITLE"
        ));
        row(&format!(
            "  {:<3} {:<6} {:<12} {:>7}  {}",
            "─".repeat(3),
            "─".repeat(6),
            "─".repeat(12),
            "─".repeat(7),
            "─".repeat(39)
        ));

        for (i, r) in results.iter().enumerate() {
            let doc_title = searcher
                .docs()
                .get(r.doc_id)
                .map(|d| d.title.as_str())
                .unwrap_or("unknown");
            let tier = tier_label(r.tier);
            let match_type = match_type_label(&format!("{:?}", r.match_type));
            let score = score_value(r.score);
            let truncated_title = if doc_title.len() > 39 {
                format!("{}...", &doc_title[..36])
            } else {
                doc_title.to_string()
            };

            // Pad colored strings to fixed visible width
            let tier_padded = format!("{}{}", tier, " ".repeat(6 - visible_len(&tier)));
            let match_padded = format!("{}{}", match_type, " ".repeat(12 - visible_len(&match_type)));

            row(&format!(
                "  {:<3} {} {} {}  {}",
                i + 1,
                tier_padded,
                match_padded,
                score,
                truncated_title
            ));

            if r.section_idx > 0 {
                if let Some(section_id) = searcher.section_table().get((r.section_idx - 1) as usize) {
                    row(&format!("      └─ #{}", section_id));
                }
            }
        }

        row("");
        section_bot();
    }
    println!();
}

/// Benchmark WASM search via Deno runtime
#[allow(unused_variables)]
fn benchmark_wasm(
    path: &str,
    bytes: &[u8],
    query: &str,
    limit: usize,
    confidence: u8,
    load_time: std::time::Duration,
) {
    #[cfg(feature = "deno-runtime")]
    {
        use sorex::deno_runtime::{DenoSearchContext, TURBOFAN_WARMUP_ITERATIONS};

        const MIN_SAMPLES: usize = 30;
        const MAX_SAMPLES: usize = 1000;

        // Get loader JS
        let loader_js: String = {
            let index_dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
            let loader_path = index_dir.join("sorex.js");
            if loader_path.exists() {
                fs::read_to_string(&loader_path)
                    .expect("failed to read sorex.js from index directory")
            } else {
                eprintln!("Error: sorex.js not found");
                std::process::exit(1);
            }
        };

        // Initialize WASM
        let init_start = Instant::now();
        let ctx_result = DenoSearchContext::new(bytes, &loader_js);
        let init_time = init_start.elapsed();

        match ctx_result {
            Ok(mut ctx) => {
                // Warm up TurboFan - this forces WASM optimization
                print!("  Warming up TurboFan ({} iterations)... ", TURBOFAN_WARMUP_ITERATIONS);
                std::io::stdout().flush().unwrap();
                ctx.warmup_turbofan(query, limit);
                println!("done");

                let mut total_samples = Vec::with_capacity(MAX_SAMPLES);
                let mut t1_samples = Vec::with_capacity(MAX_SAMPLES);
                let mut t2_samples = Vec::with_capacity(MAX_SAMPLES);
                let mut t3_samples = Vec::with_capacity(MAX_SAMPLES);
                let mut final_results = None;

                print!("  Sampling: ");
                std::io::stdout().flush().unwrap();

                // Collect samples
                loop {
                    let timing = ctx.search_with_tier_timing(query, limit)
                        .expect("search failed");

                    t1_samples.push(timing.t1_time_us);
                    t2_samples.push(timing.t2_time_us);
                    t3_samples.push(timing.t3_time_us);
                    total_samples.push(timing.t1_time_us + timing.t2_time_us + timing.t3_time_us);

                    if final_results.is_none() {
                        final_results = Some(timing);
                    }

                    let n = total_samples.len();
                    if n % 50 == 0 {
                        print!(".");
                        std::io::stdout().flush().unwrap();
                    }

                    if n >= MIN_SAMPLES {
                        let stats = BenchStats::from_samples(&total_samples, confidence);
                        if stats.is_stable() || n >= MAX_SAMPLES {
                            break;
                        }
                    }
                }
                println!(" done ({} samples)", total_samples.len());
                println!();

                // Calculate statistics
                let t1_stats = BenchStats::from_samples(&t1_samples, confidence);
                let t2_stats = BenchStats::from_samples(&t2_samples, confidence);
                let t3_stats = BenchStats::from_samples(&t3_samples, confidence);
                let total_stats = BenchStats::from_samples(&total_samples, confidence);

                let timing = final_results.unwrap();

                // Display results
                section_top(&format!("BENCHMARK RESULTS ({}% CI)", confidence));
                row(&format!(
                    "  Index load:    {} ms",
                    timing_ms(load_time.as_secs_f64() * 1000.0)
                ));
                row(&format!(
                    "  WASM init:     {} ms  (includes compilation)",
                    timing_ms(init_time.as_secs_f64() * 1000.0)
                ));
                row("");
                row(&format!(
                    "  {} Exact:    {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]  ({:>2} hits)",
                    tier_label(1), timing_us(t1_stats.mean), t1_stats.std_dev, t1_stats.ci_lower, t1_stats.ci_upper, timing.t1_count
                ));
                row(&format!(
                    "  {} Prefix:   {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]  ({:>2} hits)",
                    tier_label(2), timing_us(t2_stats.mean), t2_stats.std_dev, t2_stats.ci_lower, t2_stats.ci_upper, timing.t2_count
                ));
                row(&format!(
                    "  {} Fuzzy:    {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]  ({:>2} hits)",
                    tier_label(3), timing_us(t3_stats.mean), t3_stats.std_dev, t3_stats.ci_lower, t3_stats.ci_upper, timing.t3_count
                ));
                row("");
                row(&format!(
                    "  Search total:  {} µs ± {:>6.3} µs  [{:>8.3}, {:>8.3}]",
                    timing_us(total_stats.mean), total_stats.std_dev, total_stats.ci_lower, total_stats.ci_upper
                ));
                row(&format!("  Samples:       {:>10}", total_stats.n));
                section_bot();
                println!();

                display_wasm_results(&timing.results);
            }
            Err(e) => {
                display_wasm_error(&e);
            }
        }
    }

    #[cfg(not(feature = "deno-runtime"))]
    {
        display_wasm_error("Deno runtime not enabled. Build with --features deno-runtime");
    }
}
