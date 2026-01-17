// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for search query handling.
//!
//! Throws arbitrary byte sequences at the search API to verify it never panics,
//! never returns invalid results, and never violates the bucketed ranking invariants.
//! If your search engine crashes on emoji or null bytes, you have a bad day.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::fs;
use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;

/// The searcher must survive whatever users throw at it.
///
/// Emoji, RTL text, null bytes, queries longer than War and Peace. None
/// of it should crash. The fuzzer will find the string that breaks your
/// assumptions about "reasonable input."
fuzz_target!(|query: &[u8]| {
    // Load index once per process
    // Note: fuzz tests run from the fuzz/ directory, so path is relative to parent
    static SEARCHER: std::sync::OnceLock<TierSearcher> = std::sync::OnceLock::new();
    let searcher = SEARCHER.get_or_init(|| {
        // Try multiple paths to handle different working directories
        let paths = [
            "target/datasets/cutlass/index.sorex",
            "../target/datasets/cutlass/index.sorex",
        ];
        let bytes = paths.iter()
            .find_map(|p| fs::read(p).ok())
            .expect("Failed to read index file from any path");
        let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to load index");
        TierSearcher::from_layer(layer).expect("Failed to create searcher")
    });

    // Convert to string, handling invalid UTF-8
    let query_str = match std::str::from_utf8(query) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(query).into_owned(),
    };

    // Cap query length to avoid timeout
    let query_str = &query_str[..query_str.len().min(200)];

    // INVARIANT 1: search() should never panic
    let results = searcher.search(query_str, 10);

    // INVARIANT 2: Results should be bounded by limit
    assert!(
        results.len() <= 10,
        "Got {} results, expected at most 10",
        results.len()
    );

    // INVARIANT 3: All doc_ids in results should be valid
    for result in &results {
        assert!(
            result.doc_id < searcher.docs().len(),
            "Result doc_id {} out of bounds (doc_count = {})",
            result.doc_id,
            searcher.docs().len()
        );
    }

    // INVARIANT 4: Results should be sorted by (match_type, score) using bucketed ranking
    // Note: match_type is primary key, score is secondary within each bucket
    // So score can go DOWN when match_type changes (e.g., Title with score 100 > Content with score 18800)
    for i in 1..results.len() {
        // Results are correctly sorted if:
        // - match_type[i-1] < match_type[i] (higher bucket wins), OR
        // - match_type[i-1] == match_type[i] AND score[i-1] >= score[i]
        let correct_order = results[i - 1].match_type < results[i].match_type ||
            (results[i - 1].match_type == results[i].match_type && results[i - 1].score >= results[i].score);
        assert!(
            correct_order,
            "Results not correctly sorted at position {}: (match_type={:?}, score={}) should come before (match_type={:?}, score={})",
            i, results[i - 1].match_type, results[i - 1].score, results[i].match_type, results[i].score
        );
    }

    // INVARIANT 5: No duplicate doc_ids in results
    let mut seen_docs = std::collections::HashSet::new();
    for result in &results {
        assert!(
            seen_docs.insert(result.doc_id),
            "Duplicate doc_id {} in results",
            result.doc_id
        );
    }

    // INVARIANT 6: Empty query should return empty results
    if query_str.is_empty() {
        assert!(
            results.is_empty(),
            "Empty query should return empty results, got {}",
            results.len()
        );
    }

    // Test individual tiers too
    let tier1 = searcher.search_tier1_exact(query_str, 10);
    let tier2 = searcher.search_tier2_prefix(query_str, &std::collections::HashSet::new(), 10);
    let tier3 = searcher.search_tier3_fuzzy(query_str, &std::collections::HashSet::new(), 10);

    // INVARIANT 7: Each tier should respect limit
    assert!(tier1.len() <= 10);
    assert!(tier2.len() <= 10);
    assert!(tier3.len() <= 10);

    // INVARIANT 8: Each tier should return unique doc_ids (no duplicates within tier)
    let mut tier1_seen = std::collections::HashSet::new();
    for result in &tier1 {
        assert!(
            tier1_seen.insert(result.doc_id),
            "Duplicate doc_id {} in Tier 1 results",
            result.doc_id
        );
    }

    let mut tier2_seen = std::collections::HashSet::new();
    for result in &tier2 {
        assert!(
            tier2_seen.insert(result.doc_id),
            "Duplicate doc_id {} in Tier 2 results",
            result.doc_id
        );
    }

    let mut tier3_seen = std::collections::HashSet::new();
    for result in &tier3 {
        assert!(
            tier3_seen.insert(result.doc_id),
            "Duplicate doc_id {} in Tier 3 results",
            result.doc_id
        );
    }
});
