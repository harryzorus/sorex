//! Benchmark decode times for each section to optimize file layout
#![allow(unused_imports, dead_code)]

use sorex::binary::{
    decode_postings, decode_section_table, decode_suffix_array, decode_vocabulary, SorexHeader,
};
use sorex::dict_table::DictTables;
use std::fs;
use std::io::Cursor;
use std::time::Instant;

/// Path to the Cutlass dataset index.
const CUTLASS_INDEX: &str = "target/datasets/cutlass/index.sorex";

/// Path to the PyTorch dataset index.
const PYTORCH_INDEX: &str = "target/datasets/pytorch/index.sorex";

fn main() {
    // This is a test benchmark, run with: cargo test --bench decode_sections
    println!("Run with: cargo test --bench decode_sections -- --nocapture");
}

#[test]
fn benchmark_section_decode_times() {
    let cutlass_bytes = fs::read(CUTLASS_INDEX).unwrap();
    let pytorch_bytes = fs::read(PYTORCH_INDEX).unwrap();

    println!("\n=== SECTION DECODE TIMING BENCHMARK ===\n");

    for (name, bytes) in [
        ("CUTLASS (70 docs)", &cutlass_bytes),
        ("PYTORCH (300 docs)", &pytorch_bytes),
    ] {
        println!("--- {} ({} KB) ---", name, bytes.len() / 1024);
        benchmark_sections(bytes);
        println!();
    }
}

fn benchmark_sections(bytes: &[u8]) {
    const ITERATIONS: u32 = 100;

    // Parse header to get section offsets (SINGLE SOURCE OF TRUTH)
    let header = SorexHeader::read(&mut Cursor::new(bytes)).unwrap();
    let offsets = header.section_offsets();

    // Extract section slices using SectionOffsets
    let wasm_bytes = &bytes[offsets.wasm.0..offsets.wasm.1];
    let vocab_bytes = &bytes[offsets.vocabulary.0..offsets.vocabulary.1];
    let dict_table_bytes = &bytes[offsets.dict_tables.0..offsets.dict_tables.1];
    let postings_bytes = &bytes[offsets.postings.0..offsets.postings.1];
    let sa_bytes = &bytes[offsets.suffix_array.0..offsets.suffix_array.1];
    let docs_bytes = &bytes[offsets.docs.0..offsets.docs.1];
    let section_table_bytes = &bytes[offsets.section_table.0..offsets.section_table.1];
    let skip_bytes = &bytes[offsets.skip_lists.0..offsets.skip_lists.1];
    let lev_dfa_bytes = &bytes[offsets.lev_dfa.0..offsets.lev_dfa.1];

    struct Section {
        name: &'static str,
        size: usize,
        time_ns: u128,
        deps: &'static str,
    }

    let mut results = vec![];

    // HEADER
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = SorexHeader::read(&mut Cursor::new(bytes)).unwrap();
    }
    results.push(Section {
        name: "HEADER",
        size: SorexHeader::SIZE,
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // WASM (memcpy only)
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _: Vec<u8> = wasm_bytes.to_vec();
    }
    results.push(Section {
        name: "WASM",
        size: wasm_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // VOCABULARY
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = decode_vocabulary(vocab_bytes, header.term_count as usize).unwrap();
    }
    results.push(Section {
        name: "VOCABULARY",
        size: vocab_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // DICT_TABLES
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        if !dict_table_bytes.is_empty() {
            let _ = DictTables::decode(dict_table_bytes).unwrap();
        }
    }
    results.push(Section {
        name: "DICT_TABLES",
        size: dict_table_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // POSTINGS
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let mut pos = 0;
        while pos < postings_bytes.len() {
            let (_, consumed) = decode_postings(&postings_bytes[pos..]).unwrap();
            pos += consumed;
        }
    }
    results.push(Section {
        name: "POSTINGS",
        size: postings_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // SUFFIX_ARRAY
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = decode_suffix_array(sa_bytes).unwrap();
    }
    results.push(Section {
        name: "SUFFIX_ARRAY",
        size: sa_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "VOCABULARY",
    });

    // DOCS
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _: Vec<u8> = docs_bytes.to_vec();
    }
    results.push(Section {
        name: "DOCS",
        size: docs_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "DICT_TABLES",
    });

    // SECTION_TABLE
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _ = decode_section_table(section_table_bytes).unwrap();
    }
    results.push(Section {
        name: "SECTION_TABLE",
        size: section_table_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // SKIP_LISTS
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _: Vec<u8> = skip_bytes.to_vec();
    }
    results.push(Section {
        name: "SKIP_LISTS",
        size: skip_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // LEV_DFA
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        let _: Vec<u8> = lev_dfa_bytes.to_vec();
    }
    results.push(Section {
        name: "LEV_DFA",
        size: lev_dfa_bytes.len(),
        time_ns: start.elapsed().as_nanos() / ITERATIONS as u128,
        deps: "-",
    });

    // Print table
    println!(
        "{:<15} {:>10} {:>12} {:>10} {:<15}",
        "SECTION", "SIZE", "TIME_NS", "NS/BYTE", "DEPENDS_ON"
    );
    println!("{:-<65}", "");
    for s in &results {
        let ns_per_byte = if s.size > 0 {
            s.time_ns as f64 / s.size as f64
        } else {
            0.0
        };
        println!(
            "{:<15} {:>10} {:>12} {:>10.2} {:<15}",
            s.name, s.size, s.time_ns, ns_per_byte, s.deps
        );
    }
    println!("{:-<65}", "");
    println!(
        "{:<15} {:>10} {:>12}",
        "TOTAL",
        bytes.len(),
        results.iter().map(|s| s.time_ns).sum::<u128>()
    );
}
