//! Tests for tier exclusion properties.
//!
//! Validates that each tier excludes documents found in earlier tiers:
//! - Tier 2 (prefix) doesn't return docs from Tier 1 (exact)
//! - Tier 3 (fuzzy) doesn't return docs from Tier 1 or Tier 2
//!
//! This is critical for ranked search: results should come from the best tier only.
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use sorex::TierSearcher;
use sorex::binary::LoadedLayer;
use std::collections::HashSet;
use std::fs;

#[cfg(test)]
mod tier_exclusion_tests {
    use super::*;

    fn load_test_index() -> TierSearcher {
        let bytes = fs::read("target/datasets/cutlass/index.sorex")
            .expect("Failed to load test index");
        let layer = LoadedLayer::from_bytes(&bytes)
            .expect("Failed to parse index");
        TierSearcher::from_layer(layer)
            .expect("Failed to create searcher")
    }

    /// Tier 2 should not return docs that were found in Tier 1
    #[test]
    fn test_tier2_excludes_tier1() {
        let searcher = load_test_index();

        // Get results from combined search
        let combined_results = searcher.search("rust", 100);

        let tier1_docs: HashSet<_> = combined_results
            .iter()
            .filter(|r| r.tier == 1)
            .map(|r| r.doc_id)
            .collect();

        let tier2_docs: HashSet<_> = combined_results
            .iter()
            .filter(|r| r.tier == 2)
            .map(|r| r.doc_id)
            .collect();

        // Tier 1 and Tier 2 should be disjoint
        let overlap: Vec<_> = tier1_docs.intersection(&tier2_docs).collect();
        assert!(
            overlap.is_empty(),
            "Tier 2 returned docs from Tier 1: {:?}",
            overlap
        );
    }

    /// Tier 3 should not return docs found in Tier 1 or Tier 2
    #[test]
    fn test_tier3_excludes_tier1_and_tier2() {
        let searcher = load_test_index();

        let combined_results = searcher.search("rust", 100);

        let earlier_tiers: HashSet<_> = combined_results
            .iter()
            .filter(|r| r.tier <= 2)
            .map(|r| r.doc_id)
            .collect();

        let tier3_docs: HashSet<_> = combined_results
            .iter()
            .filter(|r| r.tier == 3)
            .map(|r| r.doc_id)
            .collect();

        // Tier 3 should not overlap with earlier tiers
        let overlap: Vec<_> = earlier_tiers.intersection(&tier3_docs).collect();
        assert!(
            overlap.is_empty(),
            "Tier 3 returned docs from Tier 1 or 2: {:?}",
            overlap
        );
    }

    /// Each tier respects the limit
    #[test]
    fn test_each_tier_respects_limit() {
        let searcher = load_test_index();

        let limit = 20;
        let results = searcher.search("the", limit);

        // Collect by tier
        let mut tier_counts = std::collections::BTreeMap::new();
        for r in &results {
            *tier_counts.entry(r.tier).or_insert(0) += 1;
        }

        // Each tier should have at most `limit` results (though combined they're limited to `limit`)
        for (_tier, count) in tier_counts {
            assert!(
                count <= limit,
                "Tier exceeded limit: {} > {}",
                count, limit
            );
        }

        // Total should not exceed limit
        assert!(
            results.len() <= limit,
            "Total results {} exceed limit {}",
            results.len(),
            limit
        );
    }

    /// When searching for an exact match, Tier 1 results should come before others
    #[test]
    fn test_tier1_results_come_first() {
        let searcher = load_test_index();

        // Search for a common word
        let results = searcher.search("rust", 50);

        // Find first Tier 1 and first Tier 2+ result
        let first_tier1 = results.iter().position(|r| r.tier == 1);
        let first_tier2plus = results.iter().position(|r| r.tier > 1);

        // If both exist, Tier 1 should come first
        if let (Some(t1_pos), Some(t2_pos)) = (first_tier1, first_tier2plus) {
            assert!(
                t1_pos < t2_pos,
                "Tier 1 results should come before Tier 2+ results"
            );
        }

        // Verify no duplicates within each tier
        for tier in 1..=3 {
            let mut seen = HashSet::new();
            for r in results.iter().filter(|r| r.tier == tier) {
                assert!(
                    seen.insert(r.doc_id),
                    "Duplicate doc_id in Tier {}: {}",
                    tier, r.doc_id
                );
            }
        }
    }

    /// Multi-word exact match should not appear in fuzzy tier
    #[test]
    fn test_multiword_exact_not_fuzzy() {
        let searcher = load_test_index();

        // Search for a multi-word phrase
        let results = searcher.search("data structure", 100);

        let tier1_docs: HashSet<_> = results
            .iter()
            .filter(|r| r.tier == 1)
            .map(|r| r.doc_id)
            .collect();

        let tier3_docs: HashSet<_> = results
            .iter()
            .filter(|r| r.tier == 3)
            .map(|r| r.doc_id)
            .collect();

        // Tier 1 and Tier 3 should be disjoint
        let overlap: Vec<_> = tier1_docs.intersection(&tier3_docs).collect();
        assert!(
            overlap.is_empty(),
            "Tier 3 (fuzzy) contains docs from Tier 1 (exact): {:?}",
            overlap
        );
    }

    /// Verify results are properly sorted (by match_type first, then score)
    #[test]
    fn test_results_properly_sorted() {
        let searcher = load_test_index();

        let results = searcher.search("search", 100);

        // Results should be sorted by match_type (lower is better)
        // and within same match_type by score (higher is better)
        for i in 1..results.len() {
            let prev = &results[i - 1];
            let curr = &results[i];

            // Check match_type ordering
            if prev.match_type != curr.match_type {
                assert!(
                    prev.match_type < curr.match_type,
                    "Match types not ordered correctly at position {}: {:?} > {:?}",
                    i, prev.match_type, curr.match_type
                );
            } else {
                // Same match_type: scores should be descending
                assert!(
                    prev.score >= curr.score,
                    "Scores not descending within same match_type at position {}",
                    i
                );
            }
        }
    }

    /// Verify no duplicate doc_ids across entire result set
    #[test]
    fn test_no_duplicate_docs_across_tiers() {
        let searcher = load_test_index();

        let results = searcher.search("program", 100);
        let mut seen = HashSet::new();

        for r in results {
            assert!(
                seen.insert(r.doc_id),
                "Duplicate doc_id {} found across tiers",
                r.doc_id
            );
        }
    }
}
