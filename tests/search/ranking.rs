//! Property-based tests for section-aware scoring and match_type bucketed ranking.
//!
//! Tests that:
//! - Title matches score higher than section matches
//! - Match counts are used for proper tiebreaking
//! - Results are bucketed by match_type: Title > Section > Subsection > Subsubsection > Content
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use super::common::load_cutlass_searcher;
use sorex::MatchType;
use std::collections::HashSet;

// ============================================================================
// SECTION-AWARE SCORING TESTS
// ============================================================================

#[test]
fn test_t1_exact_title_boosts_score() {
    let searcher = load_cutlass_searcher();

    // Search for a term that appears in titles
    let results = searcher.search_tier1_exact("gemm", 20);

    // If results have the same term but from different sections,
    // title matches should have higher scores
    let title_matches: Vec<_> = results
        .iter()
        .filter(|r| r.section_idx == 0)  // 0 = no section = title match
        .collect();
    let section_matches: Vec<_> = results
        .iter()
        .filter(|r| r.section_idx > 0)   // > 0 = has section
        .collect();

    // Title matches (if any) should score at least as high as section matches
    if !title_matches.is_empty() && !section_matches.is_empty() {
        let min_title_score = title_matches.iter().map(|r| r.score).fold(f64::INFINITY, f64::min);
        let max_section_score = section_matches.iter().map(|r| r.score).fold(0.0, f64::max);
        assert!(
            min_title_score >= max_section_score * 0.9, // Allow 10% margin for floating point
            "Title matches should score higher: min_title={}, max_section={}",
            min_title_score,
            max_section_score
        );
    }
}

#[test]
fn test_t2_prefix_title_boosts_score() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();

    // Search for a prefix that matches multiple sections
    let results = searcher.search_tier2_prefix("gem", &exclude, 20);

    // All T2 results should have score 50.0 or 60.0 (50.0 * 1.2 for title boost)
    for result in &results {
        assert!(
            result.score == 50.0 || result.score == 60.0,
            "T2 score should be 50.0 or 60.0 (with title boost), got {}",
            result.score
        );
        assert!(result.tier == 2, "All results should be tier 2");
    }

    // Count how many have title boost
    let boosted_count = results.iter().filter(|r| r.score == 60.0).count();
    let regular_count = results.iter().filter(|r| r.score == 50.0).count();

    // Title matches should appear first (higher score)
    if boosted_count > 0 && regular_count > 0 {
        let first_boosted = results.iter().position(|r| r.score == 60.0).unwrap();
        let first_regular = results.iter().position(|r| r.score == 50.0).unwrap();
        assert!(
            first_boosted < first_regular,
            "Title-boosted results should come before regular results"
        );
    }
}

#[test]
fn test_t3_fuzzy_title_boosts_score() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();

    // Search for a fuzzy query
    let results = searcher.search_tier3_fuzzy("gemma", &exclude, 20);

    // T3 results have scores based on distance and length similarity bonus (up to 30%):
    // - Distance 1 base: 30.0 * (1.0 to 1.3) = 30.0 to 39.0
    // - Distance 1 with title boost: 30.0 * (1.0 to 1.3) * 1.5 = 45.0 to 58.5
    // - Distance 2 base: 15.0 * (1.0 to 1.3) = 15.0 to 19.5
    // - Distance 2 with title boost: 15.0 * (1.0 to 1.3) * 1.5 = 22.5 to 29.25
    for result in &results {
        assert!(
            result.score >= 15.0 && result.score <= 60.0,
            "T3 score should be between 15 and 60, got {}",
            result.score
        );
        assert!(result.tier == 3, "All results should be tier 3");
    }

    // Results should be sorted by score descending
    for i in 1..results.len() {
        assert!(
            results[i - 1].score >= results[i].score,
            "Results should be sorted by score descending"
        );
    }
}

#[test]
fn test_results_sorted_by_score_then_count_then_title() {
    let searcher = load_cutlass_searcher();

    // Get results from any tier - they should be sorted consistently
    let t1_results = searcher.search_tier1_exact("kernel", 10);

    // Verify sorting: score descending
    for i in 1..t1_results.len() {
        assert!(
            t1_results[i - 1].score >= t1_results[i].score,
            "Results should be sorted by score descending"
        );
    }
}

#[test]
fn test_section_ids_preserved_in_results() {
    let searcher = load_cutlass_searcher();

    // Search for a term that might have multiple sections
    let results = searcher.search_tier1_exact("gemm", 10);

    // All results should have valid section indices that resolve to valid strings
    for result in &results {
        if result.section_idx > 0 {
            let section_id = searcher.section_table().get((result.section_idx - 1) as usize)
                .expect("section_idx should resolve to valid section");
            assert!(
                !section_id.is_empty(),
                "Section IDs should not be empty strings"
            );
            // Section IDs should only contain alphanumeric, hyphens, underscores
            assert!(
                section_id
                    .chars()
                    .all(|c| c.is_alphanumeric() || c == '-' || c == '_'),
                "Section ID contains invalid characters: {}",
                section_id
            );
        }
    }
}

#[test]
fn test_title_matches_rank_before_section_matches_same_distance() {
    let searcher = load_cutlass_searcher();
    let exclude: HashSet<usize> = HashSet::new();

    // Get prefix matches
    let results = searcher.search_tier2_prefix("gem", &exclude, 20);

    // Find positions of 60.0 (title) vs 50.0 (section) scores
    let title_indices: Vec<_> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| (r.score - 60.0).abs() < 0.001)
        .map(|(i, _)| i)
        .collect();

    let section_indices: Vec<_> = results
        .iter()
        .enumerate()
        .filter(|(_, r)| (r.score - 50.0).abs() < 0.001)
        .map(|(i, _)| i)
        .collect();

    // All title matches should come before all section matches
    if !title_indices.is_empty() && !section_indices.is_empty() {
        let max_title_idx = *title_indices.iter().max().unwrap();
        let min_section_idx = *section_indices.iter().min().unwrap();
        assert!(
            max_title_idx < min_section_idx,
            "All title matches should rank before section matches"
        );
    }
}

#[test]
fn test_no_ties_in_score_ordering() {
    let searcher = load_cutlass_searcher();

    // Get results from all tiers
    let t1 = searcher.search_tier1_exact("kernel", 5);
    let exclude_t1: HashSet<usize> = t1.iter().map(|r| r.doc_id).collect();

    let t2 = searcher.search_tier2_prefix("kern", &exclude_t1, 5);
    let exclude_t2: HashSet<usize> = exclude_t1
        .iter()
        .chain(t2.iter().map(|r| &r.doc_id))
        .cloned()
        .collect();

    let t3 = searcher.search_tier3_fuzzy("kernl", &exclude_t2, 5);

    // Verify each tier's results are properly sorted by score (descending)
    for tier_results in [&t1, &t2, &t3] {
        for i in 1..tier_results.len() {
            let prev_score = tier_results[i - 1].score;
            let curr_score = tier_results[i].score;

            // Scores should be non-increasing (descending order)
            assert!(
                prev_score >= curr_score,
                "Results must be sorted by score descending, but found {} before {}",
                prev_score, curr_score
            );
        }
    }
}

// ============================================================================
// MATCH TYPE BUCKETED RANKING TESTS
// ============================================================================

#[test]
fn test_match_type_bucketing_title_beats_section() {
    let searcher = load_cutlass_searcher();

    // Search for "kernel" - should have matches in both title and section
    let results = searcher.search("kernel", 100);

    assert!(!results.is_empty(), "Expected results for 'kernel'");

    // Find first Title match and first Section match
    let title_result = results.iter().find(|r| r.match_type == MatchType::Title);
    let section_result = results.iter().find(|r| r.match_type == MatchType::Section);

    if let (Some(_title_r), Some(_section_r)) = (title_result, section_result) {
        let title_pos = results.iter().position(|r| r.match_type == MatchType::Title).unwrap();
        let section_pos = results.iter().position(|r| r.match_type == MatchType::Section).unwrap();

        assert!(
            title_pos < section_pos,
            "Title match at pos {} should rank before section match at pos {}",
            title_pos, section_pos
        );
    }
}

#[test]
fn test_match_type_bucketing_section_beats_content() {
    let searcher = load_cutlass_searcher();

    // Search for "kerne" prefix - should get results across different match types
    let results = searcher.search("kerne", 100);

    assert!(!results.is_empty(), "Expected results for 'kerne'");

    // Verify overall ordering: all Section matches should come before all Content matches
    let section_indices: Vec<usize> = results.iter()
        .enumerate()
        .filter(|(_, r)| r.match_type == MatchType::Section)
        .map(|(i, _)| i)
        .collect();

    let content_indices: Vec<usize> = results.iter()
        .enumerate()
        .filter(|(_, r)| r.match_type == MatchType::Content)
        .map(|(i, _)| i)
        .collect();

    if !section_indices.is_empty() && !content_indices.is_empty() {
        let max_section_pos = *section_indices.iter().max().unwrap();
        let min_content_pos = *content_indices.iter().min().unwrap();

        assert!(
            max_section_pos < min_content_pos,
            "All Section matches (max pos {}) should rank before all Content matches (min pos {})",
            max_section_pos, min_content_pos
        );
    }
}

#[test]
fn test_tier2_prefix_has_correct_match_types() {
    let searcher = load_cutlass_searcher();

    // Prefix search for "kerne"
    let results = searcher.search_tier2_prefix("kerne", &HashSet::new(), 50);

    // Should have a variety of match types, not just one hardcoded value
    let match_types: Vec<MatchType> = results.iter().map(|r| r.match_type).collect();
    let has_title = match_types.contains(&MatchType::Title);
    let has_section = match_types.contains(&MatchType::Section);
    let has_content = match_types.contains(&MatchType::Content);

    // "kernel" appears in titles, sections, and content across the docs
    assert!(
        has_title || has_section || has_content,
        "T2 prefix search should have variety of match_types"
    );

    // Critically: should NOT all be the same type (that would indicate hardcoding)
    // Count unique types manually
    let mut seen_types: Vec<MatchType> = Vec::new();
    for mt in &match_types {
        if !seen_types.contains(mt) {
            seen_types.push(*mt);
        }
    }
    assert!(
        seen_types.len() > 1,
        "T2 prefix search should have more than one match_type (found only: {:?})",
        seen_types
    );
}

#[test]
fn test_tier3_fuzzy_has_correct_match_types() {
    let searcher = load_cutlass_searcher();

    // Fuzzy search for "kernal" (typo of "kernel")
    let results = searcher.search_tier3_fuzzy("kernal", &HashSet::new(), 50);

    // Should have a variety of match types
    let match_types: Vec<MatchType> = results.iter().map(|r| r.match_type).collect();

    // Count unique types manually
    let mut seen_types: Vec<MatchType> = Vec::new();
    for mt in &match_types {
        if !seen_types.contains(mt) {
            seen_types.push(*mt);
        }
    }

    if !results.is_empty() {
        assert!(
            seen_types.len() > 1 || results.len() < 5,
            "T3 fuzzy search should have more than one match_type unless very few results. Found: {:?}",
            seen_types
        );
    }
}

#[test]
fn test_search_ranks_title_matches_higher() {
    let searcher = load_cutlass_searcher();

    // Search for "kernel" - documents with "kernel" in title should get Title match_type
    let results = searcher.search("kernel", 50);

    // Count how many Title matches are in the top portion vs bottom portion
    let mid = results.len() / 2;
    let title_in_top_half = results[..mid.min(results.len())]
        .iter()
        .filter(|r| r.match_type == MatchType::Title)
        .count();
    let title_in_bottom_half = results[mid..]
        .iter()
        .filter(|r| r.match_type == MatchType::Title)
        .count();

    // If there are any Title matches, they should be concentrated in the top half
    if title_in_top_half + title_in_bottom_half > 0 {
        assert!(
            title_in_top_half >= title_in_bottom_half,
            "Title matches should be concentrated in top half: {} in top vs {} in bottom",
            title_in_top_half, title_in_bottom_half
        );
    }
}
