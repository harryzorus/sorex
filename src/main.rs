use clap::Parser;
use std::fs;

use sieve::binary::{SieveFooter, SieveHeader, VERSION};
use sieve::build::run_build;

mod cli;
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
            inspect_sieve_file(&file);
        }
    }
}

// Box drawing constants - width between │ and │ (excluding border chars)
const W: usize = 68;

/// Print a content line: │ content          │
fn row(content: &str) {
    let len = content.chars().count();
    let pad = W.saturating_sub(len);
    println!("│{}{}│", content, " ".repeat(pad));
}

/// Print section header: ┌─ LABEL ──────────┐
fn section_top(label: &str) {
    let label_part = format!("─ {} ", label);
    let remaining = W - label_part.chars().count();
    println!("┌{}{}┐", label_part, "─".repeat(remaining));
}

/// Print section divider: ├─ LABEL ──────────┤
fn section_mid(label: &str) {
    let label_part = format!("─ {} ", label);
    let remaining = W - label_part.chars().count();
    println!("├{}{}┤", label_part, "─".repeat(remaining));
}

/// Print section footer: └──────────────────┘
fn section_bot() {
    println!("└{}┘", "─".repeat(W));
}

/// Print double-line header: ╔══════════════════╗
fn double_header() {
    println!("╔{}╗", "═".repeat(W));
}

/// Print double-line divider: ╠══════════════════╣
fn double_divider() {
    println!("╠{}╣", "═".repeat(W));
}

/// Print double-line footer: ╚══════════════════╝
fn double_footer() {
    println!("╚{}╝", "═".repeat(W));
}

/// Print centered content line: ║      TEXT        ║
fn row_double(content: &str) {
    let len = content.chars().count();
    let pad = W.saturating_sub(len);
    println!("║{}{}║", content, " ".repeat(pad));
}

/// Print centered title
fn title(text: &str) {
    let len = text.chars().count();
    let total_pad = W.saturating_sub(len);
    let left_pad = total_pad / 2;
    let right_pad = total_pad - left_pad;
    println!("║{}{}{}║", " ".repeat(left_pad), text, " ".repeat(right_pad));
}

/// Inspect a .sieve file and display its structure diagram
fn inspect_sieve_file(path: &str) {
    let bytes = fs::read(path).expect("failed to read file");
    let total_size = bytes.len();

    // Validate minimum size
    let min_header_size = 36;
    let min_size = min_header_size + SieveFooter::SIZE;
    if total_size < min_size {
        eprintln!("Error: File too small ({} bytes, minimum {})", total_size, min_size);
        std::process::exit(1);
    }

    // Read header
    let version = bytes[4];
    let (hdr, header_size) = if version >= 3 {
        let h = SieveHeader::read(&mut std::io::Cursor::new(&bytes)).expect("failed to read header");
        (h, SieveHeader::SIZE)
    } else {
        let mut cursor = std::io::Cursor::new(&bytes);
        let mut magic = [0u8; 4];
        std::io::Read::read_exact(&mut cursor, &mut magic).expect("failed to read magic");
        if magic != [0x53, 0x49, 0x46, 0x54] {
            eprintln!("Error: Invalid magic bytes");
            std::process::exit(1);
        }
        let mut buf = [0u8; 32];
        std::io::Read::read_exact(&mut cursor, &mut buf).expect("failed to read header");
        let h = SieveHeader {
            version: buf[0],
            flags: sieve::binary::FormatFlags::default(),
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
    let ftr = SieveFooter::read(&bytes).expect("failed to read footer");
    let content = &bytes[..bytes.len() - SieveFooter::SIZE];
    let computed_crc = SieveFooter::compute_crc32(content);
    let crc_valid = ftr.crc32 == computed_crc;

    // Calculate section offsets
    let header_end = header_size;
    let vocab_end = header_end + hdr.vocab_len as usize;
    let sa_end = vocab_end + hdr.sa_len as usize;
    let postings_end = sa_end + hdr.postings_len as usize;
    let skip_end = postings_end + hdr.skip_len as usize;
    let section_table_end = skip_end + hdr.section_table_len as usize;
    let lev_dfa_end = section_table_end + hdr.lev_dfa_len as usize;
    let docs_end = lev_dfa_end + hdr.docs_len as usize;
    let wasm_end = docs_end + hdr.wasm_len as usize;
    let dict_table_end = wasm_end + hdr.dict_table_len as usize;

    // Build sections list
    struct Section {
        name: &'static str,
        size: usize,
        offset: usize,
    }

    let mut sections = vec![
        Section { name: "HEADER", size: header_size, offset: 0 },
        Section { name: "VOCABULARY", size: hdr.vocab_len as usize, offset: header_end },
        Section { name: "SUFFIX ARRAY", size: hdr.sa_len as usize, offset: vocab_end },
        Section { name: "POSTINGS", size: hdr.postings_len as usize, offset: sa_end },
        Section { name: "SKIP LISTS", size: hdr.skip_len as usize, offset: postings_end },
        Section { name: "SECTION TABLE", size: hdr.section_table_len as usize, offset: skip_end },
    ];

    if hdr.version >= 3 {
        sections.push(Section { name: "LEV DFA", size: hdr.lev_dfa_len as usize, offset: section_table_end });
    }
    if hdr.version >= 5 {
        sections.push(Section { name: "DOCS", size: hdr.docs_len as usize, offset: lev_dfa_end });
    }
    if hdr.version >= 7 {
        if hdr.wasm_len > 0 {
            sections.push(Section { name: "WASM", size: hdr.wasm_len as usize, offset: docs_end });
        }
        if hdr.dict_table_len > 0 {
            sections.push(Section { name: "DICT TABLES", size: hdr.dict_table_len as usize, offset: wasm_end });
        }
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: dict_table_end });
    } else if hdr.version >= 5 {
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: docs_end });
    } else if hdr.version >= 3 {
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: lev_dfa_end });
    } else {
        sections.push(Section { name: "FOOTER", size: SieveFooter::SIZE, offset: section_table_end });
    }

    // ═══════════════════════════════════════════════════════════════════
    // HEADER BOX
    // ═══════════════════════════════════════════════════════════════════
    println!();
    double_header();
    title("SIEVE FILE INSPECTOR");
    double_divider();
    row_double(&format!("  File:     {}", truncate_path(path, 55)));
    row_double(&format!("  Size:     {}", format_size(total_size)));
    row_double(&format!("  Version:  {} (current: {})", hdr.version, VERSION));
    double_footer();
    println!();

    // ═══════════════════════════════════════════════════════════════════
    // METADATA BOX
    // ═══════════════════════════════════════════════════════════════════
    section_top("METADATA");
    row(&format!("  Documents:      {:>10}", hdr.doc_count));
    row(&format!("  Terms:          {:>10}", hdr.term_count));
    row(&format!("  CRC32:          {:#010x} {}", ftr.crc32, if crc_valid { "✓ valid" } else { "✗ BAD" }));
    row(&format!("  Skip Lists:     {:>10}", if hdr.flags.has_skip_lists() { "yes" } else { "no" }));
    section_bot();
    println!();

    // ═══════════════════════════════════════════════════════════════════
    // BINARY STRUCTURE BOX
    // ═══════════════════════════════════════════════════════════════════
    section_top("BINARY STRUCTURE");
    row("");

    let max_size = sections.iter().map(|s| s.size).max().unwrap_or(1);
    let bar_width = 30;

    for section in &sections {
        let pct = (section.size as f64 / total_size as f64) * 100.0;
        let bar_len = if max_size > 0 && section.size > 0 {
            ((section.size as f64 / max_size as f64 * bar_width as f64) as usize).max(1)
        } else {
            0
        };
        let bar = format!("{}{}",  "█".repeat(bar_len), "░".repeat(bar_width - bar_len));
        row(&format!("  {:<13} │{}│ {:>8} {:>6.1}%", section.name, bar, format_size(section.size), pct));
    }

    row("");
    section_mid("OFFSETS");
    row("");
    row(&format!("  {:<13} {:>10}  {:>10}  {:>10}", "SECTION", "OFFSET", "LENGTH", "END"));
    row(&format!("  {:<13} {:>10}  {:>10}  {:>10}", "─".repeat(13), "─".repeat(10), "─".repeat(10), "─".repeat(10)));

    for section in &sections {
        let end = section.offset + section.size;
        row(&format!(
            "  {:<13} {:>10}  {:>10}  {:>10}",
            section.name,
            format!("0x{:06X}", section.offset),
            format_size(section.size),
            format!("0x{:06X}", end)
        ));
    }

    row("");
    section_bot();
    println!();

    // ═══════════════════════════════════════════════════════════════════
    // SIZE BREAKDOWN BOX
    // ═══════════════════════════════════════════════════════════════════
    let mut content_size = hdr.vocab_len + hdr.sa_len + hdr.postings_len
        + hdr.skip_len + hdr.section_table_len + hdr.lev_dfa_len + hdr.docs_len;
    if hdr.version >= 7 {
        content_size += hdr.wasm_len + hdr.dict_table_len;
    }
    let overhead = header_size + SieveFooter::SIZE;

    let largest = sections.iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER")
        .max_by_key(|s| s.size);
    let smallest = sections.iter()
        .filter(|s| s.name != "HEADER" && s.name != "FOOTER" && s.size > 0)
        .min_by_key(|s| s.size);

    section_top("SIZE BREAKDOWN");
    row(&format!("  Content:   {:>10}  ({:.1}% of total)", format_size(content_size as usize), content_size as f64 / total_size as f64 * 100.0));
    row(&format!("  Overhead:  {:>10}  (header + footer)", format_size(overhead)));
    row("");
    if let Some(s) = largest {
        row(&format!("  Largest:   {:>10}  {}", format_size(s.size), s.name));
    }
    if let Some(s) = smallest {
        row(&format!("  Smallest:  {:>10}  {}", format_size(s.size), s.name));
    }
    section_bot();
    println!();
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        path.to_string()
    } else {
        format!("...{}", &path[path.len() - max_len + 3..])
    }
}
