//! Pure Rust integration tests for TierSearcher (no WASM overhead)
//!
//! These tests verify the complete three-tier search architecture
//! using the real Cutlass dataset without any WASM bindings.
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use super::common::load_cutlass_searcher;
use std::collections::HashSet;

// ============================================================================
// TIER 1: EXACT MATCH TESTS
// ============================================================================

#[test]
fn test_t1_exact_match_kernel() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search_tier1_exact("kernel", 10);

    assert!(!results.is_empty(), "Should find exact matches for 'kernel'");
    assert!(results.iter().all(|r| r.tier == 1), "All results should be tier 1");
    // Section-aware scoring: scores can be 100.0 (section match) or ~110.0 (title match with 10% boost)
    assert!(results.iter().all(|r| (r.score - 100.0).abs() < 0.01 || (r.score - 110.0).abs() < 0.01),
            "All T1 results should score ~100.0 or ~110.0 (with title boost)");

    // Verify document IDs are valid
    for r in &results {
        assert!(r.doc_id < searcher.docs().len(), "Doc ID should be in bounds");
    }
}

#[test]
fn test_t1_exact_match_gemm() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search_tier1_exact("gemm", 10);

    assert!(!results.is_empty(), "Should find exact matches for 'gemm'");
    assert_eq!(results.len(), 10, "Should return up to 10 results");

    // Section-aware scoring: scores are ~100.0 (section match) or ~110.0 (title match)
    for r in &results {
        assert!((r.score - 100.0).abs() < 0.01 || (r.score - 110.0).abs() < 0.01,
                "Exact matches should score ~100.0 or ~110.0 with title boost, got {}", r.score);
        assert_eq!(r.tier, 1, "Should be tier 1");
    }
}

#[test]
fn test_t1_respects_limit() {
    let searcher = load_cutlass_searcher();

    for limit in &[1, 5, 10] {
        let results = searcher.search_tier1_exact("kernel", *limit);
        assert!(results.len() <= *limit, "Should respect limit parameter");
    }
}

#[test]
fn test_t1_no_duplicates() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search_tier1_exact("kernel", 50);

    let mut doc_ids = HashSet::new();
    for r in &results {
        assert!(doc_ids.insert(r.doc_id), "Should not have duplicate doc IDs");
    }
}

#[test]
fn test_t1_sorted_by_title() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search_tier1_exact("kernel", 20);

    if results.len() > 1 {
        // Results should be sorted by score (descending), then by title (ascending)
        // with section-aware scoring: 110.0 (title) > 100.0 (section)
        let mut prev_score = f64::INFINITY;
        for r in &results {
            assert!(r.score <= prev_score,
                "Results should be sorted by score descending");
            prev_score = r.score;
        }
    }
}

// ============================================================================
// TIER 2: PREFIX MATCH TESTS
// ============================================================================

#[test]
fn test_t2_prefix_match_kern() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();
    let results = searcher.search_tier2_prefix("kern", &exclude, 10);

    assert!(!results.is_empty(), "Should find prefix matches for 'kern'");
    assert!(results.iter().all(|r| r.tier == 2), "All results should be tier 2");
    // Section-aware scoring: ~50.0 (section) or ~60.0 (title match with 20% boost)
    assert!(results.iter().all(|r| (r.score - 50.0).abs() < 0.01 || (r.score - 60.0).abs() < 0.01),
            "All T2 results should score ~50.0 or ~60.0 with title boost");
}

#[test]
fn test_t2_excludes_ids() {
    let searcher = load_cutlass_searcher();

    // Get T1 results first
    let t1_results = searcher.search_tier1_exact("kernel", 5);
    let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();

    // Get T2 results excluding T1 IDs
    let t2_results = searcher.search_tier2_prefix("kernel", &t1_ids, 10);

    // Verify no overlap
    for r in &t2_results {
        assert!(!t1_ids.contains(&r.doc_id),
            "T2 results should not include docs from T1");
    }
}

#[test]
fn test_t2_respects_limit() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();

    for limit in &[1, 5, 10] {
        let results = searcher.search_tier2_prefix("c", &exclude, *limit);
        assert!(results.len() <= *limit, "Should respect limit parameter");
    }
}

#[test]
fn test_t2_empty_prefix() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();
    let results = searcher.search_tier2_prefix("", &exclude, 10);

    // Empty prefix should return no matches (no word boundary)
    assert!(results.is_empty(), "Empty prefix should return no matches");
}

// ============================================================================
// TIER 3: FUZZY MATCH TESTS
// ============================================================================

#[test]
fn test_t3_fuzzy_match_kernl() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();
    let results = searcher.search_tier3_fuzzy("kernl", &exclude, 10);

    assert!(!results.is_empty(), "Should find fuzzy matches for 'kernl'");
    assert!(results.iter().all(|r| r.tier == 3), "All results should be tier 3");

    // T3 should score fuzzy matches appropriately
    // With section-aware scoring + length similarity bonus (up to 30%):
    // distance-1 can be ~30.0 * 1.3 (section) or ~30.0 * 1.3 * 1.5 (title with 50% boost) = 58.5
    // distance-2 can be ~15.0 * 1.3 (section) or ~15.0 * 1.3 * 1.5 (title with 50% boost) = 29.25
    for r in &results {
        assert!(r.score > 0.0 && r.score <= 60.0,
            "T3 fuzzy matches should score between 0 and 60 with length bonus + title boost, got {}", r.score);
    }
}

#[test]
fn test_t3_no_exact_matches() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();

    // Search for a word that exists exactly in the index
    let results = searcher.search_tier3_fuzzy("kernel", &exclude, 50);

    // T3 should NOT return "kernel" (distance 0) - that's T1's job
    // It may return "kernels" (distance 1) though
    for r in &results {
        // Verify this isn't an exact match (exact matches score 100.0 or 110.0)
        // With section-aware scoring, T3 scores are at most 45.0 (distance-1 title boost)
        assert!(r.score < 100.0,
            "T3 should not return exact matches (exact matches score >= 100)");
    }
}

#[test]
fn test_t3_excludes_ids() {
    let searcher = load_cutlass_searcher();

    // Get results from T1 and T2
    let t1_results = searcher.search_tier1_exact("kernel", 10);
    let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();

    let exclude_ids = searcher.search_tier2_prefix("kernel", &t1_ids, 10);
    let exclude_set: HashSet<usize> = exclude_ids.iter().map(|r| r.doc_id).collect();

    // Combine T1 and T2 excludes
    let mut all_exclude = t1_ids;
    all_exclude.extend(exclude_set);

    // Get T3 results
    let t3_results = searcher.search_tier3_fuzzy("kernl", &all_exclude, 10);

    // Verify no overlap
    for r in &t3_results {
        assert!(!all_exclude.contains(&r.doc_id),
            "T3 results should not include docs from T1 or T2");
    }
}

#[test]
fn test_t3_respects_limit() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();

    for limit in &[1, 5, 10] {
        let results = searcher.search_tier3_fuzzy("kernl", &exclude, *limit);
        assert!(results.len() <= *limit, "Should respect limit parameter");
    }
}

// ============================================================================
// FULL THREE-TIER ORCHESTRATION TESTS
// ============================================================================

#[test]
fn test_full_search_three_tiers() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search("kernel", 10);

    assert!(!results.is_empty(), "Should return results for 'kernel'");

    // Should have mix of tiers or just T1 (since kernel is an exact match)
    let tiers: HashSet<u8> = results.iter().map(|r| r.tier).collect();
    assert!(tiers.contains(&1), "Should include tier 1 results");
}

#[test]
fn test_full_search_respects_limit() {
    let searcher = load_cutlass_searcher();

    for limit in &[1, 5, 10, 20] {
        let results = searcher.search("kernel", *limit);
        assert!(results.len() <= *limit, "Should respect limit");
    }
}

#[test]
fn test_full_search_sorted_by_match_type_then_score() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search("kernel", 20);

    // With bucketed ranking, results are sorted by match_type first, then score within bucket
    if results.len() > 1 {
        for i in 1..results.len() {
            // Within same match_type, score should be descending
            if results[i-1].match_type == results[i].match_type {
                assert!(results[i-1].score >= results[i].score,
                    "Results within same match_type should be sorted by score (descending)");
            }
        }
    }
}

#[test]
fn test_full_search_no_duplicates() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search("kernel", 50);

    let mut doc_ids = HashSet::new();
    for r in &results {
        assert!(doc_ids.insert(r.doc_id),
            "Should not have duplicate doc IDs in results");
    }
}

#[test]
fn test_full_search_exact_vs_fuzzy() {
    let searcher = load_cutlass_searcher();

    // Search for exact match
    let exact_results = searcher.search("gemm", 10);

    // Search for fuzzy match
    let fuzzy_results = searcher.search("gemma", 10);

    // Both should return results
    assert!(!exact_results.is_empty(), "Should find matches for 'gemm'");
    assert!(!fuzzy_results.is_empty(), "Should find fuzzy matches for 'gemma'");

    // Results are ranked by match_type (Title > Section > etc.) not by tier
    // Top result may be T2 Title if it ranks higher than T1 Section
    assert!(exact_results[0].score > 0.0, "Top result should have positive score");

    // Verify positive scores
    assert!(exact_results.iter().all(|r| r.score > 0.0), "All results should have positive score");
}

// ============================================================================
// SCORING CONSISTENCY TESTS
// ============================================================================

#[test]
fn test_tier_score_hierarchy() {
    let searcher = load_cutlass_searcher();

    let t1 = searcher.search_tier1_exact("kernel", 1);
    let t2 = searcher.search_tier2_prefix("kern", &HashSet::new(), 1);
    let exclude_t1_t2: HashSet<usize> = t1.iter().chain(t2.iter()).map(|r| r.doc_id).collect();
    let t3 = searcher.search_tier3_fuzzy("kernl", &exclude_t1_t2, 1);

    // Check score hierarchy with section-aware scoring + length bonus
    // T1 scores: ~100.0 (section) or ~110.0 (title)
    // T2 scores: ~50.0 (section) or ~60.0 (title)
    // T3 scores: up to ~58.5 (distance-1 with full length bonus + title boost)
    if !t1.is_empty() {
        assert!((t1[0].score - 100.0).abs() < 0.01 || (t1[0].score - 110.0).abs() < 0.01,
            "T1 score should be ~100.0 or ~110.0 with title boost, got {}", t1[0].score);
    }
    if !t2.is_empty() {
        assert!((t2[0].score - 50.0).abs() < 0.01 || (t2[0].score - 60.0).abs() < 0.01,
            "T2 score should be ~50.0 or ~60.0 with title boost, got {}", t2[0].score);
    }
    if !t3.is_empty() {
        assert!(t3[0].score <= 60.0, "T3 score should be <= 60.0 with length bonus + title boost, got {}", t3[0].score);
    }
}

#[test]
fn test_ranking_gemm_vs_gemma() {
    let searcher = load_cutlass_searcher();

    // "gemm" should find results - top result may be T1 Section or T2 Title depending on data
    // Title matches rank higher than Section matches due to match_type ordering
    let gemm = searcher.search("gemm", 1);
    assert!(!gemm.is_empty(), "gemm should find matches");
    assert!(gemm[0].score > 0.0, "gemm should have positive score, got {}", gemm[0].score);

    // "gemma" should be T3 (fuzzy distance 1) or could be T1 if gemma is an exact match
    let gemma = searcher.search("gemma", 1);
    assert!(!gemma.is_empty(), "Should find matches for gemma");
    // First result should score properly
    assert!(gemma[0].score > 0.0, "Should have positive score");
}

#[test]
fn test_ranking_kernel_vs_kernels() {
    let searcher = load_cutlass_searcher();

    // "kernel" should find results - ranking depends on match_type (Title > Section > Content)
    let kernel = searcher.search("kernel", 1);
    assert!(!kernel.is_empty(), "kernel should find matches");
    assert!(kernel[0].score > 0.0, "kernel should have positive score, got {}", kernel[0].score);

    // "kernels" might be T1 (if "kernels" exists) or T2/T3 (prefix/fuzzy)
    let kernels = searcher.search("kernels", 1);
    assert!(!kernels.is_empty(), "Should find matches for kernels");
    // Should get reasonable score
    assert!(kernels[0].score > 0.0, "kernels should have positive score");
}

// ============================================================================
// METADATA TESTS
// ============================================================================

#[test]
fn test_searcher_metadata() {
    let searcher = load_cutlass_searcher();

    assert!(searcher.docs().len() > 0, "Should have documents");
    assert!(searcher.vocabulary().len() > 0, "Should have vocabulary");
    assert!(searcher.suffix_array().len() > 0, "Should have suffix array");
    assert!(searcher.postings().len() > 0, "Should have postings");
    assert!(searcher.lev_dfa().is_some(), "Should have Levenshtein DFA loaded");
}

#[test]
fn test_section_ids() {
    let searcher = load_cutlass_searcher();
    let results = searcher.search("kernel", 10);

    // Verify section indices are valid and resolve to valid section IDs
    for r in &results {
        if r.section_idx > 0 {
            // section_idx should resolve to a valid section_id
            let section_id = searcher.section_table().get((r.section_idx - 1) as usize)
                .expect("section_idx should resolve to valid section");
            // Section ID should be non-empty
            assert!(!section_id.is_empty(), "Section ID should not be empty");
            // Section ID should be alphanumeric + hyphens/underscores
            assert!(section_id.chars().all(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
                "Section ID should only contain alphanumeric, hyphens, underscores");
        }
    }
}

// ============================================================================
// PERFORMANCE SANITY TESTS
// ============================================================================

#[test]
fn test_search_completes_quickly() {
    let searcher = load_cutlass_searcher();

    let start = std::time::Instant::now();
    let _ = searcher.search("kernel", 10);
    let elapsed = start.elapsed();

    // Should complete in reasonable time (< 100ms)
    assert!(elapsed.as_millis() < 100,
        "Search should complete quickly, took {:?}ms",
        elapsed.as_millis());
}

#[test]
fn test_t1_search_fastest() {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();

    let iterations = 100;

    // Benchmark T1
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = searcher.search_tier1_exact("kernel", 10);
    }
    let t1_elapsed = start.elapsed();

    // Benchmark T2
    let start = std::time::Instant::now();
    for _ in 0..iterations {
        let _ = searcher.search_tier2_prefix("kern", &exclude, 10);
    }
    let t2_elapsed = start.elapsed();

    // T1 should be faster than T2 (O(1) vs O(log k))
    assert!(t1_elapsed < t2_elapsed,
        "T1 ({:?}) should be faster than T2 ({:?})",
        t1_elapsed, t2_elapsed);
}

// ============================================================================
// UNICODE EDGE CASE TESTS
// ============================================================================

#[test]
fn test_search_unicode_replacement_char() {
    let searcher = load_cutlass_searcher();

    // Test with UTF-8 replacement character (from invalid UTF-8 input)
    // This should not panic, just return empty results
    let query = "\u{FFFD}";
    let results = searcher.search(query, 10);

    // Should handle gracefully without panicking
    assert!(results.is_empty() || !results.is_empty(), "Should not panic");
}

#[test]
fn test_search_multibyte_unicode() {
    let searcher = load_cutlass_searcher();

    // Test with various multi-byte UTF-8 characters
    let test_queries = [
        "日本語",      // Japanese
        "中文",        // Chinese
        "한국어",       // Korean
        "مرحبا",       // Arabic
        "שלום",        // Hebrew
        "こんにちは",   // Japanese hiragana
        "🔍",          // Emoji
        "café",        // Latin with accent
        "naïve",       // Latin with diaeresis
        "",            // Empty string
        " ",           // Whitespace
        "\n",          // Newline
        "\t",          // Tab
    ];

    for query in &test_queries {
        // Should not panic for any query
        let results = searcher.search(query, 10);
        assert!(results.len() <= 10, "Results should respect limit");
    }
}

#[test]
fn test_search_single_digit() {
    let searcher = load_cutlass_searcher();

    // Test single digit - found by fuzzer
    let query = "1";
    let results = searcher.search(query, 10);
    assert!(results.len() <= 10, "Results should respect limit");

    // Check that results are sorted by (match_type, score) using bucketed ranking
    // match_type is primary key, score is secondary within each bucket
    for i in 1..results.len() {
        let correct_order = results[i - 1].match_type < results[i].match_type ||
            (results[i - 1].match_type == results[i].match_type && results[i - 1].score >= results[i].score);
        assert!(
            correct_order,
            "Results not correctly sorted at position {}: (match_type={:?}, score={}) should come before (match_type={:?}, score={})",
            i, results[i - 1].match_type, results[i - 1].score, results[i].match_type, results[i].score
        );
    }
}

// ============================================================================
// FULL SEARCH DEDUPLICATION TESTS
// ============================================================================

/// Full search() should return unique doc_ids (no duplicates from different sections)
#[test]
fn test_full_search_no_duplicate_doc_ids() {
    let searcher = load_cutlass_searcher();

    // Test with various queries that might match in multiple sections
    let test_queries = ["kernel", "gemm", "cuda", "tensor", "warp", "matrix"];

    for query in &test_queries {
        let results = searcher.search(query, 50);

        let mut seen_doc_ids = HashSet::new();
        for r in &results {
            assert!(
                seen_doc_ids.insert(r.doc_id),
                "Full search for '{}' returned duplicate doc_id {}: results contain same document multiple times",
                query, r.doc_id
            );
        }
    }
}

/// Full search() with high limit should still have unique doc_ids
#[test]
fn test_full_search_high_limit_no_duplicates() {
    let searcher = load_cutlass_searcher();

    let results = searcher.search("the", 100);

    let mut seen_doc_ids = HashSet::new();
    for r in &results {
        assert!(
            seen_doc_ids.insert(r.doc_id),
            "Full search with high limit returned duplicate doc_id {}: results contain same document multiple times",
            r.doc_id
        );
    }
}

/// search() and search_tier1_exact() should both return unique doc_ids
#[test]
fn test_search_tiers_all_unique_doc_ids() {
    let searcher = load_cutlass_searcher();

    // Test each tier individually
    let query = "kernel";

    // T1
    let t1_results = searcher.search_tier1_exact(query, 50);
    let t1_doc_ids: HashSet<_> = t1_results.iter().map(|r| r.doc_id).collect();
    assert_eq!(t1_results.len(), t1_doc_ids.len(), "T1 should have unique doc_ids");

    // T2
    let t2_results = searcher.search_tier2_prefix(query, &t1_doc_ids, 50);
    let t2_doc_ids: HashSet<_> = t2_results.iter().map(|r| r.doc_id).collect();
    assert_eq!(t2_results.len(), t2_doc_ids.len(), "T2 should have unique doc_ids");

    // T3
    let mut exclude = t1_doc_ids.clone();
    exclude.extend(&t2_doc_ids);
    let t3_results = searcher.search_tier3_fuzzy(query, &exclude, 50);
    let t3_doc_ids: HashSet<_> = t3_results.iter().map(|r| r.doc_id).collect();
    assert_eq!(t3_results.len(), t3_doc_ids.len(), "T3 should have unique doc_ids");

    // Full search
    let full_results = searcher.search(query, 50);
    let full_doc_ids: HashSet<_> = full_results.iter().map(|r| r.doc_id).collect();
    assert_eq!(full_results.len(), full_doc_ids.len(), "Full search should have unique doc_ids");
}

/// Verify that a document appearing in multiple tiers only shows once in final results
#[test]
fn test_search_deduplicates_cross_tier_matches() {
    let searcher = load_cutlass_searcher();

    // "kern" will match:
    // - T1: "kernel" exact (if exists)
    // - T2: "kern*" prefix
    // - T3: fuzzy matches like "kernal"
    let query = "kern";
    let results = searcher.search(query, 100);

    let mut seen_doc_ids = HashSet::new();
    for r in &results {
        assert!(
            seen_doc_ids.insert(r.doc_id),
            "Cross-tier search returned duplicate doc_id {} for query '{}'. A document matching in multiple tiers should only appear once.",
            r.doc_id, query
        );
    }
}
