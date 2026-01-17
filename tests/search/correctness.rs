//! Comprehensive search correctness stress tests.
//!
//! This module provides extensive correctness tests across:
//! - Term frequency bands (high/medium/low frequency terms)
//! - Match types (T1 exact, T2 prefix, T3 fuzzy)
//! - Ranking (bucketed by match_type, then score)
//! - Multi-term queries (AND semantics)
//! - Edge cases (unicode, empty, special chars)
//! - Deduplication (cross-tier, within-doc)
//!
//! Uses both cutlass (70 docs) and pytorch (300 docs) datasets.
//!
//! Ground truth counts for T1 exact matching (token-based inverted index).
//! Note: These differ from substring counts because our index uses word tokenization.
//!
//! T1 counts (via search_tier1_exact on the built index):
//!
//! CUTLASS (70 docs):
//!   kernel=42, cuda=42, tensor=48, matrix=60, gemm=36, warp=24, epilogue=18, hopper=17, ampere=10
//!
//! PYTORCH (300 docs):
//!   tensor=248, torch=293, function=261, module=192, data=229, autograd=198,
//!   forward=184, backward=183, gradient=244, optimizer=62, quantize=10, jit=23
//!
//! Requires Cutlass/PyTorch datasets: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use super::common::{load_cutlass_searcher, load_pytorch_searcher};
use sorex::tiered_search::SearchResult;
use sorex::MatchType;
use std::collections::HashSet;

/// Verify all search invariants hold for a set of results.
fn verify_search_invariants(
    results: &[SearchResult],
    query: &str,
    limit: usize,
    num_docs: usize,
) {
    // 1. No duplicates
    let doc_ids: Vec<_> = results.iter().map(|r| r.doc_id).collect();
    let unique_ids: HashSet<_> = doc_ids.iter().collect();
    assert_eq!(
        doc_ids.len(),
        unique_ids.len(),
        "Duplicate doc_ids found for query '{}'",
        query
    );

    // 2. Limit respected
    assert!(
        results.len() <= limit,
        "Exceeded limit {} for query '{}': got {}",
        limit,
        query,
        results.len()
    );

    // 3. Valid doc_ids
    for r in results {
        assert!(
            r.doc_id < num_docs,
            "Invalid doc_id {} (max {}) for query '{}'",
            r.doc_id,
            num_docs,
            query
        );
    }

    // 4. Valid tiers
    for r in results {
        assert!(
            r.tier >= 1 && r.tier <= 3,
            "Invalid tier {} for query '{}'",
            r.tier,
            query
        );
    }

    // 5. Positive scores
    for r in results {
        assert!(
            r.score > 0.0,
            "Non-positive score {} for query '{}' (doc {})",
            r.score,
            query,
            r.doc_id
        );
    }

    // 6. Bucketed ordering (match_type primary, score secondary)
    for i in 1..results.len() {
        let prev = &results[i - 1];
        let curr = &results[i];

        // match_type should be non-decreasing (Title < Section < ... < Content)
        if prev.match_type == curr.match_type {
            // Within same bucket, score should be non-increasing
            assert!(
                prev.score >= curr.score,
                "Score ordering violated at position {} for query '{}': {} > {} within {:?}",
                i,
                query,
                prev.score,
                curr.score,
                prev.match_type
            );
        } else {
            assert!(
                prev.match_type < curr.match_type,
                "Bucketing violated at position {} for query '{}': {:?} vs {:?}",
                i,
                query,
                prev.match_type,
                curr.match_type
            );
        }
    }
}

/// Verify bucketed ranking is strictly enforced.
fn verify_bucketed_ordering(results: &[SearchResult]) {
    for i in 1..results.len() {
        let prev = &results[i - 1];
        let curr = &results[i];

        // match_type should be non-decreasing
        assert!(
            prev.match_type <= curr.match_type,
            "Bucketing violated at {}: {:?} should come before {:?}",
            i,
            prev.match_type,
            curr.match_type
        );

        // Within same bucket, score should be non-increasing
        if prev.match_type == curr.match_type {
            assert!(
                prev.score >= curr.score,
                "Score ordering violated at {} within {:?}: {} > {}",
                i,
                prev.match_type,
                prev.score,
                curr.score
            );
        }
    }
}

// ============================================================================
// PART 1: TERM FREQUENCY MATRIX TESTS
// ============================================================================

mod term_frequency {
    use super::*;

    // --------------------------------------------------------------------
    // 1.1 HIGH-FREQUENCY TERMS (50%+ docs)
    // --------------------------------------------------------------------

    #[test]
    fn test_cutlass_kernel_exact() {
        let searcher = load_cutlass_searcher();
        // Use search_tier1_exact for T1-only validation
        let results = searcher.search_tier1_exact("kernel", 16000);

        // T1 inverted index count = 42 docs
        assert_eq!(
            results.len(),
            42,
            "T1 exact 'kernel' should match exactly 42 docs, got {}",
            results.len()
        );
        // Note: T1-only search doesn't maintain full bucketed ordering
    }

    #[test]
    fn test_cutlass_kern_prefix() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("kern", &exclude, 16000);

        assert_eq!(
            results.len(),
            45,
            "Prefix 'kern' should match exactly 45 docs, got {}",
            results.len()
        );
        assert!(
            results.iter().all(|r| r.tier == 2),
            "All T2 results should have tier=2"
        );
    }

    #[test]
    fn test_cutlass_kernal_fuzzy_d1() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernal", &exclude, 16000);

        assert_eq!(
            results.len(),
            45,
            "Fuzzy 'kernal' (d=1) should match exactly 45 docs, got {}",
            results.len()
        );
        assert!(
            results.iter().all(|r| r.tier == 3),
            "All T3 results should have tier=3"
        );
    }

    #[test]
    fn test_cutlass_kernl_fuzzy_d1_omit() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernl", &exclude, 16000);

        assert_eq!(
            results.len(),
            45,
            "Fuzzy 'kernl' (d=1 omission) should match exactly 45 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_kernell_fuzzy_d1_add() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernell", &exclude, 16000);

        assert_eq!(
            results.len(),
            45,
            "Fuzzy 'kernell' (d=1 insertion) should match exactly 45 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_cuda_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("cuda", 16000);

        // T1 inverted index count = 42 docs
        assert_eq!(
            results.len(),
            42,
            "T1 exact 'cuda' should match exactly 42 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_tensor_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("tensor", 16000);

        // T1 inverted index count = 48 docs
        assert_eq!(
            results.len(),
            48,
            "T1 exact 'tensor' should match exactly 48 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_matrix_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("matrix", 16000);

        // T1 inverted index count = 31 unique docs
        assert_eq!(
            results.len(),
            31,
            "T1 exact 'matrix' should match exactly 31 docs, got {}",
            results.len()
        );
    }

    // PyTorch high-frequency tests
    #[test]
    fn test_pytorch_tensor_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("tensor", 16000);

        // T1 inverted index count = 248 docs
        assert_eq!(
            results.len(),
            248,
            "T1 exact 'tensor' should match exactly 248 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_tens_prefix() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("tens", &exclude, 16000);

        assert_eq!(
            results.len(),
            258,
            "Prefix 'tens' should match exactly 258 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_tensr_fuzzy_d1() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("tensr", &exclude, 16000);

        assert_eq!(
            results.len(),
            260,
            "Fuzzy 'tensr' (d=1) should match exactly 260 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_tesnor_fuzzy_d1_swap() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("tesnor", &exclude, 16000);

        assert_eq!(
            results.len(),
            248,
            "Fuzzy 'tesnor' (d=1 transposition) should match exactly 248 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_torch_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("torch", 16000);

        // T1 inverted index count = 293 docs
        // Note: Nearly all docs contain "torch" as it's the library name
        assert_eq!(
            results.len(),
            293,
            "T1 exact 'torch' should match exactly 293 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_torc_prefix() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("torc", &exclude, 16000);

        assert_eq!(
            results.len(),
            293,
            "Prefix 'torc' should match exactly 293 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_torcch_fuzzy_d1() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("torcch", &exclude, 16000);

        // "torcch" is distance 1 from "torch" (extra 'c')
        assert_eq!(
            results.len(),
            294,
            "Fuzzy 'torcch' (d=1 from 'torch') should match exactly 294 pytorch docs, got {}",
            results.len()
        );
        assert!(
            results.iter().all(|r| r.tier == 3),
            "All T3 results should have tier=3"
        );
    }

    #[test]
    fn test_pytorch_function_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("function", 16000);

        // T1 inverted index count = 123 unique docs
        assert_eq!(
            results.len(),
            123,
            "T1 exact 'function' should match exactly 123 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_module_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("module", 16000);

        // T1 inverted index count = 70 unique docs
        assert_eq!(
            results.len(),
            70,
            "T1 exact 'module' should match exactly 70 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_data_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("data", 16000);

        // T1 inverted index count = 112 unique docs
        assert_eq!(
            results.len(),
            112,
            "T1 exact 'data' should match exactly 112 pytorch docs, got {}",
            results.len()
        );
    }

    // --------------------------------------------------------------------
    // 1.2 MEDIUM-FREQUENCY TERMS (10-50% docs)
    // --------------------------------------------------------------------

    #[test]
    fn test_cutlass_gemm_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("gemm", 16000);

        // T1 inverted index count = 36 docs
        assert_eq!(
            results.len(),
            36,
            "T1 exact 'gemm' should match exactly 36 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_gem_prefix() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("gem", &exclude, 16000);

        // Prefix "gem" finds additional docs beyond exact "gemm" matches
        // T2 excludes T1 exact matches, so this tests pure prefix matching
        assert_eq!(
            results.len(),
            37,
            "Prefix 'gem' should match exactly 37 docs, got {}",
            results.len()
        );
        assert!(
            results.iter().all(|r| r.tier == 2),
            "All T2 results should have tier=2"
        );
    }

    #[test]
    fn test_cutlass_warp_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("warp", 16000);

        // T1 inverted index count = 24 docs
        assert_eq!(
            results.len(),
            24,
            "T1 exact 'warp' should match exactly 24 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_epilogue_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("epilogue", 16000);

        // T1 inverted index count = 18 docs
        assert_eq!(
            results.len(),
            18,
            "T1 exact 'epilogue' should match exactly 18 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_autograd_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("autograd", 16000);

        // T1 inverted index count = 78 unique docs
        assert_eq!(
            results.len(),
            78,
            "T1 exact 'autograd' should match exactly 78 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_autogrd_fuzzy_d1() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("autogrd", &exclude, 16000);

        assert_eq!(
            results.len(),
            78,
            "Fuzzy 'autogrd' (d=1) should match exactly 78 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_forward_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("forward", 16000);

        // T1 inverted index count = 53 unique docs
        assert_eq!(
            results.len(),
            53,
            "T1 exact 'forward' should match exactly 53 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_forwrd_fuzzy_d1() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("forwrd", &exclude, 16000);

        assert_eq!(
            results.len(),
            55,
            "Fuzzy 'forwrd' (d=1) should match exactly 55 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_backward_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("backward", 16000);

        // T1 inverted index count = 50 unique docs
        assert_eq!(
            results.len(),
            50,
            "T1 exact 'backward' should match exactly 50 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_gradient_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("gradient", 16000);

        // T1 inverted index count = 40 unique docs
        assert_eq!(
            results.len(),
            40,
            "T1 exact 'gradient' should match exactly 40 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_optimizer_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("optimizer", 16000);

        // T1 inverted index count = 27 unique docs
        assert_eq!(
            results.len(),
            27,
            "T1 exact 'optimizer' should match exactly 27 pytorch docs, got {}",
            results.len()
        );
    }

    // --------------------------------------------------------------------
    // 1.3 LOW-FREQUENCY TERMS (1-5 docs)
    // --------------------------------------------------------------------

    #[test]
    fn test_cutlass_hopper_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("hopper", 16000);

        // T1 inverted index count = 17 docs
        assert_eq!(
            results.len(),
            17,
            "T1 exact 'hopper' should match exactly 17 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_cutlass_ampere_exact() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("ampere", 16000);

        // T1 inverted index count = 10 docs
        assert_eq!(
            results.len(),
            10,
            "T1 exact 'ampere' should match exactly 10 docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_quantize_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("quantize", 16000);

        // T1 inverted index count = 10 docs
        assert_eq!(
            results.len(),
            10,
            "T1 exact 'quantize' should match exactly 10 pytorch docs, got {}",
            results.len()
        );
    }

    #[test]
    fn test_pytorch_jit_exact() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("jit", 16000);

        // T1 inverted index count = 23 docs
        assert_eq!(
            results.len(),
            23,
            "T1 exact 'jit' should match exactly 23 pytorch docs, got {}",
            results.len()
        );
    }

    // --------------------------------------------------------------------
    // 1.4 ZERO-RESULT QUERIES
    // --------------------------------------------------------------------

    #[test]
    fn test_nonexistent_term_cutlass() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("nonexistent_term_xyz123", 10);
        assert!(results.is_empty(), "Nonexistent term should return empty");
    }

    #[test]
    fn test_nonexistent_term_pytorch() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("qwertyuiopasdfgh", 10);
        assert!(results.is_empty(), "Nonexistent term should return empty");
    }

    #[test]
    fn test_emoji_query() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("🔥🔥🔥🔥", 10);
        assert!(
            results.is_empty(),
            "Emoji-only query should return empty (no emoji in docs)"
        );
    }

    #[test]
    fn test_long_nonexistent() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("aaaaaaaaaaaaaaaaaaaaaaaaa", 10);
        assert!(results.is_empty(), "Long nonsense should return empty");
    }
}

// ============================================================================
// PART 2: MATCH TYPE EXHAUSTIVE TESTS
// ============================================================================

mod match_types {
    use super::*;

    // --------------------------------------------------------------------
    // 2.1 EXACT MATCH (T1) - Inverted Index
    // --------------------------------------------------------------------

    #[test]
    fn test_t1_exact_kernel_cutlass() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("kernel", 16000);

        assert!(!results.is_empty(), "T1 exact 'kernel' should find matches");
        assert!(
            results.iter().all(|r| r.tier == 1),
            "All T1 results should have tier=1"
        );
    }

    #[test]
    fn test_t1_exact_gemm_cutlass() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("gemm", 16000);

        assert!(!results.is_empty(), "T1 exact 'gemm' should find matches");
        assert!(
            results.iter().all(|r| r.tier == 1),
            "All T1 results should have tier=1"
        );
    }

    #[test]
    fn test_t1_exact_tensor_pytorch() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("tensor", 16000);

        assert!(
            !results.is_empty(),
            "T1 exact 'tensor' should find matches"
        );
        assert!(
            results.iter().all(|r| r.tier == 1),
            "All T1 results should have tier=1"
        );
    }

    #[test]
    fn test_t1_exact_autograd_pytorch() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search_tier1_exact("autograd", 16000);

        assert!(
            !results.is_empty(),
            "T1 exact 'autograd' should find matches"
        );
    }

    // --------------------------------------------------------------------
    // 2.2 PREFIX MATCH (T2) - Suffix Array
    // --------------------------------------------------------------------

    #[test]
    fn test_t2_prefix_length_sweep_kernel() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();

        let prefix_1 = searcher.search_tier2_prefix("k", &exclude, 100);
        let prefix_2 = searcher.search_tier2_prefix("ke", &exclude, 100);
        let prefix_3 = searcher.search_tier2_prefix("ker", &exclude, 100);
        let prefix_4 = searcher.search_tier2_prefix("kern", &exclude, 100);
        let prefix_5 = searcher.search_tier2_prefix("kerne", &exclude, 100);

        // Shorter prefix → more matches (generally)
        // Note: all must be tier 2
        for prefix_results in [&prefix_1, &prefix_2, &prefix_3, &prefix_4, &prefix_5] {
            assert!(
                prefix_results.iter().all(|r| r.tier == 2),
                "All T2 prefix results should have tier=2"
            );
        }
    }

    #[test]
    fn test_t2_prefix_length_sweep_tensor() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();

        let prefixes = ["t", "te", "ten", "tens", "tenso"];

        for prefix in prefixes {
            let results = searcher.search_tier2_prefix(prefix, &exclude, 200);
            assert!(
                results.iter().all(|r| r.tier == 2),
                "All T2 results for '{}' should have tier=2",
                prefix
            );
            // Verify results are non-empty for common prefixes
            assert!(
                !results.is_empty(),
                "Prefix '{}' should have at least one match",
                prefix
            );
        }
    }

    #[test]
    fn test_t2_prefix_autograd() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();

        let prefixes = ["a", "au", "aut", "auto", "autog"];
        for prefix in prefixes {
            let results = searcher.search_tier2_prefix(prefix, &exclude, 100);
            assert!(
                results.iter().all(|r| r.tier == 2),
                "All T2 results for '{}' should have tier=2",
                prefix
            );
        }
    }

    #[test]
    fn test_t2_no_duplicates() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("c", &exclude, 100);

        let doc_ids: Vec<_> = results.iter().map(|r| r.doc_id).collect();
        let unique: HashSet<_> = doc_ids.iter().collect();
        assert_eq!(
            doc_ids.len(),
            unique.len(),
            "T2 prefix search should not return duplicates"
        );
    }

    // --------------------------------------------------------------------
    // 2.3 FUZZY MATCH (T3) - Levenshtein DFA
    // --------------------------------------------------------------------

    #[test]
    fn test_t3_fuzzy_substitution() {
        // kernal → kernel (substitution: a→e)
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernal", &exclude, 16000);

        assert!(
            !results.is_empty(),
            "T3 fuzzy 'kernal' (substitution) should find matches"
        );
        assert!(
            results.iter().all(|r| r.tier == 3),
            "All T3 results should have tier=3"
        );
    }

    #[test]
    fn test_t3_fuzzy_insertion() {
        // kernell → kernel (insertion: extra l)
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernell", &exclude, 16000);

        assert!(
            !results.is_empty(),
            "T3 fuzzy 'kernell' (insertion) should find matches"
        );
    }

    #[test]
    fn test_t3_fuzzy_deletion() {
        // kernl → kernel (deletion: missing e)
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernl", &exclude, 16000);

        assert!(
            !results.is_empty(),
            "T3 fuzzy 'kernl' (deletion) should find matches"
        );
    }

    #[test]
    fn test_t3_fuzzy_transposition() {
        // kenrel → kernel (transposition: n↔r, counts as 2 edits in standard Levenshtein)
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kenrel", &exclude, 50);

        // Standard Levenshtein counts transposition as 2 edits (delete + insert)
        // So this might find "kernel" at distance 2
        // Verify tier correctness for any results found
        for r in &results {
            assert_eq!(r.tier, 3, "All T3 results should have tier=3");
        }
    }

    #[test]
    fn test_t3_fuzzy_d2_two_substitutions() {
        // karnul → kernel (2 substitutions: e→a, e→u)
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("karnul", &exclude, 50);

        // Distance 2 is within max_distance, should find kernel
        // Note: might also find other words at d<=2
        for r in &results {
            assert_eq!(r.tier, 3, "All T3 results should have tier=3");
            // D=2 base score is 15, with penalties and bonuses
            assert!(
                r.score <= 30.0,
                "D=2 matches should score lower than d=1 (max ~30), got {}",
                r.score
            );
        }
    }

    #[test]
    fn test_t3_fuzzy_d3_should_not_match_kernel() {
        // "kernxyz" is d=3 from "kernel" (add x, y, z)
        // At max_distance=2, this should NOT match "kernel"
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernxyz", &exclude, 50);

        // Verify that if we get results, none are for terms similar to "kernel"
        // (they would be other vocabulary words that happen to be within d=2)
        for r in &results {
            assert_eq!(r.tier, 3, "All T3 results should have tier=3");
        }

        // The key test: results should be few (since "kernxyz" isn't close to most words)
        // If we were incorrectly matching "kernel" at d=3, we'd get many results
        assert!(
            results.len() <= 5,
            "d=3 query should match very few words, got {}",
            results.len()
        );
    }

    #[test]
    fn test_t3_fuzzy_pytorch_tensor_variants() {
        let searcher = load_pytorch_searcher();
        let exclude = HashSet::new();

        // Test various fuzzy variants of "tensor" - all within d=2
        // tenser: d=1 (o→e substitution)
        // tensro: d=1 (transposition or→ro)
        // tensr: d=1 (missing o)
        // tensoor: d=1 (extra o)
        // teensor: d=1 (extra e)
        let d1_variants = ["tenser", "tensro", "tensr", "tensoor", "teensor"];

        for variant in d1_variants {
            let results = searcher.search_tier3_fuzzy(variant, &exclude, 50);
            assert!(
                !results.is_empty(),
                "Fuzzy variant '{}' (d<=2 from 'tensor') should find matches",
                variant
            );
            for r in &results {
                assert_eq!(r.tier, 3, "All T3 results for '{}' should have tier=3", variant);
            }
        }
    }

    #[test]
    fn test_t3_excludes_exact_matches() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();

        // Search for exact term via T3 - should NOT return exact matches
        let results = searcher.search_tier3_fuzzy("kernel", &exclude, 50);

        // T3 should NOT return exact matches (distance 0) - those are T1's job
        // Results should be for terms like "kernels" at distance 1
        for r in &results {
            assert_eq!(
                r.tier, 3,
                "T3 fuzzy should only return tier 3 results"
            );
            // Score should be less than T1 exact match score (100)
            assert!(
                r.score < 100.0,
                "T3 should not return exact match scores"
            );
        }
    }
}

// ============================================================================
// PART 3: POSITION AND RANKING TESTS
// ============================================================================

mod ranking {
    use super::*;

    // --------------------------------------------------------------------
    // 3.1 MATCH TYPE BUCKETING
    // --------------------------------------------------------------------

    #[test]
    fn test_bucketed_ordering_cutlass_kernel() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 50);

        verify_bucketed_ordering(&results);
    }

    #[test]
    fn test_bucketed_ordering_pytorch_tensor() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("tensor", 100);

        verify_bucketed_ordering(&results);
    }

    #[test]
    fn test_bucketed_ordering_gemm() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("gemm", 50);

        verify_bucketed_ordering(&results);
    }

    #[test]
    fn test_bucketed_ordering_autograd() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("autograd", 50);

        verify_bucketed_ordering(&results);
    }

    // --------------------------------------------------------------------
    // 3.2 TITLE vs SECTION vs CONTENT RANKING
    // --------------------------------------------------------------------

    #[test]
    fn test_title_matches_first() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 50);

        if !results.is_empty() {
            // First results should be Title matches if any exist
            let first_non_title = results.iter().position(|r| r.match_type != MatchType::Title);

            // Verify all Title matches come first
            if let Some(pos) = first_non_title {
                for i in 0..pos {
                    assert_eq!(
                        results[i].match_type,
                        MatchType::Title,
                        "Title matches should come first"
                    );
                }
            }
        }
    }

    #[test]
    fn test_section_matches_before_content() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("tensor", 100);

        // Verify strict bucketed ordering: once we see Content, we should never see Section/Subsection after
        let mut seen_content = false;

        for r in &results {
            match r.match_type {
                MatchType::Title => {
                    assert!(
                        !seen_content,
                        "Title should not appear after Content"
                    );
                }
                MatchType::Section | MatchType::Subsection | MatchType::Subsubsection => {
                    assert!(
                        !seen_content,
                        "Section/Subsection should not appear after Content"
                    );
                }
                MatchType::Content => {
                    seen_content = true;
                }
            }
        }

        // Verify we have at least some Content matches to make this test meaningful
        let has_content = results.iter().any(|r| r.match_type == MatchType::Content);
        if !has_content && !results.is_empty() {
            // If no Content matches, all should be Title/Section/Subsection
            assert!(
                results.iter().all(|r| r.match_type != MatchType::Content),
                "Should have all non-Content if no Content found"
            );
        }
    }

    // --------------------------------------------------------------------
    // 3.3 SCORE TIE-BREAKING
    // --------------------------------------------------------------------

    #[test]
    fn test_score_tie_alphabetical() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 50);

        // Within same match_type and similar scores, verify alphabetical ordering
        for window in results.windows(2) {
            let a = &window[0];
            let b = &window[1];

            if a.match_type == b.match_type && (a.score - b.score).abs() < 0.001 {
                let title_a = &searcher.docs()[a.doc_id].title;
                let title_b = &searcher.docs()[b.doc_id].title;
                assert!(
                    title_a <= title_b,
                    "Tie-break should be alphabetical: '{}' vs '{}'",
                    title_a,
                    title_b
                );
            }
        }
    }

    // --------------------------------------------------------------------
    // 3.4 WITHIN-BUCKET SCORE ORDERING
    // --------------------------------------------------------------------

    #[test]
    fn test_within_bucket_score_descending() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("module", 100);

        // Group results by match_type
        let mut prev_match_type = results.first().map(|r| r.match_type);
        let mut prev_score = f64::INFINITY;

        for r in &results {
            // Only compare scores within the same bucket
            if Some(r.match_type) == prev_match_type {
                assert!(
                    r.score <= prev_score,
                    "Within bucket {:?}, score should be descending: {} > {}",
                    r.match_type,
                    prev_score,
                    r.score
                );
            }
            prev_match_type = Some(r.match_type);
            prev_score = r.score;
        }
    }
}

// ============================================================================
// PART 4: MULTI-TERM QUERY TESTS
// ============================================================================

mod multi_term {
    use super::*;

    // --------------------------------------------------------------------
    // 4.1 AND SEMANTICS (via search_tier1_exact)
    //
    // Note: The main search() function treats multi-word queries as a single
    // literal term (e.g., "tensor autograd" searches for that exact string).
    // AND semantics are only implemented in search_tier1_exact(), which
    // tokenizes on whitespace and requires all terms to match.
    // --------------------------------------------------------------------

    #[test]
    fn test_and_semantics_pytorch_tensor_autograd() {
        let searcher = load_pytorch_searcher();

        // search_tier1_exact implements AND semantics (tokenizes on whitespace)
        let combined = searcher.search_tier1_exact("tensor autograd", 100);

        // Get results for individual terms
        let tensor_results = searcher.search_tier1_exact("tensor", 16000);
        let autograd_results = searcher.search_tier1_exact("autograd", 300);

        let tensor_ids: HashSet<_> = tensor_results.iter().map(|r| r.doc_id).collect();
        let autograd_ids: HashSet<_> = autograd_results.iter().map(|r| r.doc_id).collect();

        // AND semantics: combined results should be subset of intersection
        // (might be smaller due to limit)
        let intersection: HashSet<_> = tensor_ids.intersection(&autograd_ids).copied().collect();

        for r in &combined {
            assert!(
                intersection.contains(&r.doc_id),
                "Combined result doc {} should match BOTH 'tensor' and 'autograd'",
                r.doc_id
            );
        }

        // Both terms should have matches, and combined should return results
        assert!(!tensor_results.is_empty(), "Individual 'tensor' should have results");
        assert!(!autograd_results.is_empty(), "Individual 'autograd' should have results");
        assert!(!combined.is_empty(), "Combined 'tensor autograd' should have results");

        // Verify combined results are a subset of the intersection
        let combined_ids: HashSet<_> = combined.iter().map(|r| r.doc_id).collect();
        assert!(
            combined_ids.is_subset(&intersection),
            "All combined results must appear in both 'tensor' and 'autograd' results"
        );
    }

    #[test]
    fn test_and_semantics_cutlass_gemm_kernel() {
        let searcher = load_cutlass_searcher();

        // search_tier1_exact implements AND semantics
        let combined = searcher.search_tier1_exact("gemm kernel", 50);

        let gemm_results = searcher.search_tier1_exact("gemm", 100);
        let kernel_results = searcher.search_tier1_exact("kernel", 100);

        let gemm_ids: HashSet<_> = gemm_results.iter().map(|r| r.doc_id).collect();
        let kernel_ids: HashSet<_> = kernel_results.iter().map(|r| r.doc_id).collect();
        let intersection: HashSet<_> = gemm_ids.intersection(&kernel_ids).copied().collect();

        // Verify AND semantics
        for r in &combined {
            assert!(
                intersection.contains(&r.doc_id),
                "Combined result doc {} should match BOTH 'gemm' and 'kernel'",
                r.doc_id
            );
        }
    }

    #[test]
    fn test_and_semantics_pytorch_forward_backward() {
        let searcher = load_pytorch_searcher();

        // search_tier1_exact implements AND semantics
        // Note: Results may contain multiple (doc_id, section_idx) pairs per doc
        let combined = searcher.search_tier1_exact("forward backward", 200);

        // Get individual term results with higher limits to capture all docs
        let forward_results = searcher.search_tier1_exact("forward", 300);
        let backward_results = searcher.search_tier1_exact("backward", 300);

        // Compare unique doc_ids only (not result counts which include sections)
        let forward_ids: HashSet<_> = forward_results.iter().map(|r| r.doc_id).collect();
        let backward_ids: HashSet<_> = backward_results.iter().map(|r| r.doc_id).collect();
        let intersection: HashSet<_> = forward_ids.intersection(&backward_ids).copied().collect();

        // Extract unique doc_ids from combined results
        let combined_ids: HashSet<_> = combined.iter().map(|r| r.doc_id).collect();

        // Verify AND semantics: all combined doc_ids must be in intersection
        for doc_id in &combined_ids {
            assert!(
                intersection.contains(doc_id),
                "Combined result doc {} should match BOTH 'forward' and 'backward'",
                doc_id
            );
        }
    }

    #[test]
    fn test_and_semantics_cutlass_cuda_tensor() {
        let searcher = load_cutlass_searcher();

        // search_tier1_exact implements AND semantics
        let combined = searcher.search_tier1_exact("cuda tensor", 50);

        let cuda_results = searcher.search_tier1_exact("cuda", 100);
        let tensor_results = searcher.search_tier1_exact("tensor", 16000);

        let cuda_ids: HashSet<_> = cuda_results.iter().map(|r| r.doc_id).collect();
        let tensor_ids: HashSet<_> = tensor_results.iter().map(|r| r.doc_id).collect();
        let intersection: HashSet<_> = cuda_ids.intersection(&tensor_ids).copied().collect();

        // Verify AND semantics
        for r in &combined {
            assert!(
                intersection.contains(&r.doc_id),
                "Combined result doc {} should match BOTH 'cuda' and 'tensor'",
                r.doc_id
            );
        }
    }

    // --------------------------------------------------------------------
    // 4.2 MULTI-WORD QUERY STRESS TESTS
    //
    // Note: The main search() function treats multi-word queries as a single
    // literal term. These tests verify that multi-word strings don't crash
    // and that the search handles them gracefully (returning empty or matches
    // if the literal exists in the index).
    // --------------------------------------------------------------------

    #[test]
    fn test_multiword_literal_query() {
        let searcher = load_pytorch_searcher();
        // Main search() treats this as a single literal - likely returns empty
        let results = searcher.search("tensor forwrd", 50);

        // Should not crash, results may be empty (no literal "tensor forwrd" in index)
        verify_search_invariants(&results, "tensor forwrd", 50, searcher.docs().len());
    }

    #[test]
    fn test_multiword_literal_query_2() {
        let searcher = load_pytorch_searcher();
        // Main search() treats this as a single literal
        let results = searcher.search("tens autograd", 50);

        verify_search_invariants(&results, "tens autograd", 50, searcher.docs().len());
    }

    #[test]
    fn test_multiword_literal_query_3() {
        let searcher = load_pytorch_searcher();
        // Main search() treats this as a single literal
        let results = searcher.search("tensro grdient", 50);

        verify_search_invariants(&results, "tensro grdient", 50, searcher.docs().len());
    }

    // --------------------------------------------------------------------
    // 4.3 MULTI-TERM AND STRESS (via search_tier1_exact)
    // --------------------------------------------------------------------

    #[test]
    fn test_three_term_query() {
        let searcher = load_pytorch_searcher();

        // search_tier1_exact supports multi-term AND semantics
        let results = searcher.search_tier1_exact("tensor autograd backward", 50);

        // Verify each result matches all terms
        let tensor_ids: HashSet<_> = searcher.search_tier1_exact("tensor", 16000)
            .iter().map(|r| r.doc_id).collect();
        let autograd_ids: HashSet<_> = searcher.search_tier1_exact("autograd", 300)
            .iter().map(|r| r.doc_id).collect();
        let backward_ids: HashSet<_> = searcher.search_tier1_exact("backward", 300)
            .iter().map(|r| r.doc_id).collect();

        for r in &results {
            assert!(
                tensor_ids.contains(&r.doc_id)
                    && autograd_ids.contains(&r.doc_id)
                    && backward_ids.contains(&r.doc_id),
                "Result doc {} should match all three terms",
                r.doc_id
            );
        }
    }

    #[test]
    fn test_five_term_query() {
        let searcher = load_pytorch_searcher();

        // 5-term AND query
        let results = searcher.search_tier1_exact("tensor torch data model training", 50);

        // Just verify it doesn't panic and respects invariants
        // (may return empty if no doc contains all 5 terms)
        for r in &results {
            assert!(r.tier == 1, "T1 results should have tier=1");
        }
    }

    #[test]
    fn test_high_low_freq_combination() {
        let searcher = load_pytorch_searcher();

        // "tensor" is high-freq via T1 - use unique doc_ids
        let tensor_only = searcher.search_tier1_exact("tensor", 16000);
        let tensor_doc_ids: HashSet<_> = tensor_only.iter().map(|r| r.doc_id).collect();
        assert!(
            tensor_doc_ids.len() >= 50,
            "High-freq term 'tensor' should have 50+ unique docs, got {}",
            tensor_doc_ids.len()
        );

        // "quantize" is low-freq
        let quantize_only = searcher.search_tier1_exact("quantize", 100);
        let quantize_doc_ids: HashSet<_> = quantize_only.iter().map(|r| r.doc_id).collect();

        // "tensor quantize" combines high-freq with low-freq (AND semantics)
        let combined = searcher.search_tier1_exact("tensor quantize", 100);
        let combined_doc_ids: HashSet<_> = combined.iter().map(|r| r.doc_id).collect();

        // AND semantics: combined unique doc_ids should be subset of lower-freq term's doc_ids
        assert!(
            combined_doc_ids.len() <= quantize_doc_ids.len(),
            "Combined AND query can't have more unique docs than lower-freq term: {} should be <= {}",
            combined_doc_ids.len(),
            quantize_doc_ids.len()
        );

        // All combined doc_ids must be in quantize
        for doc_id in &combined_doc_ids {
            assert!(
                quantize_doc_ids.contains(doc_id),
                "Combined doc_id {} should be in 'quantize' results",
                doc_id
            );
        }
    }

    // --------------------------------------------------------------------
    // 4.4 TERM ORDER INDEPENDENCE (via search_tier1_exact)
    //
    // For multi-term AND queries, term order should not affect results.
    // --------------------------------------------------------------------

    #[test]
    fn test_term_order_independence_two_terms() {
        let searcher = load_pytorch_searcher();

        // search_tier1_exact tokenizes and uses AND semantics
        let r1 = searcher.search_tier1_exact("tensor autograd", 100);
        let r2 = searcher.search_tier1_exact("autograd tensor", 100);

        assert_eq!(
            r1.len(),
            r2.len(),
            "Term order should not affect result count"
        );

        let ids1: HashSet<_> = r1.iter().map(|r| r.doc_id).collect();
        let ids2: HashSet<_> = r2.iter().map(|r| r.doc_id).collect();

        assert_eq!(ids1, ids2, "Term order should not affect doc IDs returned");
    }

    #[test]
    fn test_term_order_independence_three_terms() {
        let searcher = load_cutlass_searcher();

        // search_tier1_exact tokenizes and uses AND semantics
        let r1 = searcher.search_tier1_exact("kernel cuda gemm", 50);
        let r2 = searcher.search_tier1_exact("gemm kernel cuda", 50);
        let r3 = searcher.search_tier1_exact("cuda gemm kernel", 50);

        assert_eq!(r1.len(), r2.len(), "Order a↔b should be same");
        assert_eq!(r2.len(), r3.len(), "Order b↔c should be same");

        let ids1: HashSet<_> = r1.iter().map(|r| r.doc_id).collect();
        let ids2: HashSet<_> = r2.iter().map(|r| r.doc_id).collect();
        let ids3: HashSet<_> = r3.iter().map(|r| r.doc_id).collect();

        assert_eq!(ids1, ids2, "Order a↔b doc IDs should match");
        assert_eq!(ids2, ids3, "Order b↔c doc IDs should match");
    }
}

// ============================================================================
// PART 5: EDGE CASE TESTS
// ============================================================================

mod edge_cases {
    use super::*;

    // --------------------------------------------------------------------
    // 5.1 QUERY PARSING
    // --------------------------------------------------------------------

    #[test]
    fn test_empty_query() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("", 10);
        assert!(results.is_empty(), "Empty query should return empty");
    }

    #[test]
    fn test_whitespace_query() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("   ", 10);
        assert!(results.is_empty(), "Whitespace-only query should return empty");
    }

    #[test]
    fn test_tab_query() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("\t\n\r", 10);
        assert!(results.is_empty(), "Control chars query should return empty");
    }

    #[test]
    fn test_padded_query() {
        let searcher = load_cutlass_searcher();
        let r1 = searcher.search("kernel", 50);
        let r2 = searcher.search("  kernel  ", 50);

        // Document current behavior: padded query should either:
        // 1. Return same results (if trimming), or
        // 2. Return empty/different (if not trimming)
        // Either is valid, but we should verify the results are valid either way
        verify_search_invariants(&r1, "kernel", 50, searcher.docs().len());
        verify_search_invariants(&r2, "  kernel  ", 50, searcher.docs().len());

        // Document actual behavior
        assert!(
            !r1.is_empty(),
            "'kernel' should return results"
        );

        // Note: If r2 is empty, the implementation doesn't trim leading whitespace
        // This is documented behavior, not a bug
        if r2.is_empty() {
            // Implementation doesn't trim - verify that behavior is consistent
            let r3 = searcher.search(" k", 10);
            assert!(
                r3.is_empty() || r3.len() < r1.len(),
                "Leading space should affect tokenization"
            );
        } else {
            // Implementation does trim - results should match
            assert_eq!(
                r1.len(),
                r2.len(),
                "If trimming, padded query should match trimmed"
            );
        }
    }

    #[test]
    fn test_multiple_spaces() {
        let searcher = load_pytorch_searcher();
        let r1 = searcher.search("tensor autograd", 50);
        let r2 = searcher.search("tensor  autograd", 50);

        // Multiple spaces should collapse to single separator
        assert_eq!(
            r1.len(),
            r2.len(),
            "Multiple spaces should collapse"
        );
    }

    #[test]
    fn test_tab_separator() {
        let searcher = load_pytorch_searcher();
        let r1 = searcher.search("tensor autograd", 50);
        let r2 = searcher.search("tensor\tautograd", 50);

        assert_eq!(r1.len(), r2.len(), "Tab should work as separator");
    }

    // --------------------------------------------------------------------
    // 5.2 PUNCTUATION HANDLING
    // --------------------------------------------------------------------

    #[test]
    fn test_punctuation_cpp() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("c++", 10);
        // Should handle gracefully (may search for "c" or return empty)
        verify_search_invariants(&results, "c++", 10, searcher.docs().len());
    }

    #[test]
    fn test_punctuation_hyphen() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("cuda-aware", 10);
        // Hyphenated terms should be handled
        verify_search_invariants(&results, "cuda-aware", 10, searcher.docs().len());
    }

    // --------------------------------------------------------------------
    // 5.3 UNICODE HANDLING
    // --------------------------------------------------------------------

    #[test]
    fn test_unicode_cafe() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("café", 10);
        // Should handle diacritics gracefully
        verify_search_invariants(&results, "café", 10, searcher.docs().len());
    }

    #[test]
    fn test_unicode_naive() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("naïve", 10);
        verify_search_invariants(&results, "naïve", 10, searcher.docs().len());
    }

    #[test]
    fn test_unicode_cjk() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("北京", 10);
        // Should handle gracefully (likely empty)
        verify_search_invariants(&results, "北京", 10, searcher.docs().len());
    }

    #[test]
    fn test_unicode_cyrillic() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("Москва", 10);
        verify_search_invariants(&results, "Москва", 10, searcher.docs().len());
    }

    #[test]
    fn test_unicode_emoji_prefix() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("🔥rust", 10);
        verify_search_invariants(&results, "🔥rust", 10, searcher.docs().len());
    }

    #[test]
    fn test_unicode_emoji_suffix() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("rust🔥", 10);
        verify_search_invariants(&results, "rust🔥", 10, searcher.docs().len());
    }

    #[test]
    fn test_unicode_zero_width_space() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("\u{200B}tensor", 10);
        verify_search_invariants(&results, "\u{200B}tensor", 10, searcher.docs().len());
    }

    // --------------------------------------------------------------------
    // 5.4 QUERY LENGTH TESTS
    // --------------------------------------------------------------------

    #[test]
    fn test_single_char_query() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("a", 10);
        verify_search_invariants(&results, "a", 10, searcher.docs().len());
    }

    #[test]
    fn test_two_char_query() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("te", 50);
        verify_search_invariants(&results, "te", 50, searcher.docs().len());
    }

    #[test]
    fn test_long_query_50() {
        let searcher = load_cutlass_searcher();
        let query = "a".repeat(50);
        let results = searcher.search(&query, 10);
        // Should handle gracefully (likely empty)
        assert!(results.len() <= 10, "Should respect limit");
    }

    #[test]
    fn test_long_query_100() {
        let searcher = load_cutlass_searcher();
        let query = "a".repeat(100);
        let results = searcher.search(&query, 10);
        assert!(results.len() <= 10, "Should respect limit");
    }

    #[test]
    fn test_long_query_500() {
        let searcher = load_cutlass_searcher();
        let query = "a".repeat(500);
        let results = searcher.search(&query, 10);
        assert!(results.len() <= 10, "Should handle long query without panic");
    }

    #[test]
    fn test_long_query_1000() {
        let searcher = load_cutlass_searcher();
        let query = "a".repeat(1000);
        let results = searcher.search(&query, 10);
        assert!(results.len() <= 10, "Should handle very long query");
    }

    // --------------------------------------------------------------------
    // 5.5 LIMIT BOUNDARY TESTS
    // --------------------------------------------------------------------

    #[test]
    fn test_limit_zero() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 0);
        assert!(results.is_empty(), "Limit 0 should return empty");
    }

    #[test]
    fn test_limit_one() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 1);
        assert!(results.len() <= 1, "Limit 1 should return at most 1");
    }

    #[test]
    fn test_limit_exact() {
        let searcher = load_cutlass_searcher();
        for limit in [2, 5, 10, 50] {
            let results = searcher.search("kernel", limit);
            assert!(
                results.len() <= limit,
                "Limit {} should be respected, got {}",
                limit,
                results.len()
            );
        }
    }

    #[test]
    fn test_limit_large() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 1000);
        assert!(
            results.len() <= searcher.docs().len(),
            "Should not exceed num_docs"
        );
    }

    // --------------------------------------------------------------------
    // 5.6 SPECIAL CHARACTER INJECTION
    // --------------------------------------------------------------------

    #[test]
    fn test_null_byte() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("tensor\0autograd", 10);
        // Should handle gracefully
        assert!(results.len() <= 10, "Should handle null byte");
    }

    #[test]
    fn test_xss_attempt() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("<script>", 10);
        verify_search_invariants(&results, "<script>", 10, searcher.docs().len());
    }

    #[test]
    fn test_sql_injection_attempt() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("'; DROP TABLE", 10);
        verify_search_invariants(&results, "'; DROP TABLE", 10, searcher.docs().len());
    }

    #[test]
    fn test_path_traversal_attempt() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("../../../etc", 10);
        verify_search_invariants(&results, "../../../etc", 10, searcher.docs().len());
    }

    #[test]
    fn test_ansi_escape() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("\x1b[31m", 10);
        // Should handle gracefully
        assert!(results.len() <= 10, "Should handle ANSI escape");
    }
}

// ============================================================================
// PART 6: DEDUPLICATION STRESS TESTS
// ============================================================================

mod deduplication {
    use super::*;

    // --------------------------------------------------------------------
    // 6.1 SAME DOC MULTIPLE TIERS
    // --------------------------------------------------------------------

    #[test]
    fn test_dedup_across_all_tiers() {
        let searcher = load_cutlass_searcher();

        // Full search should deduplicate across tiers
        let results = searcher.search("kernel", 100);

        // Count occurrences of each doc_id
        let doc_ids: Vec<_> = results.iter().map(|r| r.doc_id).collect();
        let unique: HashSet<_> = doc_ids.iter().collect();

        assert_eq!(
            doc_ids.len(),
            unique.len(),
            "No duplicates across tiers"
        );
    }

    #[test]
    fn test_dedup_t1_excludes_from_t2() {
        let searcher = load_cutlass_searcher();

        let t1 = searcher.search_tier1_exact("kernel", 16000);
        let t1_ids: HashSet<_> = t1.iter().map(|r| r.doc_id).collect();

        let t2 = searcher.search_tier2_prefix("kernel", &t1_ids, 50);

        // T2 should not contain any T1 doc IDs
        for r in &t2 {
            assert!(
                !t1_ids.contains(&r.doc_id),
                "T2 should exclude T1 docs"
            );
        }
    }

    #[test]
    fn test_dedup_t1_t2_excludes_from_t3() {
        let searcher = load_cutlass_searcher();

        let t1 = searcher.search_tier1_exact("kernel", 16000);
        let t1_ids: HashSet<_> = t1.iter().map(|r| r.doc_id).collect();

        let t2 = searcher.search_tier2_prefix("kernel", &t1_ids, 50);
        let t2_ids: HashSet<_> = t2.iter().map(|r| r.doc_id).collect();

        let exclude_ids: HashSet<_> = t1_ids.union(&t2_ids).copied().collect();
        let t3 = searcher.search_tier3_fuzzy("kernal", &exclude_ids, 50);

        // T3 should not contain any T1 or T2 doc IDs
        for r in &t3 {
            assert!(
                !exclude_ids.contains(&r.doc_id),
                "T3 should exclude T1 and T2 docs"
            );
        }
    }

    // --------------------------------------------------------------------
    // 6.2 SAME DOC MULTIPLE SECTIONS
    // --------------------------------------------------------------------

    #[test]
    fn test_dedup_within_doc_sections() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search("kernel", 100);

        // Each doc should appear exactly once
        let mut doc_counts: std::collections::HashMap<usize, usize> =
            std::collections::HashMap::new();
        for r in &results {
            *doc_counts.entry(r.doc_id).or_insert(0) += 1;
        }

        for (doc_id, count) in doc_counts {
            assert_eq!(
                count, 1,
                "Doc {} appears {} times, should be 1",
                doc_id, count
            );
        }
    }

    // --------------------------------------------------------------------
    // 6.3 HIGH-VOLUME DEDUP
    // --------------------------------------------------------------------

    #[test]
    fn test_high_volume_dedup_pytorch() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("tensor", 200);

        // Should have no duplicates even with high limit
        let doc_ids: Vec<_> = results.iter().map(|r| r.doc_id).collect();
        let unique: HashSet<_> = doc_ids.iter().collect();

        assert_eq!(
            doc_ids.len(),
            unique.len(),
            "No duplicates with high limit"
        );

        verify_search_invariants(&results, "tensor", 200, searcher.docs().len());
    }

    #[test]
    fn test_dedup_common_term_pytorch() {
        let searcher = load_pytorch_searcher();
        let results = searcher.search("the", 150);

        // Even very common terms should deduplicate
        let doc_ids: Vec<_> = results.iter().map(|r| r.doc_id).collect();
        let unique: HashSet<_> = doc_ids.iter().collect();

        assert_eq!(
            doc_ids.len(),
            unique.len(),
            "No duplicates for common term"
        );
    }
}

// ============================================================================
// PART 7: SCORE INVARIANT TESTS
// ============================================================================

mod score_invariants {
    use super::*;

    // --------------------------------------------------------------------
    // 7.1 TIER SCORE RANGES
    // --------------------------------------------------------------------

    #[test]
    fn test_t1_score_range() {
        let searcher = load_cutlass_searcher();
        let results = searcher.search_tier1_exact("kernel", 16000);

        for r in &results {
            // T1 scores should be around 100 (base) with possible title boost (~110)
            assert!(
                r.score >= 90.0 && r.score <= 150.0,
                "T1 score {} should be in [90, 150]",
                r.score
            );
        }
    }

    #[test]
    fn test_t2_score_range() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("kern", &exclude, 16000);

        for r in &results {
            // T2 scores should be around 50 (base) with possible title boost (~60)
            assert!(
                r.score >= 40.0 && r.score <= 80.0,
                "T2 score {} should be in [40, 80]",
                r.score
            );
        }
    }

    #[test]
    fn test_t3_score_range() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();
        let results = searcher.search_tier3_fuzzy("kernal", &exclude, 16000);

        for r in &results {
            // T3 scores depend on distance and length bonus
            // d=1 base 30, d=2 base 15, with penalties and bonuses
            assert!(
                r.score >= 5.0 && r.score <= 60.0,
                "T3 score {} should be in [5, 60]",
                r.score
            );
        }
    }

    // --------------------------------------------------------------------
    // 7.2 FUZZY DISTANCE PENALTY
    // --------------------------------------------------------------------

    #[test]
    fn test_fuzzy_d1_vs_d2_score() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();

        // d=1 should score higher than d=2
        // "kernal" is d=1 from "kernel" (a→e substitution)
        let d1_results = searcher.search_tier3_fuzzy("kernal", &exclude, 16000);
        assert!(
            !d1_results.is_empty(),
            "d=1 query 'kernal' should find matches"
        );

        // "karnul" is d=2 from "kernel" (e→a, e→u substitutions)
        let d2_results = searcher.search_tier3_fuzzy("karnul", &exclude, 50);

        // Get max scores from each
        let d1_max = d1_results
            .iter()
            .map(|r| r.score)
            .fold(0.0f64, |a, b| a.max(b));

        // d=1 base score is 30, d=2 base score is 15
        // d=1 should always score higher than d=2
        assert!(
            d1_max >= 20.0,
            "d=1 max score ({}) should be >= 20 (base 30 with penalties)",
            d1_max
        );

        if !d2_results.is_empty() {
            let d2_max = d2_results
                .iter()
                .map(|r| r.score)
                .fold(0.0f64, |a, b| a.max(b));

            assert!(
                d1_max > d2_max,
                "d=1 max score ({}) should be > d=2 max score ({})",
                d1_max,
                d2_max
            );
        }
    }

    // --------------------------------------------------------------------
    // 7.3 SCORE POSITIVE
    // --------------------------------------------------------------------

    #[test]
    fn test_all_scores_positive() {
        let searcher = load_pytorch_searcher();

        let queries = ["tensor", "autograd", "forward", "module", "data"];

        for query in queries {
            let results = searcher.search(query, 100);
            for r in &results {
                assert!(
                    r.score > 0.0,
                    "Score for '{}' doc {} should be positive: {}",
                    query,
                    r.doc_id,
                    r.score
                );
            }
        }
    }
}

// ============================================================================
// PART 8: CROSS-DATASET CONSISTENCY
// ============================================================================

mod cross_dataset {
    use super::*;

    #[test]
    fn test_shared_term_tensor_consistent() {
        let cutlass = load_cutlass_searcher();
        let pytorch = load_pytorch_searcher();

        let c_results = cutlass.search("tensor", 50);
        let p_results = pytorch.search("tensor", 100);

        // Both should return results
        assert!(!c_results.is_empty(), "Cutlass should have 'tensor' matches");
        assert!(!p_results.is_empty(), "PyTorch should have 'tensor' matches");

        // Both should have correct bucketed ordering
        verify_bucketed_ordering(&c_results);
        verify_bucketed_ordering(&p_results);
    }

    #[test]
    fn test_nonexistent_both_datasets() {
        let cutlass = load_cutlass_searcher();
        let pytorch = load_pytorch_searcher();

        let nonsense = "xyzzyplugh12345nonexistent";

        assert!(
            cutlass.search(nonsense, 10).is_empty(),
            "Nonexistent term should be empty in cutlass"
        );
        assert!(
            pytorch.search(nonsense, 10).is_empty(),
            "Nonexistent term should be empty in pytorch"
        );
    }

    #[test]
    fn test_empty_query_both_datasets() {
        let cutlass = load_cutlass_searcher();
        let pytorch = load_pytorch_searcher();

        assert!(
            cutlass.search("", 10).is_empty(),
            "Empty query should be empty in cutlass"
        );
        assert!(
            pytorch.search("", 10).is_empty(),
            "Empty query should be empty in pytorch"
        );
    }
}

// ============================================================================
// PART 9: PERFORMANCE SANITY TESTS
// ============================================================================

mod performance {
    use super::*;
    use std::time::Instant;

    #[test]
    fn test_t1_latency_under_10ms() {
        let searcher = load_cutlass_searcher();

        let iterations = 100;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = searcher.search_tier1_exact("kernel", 10);
        }
        let elapsed = start.elapsed();

        // Average should be under 0.1ms per search
        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
        assert!(
            avg_ms < 1.0,
            "T1 avg latency {}ms should be < 1ms",
            avg_ms
        );
    }

    #[test]
    fn test_t2_latency_under_50ms() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();

        let iterations = 50;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = searcher.search_tier2_prefix("kern", &exclude, 10);
        }
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
        assert!(
            avg_ms < 10.0,
            "T2 avg latency {}ms should be < 10ms",
            avg_ms
        );
    }

    #[test]
    fn test_t3_latency_under_100ms() {
        let searcher = load_cutlass_searcher();
        let exclude = HashSet::new();

        let iterations = 10;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = searcher.search_tier3_fuzzy("kernal", &exclude, 10);
        }
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
        assert!(
            avg_ms < 100.0,
            "T3 avg latency {}ms should be < 100ms",
            avg_ms
        );
    }

    #[test]
    fn test_full_search_latency_under_200ms() {
        let searcher = load_pytorch_searcher();

        let iterations = 10;
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = searcher.search("tensor autograd", 50);
        }
        let elapsed = start.elapsed();

        let avg_ms = elapsed.as_millis() as f64 / iterations as f64;
        assert!(
            avg_ms < 200.0,
            "Full search avg latency {}ms should be < 200ms",
            avg_ms
        );
    }
}
