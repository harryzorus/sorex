//! Tests for Deno-based WASM search execution.
//!
//! These tests verify that the WASM search through Deno produces the same
//! results as the native Rust implementation.
//!
//! Run with: cargo test --features deno-runtime test_deno
//!
//! Note: These tests are only compiled when deno-runtime feature is enabled.

#![cfg(feature = "deno-runtime")]

use super::common::{load_fixtures_searcher, FIXTURES_INDEX};
use sorex::deno_runtime::{DenoRuntime, DenoSearchContext};
use sorex::tiered_search::TierSearcher;
use std::fs;

/// The generated sorex.js (from scripts/build-loader.ts)
/// This is built from target/pkg/sorex.js (wasm-pack output) + .sorex parser + convenience functions.
/// Rebuild with: bun scripts/build-loader.ts
const SOREX_LOADER_JS: &str = include_str!("../../target/loader/sorex.js");

/// Return the raw sorex.js content.
/// The deno_runtime::strip_esm_exports() will handle stripping.
fn get_loader_js() -> String {
    SOREX_LOADER_JS.to_string()
}

fn load_native_searcher() -> TierSearcher {
    load_fixtures_searcher()
}

fn load_sorex_bytes() -> Vec<u8> {
    fs::read(FIXTURES_INDEX).expect("Failed to read fixtures index file")
}

#[test]
fn test_deno_runtime_creates_successfully() {
    let runtime = DenoRuntime::new();
    assert!(
        runtime.is_ok(),
        "Deno runtime should initialize successfully"
    );
}

#[test]
fn test_deno_wasm_search_basic() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    let results = runtime
        .search(&sorex_bytes, &get_loader_js(), "rust", 10)
        .expect("Deno search should succeed");

    assert!(
        !results.is_empty(),
        "Search for 'rust' should return results"
    );

    // Verify result structure
    for r in &results {
        assert!(!r.title.is_empty(), "Result should have a title");
        assert!(!r.href.is_empty(), "Result should have an href");
        assert!(r.tier >= 1 && r.tier <= 3, "Tier should be 1, 2, or 3");
        assert!(r.match_type <= 4, "Match type should be 0-4");
    }
}

#[test]
fn test_deno_wasm_matches_native_exact() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();
    let native_searcher = load_native_searcher();

    // Test exact match query
    let query = "rust";
    let limit = 10;

    let wasm_results = runtime
        .search(&sorex_bytes, &get_loader_js(), query, limit)
        .expect("Deno search should succeed");

    let native_results = native_searcher.search(query, limit);

    // Compare result counts
    assert_eq!(
        wasm_results.len(),
        native_results.len(),
        "WASM and native should return same number of results for '{}'",
        query
    );

    // Compare titles (order should match)
    for (i, (wasm_r, native_r)) in wasm_results.iter().zip(native_results.iter()).enumerate() {
        let native_doc = &native_searcher.docs()[native_r.doc_id];
        assert_eq!(
            wasm_r.title, native_doc.title,
            "Result {} title mismatch: WASM='{}' vs Native='{}'",
            i, wasm_r.title, native_doc.title
        );
        assert_eq!(
            wasm_r.tier, native_r.tier,
            "Result {} tier mismatch: WASM={} vs Native={}",
            i, wasm_r.tier, native_r.tier
        );
    }
}

#[test]
fn test_deno_wasm_matches_native_prefix() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();
    let native_searcher = load_native_searcher();

    // Test prefix match query (shorter query for prefix matching)
    let query = "typ";
    let limit = 10;

    let wasm_results = runtime
        .search(&sorex_bytes, &get_loader_js(), query, limit)
        .expect("Deno search should succeed");

    let native_results = native_searcher.search(query, limit);

    // Compare result counts
    assert_eq!(
        wasm_results.len(),
        native_results.len(),
        "WASM and native should return same number of results for prefix '{}'",
        query
    );

    // Compare result order by title
    for (i, (wasm_r, native_r)) in wasm_results.iter().zip(native_results.iter()).enumerate() {
        let native_doc = &native_searcher.docs()[native_r.doc_id];
        assert_eq!(
            wasm_r.title, native_doc.title,
            "Prefix search result {} title mismatch",
            i
        );
    }
}

#[test]
fn test_deno_wasm_matches_native_fuzzy() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();
    let native_searcher = load_native_searcher();

    // Test fuzzy match query (typo in "rust")
    let query = "rast";
    let limit = 10;

    let wasm_results = runtime
        .search(&sorex_bytes, &get_loader_js(), query, limit)
        .expect("Deno search should succeed");

    let native_results = native_searcher.search(query, limit);

    // Compare result counts
    assert_eq!(
        wasm_results.len(),
        native_results.len(),
        "WASM and native should return same number of results for fuzzy '{}'",
        query
    );

    // At least verify the top results match
    if !wasm_results.is_empty() {
        let native_doc = &native_searcher.docs()[native_results[0].doc_id];
        assert_eq!(
            wasm_results[0].title, native_doc.title,
            "Top fuzzy result should match"
        );
    }
}

#[test]
fn test_deno_wasm_empty_query() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    let results = runtime
        .search(&sorex_bytes, &get_loader_js(), "", 10)
        .expect("Deno search should succeed for empty query");

    assert!(
        results.is_empty(),
        "Empty query should return empty results"
    );
}

#[test]
fn test_deno_wasm_no_results_query() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    let results = runtime
        .search(&sorex_bytes, &get_loader_js(), "xyzzyplugh", 10)
        .expect("Deno search should succeed for nonsense query");

    assert!(
        results.is_empty(),
        "Nonsense query should return empty results"
    );
}

#[test]
fn test_deno_wasm_bucketed_ranking() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    // Search for something that should have different match types
    let results = runtime
        .search(&sorex_bytes, &get_loader_js(), "typescript", 20)
        .expect("Deno search should succeed");

    // Verify bucketed ranking: results should be sorted by match_type ascending
    // (Title=0 comes before Section=1 comes before Content=4)
    for i in 1..results.len() {
        assert!(
            results[i - 1].match_type <= results[i].match_type,
            "Results should be bucketed by match_type: {:?} at {} should come before {:?} at {}",
            results[i - 1].match_type,
            i - 1,
            results[i].match_type,
            i
        );
    }
}

#[test]
fn test_deno_context_reuse() {
    let sorex_bytes = load_sorex_bytes();
    let loader_js = get_loader_js();

    // Create context once
    let mut ctx = DenoSearchContext::new(&sorex_bytes, &loader_js)
        .expect("Failed to create DenoSearchContext");

    // Execute multiple searches on the same context
    // Use terms from the e2e fixture (data/e2e/fixtures)
    let results1 = ctx.search("rust", 10).expect("First search should succeed");
    let results2 = ctx
        .search("typescript", 10)
        .expect("Second search should succeed");
    let results3 = ctx
        .search("webassembly", 10)
        .expect("Third search should succeed");

    // All searches should return results
    assert!(!results1.is_empty(), "First search should return results");
    assert!(!results2.is_empty(), "Second search should return results");
    assert!(!results3.is_empty(), "Third search should return results");

    // Verify context is reused correctly by checking results have non-empty hrefs
    assert!(
        !results1[0].href.is_empty(),
        "First result should have href"
    );
    assert!(
        !results2[0].href.is_empty(),
        "Second result should have href"
    );
    assert!(
        !results3[0].href.is_empty(),
        "Third result should have href"
    );
}

#[test]
fn test_deno_special_characters_in_query() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    // Test queries with special characters that need escaping
    let special_queries = [
        "\"quoted\"",       // Double quotes
        "back\\slash",      // Backslash
        "single'quote",     // Single quote
        "unicode\u{00e9}",  // Unicode (accented e)
        "multi word query", // Spaces
        "<tag>",            // Angle brackets
    ];

    for query in special_queries {
        // Should not panic or error
        let result = runtime.search(&sorex_bytes, &get_loader_js(), query, 10);
        assert!(
            result.is_ok(),
            "Query with special chars '{}' should not error: {:?}",
            query,
            result.err()
        );
    }
}

// ============================================================================
// MATCHED_TERM AND SCORE PARITY TESTS
// ============================================================================

#[test]
fn test_deno_wasm_matched_term_populated() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    // Search for a common term
    let results = runtime
        .search(&sorex_bytes, &get_loader_js(), "rust", 10)
        .expect("Deno search should succeed");

    assert!(!results.is_empty(), "Should find results for 'rust'");

    // WASM results should have matched_term populated
    let with_term = results.iter().filter(|r| r.matched_term.is_some()).count();

    assert!(
        with_term > 0,
        "Expected some WASM results to have matched_term for 'rust', got 0/{} results",
        results.len()
    );

    // Verify matched_term makes sense
    for r in &results {
        if let Some(ref term) = r.matched_term {
            // For exact match on "rust", matched_term should be "rust"
            if r.tier == 1 {
                assert_eq!(
                    term, "rust",
                    "T1 exact match should have matched_term='rust', got '{}'",
                    term
                );
            }
        }
    }
}

#[test]
fn test_deno_wasm_matched_term_matches_native() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();
    let native_searcher = load_native_searcher();

    let query = "rust";
    let limit = 10;

    let wasm_results = runtime
        .search(&sorex_bytes, &get_loader_js(), query, limit)
        .expect("Deno search should succeed");

    let native_results = native_searcher.search(query, limit);
    let vocabulary = native_searcher.vocabulary();

    // Compare matched_term between WASM and native
    for (i, (wasm_r, native_r)) in wasm_results.iter().zip(native_results.iter()).enumerate() {
        // Resolve native matched_term index to string
        let native_term = native_r
            .matched_term
            .and_then(|idx| vocabulary.get(idx as usize).cloned());

        assert_eq!(
            wasm_r.matched_term, native_term,
            "Result {} matched_term mismatch: WASM={:?} vs Native={:?}",
            i, wasm_r.matched_term, native_term
        );
    }
}

#[test]
fn test_deno_wasm_scores_match_native() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();
    let native_searcher = load_native_searcher();

    let query = "rust";
    let limit = 10;

    let wasm_results = runtime
        .search(&sorex_bytes, &get_loader_js(), query, limit)
        .expect("Deno search should succeed");

    let native_results = native_searcher.search(query, limit);

    // Compare scores between WASM and native
    for (i, (wasm_r, native_r)) in wasm_results.iter().zip(native_results.iter()).enumerate() {
        // Scores should match within floating point tolerance
        let score_diff = (wasm_r.score - native_r.score).abs();
        assert!(
            score_diff < 0.001,
            "Result {} score mismatch: WASM={:.4} vs Native={:.4} (diff={:.6})",
            i,
            wasm_r.score,
            native_r.score,
            score_diff
        );
    }
}

#[test]
fn test_deno_wasm_t3_scores_nonzero() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();
    let native_searcher = load_native_searcher();

    // Use a typo query to trigger T3 fuzzy matching
    let query = "ruts";
    let limit = 20;

    let wasm_results = runtime
        .search(&sorex_bytes, &get_loader_js(), query, limit)
        .expect("Deno search should succeed");

    let native_results = native_searcher.search(query, limit);

    // Debug: print first few results
    eprintln!(
        "\nQuery: '{}' - comparing {} WASM vs {} native results",
        query,
        wasm_results.len(),
        native_results.len()
    );
    for (i, (w, n)) in wasm_results
        .iter()
        .zip(native_results.iter())
        .enumerate()
        .take(5)
    {
        eprintln!(
            "  [{}] WASM: tier={} score={:.2} matched_term={:?} title='{}'",
            i, w.tier, w.score, w.matched_term, w.title
        );
        eprintln!("       Native: tier={} score={:.2}", n.tier, n.score);
    }

    // Compare T3 results between WASM and native
    for (wasm_r, native_r) in wasm_results.iter().zip(native_results.iter()) {
        // WASM and native should have matching tiers and scores
        assert_eq!(
            wasm_r.tier, native_r.tier,
            "Tier mismatch for '{}': WASM={} vs Native={}",
            wasm_r.title, wasm_r.tier, native_r.tier
        );

        // If native has non-zero score, WASM should too
        if native_r.score > 0.0 {
            assert!(
                wasm_r.score > 0.0,
                "WASM score should be non-zero when native is non-zero: \
                 WASM={} vs Native={} for '{}'",
                wasm_r.score,
                native_r.score,
                wasm_r.title
            );
        }
    }
}

#[test]
fn test_deno_wasm_prefix_matched_term() {
    let runtime = DenoRuntime::new().expect("Failed to create Deno runtime");
    let sorex_bytes = load_sorex_bytes();

    // Use a prefix query
    let results = runtime
        .search(&sorex_bytes, &get_loader_js(), "typ", 10)
        .expect("Deno search should succeed");

    // Check T2 prefix results have valid matched_term
    for r in &results {
        if r.tier == 2 {
            if let Some(ref term) = r.matched_term {
                assert!(
                    term.starts_with("typ"),
                    "T2 prefix matched_term '{}' should start with 'typ'",
                    term
                );
            }
        }
    }
}
