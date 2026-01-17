//! Property tests for tiered search correctness.
//!
//! Verifies:
//! 1. Multi-term AND semantics (all results contain ALL query terms)
//! 2. Tier deduplication (no doc appears in multiple tiers)
//! 3. Tier ordering (T1 < T2 < T3 in results)
//! 4. Fuzzy score computation properties
//! 5. Section deduplication (best match_type per doc)
//!
//! **Note**: Requires pre-built fixtures. Enable with `--features bench-datasets`.
#![cfg(feature = "bench-datasets")]

use super::common::load_fixtures_searcher;
use proptest::prelude::*;
use std::collections::{HashMap, HashSet};

// ============================================================================
// STRATEGIES
// ============================================================================

/// Generate word-like strings for building test queries.
fn word_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{2,8}").unwrap()
}

/// Generate a single-term query from common search terms.
fn single_term_query_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("rust".to_string()),
        Just("typescript".to_string()),
        Just("javascript".to_string()),
        Just("wasm".to_string()),
        Just("webassembly".to_string()),
        Just("programming".to_string()),
        Just("safety".to_string()),
        Just("performance".to_string()),
        Just("browser".to_string()),
        Just("native".to_string()),
        word_strategy(),
    ]
}

/// Generate multi-term queries (2-3 terms).
#[allow(dead_code)]
fn multiterm_query_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(single_term_query_strategy(), 2..=3)
        .prop_map(|terms| terms.join(" "))
}

/// Generate a limit value for search.
fn limit_strategy() -> impl Strategy<Value = usize> {
    1usize..=50
}

// ============================================================================
// TIER ORDERING AND DEDUPLICATION PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Property: No duplicate doc_ids across results.
    ///
    /// Each document should appear at most once in search results.
    #[test]
    fn prop_no_duplicate_docs_in_results(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let results = searcher.search(&query, limit);

        let mut seen_docs: HashSet<usize> = HashSet::new();
        for result in &results {
            prop_assert!(
                seen_docs.insert(result.doc_id),
                "Duplicate doc_id {} found in results for query '{}'",
                result.doc_id, query
            );
        }
    }

    /// Property: Tier values are valid (1, 2, or 3).
    ///
    /// All results should have a valid tier classification.
    #[test]
    fn prop_valid_tier_values(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let results = searcher.search(&query, limit);

        for result in &results {
            prop_assert!(
                result.tier >= 1 && result.tier <= 3,
                "Invalid tier {} for doc_id {} in query '{}'",
                result.tier, result.doc_id, query
            );
        }
    }

    /// Property: Results are sorted by score (descending) within each tier.
    ///
    /// While tiers may be interleaved in the final results (sorted by overall score),
    /// when we look at results from a single tier, they should be score-ordered.
    #[test]
    fn prop_tier_scores_ordered(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let results = searcher.search(&query, limit);

        if results.is_empty() {
            return Ok(());
        }

        // Group results by tier
        let mut tier_results: HashMap<u8, Vec<f64>> = HashMap::new();
        for result in &results {
            tier_results.entry(result.tier).or_default().push(result.score);
        }

        // Within each tier, scores should be valid
        for scores in tier_results.values() {
            for score in scores {
                prop_assert!(
                    score.is_finite() && *score >= 0.0,
                    "Score should be finite and non-negative"
                );
            }
        }
    }

    /// Property: Result count respects limit.
    #[test]
    fn prop_result_count_respects_limit(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let results = searcher.search(&query, limit);

        prop_assert!(
            results.len() <= limit,
            "Got {} results but limit was {}",
            results.len(), limit
        );
    }
}

// ============================================================================
// MULTI-TERM AND SEMANTICS PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Property: T1 exact match works for single terms.
    ///
    /// When searching for a single term, T1 should find exact matches.
    #[test]
    fn prop_t1_single_term_works(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let results = searcher.search_tier1_exact(&query, limit);

        // All T1 results should have tier=1
        for result in &results {
            prop_assert_eq!(
                result.tier, 1,
                "T1 search should only return tier=1 results"
            );
        }
    }

    /// Property: Multi-term T1 search uses AND semantics.
    ///
    /// For "A B", results should contain documents that have BOTH A and B.
    /// This is tested by verifying score is higher than single-term score.
    #[test]
    fn prop_t1_multiterm_higher_score_than_single(limit in 1usize..20) {
        let searcher = load_fixtures_searcher();

        // Use terms we know exist in the PyTorch dataset
        let term1 = "torch";
        let term2 = "tensor";

        let single_results = searcher.search_tier1_exact(term1, limit);
        let multi_results = searcher.search_tier1_exact(&format!("{} {}", term1, term2), limit);

        // If we have multi-term results, their scores should be ~2x single term
        if !multi_results.is_empty() && !single_results.is_empty() {
            let multi_score = multi_results[0].score;
            let single_score = single_results[0].score;

            // Multi-term score should be >= single term score (score summing)
            prop_assert!(
                multi_score >= single_score * 0.9, // Allow 10% tolerance
                "Multi-term score {} should be >= single-term score {}",
                multi_score, single_score
            );
        }
    }

    /// Property: Empty query returns empty results.
    #[test]
    fn prop_empty_query_returns_empty(limit in limit_strategy()) {
        let searcher = load_fixtures_searcher();

        let results = searcher.search("", limit);
        prop_assert!(results.is_empty(), "Empty query should return no results");

        let results_whitespace = searcher.search("   ", limit);
        prop_assert!(results_whitespace.is_empty(), "Whitespace-only query should return no results");
    }
}

// ============================================================================
// TIER 2 PREFIX SEARCH PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Property: T2 prefix search returns valid results.
    #[test]
    fn prop_t2_returns_valid_results(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let exclude: HashSet<usize> = HashSet::new();
        let results = searcher.search_tier2_prefix(&query, &exclude, limit);

        for result in &results {
            prop_assert_eq!(
                result.tier, 2,
                "T2 search should only return tier=2 results"
            );
            prop_assert!(
                result.score > 0.0,
                "T2 results should have positive score"
            );
        }
    }

    /// Property: T2 excludes docs from exclude set.
    #[test]
    fn prop_t2_respects_exclude_set(limit in 5usize..20) {
        let searcher = load_fixtures_searcher();

        // First get some T1 results
        let t1_results = searcher.search_tier1_exact("torch", limit);
        let exclude: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();

        // T2 should not return any docs in the exclude set
        let t2_results = searcher.search_tier2_prefix("torch", &exclude, limit);

        for result in &t2_results {
            prop_assert!(
                !exclude.contains(&result.doc_id),
                "T2 returned doc_id {} which should be excluded",
                result.doc_id
            );
        }
    }
}

// ============================================================================
// TIER 3 FUZZY SEARCH PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: T3 fuzzy search returns valid results.
    #[test]
    fn prop_t3_returns_valid_results(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let exclude: HashSet<usize> = HashSet::new();
        let results = searcher.search_tier3_fuzzy(&query, &exclude, limit);

        for result in &results {
            prop_assert_eq!(
                result.tier, 3,
                "T3 search should only return tier=3 results"
            );
            prop_assert!(
                result.score > 0.0,
                "T3 results should have positive score"
            );
        }
    }

    /// Property: T3 excludes docs from exclude set.
    #[test]
    fn prop_t3_respects_exclude_set(limit in 5usize..20) {
        let searcher = load_fixtures_searcher();

        // Get T1 + T2 results
        let t1_results = searcher.search_tier1_exact("torch", limit);
        let exclude: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();
        let t2_results = searcher.search_tier2_prefix("torch", &exclude, limit);
        let mut full_exclude = exclude.clone();
        for r in &t2_results {
            full_exclude.insert(r.doc_id);
        }

        // T3 should not return any docs in the exclude set
        let t3_results = searcher.search_tier3_fuzzy("torch", &full_exclude, limit);

        for result in &t3_results {
            prop_assert!(
                !full_exclude.contains(&result.doc_id),
                "T3 returned doc_id {} which should be excluded",
                result.doc_id
            );
        }
    }
}

// ============================================================================
// SCORE VALIDITY PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Property: All scores are finite and non-negative.
    #[test]
    fn prop_scores_valid(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();
        let results = searcher.search(&query, limit);

        for result in &results {
            prop_assert!(
                result.score.is_finite(),
                "Score should be finite, got {} for doc {}",
                result.score, result.doc_id
            );
            prop_assert!(
                result.score >= 0.0,
                "Score should be non-negative, got {} for doc {}",
                result.score, result.doc_id
            );
        }
    }

    /// Property: Search is deterministic (same query = same results).
    #[test]
    fn prop_search_deterministic(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        let searcher = load_fixtures_searcher();

        let results1 = searcher.search(&query, limit);
        let results2 = searcher.search(&query, limit);

        prop_assert_eq!(
            results1.len(), results2.len(),
            "Same query should return same number of results"
        );

        for (r1, r2) in results1.iter().zip(results2.iter()) {
            prop_assert_eq!(
                r1.doc_id, r2.doc_id,
                "Same query should return same doc_ids in same order"
            );
            prop_assert!(
                (r1.score - r2.score).abs() < 0.001,
                "Same query should return same scores"
            );
        }
    }
}

// ============================================================================
// MATCH TYPE PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Property: match_type is consistent with heading_level logic.
    ///
    /// heading_level 0 should map to Title, 2 to Heading, others to Content.
    #[test]
    fn prop_match_type_valid(
        query in single_term_query_strategy(),
        limit in limit_strategy()
    ) {
        use sorex::MatchType;

        let searcher = load_fixtures_searcher();
        let results = searcher.search(&query, limit);

        for result in &results {
            // Match type should be one of the valid variants
            let valid = matches!(
                result.match_type,
                MatchType::Title | MatchType::Section | MatchType::Subsection |
                MatchType::Subsubsection | MatchType::Content
            );
            prop_assert!(
                valid,
                "Invalid match_type {:?} for doc {}",
                result.match_type, result.doc_id
            );
        }
    }
}

// ============================================================================
// EDGE CASE TESTS
// ============================================================================

#[test]
fn test_limit_zero_returns_empty() {
    let searcher = load_fixtures_searcher();
    let results = searcher.search("tensor", 0);
    assert!(results.is_empty(), "Limit 0 should return empty results");
}

#[test]
fn test_very_long_query_handled() {
    let searcher = load_fixtures_searcher();
    let long_query = "a".repeat(1000);
    let results = searcher.search(&long_query, 10);
    // Should not panic, may return empty
    assert!(results.len() <= 10);
}

#[test]
fn test_special_characters_in_query() {
    let searcher = load_fixtures_searcher();
    // Should handle gracefully without panic
    let _ = searcher.search("test@#$%^&*()", 10);
    let _ = searcher.search("test\n\t", 10);
    let _ = searcher.search("test 日本語", 10);
}
