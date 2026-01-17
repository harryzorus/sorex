// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for Levenshtein DFA matching.
//!
//! Verifies that the parametric DFA correctly bounds edit distance and respects
//! the triangle inequality. The DFA is the heart of fuzzy search. If it lies
//! about distances, users get garbage results.

#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use std::fs;
use sorex::binary::LoadedLayer;
use sorex::levenshtein_dfa::{ParametricDFA, QueryMatcher};

/// Fuzz input for Levenshtein DFA matching
#[derive(Debug, Arbitrary)]
struct MatchInput {
    /// Query string (UTF-8, capped to avoid timeout)
    query_bytes: Vec<u8>,
    /// Target string to match against
    target_bytes: Vec<u8>,
}

fuzz_target!(|input: MatchInput| {
    // Convert to valid UTF-8 strings (skip invalid sequences)
    let query = match std::str::from_utf8(&input.query_bytes) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(&input.query_bytes).into_owned(),
    };

    let target = match std::str::from_utf8(&input.target_bytes) {
        Ok(s) => s.to_string(),
        Err(_) => String::from_utf8_lossy(&input.target_bytes).into_owned(),
    };

    // Cap lengths to avoid timeouts
    let query = &query[..query.len().min(50)];
    let target = &target[..target.len().min(100)];

    // Skip empty inputs
    if query.is_empty() || target.is_empty() {
        return;
    }

    // Load DFA from real index (once per process via lazy_static)
    // Note: fuzz tests run from the fuzz/ directory, so path is relative to parent
    static DFA_BYTES: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();
    let dfa_bytes = DFA_BYTES.get_or_init(|| {
        // Try multiple paths to handle different working directories
        let paths = [
            "target/datasets/cutlass/index.sorex",
            "../target/datasets/cutlass/index.sorex",
        ];
        let bytes = paths.iter()
            .find_map(|p| fs::read(p).ok())
            .expect("Failed to read index file from any path");
        let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to load index");
        layer.lev_dfa_bytes
    });

    let dfa = match ParametricDFA::from_bytes(dfa_bytes) {
        Ok(dfa) => dfa,
        Err(_) => return, // Skip if DFA loading fails
    };

    // Test matching
    let matcher = QueryMatcher::new(&dfa, query);
    let result = matcher.matches(target);

    // INVARIANT 1: Result should be Some(d) where d <= 2, or None
    if let Some(distance) = result {
        assert!(
            distance <= 2,
            "Levenshtein distance {} exceeds max k=2 for query='{}', target='{}'",
            distance, query, target
        );

        // INVARIANT 2: Triangle inequality - |len(query) - len(target)| <= distance
        let len_diff = (query.chars().count() as i32 - target.chars().count() as i32).unsigned_abs() as u8;
        assert!(
            len_diff <= distance,
            "Length difference {} exceeds distance {} for query='{}', target='{}'",
            len_diff, distance, query, target
        );
    }

    // INVARIANT 3: Exact match should return distance 0
    if query == target {
        assert_eq!(
            result,
            Some(0),
            "Exact match should return distance 0 for query=target='{}'",
            query
        );
    }

    // INVARIANT 4: Query matching itself should return 0
    let self_result = matcher.matches(query);
    assert_eq!(
        self_result,
        Some(0),
        "Query matching itself should return 0 for query='{}'",
        query
    );
});
