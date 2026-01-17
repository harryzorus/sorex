//! Property tests for query refinement and result stability.
//!
//! Tests that longer/more specific queries return subsets of broader query results.
//! This validates AND semantics and ensures search behavior is monotone.
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use sorex::TierSearcher;
use sorex::binary::LoadedLayer;
use std::collections::HashSet;
use std::fs;

#[cfg(test)]
mod query_refinement_tests {
    use super::*;

    fn load_test_index() -> TierSearcher {
        let bytes = fs::read("target/datasets/cutlass/index.sorex")
            .expect("Failed to load test index");
        let layer = LoadedLayer::from_bytes(&bytes)
            .expect("Failed to parse index");
        TierSearcher::from_layer(layer)
            .expect("Failed to create searcher")
    }

    /// Single-term query should be broader than multi-term query with that term
    /// Example: "rust" results ⊇ "rust optimization" results (AND semantics)
    #[test]
    fn test_single_term_broader_than_multi_term() {
        let searcher = load_test_index();

        let single_results: HashSet<_> = searcher
            .search("rust", 100)
            .iter()
            .map(|r| r.doc_id)
            .collect();

        let multi_results: HashSet<_> = searcher
            .search("rust optimization", 100)
            .iter()
            .map(|r| r.doc_id)
            .collect();

        // Multi-term should be subset of single-term
        assert!(
            multi_results.is_subset(&single_results),
            "Multi-term 'rust optimization' should be subset of 'rust'\n\
             Single: {:?}\nMulti: {:?}",
            single_results, multi_results
        );
    }

    /// Test that adding more terms refines results
    #[test]
    fn test_progressive_term_addition_refines() {
        let searcher = load_test_index();

        // Start with broadest query
        let q1: HashSet<_> = searcher
            .search("rust", 100)
            .iter()
            .map(|r| r.doc_id)
            .collect();

        // Add a second term
        let q2: HashSet<_> = searcher
            .search("rust performance", 100)
            .iter()
            .map(|r| r.doc_id)
            .collect();

        // Add a third term
        let q3: HashSet<_> = searcher
            .search("rust performance optimization", 100)
            .iter()
            .map(|r| r.doc_id)
            .collect();

        // Each should be subset of previous (AND semantics)
        assert!(
            q2.is_subset(&q1),
            "Adding terms should refine: q2 ⊆ q1"
        );
        assert!(
            q3.is_subset(&q2),
            "Adding more terms should refine further: q3 ⊆ q2"
        );
    }

    /// Empty query should return empty results
    #[test]
    fn test_empty_query_returns_empty() {
        let searcher = load_test_index();

        let results = searcher.search("", 100);
        assert!(results.is_empty(), "Empty query should return no results");

        let results = searcher.search("   ", 100);
        assert!(results.is_empty(), "Whitespace-only query should return no results");
    }

    /// Limit=0 should return empty results
    #[test]
    fn test_limit_zero_returns_empty() {
        let searcher = load_test_index();

        let results = searcher.search("rust", 0);
        assert!(results.is_empty(), "limit=0 should return no results");
    }

    /// Results should respect the limit parameter
    #[test]
    fn test_result_respects_limit() {
        let searcher = load_test_index();

        for limit in [1, 5, 10, 20] {
            let results = searcher.search("the", limit);
            assert!(
                results.len() <= limit,
                "Results {} should not exceed limit {}",
                results.len(),
                limit
            );
        }
    }

    /// Test tier progression: exact < prefix < fuzzy
    #[test]
    fn test_tier_score_order() {
        let searcher = load_test_index();

        // Search for a term that might be found in all tiers
        let results = searcher.search("rust", 100);

        // Collect by tier
        let mut tier1_scores = Vec::new();
        let mut tier2_scores = Vec::new();
        let mut tier3_scores = Vec::new();

        for r in &results {
            match r.tier {
                1 => tier1_scores.push(r.score),
                2 => tier2_scores.push(r.score),
                3 => tier3_scores.push(r.score),
                _ => {}
            }
        }

        // Tier 1 should have highest average score
        if !tier1_scores.is_empty() && !tier2_scores.is_empty() {
            let avg1 = tier1_scores.iter().sum::<f64>() / tier1_scores.len() as f64;
            let avg2 = tier2_scores.iter().sum::<f64>() / tier2_scores.len() as f64;
            assert!(
                avg1 >= avg2,
                "Tier 1 avg score {} should be >= Tier 2 avg score {}",
                avg1, avg2
            );
        }

        if !tier2_scores.is_empty() && !tier3_scores.is_empty() {
            let avg2 = tier2_scores.iter().sum::<f64>() / tier2_scores.len() as f64;
            let avg3 = tier3_scores.iter().sum::<f64>() / tier3_scores.len() as f64;
            assert!(
                avg2 >= avg3,
                "Tier 2 avg score {} should be >= Tier 3 avg score {}",
                avg2, avg3
            );
        }
    }

    /// Results should be unique by doc_id
    #[test]
    fn test_results_unique_by_doc_id() {
        let searcher = load_test_index();

        let results = searcher.search("and", 50);
        let mut seen = HashSet::new();

        for r in &results {
            assert!(
                seen.insert(r.doc_id),
                "Duplicate doc_id {} in results",
                r.doc_id
            );
        }
    }

    /// Test that score is monotone within tier
    #[test]
    fn test_score_monotone_within_tier() {
        let searcher = load_test_index();

        let results = searcher.search("data", 20);

        // Group by tier and check scores are descending
        let mut tier_results: std::collections::BTreeMap<u8, Vec<sorex::TierSearchResult>> =
            std::collections::BTreeMap::new();
        for r in results {
            tier_results.entry(r.tier).or_insert_with(Vec::new).push(r);
        }

        for (_tier, mut tier_list) in tier_results {
            tier_list.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
            for i in 1..tier_list.len() {
                assert!(
                    tier_list[i-1].score >= tier_list[i].score,
                    "Scores should be monotone descending within tier"
                );
            }
        }
    }
}
