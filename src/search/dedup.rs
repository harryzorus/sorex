// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Type-safe result deduplication for WASM search.
//!
//! A document should appear at most once in search results. Sounds obvious,
//! but it's easy to mess up when merging results across tiers and sections.
//! The bug that motivated this module: using `(doc_id, section_idx)` as a
//! composite key, causing the same document to appear multiple times.
//!
//! `ResultMerger` enforces doc_id-only deduplication at the type level.
//! If you try to use a composite key, you're fighting the API.
//!
//! **Invariant**: Each document appears at most once in search results.
//!
//! **Verified by**:
//! - `prop_no_duplicate_docs_in_results` (tests/property/tiered_search.rs)
//! - `fuzz_target/tier_merging.rs` (INVARIANT 1)

#[cfg(feature = "wasm")]
use crate::scoring::ranking::compare_results;
#[cfg(feature = "wasm")]
use crate::search::tiered::SearchResult;
#[cfg(feature = "wasm")]
use crate::types::SearchDoc;
#[cfg(feature = "wasm")]
use std::cmp::Ordering;
#[cfg(feature = "wasm")]
use std::collections::HashMap;

/// Type-safe result merger that enforces doc_id-only deduplication.
///
/// This is the **single source of truth** for result deduplication logic.
/// All search paths (native, WASM, streaming) should use this to merge results.
///
/// # Deduplication Strategy
///
/// When a document appears multiple times (e.g., matches in different sections):
/// 1. Keep the result with the **best match_type** (Title > Section > Content)
/// 2. If same match_type, keep the **highest score**
/// 3. If same score, keep the **first occurrence** (deterministic)
///
/// # Example
///
/// ```ignore
/// let mut merger = ResultMerger::new(docs);
/// for result in t1_results {
///     merger.merge(result);
/// }
/// for result in t2_results {
///     merger.merge(result);
/// }
/// let final_results = merger.into_sorted(limit);
/// ```
#[cfg(feature = "wasm")]
pub struct ResultMerger<'a> {
    /// Map from doc_id to best result for that document.
    /// Using `usize` (doc_id) as key ensures no composite key bugs.
    map: HashMap<usize, SearchResult>,
    /// Reference to document metadata for ranking comparison.
    docs: &'a [SearchDoc],
}

#[cfg(feature = "wasm")]
impl<'a> ResultMerger<'a> {
    /// Create a new result merger.
    ///
    /// # Arguments
    ///
    /// * `docs` - Document metadata for ranking comparisons
    pub fn new(docs: &'a [SearchDoc]) -> Self {
        Self {
            map: HashMap::new(),
            docs,
        }
    }

    /// Create a new result merger with pre-allocated capacity.
    ///
    /// Use this when you know the approximate number of unique documents.
    #[allow(dead_code)]
    pub fn with_capacity(docs: &'a [SearchDoc], capacity: usize) -> Self {
        Self {
            map: HashMap::with_capacity(capacity),
            docs,
        }
    }

    /// Merge a single result, keeping the best per doc_id.
    ///
    /// "Best" is determined by `compare_results()`:
    /// 1. Lower match_type wins (Title < Section < Content)
    /// 2. Higher score wins (within same match_type)
    /// 3. Alphabetical title (for determinism)
    /// 4. Lower doc_id (final tie-breaker)
    ///
    /// # Arguments
    ///
    /// * `result` - The search result to merge
    pub fn merge(&mut self, result: SearchResult) {
        self.map
            .entry(result.doc_id) // Only doc_id as key - prevents composite key bugs
            .and_modify(|existing| {
                // Replace if new result is better
                if compare_results(&result, existing, self.docs) == Ordering::Less {
                    *existing = result.clone();
                }
            })
            .or_insert(result);
    }

    /// Merge multiple results at once.
    ///
    /// Equivalent to calling `merge()` for each result.
    pub fn merge_all(&mut self, results: impl IntoIterator<Item = SearchResult>) {
        for result in results {
            self.merge(result);
        }
    }

    /// Get the current number of unique documents.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Check if the merger is empty.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Check if a document is already in the merger.
    #[allow(dead_code)]
    pub fn contains(&self, doc_id: usize) -> bool {
        self.map.contains_key(&doc_id)
    }

    /// Get the set of document IDs currently in the merger.
    #[allow(dead_code)]
    pub fn doc_ids(&self) -> impl Iterator<Item = usize> + '_ {
        self.map.keys().copied()
    }

    /// Convert to a sorted, truncated vector of results.
    ///
    /// Results are sorted by `compare_results()` ordering:
    /// 1. Match type (ascending: Title first)
    /// 2. Score (descending: highest first)
    /// 3. Title (ascending: alphabetical)
    /// 4. Doc ID (ascending: deterministic)
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of results to return
    #[allow(dead_code)]
    pub fn into_sorted(self, limit: usize) -> Vec<SearchResult> {
        let mut results: Vec<_> = self.map.into_values().collect();
        results.sort_by(|a, b| compare_results(a, b, self.docs));
        results.truncate(limit);
        results
    }

    /// Get results as an unsorted iterator.
    ///
    /// Use this when you need to process results without the sorting overhead.
    #[allow(dead_code)]
    pub fn into_unsorted(self) -> impl Iterator<Item = SearchResult> {
        self.map.into_values()
    }

    /// Get a sorted, truncated snapshot of current results.
    ///
    /// Unlike `into_sorted()`, this borrows rather than consuming the merger,
    /// allowing multiple snapshots to be taken (e.g., for progressive callbacks).
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of results to return
    pub fn get_sorted(&self, limit: usize) -> Vec<SearchResult> {
        let mut results: Vec<_> = self.map.values().cloned().collect();
        results.sort_by(|a, b| compare_results(a, b, self.docs));
        results.truncate(limit);
        results
    }
}

#[cfg(all(test, feature = "wasm"))]
mod tests {
    use super::*;
    use crate::types::MatchType;

    fn make_result(doc_id: usize, score: f64, match_type: MatchType) -> SearchResult {
        SearchResult {
            doc_id,
            score,
            section_idx: 0,
            tier: 1,
            match_type,
        }
    }

    #[test]
    fn test_merger_keeps_unique_docs() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        merger.merge(make_result(0, 100.0, MatchType::Title));
        merger.merge(make_result(1, 80.0, MatchType::Section));
        merger.merge(make_result(2, 60.0, MatchType::Content));

        assert_eq!(merger.len(), 3);
    }

    #[test]
    fn test_merger_deduplicates_by_doc_id() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        // Same doc_id, different sections (simulates the bug we fixed)
        merger.merge(make_result(0, 50.0, MatchType::Content));
        merger.merge(make_result(0, 100.0, MatchType::Title)); // Better match_type

        assert_eq!(merger.len(), 1, "Should have only one result per doc_id");

        let results = merger.into_sorted(10);
        assert_eq!(results[0].match_type, MatchType::Title, "Should keep Title (better)");
    }

    #[test]
    fn test_merger_keeps_better_match_type() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        // Content match first, then Title match for same doc
        merger.merge(make_result(0, 100.0, MatchType::Content));
        merger.merge(make_result(0, 50.0, MatchType::Title)); // Lower score but better type

        let results = merger.into_sorted(10);
        assert_eq!(results[0].match_type, MatchType::Title);
        assert_eq!(results[0].score, 50.0);
    }

    #[test]
    fn test_merger_keeps_higher_score_in_same_bucket() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        merger.merge(make_result(0, 50.0, MatchType::Section));
        merger.merge(make_result(0, 100.0, MatchType::Section)); // Same type, higher score

        let results = merger.into_sorted(10);
        assert_eq!(results[0].score, 100.0);
    }

    #[test]
    fn test_merger_respects_limit() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        for i in 0..10 {
            merger.merge(make_result(i, 100.0 - i as f64, MatchType::Content));
        }

        let results = merger.into_sorted(5);
        assert_eq!(results.len(), 5);
    }

    #[test]
    fn test_merger_sorted_by_match_type_then_score() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        merger.merge(make_result(0, 100.0, MatchType::Content));
        merger.merge(make_result(1, 50.0, MatchType::Title));
        merger.merge(make_result(2, 75.0, MatchType::Section));

        let results = merger.into_sorted(10);

        // Should be sorted: Title, Section, Content
        assert_eq!(results[0].match_type, MatchType::Title);
        assert_eq!(results[1].match_type, MatchType::Section);
        assert_eq!(results[2].match_type, MatchType::Content);
    }

    #[test]
    fn test_merger_empty() {
        let docs = vec![];
        let merger = ResultMerger::new(&docs);

        assert!(merger.is_empty());
        assert_eq!(merger.len(), 0);

        let results = merger.into_sorted(10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_merger_merge_all() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        let results = vec![
            make_result(0, 100.0, MatchType::Title),
            make_result(1, 80.0, MatchType::Section),
            make_result(0, 50.0, MatchType::Content), // Duplicate, should be ignored
        ];

        merger.merge_all(results);

        assert_eq!(merger.len(), 2);
    }

    #[test]
    fn test_merger_contains() {
        let docs = vec![];
        let mut merger = ResultMerger::new(&docs);

        merger.merge(make_result(5, 100.0, MatchType::Title));

        assert!(merger.contains(5));
        assert!(!merger.contains(10));
    }
}
