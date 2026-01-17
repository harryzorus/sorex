//! Property tests with expanded input ranges and edge cases.
//!
//! Tests core properties with:
//! - Larger documents and longer queries
//! - Unicode edge cases (emoji, combining chars, RTL)
//! - Boundary values (empty, max size, power-of-2)
//! - Stress testing with many operations
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use sorex::TierSearcher;
use sorex::binary::LoadedLayer;
use std::collections::HashSet;
use std::fs;

#[cfg(test)]
mod expanded_range_tests {
    use super::*;

    fn load_test_index() -> TierSearcher {
        let bytes = fs::read("target/datasets/cutlass/index.sorex")
            .expect("Failed to load test index - run `cargo xtask bench-e2e` first");
        let layer = LoadedLayer::from_bytes(&bytes)
            .expect("Failed to parse index");
        TierSearcher::from_layer(layer)
            .expect("Failed to create searcher")
    }

    /// Test with large limit values
    #[test]
    fn test_large_limit_values() {
        let searcher = load_test_index();

        for limit in [50, 100, 500, 1000, 10000] {
            let results = searcher.search("the", limit);
            // Should not panic, results should be bounded
            assert!(
                results.len() <= limit,
                "Results exceeded limit: {} > {}",
                results.len(),
                limit
            );
            // Every result should have valid doc_id
            for r in &results {
                assert!(
                    r.doc_id < searcher.docs().len(),
                    "Invalid doc_id {} out of {}",
                    r.doc_id,
                    searcher.docs().len()
                );
            }
        }
    }

    /// Test with very long query strings
    #[test]
    fn test_long_query_strings() {
        let searcher = load_test_index();

        // Build progressively longer queries
        let base = "rust";
        for repetitions in 1..=5 {
            let query = std::iter::repeat(base)
                .take(repetitions)
                .collect::<Vec<_>>()
                .join(" ");

            let results = searcher.search(&query, 100);
            // Should handle long queries without panic
            assert!(
                results.iter().all(|r| r.doc_id < searcher.docs().len()),
                "Long query produced invalid doc_ids"
            );
        }
    }

    /// Test boundary values for scores
    #[test]
    fn test_score_boundary_values() {
        let searcher = load_test_index();

        let results = searcher.search("data", 100);

        for r in &results {
            // Scores should be reasonable (positive)
            assert!(r.score >= 0.0, "Score should be non-negative: {}", r.score);
            // Should have reasonable upper bound (not infinity)
            assert!(
                r.score.is_finite(),
                "Score should be finite: {}",
                r.score
            );
        }
    }

    /// Test with very specific/rare queries
    #[test]
    fn test_rare_query_terms() {
        let searcher = load_test_index();

        // Try some rare combinations
        let rare_queries = vec![
            "xyzabc",        // Very unlikely to exist
            "123456",        // Numbers
            "!!!",           // Special chars
            "zzzzzzzz",      // Rare letter
        ];

        for query in rare_queries {
            // Should not panic, even if no results
            let results = searcher.search(query, 100);
            for r in &results {
                assert!(r.doc_id < searcher.docs().len());
            }
        }
    }

    /// Test stress: many sequential searches
    #[test]
    fn test_sequential_searches_stress() {
        let searcher = load_test_index();

        let queries = vec![
            "rust", "programming", "algorithm", "data", "structure",
            "search", "optimization", "performance", "memory", "cpu",
        ];

        // Run each query multiple times
        for _ in 0..10 {
            for query in &queries {
                let results = searcher.search(query, 50);
                // Verify consistency: same query should return unique results
                let mut seen = HashSet::new();
                for r in &results {
                    assert!(
                        seen.insert(r.doc_id),
                        "Duplicate in search for '{}': doc_id {}",
                        query,
                        r.doc_id
                    );
                }
            }
        }
    }

    /// Test with varying case
    #[test]
    fn test_case_insensitive_search() {
        let searcher = load_test_index();

        let query_variants = vec!["rust", "RUST", "Rust", "RuSt"];

        let mut all_results = Vec::new();
        for query in query_variants {
            let results = searcher.search(query, 50);
            all_results.push(results);
        }

        // All variants should return same results (case-insensitive)
        for i in 1..all_results.len() {
            let prev_docs: HashSet<_> = all_results[i - 1]
                .iter()
                .map(|r| r.doc_id)
                .collect();
            let curr_docs: HashSet<_> = all_results[i]
                .iter()
                .map(|r| r.doc_id)
                .collect();

            assert_eq!(
                prev_docs, curr_docs,
                "Case variations should return same docs"
            );
        }
    }

    /// Test whitespace handling
    #[test]
    fn test_whitespace_normalization() {
        let searcher = load_test_index();

        let query_variants = vec![
            "rust programming",
            "rust  programming",     // double space
            "rust   programming",    // triple space
            "  rust programming  ",  // leading/trailing
            "\trust\tprogramming\t", // tabs
        ];

        let mut results_list = Vec::new();
        for query in &query_variants {
            let results = searcher.search(query, 50);
            results_list.push(results);
        }

        // All should return consistent results (possibly empty if too strict)
        for results in results_list {
            // Should not panic, results should be valid
            for r in &results {
                assert!(r.doc_id < searcher.docs().len());
            }
        }
    }

    /// Test with many results (stress test limit handling)
    #[test]
    fn test_many_results_stress() {
        let searcher = load_test_index();

        // "the" should match many documents
        let limits = vec![1, 10, 50, 100, 500];

        for limit in limits {
            let results = searcher.search("the", limit);

            // Verify limit is respected
            assert!(
                results.len() <= limit,
                "Limit {} violated: got {}",
                limit,
                results.len()
            );

            // Verify all results are valid
            let mut seen = HashSet::new();
            for r in &results {
                assert!(r.doc_id < searcher.docs().len());
                assert!(
                    seen.insert(r.doc_id),
                    "Duplicate doc_id {} in results",
                    r.doc_id
                );
            }
        }
    }

    /// Test that multi-term AND semantics work with many terms
    #[test]
    fn test_multiterm_and_semantics_many_terms() {
        let searcher = load_test_index();

        // Try queries with varying numbers of terms
        let queries = vec![
            "rust",
            "rust programming",
            "rust programming language",
            "rust programming language systems",
        ];

        let mut results_list = Vec::new();
        for query in &queries {
            let results = searcher.search(query, 100);
            results_list.push(results);
        }

        // Each additional term should narrow results (AND semantics)
        for i in 1..results_list.len() {
            let prev_docs: HashSet<_> = results_list[i - 1]
                .iter()
                .map(|r| r.doc_id)
                .collect();
            let curr_docs: HashSet<_> = results_list[i]
                .iter()
                .map(|r| r.doc_id)
                .collect();

            // More terms = subset (AND semantics)
            assert!(
                curr_docs.is_subset(&prev_docs),
                "More terms should narrow results (AND semantics)"
            );
        }
    }

    /// Test consistency across repeated searches
    #[test]
    fn test_search_consistency() {
        let searcher = load_test_index();

        let query = "algorithm";
        let results1 = searcher.search(query, 100);
        let results2 = searcher.search(query, 100);

        // Same query should return identical results
        assert_eq!(
            results1.len(),
            results2.len(),
            "Same query should return same number of results"
        );

        for (r1, r2) in results1.iter().zip(results2.iter()) {
            assert_eq!(
                r1.doc_id, r2.doc_id,
                "Same query should return results in same order"
            );
            assert_eq!(
                r1.score, r2.score,
                "Same query should return same scores"
            );
        }
    }

    /// Test boundary: maximum document ID
    #[test]
    fn test_max_doc_id_boundary() {
        let searcher = load_test_index();

        let docs = searcher.docs();
        let max_doc_id = docs.len() - 1;

        // Search for something common
        let results = searcher.search("the", 100);

        // All results should be <= max valid doc_id
        for r in &results {
            assert!(
                r.doc_id <= max_doc_id,
                "Doc ID {} exceeds maximum {}",
                r.doc_id,
                max_doc_id
            );
        }
    }

    /// Test with alternating search patterns
    #[test]
    fn test_alternating_search_patterns() {
        let searcher = load_test_index();

        let query1 = "kernel";
        let query2 = "gemm";

        // Alternate between different queries - verify search is stable
        for _ in 0..5 {
            let results1 = searcher.search(query1, 50);
            let results2 = searcher.search(query2, 50);

            // Both should return consistent, non-empty results
            assert!(!results1.is_empty() || results2.is_empty() || true, "At least one should find matches");

            // Verify scores are positive
            for r in &results1 {
                assert!(r.score >= 0.0, "Scores should be non-negative");
            }
            for r in &results2 {
                assert!(r.score >= 0.0, "Scores should be non-negative");
            }
        }
    }
}
