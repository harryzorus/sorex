//! Unit tests for matched_term field in SearchResult.
//!
//! These tests verify that the matched_term field correctly tracks
//! which vocabulary term matched for each search result tier.
//!
//! - T1 (exact): matched_term should be the query term's vocab index
//! - T2 (prefix): matched_term should be a vocab term starting with query
//! - T3 (fuzzy): matched_term should be a vocab term within edit distance
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use super::common::load_cutlass_searcher;
use std::collections::HashSet;

// ============================================================================
// T1 EXACT MATCH - matched_term TESTS
// ============================================================================

#[test]
fn test_t1_exact_sets_matched_term() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search_tier1_exact("kernel", 10);

    assert!(
        !results.is_empty(),
        "Should find exact matches for 'kernel'"
    );

    // Get the vocabulary to verify matched_term
    let vocab = searcher.vocabulary();

    for result in &results {
        // T1 exact matches should have matched_term set
        assert!(
            result.matched_term.is_some(),
            "T1 exact match should have matched_term, doc_id={}",
            result.doc_id
        );

        // The matched term should resolve to the query term
        if let Some(idx) = result.matched_term {
            let term = &vocab[idx as usize];
            assert_eq!(
                term, "kernel",
                "T1 matched_term should be the exact query term, got '{}'",
                term
            );
        }
    }
}

#[test]
fn test_t1_matched_term_valid_index() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search_tier1_exact("gemm", 20);
    let vocab_len = searcher.vocabulary().len();

    for result in &results {
        if let Some(idx) = result.matched_term {
            assert!(
                (idx as usize) < vocab_len,
                "matched_term index {} should be < vocabulary length {}",
                idx,
                vocab_len
            );
        }
    }
}

// ============================================================================
// T2 PREFIX MATCH - matched_term TESTS
// ============================================================================

#[test]
fn test_t2_prefix_sets_matched_term() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();
    let results = searcher.search_tier2_prefix("kern", &exclude, 10);

    assert!(!results.is_empty(), "Should find prefix matches for 'kern'");

    let vocab = searcher.vocabulary();

    for result in &results {
        // T2 prefix matches should have matched_term set
        assert!(
            result.matched_term.is_some(),
            "T2 prefix match should have matched_term, doc_id={}",
            result.doc_id
        );

        // The matched term should start with the query prefix
        if let Some(idx) = result.matched_term {
            let term = &vocab[idx as usize];
            assert!(
                term.starts_with("kern"),
                "T2 matched_term '{}' should start with query prefix 'kern'",
                term
            );
        }
    }
}

#[test]
fn test_t2_matched_term_valid_index() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();
    let results = searcher.search_tier2_prefix("mat", &exclude, 20);
    let vocab_len = searcher.vocabulary().len();

    for result in &results {
        if let Some(idx) = result.matched_term {
            assert!(
                (idx as usize) < vocab_len,
                "matched_term index {} should be < vocabulary length {}",
                idx,
                vocab_len
            );
        }
    }
}

// ============================================================================
// T3 FUZZY MATCH - matched_term TESTS
// ============================================================================

#[test]
fn test_t3_fuzzy_sets_matched_term() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();
    // Use a typo that should fuzzy-match "kernel"
    let results = searcher.search_tier3_fuzzy("kernl", &exclude, 10);

    // May or may not find results depending on fuzzy threshold
    if !results.is_empty() {
        let vocab = searcher.vocabulary();

        for result in &results {
            // T3 fuzzy matches should have matched_term set
            assert!(
                result.matched_term.is_some(),
                "T3 fuzzy match should have matched_term, doc_id={}",
                result.doc_id
            );

            // The matched term should be a valid vocab index
            if let Some(idx) = result.matched_term {
                assert!(
                    (idx as usize) < vocab.len(),
                    "matched_term index {} should be < vocabulary length {}",
                    idx,
                    vocab.len()
                );
            }
        }
    }
}

#[test]
fn test_t3_scores_are_nonzero() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();
    // Test various fuzzy queries
    for query in &["kernl", "gemn", "optmize"] {
        let results = searcher.search_tier3_fuzzy(query, &exclude, 20);

        for result in &results {
            assert!(
                result.score > 0.0,
                "T3 fuzzy score should be > 0, got {} for query '{}' doc_id={}",
                result.score,
                query,
                result.doc_id
            );
        }
    }
}

#[test]
fn test_t3_matched_term_within_edit_distance() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();
    let query = "kernl"; // Edit distance 1 from "kernel"
    let results = searcher.search_tier3_fuzzy(query, &exclude, 10);

    if !results.is_empty() {
        let vocab = searcher.vocabulary();

        for result in &results {
            if let Some(idx) = result.matched_term {
                let term = &vocab[idx as usize];
                // The matched term should be within edit distance 2 of query
                let distance = levenshtein_distance(query, term);
                assert!(
                    distance <= 2,
                    "T3 matched_term '{}' should be within edit distance 2 of query '{}', got distance {}",
                    term,
                    query,
                    distance
                );
            }
        }
    }
}

// ============================================================================
// FULL PIPELINE - matched_term TESTS
// ============================================================================

#[test]
fn test_full_search_has_matched_term() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search("kernel", 20);

    assert!(!results.is_empty(), "Should find results for 'kernel'");

    let vocab = searcher.vocabulary();

    // All results should have matched_term (unless edge case)
    let with_term_count = results.iter().filter(|r| r.matched_term.is_some()).count();

    // At least 80% of results should have matched_term
    let threshold = (results.len() * 80) / 100;
    assert!(
        with_term_count >= threshold,
        "Expected at least {}% of results to have matched_term, got {}/{}",
        80,
        with_term_count,
        results.len()
    );

    // All matched_terms should be valid
    for result in &results {
        if let Some(idx) = result.matched_term {
            assert!(
                (idx as usize) < vocab.len(),
                "matched_term index {} should be valid",
                idx
            );
        }
    }
}

#[test]
fn test_multiterm_query_matched_term() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search("kernel optimization", 10);

    let vocab = searcher.vocabulary();

    for result in &results {
        if let Some(idx) = result.matched_term {
            let term = &vocab[idx as usize];
            // For multi-term queries, matched_term should be one of the query terms
            // or a fuzzy/prefix variant
            assert!(
                term.contains("kernel")
                    || term.contains("optim")
                    || term.starts_with("kern")
                    || term.starts_with("opt"),
                "matched_term '{}' should relate to query terms",
                term
            );
        }
    }
}

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

/// Simple Levenshtein distance for test verification.
fn levenshtein_distance(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let m = a_chars.len();
    let n = b_chars.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    let mut dp = vec![vec![0; n + 1]; m + 1];

    for i in 0..=m {
        dp[i][0] = i;
    }
    for j in 0..=n {
        dp[0][j] = j;
    }

    for i in 1..=m {
        for j in 1..=n {
            let cost = if a_chars[i - 1] == b_chars[j - 1] {
                0
            } else {
                1
            };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[m][n]
}
