// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for tier merging and cross-tier deduplication.
//!
//! The three-tier search must never return duplicate documents. This fuzz target
//! exercises the exclusion logic: T2 must exclude T1 docs, T3 must exclude T1+T2.
//! The bug that motivated this: composite keys causing the same doc to appear
//! multiple times with different section_idx values.

#![no_main]

use libfuzzer_sys::fuzz_target;
use std::collections::HashSet;
use std::fs;
use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;

/// Fuzz input structure for tier merging tests.
#[derive(Debug, Clone)]
struct TierMergingInput {
    query: String,
    limit: usize,
}

impl<'a> arbitrary::Arbitrary<'a> for TierMergingInput {
    fn arbitrary(u: &mut arbitrary::Unstructured<'a>) -> arbitrary::Result<Self> {
        // Generate query from common search patterns
        let query = match u.int_in_range(0..=10)? {
            0 => String::new(), // Empty
            1 => " ".repeat(u.int_in_range(1..=5)?), // Whitespace
            2 => "a".repeat(u.int_in_range(1..=3)?), // Short prefix
            3 => {
                // Common terms that hit multiple docs
                let terms = ["tensor", "matrix", "cuda", "kernel", "memory", "torch", "device"];
                terms[u.int_in_range(0..=terms.len() - 1)?].to_string()
            }
            4 => {
                // Multi-word queries (test AND semantics)
                let term1 = ["tensor", "cuda", "kernel"][u.int_in_range(0..=2)?];
                let term2 = ["matrix", "memory", "device"][u.int_in_range(0..=2)?];
                format!("{} {}", term1, term2)
            }
            5 => {
                // Typos for fuzzy search
                let misspelled = ["tensorr", "matrx", "cudaa", "kernl", "memry"];
                misspelled[u.int_in_range(0..=misspelled.len() - 1)?].to_string()
            }
            _ => {
                // Random string
                let len = u.int_in_range(1..=20)?;
                let bytes: Vec<u8> = (0..len)
                    .map(|_| u.int_in_range(b'a'..=b'z'))
                    .collect::<Result<_, _>>()?;
                String::from_utf8(bytes).unwrap_or_default()
            }
        };

        // Vary limits to test edge cases
        let limit = match u.int_in_range(0..=5)? {
            0 => 0,
            1 => 1,
            2 => u.int_in_range(2..=10)?,
            3 => u.int_in_range(10..=50)?,
            4 => u.int_in_range(50..=100)?,
            _ => u.int_in_range(1..=200)?,
        };

        Ok(TierMergingInput { query, limit })
    }
}

/// Fuzz target for tier merging and cross-tier deduplication.
///
/// Tests critical invariants:
/// - INVARIANT 1: No duplicate doc_ids in merged results
/// - INVARIANT 2: T2 doesn't include T1 docs
/// - INVARIANT 3: T3 doesn't include T1 or T2 docs
/// - INVARIANT 4: Merged result preserves best match_type per doc
/// - INVARIANT 5: Result count respects limit
fuzz_target!(|input: TierMergingInput| {
    // Load index once per process
    static SEARCHER: std::sync::OnceLock<TierSearcher> = std::sync::OnceLock::new();
    let searcher = SEARCHER.get_or_init(|| {
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

    let TierMergingInput { query, limit } = input;

    // =========================================================================
    // INVARIANT 1: Full search pipeline produces no duplicate doc_ids
    // =========================================================================
    let results = searcher.search(&query, limit);

    let mut seen_docs: HashSet<usize> = HashSet::with_capacity(results.len());
    for result in &results {
        assert!(
            seen_docs.insert(result.doc_id),
            "INVARIANT 1 VIOLATED: Duplicate doc_id {} in results for query '{}'",
            result.doc_id, query
        );
    }

    // =========================================================================
    // INVARIANT 2: T2 excludes T1 docs when given exclude set
    // =========================================================================
    let t1_results = searcher.search_tier1_exact(&query, limit.max(100));
    let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();

    let t2_results = searcher.search_tier2_prefix(&query, &t1_ids, limit.max(100));
    for result in &t2_results {
        assert!(
            !t1_ids.contains(&result.doc_id),
            "INVARIANT 2 VIOLATED: T2 returned doc {} which was in T1 exclude set",
            result.doc_id
        );
    }

    // =========================================================================
    // INVARIANT 3: T3 excludes T1 and T2 docs when given exclude set
    // =========================================================================
    let t2_ids: HashSet<usize> = t2_results.iter().map(|r| r.doc_id).collect();
    let mut exclude_for_t3: HashSet<usize> = t1_ids.clone();
    exclude_for_t3.extend(t2_ids.iter());

    let t3_results = searcher.search_tier3_fuzzy(&query, &exclude_for_t3, limit.max(100));
    for result in &t3_results {
        assert!(
            !exclude_for_t3.contains(&result.doc_id),
            "INVARIANT 3 VIOLATED: T3 returned doc {} which was in T1+T2 exclude set",
            result.doc_id
        );
    }

    // =========================================================================
    // INVARIANT 4: Each tier produces unique doc_ids within itself
    // =========================================================================
    let mut t1_seen: HashSet<usize> = HashSet::new();
    for result in &t1_results {
        assert!(
            t1_seen.insert(result.doc_id),
            "INVARIANT 4 VIOLATED: Duplicate doc_id {} within T1 for query '{}'",
            result.doc_id, query
        );
    }

    let mut t2_seen: HashSet<usize> = HashSet::new();
    for result in &t2_results {
        assert!(
            t2_seen.insert(result.doc_id),
            "INVARIANT 4 VIOLATED: Duplicate doc_id {} within T2 for query '{}'",
            result.doc_id, query
        );
    }

    let mut t3_seen: HashSet<usize> = HashSet::new();
    for result in &t3_results {
        assert!(
            t3_seen.insert(result.doc_id),
            "INVARIANT 4 VIOLATED: Duplicate doc_id {} within T3 for query '{}'",
            result.doc_id, query
        );
    }

    // =========================================================================
    // INVARIANT 5: Result count respects limit
    // =========================================================================
    assert!(
        results.len() <= limit,
        "INVARIANT 5 VIOLATED: Got {} results but limit was {}",
        results.len(), limit
    );

    // =========================================================================
    // INVARIANT 6: Tier assignments in merged results are disjoint
    // =========================================================================
    let tier1_docs: HashSet<usize> = results.iter()
        .filter(|r| r.tier == 1)
        .map(|r| r.doc_id)
        .collect();

    let tier2_docs: HashSet<usize> = results.iter()
        .filter(|r| r.tier == 2)
        .map(|r| r.doc_id)
        .collect();

    let tier3_docs: HashSet<usize> = results.iter()
        .filter(|r| r.tier == 3)
        .map(|r| r.doc_id)
        .collect();

    // Check for overlaps between tiers
    for doc_id in &tier1_docs {
        assert!(
            !tier2_docs.contains(doc_id),
            "INVARIANT 6 VIOLATED: doc {} appears in both T1 and T2",
            doc_id
        );
        assert!(
            !tier3_docs.contains(doc_id),
            "INVARIANT 6 VIOLATED: doc {} appears in both T1 and T3",
            doc_id
        );
    }
    for doc_id in &tier2_docs {
        assert!(
            !tier3_docs.contains(doc_id),
            "INVARIANT 6 VIOLATED: doc {} appears in both T2 and T3",
            doc_id
        );
    }

    // =========================================================================
    // INVARIANT 7: All doc_ids are valid
    // =========================================================================
    let doc_count = searcher.docs().len();
    for result in &results {
        assert!(
            result.doc_id < doc_count,
            "INVARIANT 7 VIOLATED: doc_id {} out of bounds (doc_count = {})",
            result.doc_id, doc_count
        );
    }

    // =========================================================================
    // INVARIANT 8: Empty query returns empty results
    // =========================================================================
    if query.trim().is_empty() {
        assert!(
            results.is_empty(),
            "INVARIANT 8 VIOLATED: Empty query should return empty results, got {}",
            results.len()
        );
    }
});
