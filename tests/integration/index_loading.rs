//! Test loading and searching real indexes
//!
//! These tests verify that:
//! - The E2E fixtures index loads correctly
//! - Basic search operations work
//! - Index structure is valid (suffix array sorted, etc.)
//!
//! **Note**: These tests require pre-built fixtures. Run `cargo xtask verify` first.
//! They are `#[ignore]`d by default to avoid CI failures.

use super::common::{load_fixtures_layer, load_fixtures_searcher};

// ============================================================================
// DEMO INDEX TESTS
// ============================================================================

#[test]
#[ignore = "Requires pre-built fixtures (run `cargo xtask verify`)"]
fn test_load_fixtures_index() {
    let _layer = load_fixtures_layer(); // Verify layer loading works
    let searcher = load_fixtures_searcher();

    // Verify basic properties
    assert!(!searcher.docs().is_empty(), "Index should have documents");
    assert!(!searcher.vocabulary().is_empty(), "Index should have vocabulary");

    println!("Fixtures index loaded successfully");
    println!("   Docs: {}", searcher.docs().len());
    println!("   Vocab size: {}", searcher.vocabulary().len());
    println!("   Suffix array: {}", searcher.suffix_array().len());
}

#[test]
#[ignore = "Requires pre-built fixtures (run `cargo xtask verify`)"]
fn test_search_fixtures_rust() {
    let searcher = load_fixtures_searcher();

    // Test search for "rust"
    let results = searcher.search("rust", 5);

    println!("Search for 'rust' completed");
    println!("   Results: {} found", results.len());

    for (i, r) in results.iter().enumerate() {
        let doc = &searcher.docs()[r.doc_id];
        println!("   {}. T{} ({:.1}) {}", i + 1, r.tier, r.score, doc.title);
    }

    assert!(!results.is_empty(), "Should find results for 'rust'");
}

#[test]
#[ignore = "Requires pre-built fixtures (run `cargo xtask verify`)"]
fn test_search_fixtures_typescript() {
    let searcher = load_fixtures_searcher();

    // Test search for "typescript"
    let results = searcher.search("typescript", 5);

    println!("Search for 'typescript' completed");
    println!("   Results: {} found", results.len());

    for (i, r) in results.iter().enumerate() {
        let doc = &searcher.docs()[r.doc_id];
        println!("   {}. T{} ({:.1}) {}", i + 1, r.tier, r.score, doc.title);
    }

    assert!(!results.is_empty(), "Should find results for 'typescript'");
}

// ============================================================================
// INDEX INSPECTION TESTS
// ============================================================================

#[test]
#[ignore = "Requires pre-built fixtures (run `cargo xtask verify`)"]
fn load_and_inspect_fixtures_index() {
    let layer = load_fixtures_layer();

    println!("\n=== LOADED INDEX ===");
    println!("Docs: {}", layer.doc_count);
    println!("Terms: {}", layer.vocabulary.len());
    println!("Suffix array entries: {}", layer.suffix_array.len());

    // Print first 50 vocabulary terms
    println!("\nFirst 20 vocabulary terms:");
    for (i, term) in layer.vocabulary.iter().take(20).enumerate() {
        println!("  [{}] \"{}\"", i, term);
    }

    // Print first 100 suffix array entries with actual suffix strings
    println!("\nFirst 100 suffix array entries:");
    for (i, &(term_idx, offset)) in layer.suffix_array.iter().take(100).enumerate() {
        let term = &layer.vocabulary[term_idx as usize];
        let suffix = if (offset as usize) < term.len() {
            &term[offset as usize..]
        } else {
            "[OUT_OF_BOUNDS]"
        };
        println!("[{:4}] term_idx={:4}, offset={:3}, term=\"{}\", suffix=\"{}\"",
            i, term_idx, offset, term, suffix);
    }

    // Check if sorted
    println!("\n=== CHECKING SUFFIX ARRAY ORDERING ===");
    let mut sorted = true;
    for i in 1..layer.suffix_array.len().min(1000) {
        let (prev_idx, prev_off) = layer.suffix_array[i - 1];
        let (curr_idx, curr_off) = layer.suffix_array[i];

        let prev_term = &layer.vocabulary[prev_idx as usize];
        let curr_term = &layer.vocabulary[curr_idx as usize];

        let prev_suffix = if (prev_off as usize) < prev_term.len() {
            &prev_term[prev_off as usize..]
        } else {
            ""
        };

        let curr_suffix = if (curr_off as usize) < curr_term.len() {
            &curr_term[curr_off as usize..]
        } else {
            ""
        };

        if prev_suffix > curr_suffix {
            sorted = false;
            if i < 20 {
                println!("UNSORTED at [{}]: \"{}\" > \"{}\"", i, prev_suffix, curr_suffix);
            }
        }
    }

    if sorted {
        println!("Suffix array appears SORTED (checked first 1000 entries)");
    } else {
        println!("Suffix array NOT SORTED");
    }

    // Test partition_point
    println!("\n=== TESTING partition_point ===");
    let test_prefixes = vec!["a", "ja", "javascript", "ru", "rust", "typescript"];

    for prefix in test_prefixes {
        let pos = layer.suffix_array.partition_point(|(term_idx, offset)| {
            let term = &layer.vocabulary[*term_idx as usize];
            let suffix = if (*offset as usize) < term.len() {
                &term[*offset as usize..]
            } else {
                ""
            };
            suffix < prefix
        });

        let (suffix_at_pos, matches) = if pos < layer.suffix_array.len() {
            let (term_idx, offset) = layer.suffix_array[pos];
            let term = &layer.vocabulary[term_idx as usize];
            let suffix = if (offset as usize) < term.len() {
                &term[offset as usize..]
            } else {
                "[OUT_OF_BOUNDS]"
            };
            (suffix.to_string(), suffix.starts_with(prefix))
        } else {
            ("[END_OF_ARRAY]".to_string(), false)
        };

        let status = if matches { "OK" } else { "MISS" };
        println!("{} prefix=\"{}\": partition_point={}, suffix_at_pos=\"{}\"",
            status, prefix, pos, suffix_at_pos);
    }
}
