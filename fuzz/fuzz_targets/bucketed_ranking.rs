// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for bucketed ranking invariants.
//!
//! Match type MUST dominate score. A title match with score 1 beats a content
//! match with score 10000. This fuzz target ensures the ranking comparator
//! never violates this hierarchy, no matter what scores the fuzzer generates.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::fs;
use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;

// The hierarchy must hold: title beats content, period.
//
// A title match with score 1 must rank above content with score 10000.
// The fuzzer generates queries and verifies this invariant survives
// whatever scoring edge cases it discovers.
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

    // Cap query length to avoid timeout
    let query_str = &query_str[..query_str.len().min(200)];

    // Skip empty queries
    if query_str.trim().is_empty() {
        return;
    }

    // Get results
    let results = searcher.search(query_str, 100);

    // INVARIANT 1: Match type dominates score
    // Results must be sorted by match_type first (primary key),
    // then by score within the same match_type (secondary key)
    for i in 1..results.len() {
        let prev = &results[i - 1];
        let curr = &results[i];

        // Check ordering: should be sorted by match_type ascending (numerically)
        // If match_type differs, the previous should be smaller
        // If match_type same, score should be descending
        let correct_order = if prev.match_type != curr.match_type {
            prev.match_type < curr.match_type
        } else {
            prev.score >= curr.score
        };

        assert!(
            correct_order,
            "Bucketed ranking violated at position {}\n\
             Prev: match_type={:?}, score={}\n\
             Curr: match_type={:?}, score={}",
            i, prev.match_type, prev.score, curr.match_type, curr.score
        );
    }

    // INVARIANT 2: No duplicate doc_ids
    // Each document should appear at most once in results
    let mut seen_docs = std::collections::HashSet::new();
    for result in &results {
        assert!(
            seen_docs.insert(result.doc_id),
            "Duplicate doc_id {} found in results",
            result.doc_id
        );
    }

    // INVARIANT 3: All doc_ids are valid
    for result in &results {
        assert!(
            result.doc_id < searcher.docs().len(),
            "Invalid doc_id {} (doc_count = {})",
            result.doc_id,
            searcher.docs().len()
        );
    }

    // INVARIANT 4: Results respect limit
    assert!(results.len() <= 100, "Results exceed limit");

    // INVARIANT 5: All scores are finite and non-negative
    for result in &results {
        assert!(
            result.score.is_finite(),
            "Score {} is not finite at doc_id {}",
            result.score,
            result.doc_id
        );
        assert!(
            result.score >= 0.0,
            "Score {} is negative at doc_id {}",
            result.score,
            result.doc_id
        );
    }
});
