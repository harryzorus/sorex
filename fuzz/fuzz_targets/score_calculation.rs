// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for score calculation invariants.
//!
//! Scores must be finite, non-negative, and deterministic. The same query run
//! twice must produce identical scores. This catches floating-point edge cases
//! and ensures no NaN or infinity sneaks through.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::fs;
use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;

// Scores must be finite, non-negative, and deterministic.
//
// Run the same query twice, get the same scores. No NaN sneaking through.
// No overflow turning a good match into garbage. The fuzzer will find
// the floating-point edge cases that make you regret using f64.
fuzz_target!(|query: &[u8]| {
    // Load index once per process
    static SEARCHER: std::sync::OnceLock<TierSearcher> = std::sync::OnceLock::new();
    let searcher = SEARCHER.get_or_init(|| {
        let paths = [
            "target/datasets/cutlass/index.sorex",
            "../target/datasets/cutlass/index.sorex",
        ];
        let bytes = paths.iter()
            .find_map(|p| fs::read(p).ok())
            .expect("Failed to read index file");
        let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to load index");
        TierSearcher::from_layer(layer).expect("Failed to create searcher")
    });

    // Convert to string, handling invalid UTF-8
    let query_str = match std::str::from_utf8(query) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(query).into_owned(),
    };

    // Cap query length
    let query_str = &query_str[..query_str.len().min(200)];

    // Run search twice with same query
    let results1 = searcher.search(query_str, 100);
    let results2 = searcher.search(query_str, 100);

    // INVARIANT 1: Searches are deterministic
    assert_eq!(
        results1.len(),
        results2.len(),
        "Same query returned different result counts"
    );

    for (r1, r2) in results1.iter().zip(results2.iter()) {
        assert_eq!(r1.doc_id, r2.doc_id, "Result order changed between searches");
        assert_eq!(r1.score, r2.score, "Score changed between searches for same query");
        assert_eq!(r1.match_type, r2.match_type, "Match type changed between searches");
    }

    // INVARIANT 2: All scores are valid
    for result in &results1 {
        assert!(
            result.score.is_finite(),
            "Score {} is not finite",
            result.score
        );
        assert!(
            result.score >= 0.0,
            "Score {} is negative",
            result.score
        );
        // Reasonable upper bound (to catch overflow/computation errors)
        assert!(
            result.score <= 10000.0,
            "Score {} exceeds reasonable bounds",
            result.score
        );
    }

    // INVARIANT 3: Scores are monotone decreasing within same match_type
    for i in 1..results1.len() {
        let prev = &results1[i - 1];
        let curr = &results1[i];

        if prev.match_type == curr.match_type {
            assert!(
                prev.score >= curr.score,
                "Scores not monotone decreasing within match_type at position {}: {} > {}",
                i, prev.score, curr.score
            );
        }
    }

    // INVARIANT 4: Results with exact matches have higher scores than fuzzy matches
    // (assuming same document, which we can't easily check without more context,
    // but we can verify that tier ordering is respected)
    let mut prev_tier = 0;
    for result in &results1 {
        // Tier should not decrease (Tier 1 >= Tier 2 >= Tier 3)
        // Actually, tier increases (Tier 1 is best, Tier 3 is worst)
        // So tier should monotone increase or stay same
        assert!(
            result.tier >= prev_tier,
            "Tier decreased at position: {} (tier {}) after {} (tier {})",
            result.doc_id, result.tier, prev_tier, prev_tier
        );
        prev_tier = result.tier;
    }

    // INVARIANT 5: Very short queries don't crash or produce NaN
    let short_queries = vec!["a", "ab", "x"];
    for short_q in short_queries {
        let results = searcher.search(short_q, 50);
        for result in &results {
            assert!(!result.score.is_nan(), "NaN score for query '{}'", short_q);
            assert!(result.score >= 0.0, "Negative score for query '{}'", short_q);
        }
    }

    // INVARIANT 6: Empty/whitespace queries are handled safely
    let empty_results = searcher.search("", 100);
    for result in &empty_results {
        assert!(!result.score.is_nan(), "NaN score for empty query");
    }

    let whitespace_results = searcher.search("   ", 100);
    for result in &whitespace_results {
        assert!(!result.score.is_nan(), "NaN score for whitespace query");
    }
});
