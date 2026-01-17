//! Tests for Levenshtein DFA loading and fuzzy search
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.

use super::common::{cutlass_available, load_cutlass_layer};
use sorex::levenshtein_dfa::{ParametricDFA, QueryMatcher};

#[test]
fn test_dfa_in_cutlass_index() {
    if !cutlass_available() {
        println!("Skipping: Cutlass index not available");
        return;
    }
    let layer = load_cutlass_layer();

    println!("\n=== DFA in Cutlass Index ===");
    println!("DFA bytes: {}", layer.lev_dfa_bytes.len());

    if layer.lev_dfa_bytes.is_empty() {
        println!("✗ ERROR: No DFA bytes in index! This explains why T3 fuzzy search fails.");
        println!("  The DFA should be embedded in the binary file.");
        panic!("DFA is empty");
    } else {
        println!("✓ DFA is present ({} bytes)", layer.lev_dfa_bytes.len());
    }

    // Try to load the DFA
    match sorex::levenshtein_dfa::ParametricDFA::from_bytes(&layer.lev_dfa_bytes) {
        Ok(_dfa) => {
            println!("✓ Successfully loaded DFA from bytes");
            println!("  DFA structure appears valid");
        }
        Err(e) => {
            println!("✗ Failed to load DFA: {}", e);
            panic!("DFA loading failed: {}", e);
        }
    }
}

#[test]
fn test_t3_fuzzy_search_in_rust() {
    if !cutlass_available() {
        println!("Skipping: Cutlass index not available");
        return;
    }
    let layer = load_cutlass_layer();

    println!("\n=== T3 Fuzzy Search Test ===");
    println!("Vocabulary size: {}", layer.vocabulary.len());
    println!("DFA bytes: {}", layer.lev_dfa_bytes.len());

    // Load DFA
    let dfa = ParametricDFA::from_bytes(&layer.lev_dfa_bytes)
        .expect("Failed to load DFA");
    println!("✓ DFA loaded successfully");

    // Test fuzzy search for various queries
    let test_queries = vec![
        "kernel",      // Exact match exists
        "kernl",       // 1 edit distance from kernel
        "kernal",      // 1 edit distance from kernel
        "kerneł",      // 1 edit distance (unicode)
        "gemmm",       // 1 edit distance from gemm
        "wmarp",       // 1 edit distance from warp
    ];

    for query in test_queries {
        println!("\nFuzzy matching for '{}' (max distance=2):", query);

        let matcher = QueryMatcher::new(&dfa, query);
        let mut matches = Vec::new();

        for term in layer.vocabulary.iter() {
            if let Some(distance) = matcher.matches(term) {
                if distance <= 2 {
                    matches.push((term.clone(), distance));
                }
            }
        }

        // Sort by distance
        matches.sort_by_key(|(_, d)| *d);

        if matches.is_empty() {
            println!("  No matches found");
        } else {
            println!("  Found {} matches:", matches.len());
            for (term, distance) in matches.iter().take(5) {
                println!("    distance={}: \"{}\"", distance, term);
            }
        }
    }
}
