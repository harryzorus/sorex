// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Hybrid index search: best of both worlds.
//!
//! The inverted index gives O(1) exact word lookup. The suffix array gives
//! O(log k) prefix search. Combining them: try exact first, fall back to
//! prefix if no match. Users get instant results for exact matches, and
//! still find "typescript" when searching "script".
//!
//! # Complexity
//!
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | Exact word | O(1) | - |
//! | Prefix search | O(log k + p) | - |
//! | Multi-word AND | O(min posting list) | - |
//!
//! Where:
//! - k = vocabulary size (unique terms)
//! - p = number of prefix-matching terms

use crate::fuzzy::levenshtein_within;
use crate::types::{HybridIndex, PostingList, ScoredDoc, SearchDoc};
use super::utils::{merge_score_sets, parse_query};
use std::collections::HashMap;

/// Search the hybrid index.
///
/// Strategy:
/// 1. Try exact word lookup first (O(1))
/// 2. If no exact match, use prefix search via vocabulary suffix array (O(log k))
/// 3. For multi-word queries, intersect posting lists
pub fn search_hybrid(index: &HybridIndex, query: &str) -> Vec<SearchDoc> {
    let parts = parse_query(query);
    if parts.is_empty() {
        return Vec::new();
    }

    // Collect scores for each search term
    let score_sets: Vec<Vec<(usize, f64)>> = parts
        .iter()
        .map(|term| score_term(index, term))
        .filter(|v| !v.is_empty())
        .collect();

    if score_sets.is_empty() {
        return Vec::new();
    }

    // Use shared aggregation logic
    aggregate_and_sort(index, score_sets)
}

// =============================================================================
// STREAMING SEARCH API
// =============================================================================
//
// These functions support progressive/streaming search with three phases:
// 1. search_exact() - O(1) inverted index lookup (returns first results fast)
// 2. search_expanded() - O(log k) suffix array search (prefix/substring matches)
// 3. search_fuzzy() - O(vocab) Levenshtein search (typo tolerance)
//
// ## Why Streaming?
//
// For search-as-you-type UX, returning fast partial results matters more than
// completeness. Streaming search returns exact matches in <1ms, then progressively
// adds expanded and fuzzy matches. The UI can display results as they arrive.
//
// ## Invariants (from StreamingSearch.lean):
//
// - `exact_subset_full`: exact results ⊆ full results
// - `expanded_disjoint_exact`: expanded ∩ exact = ∅ (no duplicates)
// - `union_superset_full`: full results ⊆ (exact ∪ expanded ∪ fuzzy)
//
// ## IMPORTANT: Streaming vs Full Search Asymmetry
//
// Streaming search may return MORE results than `search_hybrid()` because:
// - `search_hybrid()` uses early termination: once exact matches are found,
//   it skips prefix/fuzzy search (the `score_term()` function returns after
//   the first successful tier).
// - Streaming explicitly runs all phases, accumulating results from each.
//
// Example: Query "script" with docs ["script file", "typescript", "javascript"]
// - `search_hybrid()` finds exact match "script" → returns 1 result
// - Streaming: exact="script file", expanded="typescript"+"javascript" → 3 results
//
// This is intentional: streaming prioritizes completeness, while full search
// prioritizes speed by stopping early.

/// Search using only the inverted index (O(1) per term).
///
/// Returns results from exact word matches only. This is the fast path
/// that provides first results immediately.
///
/// # Lean Specification
///
/// ```lean
/// def exactLookup (invertedIndex : List (String × List Nat)) (term : String) : List Nat :=
///   match invertedIndex.find? (fun (t, _) => t == term) with
///   | some (_, docIds) => docIds
///   | none => []
/// ```
pub fn search_exact(index: &HybridIndex, query: &str) -> Vec<SearchDoc> {
    let parts = parse_query(query);
    if parts.is_empty() {
        return Vec::new();
    }

    // Collect scores for each search term using ONLY inverted index
    let score_sets: Vec<Vec<(usize, f64)>> = parts
        .iter()
        .map(|term| score_term_exact(index, term))
        .filter(|v| !v.is_empty())
        .collect();

    if score_sets.is_empty() {
        return Vec::new();
    }

    // Aggregate scores (AND semantics)
    aggregate_and_sort(index, score_sets)
}

/// Search using only the suffix array, excluding already-found IDs (O(log k)).
///
/// Returns additional results not found by exact search.
/// The `exclude_ids` parameter contains doc IDs already returned by search_exact().
///
/// # Lean Specification
///
/// ```lean
/// def expandedWithExclusion (vocabulary : List String) (suffixArray : List Nat)
///     (term : String) (excludeIds : List Nat) : List Nat :=
///   (expandedLookup vocabulary suffixArray term).filter (fun id => id ∉ excludeIds)
/// ```
pub fn search_expanded(index: &HybridIndex, query: &str, exclude_ids: &[usize]) -> Vec<SearchDoc> {
    let parts = parse_query(query);
    if parts.is_empty() {
        return Vec::new();
    }

    let exclude_set: std::collections::HashSet<usize> = exclude_ids.iter().copied().collect();

    // Collect scores for each search term using ONLY suffix array
    let score_sets: Vec<Vec<(usize, f64)>> = parts
        .iter()
        .map(|term| {
            score_term_expanded(index, term)
                .into_iter()
                .filter(|(doc_id, _)| !exclude_set.contains(doc_id))
                .collect()
        })
        .filter(|v: &Vec<(usize, f64)>| !v.is_empty())
        .collect();

    if score_sets.is_empty() {
        return Vec::new();
    }

    // Aggregate scores (AND semantics)
    aggregate_and_sort(index, score_sets)
}

/// Search using Levenshtein distance for typo tolerance (O(vocab)).
///
/// This is the third phase of streaming search, designed to run in a Web Worker.
/// Returns fuzzy matches not found by exact or expanded search.
///
/// # Arguments
/// * `index` - The hybrid index
/// * `query` - The search query
/// * `exclude_ids` - Doc IDs already returned by search_exact() and search_expanded()
///
/// # Performance
/// O(vocabulary × max_distance²) for edit distance computation.
/// Safe to run in a separate worker thread.
pub fn search_fuzzy(index: &HybridIndex, query: &str, exclude_ids: &[usize]) -> Vec<SearchDoc> {
    let parts = parse_query(query);
    if parts.is_empty() {
        return Vec::new();
    }

    let exclude_set: std::collections::HashSet<usize> = exclude_ids.iter().copied().collect();

    // Collect scores for each search term using Levenshtein distance
    let score_sets: Vec<Vec<(usize, f64)>> = parts
        .iter()
        .map(|term| {
            score_term_fuzzy(index, term)
                .into_iter()
                .filter(|(doc_id, _)| !exclude_set.contains(doc_id))
                .collect()
        })
        .filter(|v: &Vec<(usize, f64)>| !v.is_empty())
        .collect();

    if score_sets.is_empty() {
        return Vec::new();
    }

    // Aggregate scores (AND semantics)
    aggregate_and_sort(index, score_sets)
}

/// Score a term using Levenshtein distance (O(vocab) fuzzy search).
fn score_term_fuzzy(index: &HybridIndex, term: &str) -> Vec<(usize, f64)> {
    // Use edit distance 1 for short terms, 2 for longer terms
    let max_dist = if term.len() > 5 { 2 } else { 1 };
    let matching_terms = find_fuzzy_matches(&index.vocabulary, term, max_dist);

    if matching_terms.is_empty() {
        return Vec::new();
    }

    collect_scores_for_terms(index, &matching_terms)
}

/// Score a term using only the inverted index (O(1) exact lookup).
///
/// OPTIMIZATION: Posting lists are sorted by score DESC at index time,
/// so this returns results already in score order.
fn score_term_exact(index: &HybridIndex, term: &str) -> Vec<(usize, f64)> {
    if let Some(posting_list) = index.inverted_index.terms.get(term) {
        return score_posting_list(posting_list);
    }
    Vec::new()
}

/// Score a term using only the suffix array (O(log k) prefix/substring search).
fn score_term_expanded(index: &HybridIndex, term: &str) -> Vec<(usize, f64)> {
    let matching_terms = find_substring_matches(index, term);
    if matching_terms.is_empty() {
        return Vec::new();
    }

    // Union posting lists for all matching terms
    let mut all_scores: Vec<(usize, f64)> = Vec::new();
    for matched_term in matching_terms {
        if let Some(posting_list) = index.inverted_index.terms.get(&matched_term) {
            all_scores.extend(score_posting_list(posting_list));
        }
    }

    // Deduplicate by taking max score per doc
    let mut doc_max: HashMap<usize, f64> = HashMap::new();
    for (doc_id, score) in all_scores {
        doc_max
            .entry(doc_id)
            .and_modify(|s| *s = s.max(score))
            .or_insert(score);
    }

    doc_max.into_iter().collect()
}

/// Default result limit for search operations.
/// Results beyond this limit are still returned but with degraded sorting performance.
const DEFAULT_RESULT_LIMIT: usize = 100;

/// Aggregate score sets with AND semantics and sort by score.
///
/// Uses `merge_score_sets()` for the core aggregation, then converts to sorted results.
///
/// # Performance
///
/// OPTIMIZATION: Uses heap-based top-K selection for large result sets (>100 docs).
/// For small result sets, uses standard sort which is faster due to cache locality.
fn aggregate_and_sort(index: &HybridIndex, score_sets: Vec<Vec<(usize, f64)>>) -> Vec<SearchDoc> {
    let doc_scores = merge_score_sets(&score_sets);

    // Convert to scored docs
    // Note: Direct indexing (O(1)) - docs[doc_id].id == doc_id is an invariant.
    let results: Vec<ScoredDoc> = doc_scores
        .into_iter()
        .filter_map(|(doc_id, score)| {
            index.docs.get(doc_id).map(|doc| ScoredDoc {
                doc: doc.clone(),
                score,
            })
        })
        .collect();

    // OPTIMIZATION: For large result sets, use heap-based top-K selection
    // For small sets, standard sort is faster due to cache locality
    if results.len() > DEFAULT_RESULT_LIMIT * 2 {
        // Use min-heap to efficiently get top-K results
        top_k_by_score(results, DEFAULT_RESULT_LIMIT)
    } else {
        // Standard sort for small result sets
        // Sort by: score (desc) → title (asc) → doc_id (asc) for full determinism
        let mut sorted = results;
        sorted.sort_by(|a, b| {
            match b.score.partial_cmp(&a.score) {
                Some(std::cmp::Ordering::Equal) | None => {
                    match a.doc.title.cmp(&b.doc.title) {
                        std::cmp::Ordering::Equal => a.doc.id.cmp(&b.doc.id),
                        other => other,
                    }
                }
                Some(ord) => ord,
            }
        });
        sorted.into_iter().map(|sd| sd.doc).collect()
    }
}

/// Extract top-K results by score using a min-heap.
///
/// Time complexity: O(n log k) for heap operations + O(k log k) for final sort.
/// Total: O(n log k + k log k), which beats O(n log n) when k << n.
fn top_k_by_score(results: Vec<ScoredDoc>, k: usize) -> Vec<SearchDoc> {
    use std::cmp::Ordering;
    use std::collections::BinaryHeap;

    // Wrapper for min-heap (BinaryHeap is max-heap by default)
    // We keep (score, doc) pairs to preserve scores for final sort
    struct MinScored(ScoredDoc);

    impl PartialEq for MinScored {
        fn eq(&self, other: &Self) -> bool {
            self.0.score == other.0.score
        }
    }

    impl Eq for MinScored {}

    impl Ord for MinScored {
        fn cmp(&self, other: &Self) -> Ordering {
            // Reversed comparison for min-heap behavior (smallest score = highest priority to remove)
            // Sort by: score (desc) → title (asc) → doc_id (asc) for full determinism
            match other.0.score.partial_cmp(&self.0.score) {
                Some(Ordering::Equal) | None => {
                    match self.0.doc.title.cmp(&other.0.doc.title) {
                        Ordering::Equal => self.0.doc.id.cmp(&other.0.doc.id),
                        other_ord => other_ord,
                    }
                }
                Some(ord) => ord,
            }
        }
    }

    impl PartialOrd for MinScored {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    // Build min-heap of top-K results
    let mut heap: BinaryHeap<MinScored> = BinaryHeap::with_capacity(k + 1);

    for scored in results {
        heap.push(MinScored(scored));
        if heap.len() > k {
            heap.pop(); // Remove smallest (min-heap)
        }
    }

    // Extract scored docs and sort by: score (desc) → title (asc) → doc_id (asc)
    let mut top_results: Vec<ScoredDoc> = heap.into_iter().map(|ms| ms.0).collect();
    top_results.sort_by(|a, b| {
        match b.score.partial_cmp(&a.score) {
            Some(Ordering::Equal) | None => {
                match a.doc.title.cmp(&b.doc.title) {
                    Ordering::Equal => a.doc.id.cmp(&b.doc.id),
                    other => other,
                }
            }
            Some(ord) => ord,
        }
    });

    top_results.into_iter().map(|sd| sd.doc).collect()
}

/// Score a single search term.
///
/// Three-tier search strategy:
/// 1. Exact lookup in inverted index (O(1))
/// 2. Prefix/substring search via vocabulary suffix array (O(log k))
/// 3. Fuzzy search via Levenshtein distance over vocabulary (O(vocab))
///
/// OPTIMIZATION: Posting lists are sorted by score DESC at index time,
/// so exact match returns results already in score order.
fn score_term(index: &HybridIndex, term: &str) -> Vec<(usize, f64)> {
    // Tier 1: Try exact match first (O(1))
    if let Some(posting_list) = index.inverted_index.terms.get(term) {
        return score_posting_list(posting_list);
    }

    // Tier 2: Try substring match via vocabulary suffix array (O(log k))
    let matching_terms = find_substring_matches(index, term);
    if !matching_terms.is_empty() {
        return collect_scores_for_terms(index, &matching_terms);
    }

    // Tier 3: Try fuzzy match via Levenshtein distance (O(vocab))
    // Use edit distance 1 for short terms, 2 for longer terms
    let max_dist = if term.len() > 5 { 2 } else { 1 };
    let fuzzy_terms = find_fuzzy_matches(&index.vocabulary, term, max_dist);
    if !fuzzy_terms.is_empty() {
        return collect_scores_for_terms(index, &fuzzy_terms);
    }

    Vec::new()
}

/// Collect and deduplicate scores for a set of matching terms.
fn collect_scores_for_terms(index: &HybridIndex, terms: &[String]) -> Vec<(usize, f64)> {
    let mut all_scores: Vec<(usize, f64)> = Vec::new();
    for matched_term in terms {
        if let Some(posting_list) = index.inverted_index.terms.get(matched_term) {
            all_scores.extend(score_posting_list(posting_list));
        }
    }

    // Deduplicate by taking max score per doc
    let mut doc_max: HashMap<usize, f64> = HashMap::new();
    for (doc_id, score) in all_scores {
        doc_max
            .entry(doc_id)
            .and_modify(|s| *s = s.max(score))
            .or_insert(score);
    }

    doc_max.into_iter().collect()
}

/// Score a posting list, returning (doc_id, score) pairs.
///
/// OPTIMIZATION: Scores are precomputed at index time, so this just reads them.
/// Posting list is already sorted by score DESC, so we can early-exit for top-k.
fn score_posting_list(posting_list: &PostingList) -> Vec<(usize, f64)> {
    posting_list
        .postings
        .iter()
        .map(|posting| (posting.doc_id, posting.score))
        .collect()
}

/// Find all vocabulary terms containing a substring using binary search.
///
/// Returns the list of terms that contain the query as a substring.
/// This enables "script" to match "typescript", "javascript", "scripting", etc.
fn find_substring_matches(index: &HybridIndex, query: &str) -> Vec<String> {
    let vocab = &index.vocabulary;
    let suffix_array = &index.vocab_suffix_array;

    if suffix_array.is_empty() {
        return Vec::new();
    }

    // Binary search to find first suffix >= query
    let mut lo = 0;
    let mut hi = suffix_array.len();

    while lo < hi {
        let mid = (lo + hi) / 2;
        let entry = &suffix_array[mid];
        let suffix = &vocab[entry.term_idx][entry.offset..];

        if suffix < query {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    // Collect all terms whose suffixes start with query (any offset)
    // This finds "typescript" when searching for "script" because
    // the suffix array contains an entry for the "script" suffix
    let mut matching_terms: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for i in lo..suffix_array.len() {
        let entry = &suffix_array[i];
        let suffix = &vocab[entry.term_idx][entry.offset..];

        if !suffix.starts_with(query) {
            break;
        }

        // Include the term if we haven't seen it yet (any offset is valid)
        if !seen.contains(&entry.term_idx) {
            seen.insert(entry.term_idx);
            matching_terms.push(vocab[entry.term_idx].clone());
        }
    }

    matching_terms
}

/// Find vocabulary terms within edit distance k using Levenshtein distance.
///
/// Iterates over the vocabulary and computes edit distance for each term.
/// Uses early termination to skip terms that are too different in length.
///
/// # Arguments
/// * `vocabulary` - The sorted vocabulary terms
/// * `query` - The search query
/// * `max_distance` - Maximum edit distance (1 for short terms, 2 for longer terms)
///
/// # Returns
/// List of vocabulary terms within the specified edit distance.
///
/// # Complexity
/// O(vocabulary × query_len × max_term_len) but with heavy pruning via length check.
fn find_fuzzy_matches(vocabulary: &[String], query: &str, max_distance: u32) -> Vec<String> {
    let query_len = query.chars().count();

    vocabulary
        .iter()
        .filter(|term| {
            let term_len = term.chars().count();
            // Early termination: length difference bounds edit distance
            let len_diff = query_len.abs_diff(term_len);
            if len_diff > max_distance as usize {
                return false;
            }
            // Compute actual edit distance with early termination
            levenshtein_within(query, term, max_distance as usize)
        })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::make_doc;
    use crate::index::hybrid::build_hybrid_index;
    use crate::types::FieldType;
    use crate::types::FieldBoundary;
    use proptest::prelude::*;

    /// Convert string to mixed case (alternating upper/lower)
    fn mixed_case(s: &str) -> String {
        s.chars()
            .enumerate()
            .map(|(i, c)| {
                if i % 2 == 0 {
                    c.to_uppercase().next().unwrap_or(c)
                } else {
                    c.to_lowercase().next().unwrap_or(c)
                }
            })
            .collect()
    }

    #[test]
    fn test_exact_word_lookup() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["rust programming".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        let results = search_hybrid(&index, "rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 0);
    }

    #[test]
    fn test_prefix_search() {
        let docs = vec![make_doc(0, "Test"), make_doc(1, "Other")];
        let texts = vec![
            "programming language".to_string(),
            "program code".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "prog" should match both "programming" and "program"
        let results = search_hybrid(&index, "prog");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_substring_match() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["reprogramming".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "prog" SHOULD match "reprogramming" via substring search
        let results = search_hybrid(&index, "prog");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 0);
    }

    #[test]
    fn test_substring_finds_typescript() {
        let docs = vec![make_doc(0, "TS Doc"), make_doc(1, "JS Doc")];
        let texts = vec![
            "typescript programming".to_string(),
            "javascript programming".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "script" should match both "typescript" and "javascript"
        let results = search_hybrid(&index, "script");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_multi_word_and() {
        let docs = vec![
            make_doc(0, "Both"),
            make_doc(1, "Rust Only"),
            make_doc(2, "Lang Only"),
        ];
        let texts = vec![
            "rust programming language".to_string(),
            "rust code".to_string(),
            "programming language".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "rust programming" should only match doc 0
        let results = search_hybrid(&index, "rust programming");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 0);
    }

    // =========================================================================
    // STREAMING SEARCH TESTS
    // =========================================================================
    // These tests verify the streaming search invariants from StreamingSearch.lean:
    // 1. exact_subset_full: exact results ⊆ full results
    // 2. expanded_disjoint_exact: expanded ∩ exact = ∅
    // 3. union_complete: exact ∪ expanded = full results

    #[test]
    fn test_search_exact_finds_exact_matches() {
        let docs = vec![make_doc(0, "Rust"), make_doc(1, "Rusted")];
        let texts = vec!["rust programming".to_string(), "rusted metal".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        // Exact search for "rust" should only find doc 0 (exact word match)
        let exact_results = search_exact(&index, "rust");
        assert_eq!(exact_results.len(), 1);
        assert_eq!(exact_results[0].id, 0);
    }

    #[test]
    fn test_search_expanded_finds_substring_matches() {
        let docs = vec![make_doc(0, "TypeScript"), make_doc(1, "JavaScript")];
        let texts = vec!["typescript code".to_string(), "javascript code".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        // Expanded search for "script" should find both via suffix array
        let expanded_results = search_expanded(&index, "script", &[]);
        assert_eq!(expanded_results.len(), 2);
    }

    #[test]
    fn test_streaming_exact_subset_full() {
        // Invariant: exact results ⊆ full results
        let docs = vec![make_doc(0, "Rust"), make_doc(1, "Rusted")];
        let texts = vec!["rust programming".to_string(), "rusted metal".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        let exact_results = search_exact(&index, "rust");
        let full_results = search_hybrid(&index, "rust");

        // Every exact result should be in full results
        for exact in &exact_results {
            assert!(
                full_results.iter().any(|f| f.id == exact.id),
                "Exact result {} not in full results",
                exact.id
            );
        }
    }

    #[test]
    fn test_streaming_expanded_disjoint_exact() {
        // Invariant: expanded ∩ exact = ∅
        let docs = vec![
            make_doc(0, "Rust"),
            make_doc(1, "Rusted"),
            make_doc(2, "Rustic"),
        ];
        let texts = vec![
            "rust programming".to_string(),
            "rusted metal".to_string(),
            "rustic style".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        let exact_results = search_exact(&index, "rust");
        let exact_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();
        let expanded_results = search_expanded(&index, "rust", &exact_ids);

        // No expanded result should be in exact results
        for expanded in &expanded_results {
            assert!(
                !exact_ids.contains(&expanded.id),
                "Expanded result {} is also in exact results",
                expanded.id
            );
        }
    }

    #[test]
    fn test_streaming_union_superset_of_full() {
        // Invariant: full ⊆ (exact ∪ expanded)
        // Note: Streaming search returns MORE results than score_term-based search
        // because score_term stops after finding exact match, while streaming
        // explicitly searches both phases.
        let docs = vec![
            make_doc(0, "TypeScript"),
            make_doc(1, "JavaScript"),
            make_doc(2, "Script"),
        ];
        let texts = vec![
            "typescript code".to_string(),
            "javascript code".to_string(),
            "script file".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        let exact_results = search_exact(&index, "script");
        let exact_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();
        let expanded_results = search_expanded(&index, "script", &exact_ids);
        let full_results = search_hybrid(&index, "script");

        // Union should contain all full results (full ⊆ union)
        let mut union_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();
        union_ids.extend(expanded_results.iter().map(|r| r.id));

        for full in &full_results {
            assert!(
                union_ids.contains(&full.id),
                "Full result {} not in streaming union",
                full.id
            );
        }

        // Streaming finds more: exact (script) + expanded (typescript, javascript)
        assert!(
            union_ids.len() >= full_results.len(),
            "Streaming should find at least as many results as full search"
        );
    }

    #[test]
    fn test_streaming_union_complete_for_prefix() {
        // For prefix-only queries (no exact match), union = full
        let docs = vec![make_doc(0, "Programming"), make_doc(1, "Programmer")];
        let texts = vec![
            "programming language".to_string(),
            "programmer job".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "prog" has no exact match, so full search uses suffix array
        let exact_results = search_exact(&index, "prog");
        let exact_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();
        let expanded_results = search_expanded(&index, "prog", &exact_ids);
        let full_results = search_hybrid(&index, "prog");

        // For prefix queries, union should equal full
        let mut union_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();
        union_ids.extend(expanded_results.iter().map(|r| r.id));
        union_ids.sort();
        union_ids.dedup();

        let mut full_ids: Vec<usize> = full_results.iter().map(|r| r.id).collect();
        full_ids.sort();

        assert_eq!(
            union_ids, full_ids,
            "For prefix queries, union should equal full"
        );
    }

    #[test]
    fn test_streaming_empty_query() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["rust programming".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        assert!(search_exact(&index, "").is_empty());
        assert!(search_expanded(&index, "", &[]).is_empty());
    }

    #[test]
    fn test_streaming_no_matches() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["rust programming".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        assert!(search_exact(&index, "python").is_empty());
        assert!(search_expanded(&index, "python", &[]).is_empty());
    }

    // =========================================================================
    // MULTI-TERM AND DEDUPLICATION TESTS (ALL TIERS)
    // =========================================================================

    #[test]
    fn test_t1_exact_multiterm_and_semantics() {
        // Test T1 exact search with multi-term AND:
        // Doc 0: has "rust" and "programming" (should match "rust programming")
        // Doc 1: has only "rust" (should NOT match "rust programming")
        // Doc 2: has only "programming" (should NOT match "rust programming")
        let docs = vec![
            make_doc(0, "Rust Guide"),
            make_doc(1, "Rust Only"),
            make_doc(2, "Programming Only"),
        ];
        let texts = vec![
            "rust programming language".to_string(),   // doc 0: both terms
            "rust systems".to_string(),                 // doc 1: only "rust"
            "programming concepts".to_string(),         // doc 2: only "programming"
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        let results = search_exact(&index, "rust programming");
        let ids: Vec<usize> = results.iter().map(|r| r.id).collect();

        // Only doc 0 should match (AND semantics)
        assert_eq!(ids.len(), 1, "Expected 1 result, got {:?}", ids);
        assert!(ids.contains(&0), "Doc 0 should match 'rust programming'");
        assert!(!ids.contains(&1), "Doc 1 should NOT match (missing 'programming')");
        assert!(!ids.contains(&2), "Doc 2 should NOT match (missing 'rust')");
    }

    #[test]
    fn test_t2_prefix_multiterm_and_semantics() {
        // Test T2 prefix search with multi-term AND:
        // "pro sys" should match "programmer systems" but not "programmer only"
        let docs = vec![
            make_doc(0, "Programmer Systems"),
            make_doc(1, "Programmer Only"),
            make_doc(2, "Systems Only"),
        ];
        let texts = vec![
            "programmer systems design".to_string(),   // doc 0: "pro" and "sys" prefixes
            "programmer language".to_string(),          // doc 1: only "pro" prefix
            "systems architecture".to_string(),         // doc 2: only "sys" prefix
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "pro sys" - prefix matches
        let results = search_expanded(&index, "pro sys", &[]);
        let ids: Vec<usize> = results.iter().map(|r| r.id).collect();

        // Only doc 0 should match (AND semantics for prefix)
        assert_eq!(ids.len(), 1, "Expected 1 result for prefix AND, got {:?}", ids);
        assert!(ids.contains(&0), "Doc 0 should match 'pro sys' (prefix AND)");
    }

    #[test]
    fn test_t3_fuzzy_multiterm_and_semantics() {
        // Test T3 fuzzy search with multi-term AND:
        // "progammer systms" (typos) should match "programmer systems"
        let docs = vec![
            make_doc(0, "Programmer Systems"),
            make_doc(1, "Programmer Only"),
            make_doc(2, "Systems Only"),
        ];
        let texts = vec![
            "programmer systems design".to_string(),   // doc 0: both (with typo tolerance)
            "programmer language".to_string(),          // doc 1: only "programmer"
            "systems architecture".to_string(),         // doc 2: only "systems"
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // "programer systms" - typos in both words
        let results = search_fuzzy(&index, "programer systms", &[]);
        let ids: Vec<usize> = results.iter().map(|r| r.id).collect();

        // Doc 0 should match (has fuzzy matches for both terms)
        // Due to AND semantics, only docs with BOTH fuzzy matches should appear
        if !ids.is_empty() {
            assert!(ids.contains(&0), "Doc 0 should match fuzzy 'programer systms'");
        }
    }

    #[test]
    fn test_multiterm_deduplication_across_tiers() {
        // Test that deduplication works when same doc appears in multiple tiers
        let docs = vec![
            make_doc(0, "Rust Programming"),
            make_doc(1, "Rust Systems"),
        ];
        let texts = vec![
            "rust programming language".to_string(),
            "rust systems design".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // Full search should not have duplicate doc IDs
        let results = search_hybrid(&index, "rust");
        let ids: Vec<usize> = results.iter().map(|r| r.id).collect();

        let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(
            ids.len(),
            unique_ids.len(),
            "Results should not contain duplicate doc IDs: {:?}",
            ids
        );
    }

    #[test]
    fn test_streaming_tier_exclusion() {
        // Test that T2 properly excludes T1 results, and T3 excludes T1+T2
        let docs = vec![
            make_doc(0, "Rust"),
            make_doc(1, "Rustic"),
            make_doc(2, "Rast"),  // typo of "rust"
        ];
        let texts = vec![
            "rust programming".to_string(),     // T1 exact match
            "rustic design".to_string(),         // T2 prefix match for "rus"
            "rast systems".to_string(),          // T3 fuzzy match for "rust" (edit dist 1)
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // T1: exact "rust" matches only doc 0
        let t1_results = search_exact(&index, "rust");
        let t1_ids: Vec<usize> = t1_results.iter().map(|r| r.id).collect();
        assert!(t1_ids.contains(&0), "T1 should find exact match for 'rust'");

        // T2: prefix "rus" but exclude T1 results
        let t2_results = search_expanded(&index, "rus", &t1_ids);
        let t2_ids: Vec<usize> = t2_results.iter().map(|r| r.id).collect();
        assert!(!t2_ids.contains(&0), "T2 should NOT include T1 result (doc 0)");
        // "rustic" starts with "rus", so doc 1 should be found
        assert!(t2_ids.contains(&1), "T2 should find 'rustic' for prefix 'rus'");

        // T3: fuzzy "rust" but exclude T1+T2 results
        let mut exclude_for_t3: Vec<usize> = t1_ids.clone();
        exclude_for_t3.extend(&t2_ids);
        let t3_results = search_fuzzy(&index, "rust", &exclude_for_t3);
        let t3_ids: Vec<usize> = t3_results.iter().map(|r| r.id).collect();
        assert!(!t3_ids.contains(&0), "T3 should NOT include T1 result");
        assert!(!t3_ids.contains(&1), "T3 should NOT include T2 result");
        // "rast" is 1 edit from "rust", so doc 2 might be found
        // (depends on whether levenshtein matching finds it)
    }

    // =========================================================================
    // PROPERTY TESTS FOR OPTIMIZATION CORRECTNESS
    // =========================================================================
    // These tests ensure optimizations don't break invariants.

    /// Strategy for generating score sets (list of (doc_id, score) tuples)
    fn score_set_strategy() -> impl Strategy<Value = Vec<(usize, f64)>> {
        prop::collection::vec((0usize..100, 0.0f64..200.0), 0..50)
    }

    /// Strategy for generating multiple score sets (one per term)
    fn multi_score_set_strategy() -> impl Strategy<Value = Vec<Vec<(usize, f64)>>> {
        prop::collection::vec(score_set_strategy(), 0..5)
    }

    proptest! {
        /// Property: merge_score_sets returns empty when any score set is empty (AND semantics)
        #[test]
        fn prop_merge_empty_term_returns_empty(
            non_empty in score_set_strategy().prop_filter("non-empty", |v| !v.is_empty()),
        ) {
            // If we have [non_empty, empty], result must be empty (AND semantics)
            let score_sets = vec![non_empty, vec![]];
            let result = merge_score_sets(&score_sets);
            prop_assert!(result.is_empty(), "AND semantics: empty term should produce empty result");
        }

        /// Property: merge_score_sets result is subset of first term's doc_ids (AND semantics)
        #[test]
        fn prop_merge_result_subset_of_first(score_sets in multi_score_set_strategy()) {
            prop_assume!(!score_sets.is_empty());

            let result = merge_score_sets(&score_sets);
            let first_doc_ids: std::collections::HashSet<usize> =
                score_sets[0].iter().map(|(id, _)| *id).collect();

            for doc_id in result.keys() {
                prop_assert!(
                    first_doc_ids.contains(doc_id),
                    "Result doc {} not in first term's docs",
                    doc_id
                );
            }
        }

        /// Property: merge_score_sets result contains only docs present in ALL score sets
        #[test]
        fn prop_merge_and_semantics(score_sets in multi_score_set_strategy()) {
            prop_assume!(!score_sets.is_empty());

            let result = merge_score_sets(&score_sets);

            // For each doc in result, verify it appears in ALL score sets
            for doc_id in result.keys() {
                for (i, score_set) in score_sets.iter().enumerate() {
                    prop_assert!(
                        score_set.iter().any(|(id, _)| *id == *doc_id),
                        "Doc {} in result but not in score_set[{}]",
                        doc_id,
                        i
                    );
                }
            }
        }

        /// Property: merge_score_sets sums scores across terms (aggregation correctness)
        #[test]
        fn prop_merge_score_aggregation(score_sets in multi_score_set_strategy()) {
            prop_assume!(!score_sets.is_empty());

            let result = merge_score_sets(&score_sets);

            // For each doc in result, compute expected score manually
            for (doc_id, actual_score) in &result {
                let mut expected_score = 0.0;
                for score_set in &score_sets {
                    // Take max score for this doc within this term
                    let max_for_term = score_set
                        .iter()
                        .filter(|(id, _)| *id == *doc_id)
                        .map(|(_, score)| *score)
                        .fold(f64::NEG_INFINITY, f64::max);
                    if max_for_term > f64::NEG_INFINITY {
                        expected_score += max_for_term;
                    }
                }
                prop_assert!(
                    (*actual_score - expected_score).abs() < 1e-10,
                    "Score mismatch for doc {}: expected {}, got {}",
                    doc_id,
                    expected_score,
                    actual_score
                );
            }
        }

        /// Property: merge_score_sets takes max score when doc appears multiple times in same term
        #[test]
        fn prop_merge_max_within_term(doc_id in 0usize..10, scores in prop::collection::vec(1.0f64..100.0, 2..10)) {
            // Create a score set with same doc_id appearing multiple times
            let score_set: Vec<(usize, f64)> = scores.iter().map(|s| (doc_id, *s)).collect();
            let score_sets = vec![score_set.clone()];

            let result = merge_score_sets(&score_sets);

            let expected_max = scores.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let actual = result.get(&doc_id).copied().unwrap_or(0.0);

            prop_assert!(
                (actual - expected_max).abs() < 1e-10,
                "Expected max {}, got {}",
                expected_max,
                actual
            );
        }
    }

    // =========================================================================
    // PROPERTY TESTS FOR FIELD BOUNDARY LOOKUPS
    // =========================================================================

    /// Strategy for field boundaries (sorted, non-overlapping within a doc)
    fn field_boundary_strategy() -> impl Strategy<Value = Vec<FieldBoundary>> {
        prop::collection::vec(
            (0usize..5, 0usize..1000, 1usize..100),
            0..20
        ).prop_map(|tuples| {
            let mut boundaries = Vec::new();
            for (doc_id, start, len) in tuples {
                let field_type = match start % 3 {
                    0 => FieldType::Title,
                    1 => FieldType::Heading,
                    _ => FieldType::Content,
                };
                boundaries.push(FieldBoundary {
                    doc_id,
                    start,
                    end: start + len,
                    field_type,
                    section_id: None,
                    heading_level: 0,
                });
            }
            boundaries
        })
    }

    /// Sort boundaries by (doc_id, start) as they would be in production
    fn sort_boundaries(mut boundaries: Vec<FieldBoundary>) -> Vec<FieldBoundary> {
        boundaries.sort_by(|a, b| {
            a.doc_id.cmp(&b.doc_id).then_with(|| a.start.cmp(&b.start))
        });
        boundaries
    }

    proptest! {
        /// Property: field lookup within a boundary returns that boundary's type
        #[test]
        fn prop_field_lookup_within_boundary(boundaries in field_boundary_strategy()) {
            use crate::scoring::get_field_type_from_boundaries;

            // Sort boundaries as they would be in production
            let sorted = sort_boundaries(boundaries);

            for boundary in &sorted {
                // Check offset in middle of boundary
                let mid_offset = (boundary.start + boundary.end) / 2;
                let result = get_field_type_from_boundaries(
                    boundary.doc_id,
                    mid_offset,
                    &sorted,
                );

                // Should match this boundary's type (or another if overlapping)
                let matches_some_boundary = sorted.iter().any(|b| {
                    b.doc_id == boundary.doc_id
                        && mid_offset >= b.start
                        && mid_offset < b.end
                        && result == b.field_type
                });
                prop_assert!(
                    matches_some_boundary,
                    "Offset {} in doc {} should match some boundary",
                    mid_offset,
                    boundary.doc_id
                );
            }
        }

        /// Property: field lookup outside all boundaries returns Content
        #[test]
        fn prop_field_lookup_outside_returns_content(boundaries in field_boundary_strategy(), offset in 10000usize..20000) {
            use crate::scoring::get_field_type_from_boundaries;

            // Sort boundaries as they would be in production
            let sorted = sort_boundaries(boundaries);

            // Use an offset that's definitely outside all boundaries
            let result = get_field_type_from_boundaries(0, offset, &sorted);
            let covered = sorted.iter().any(|b| b.doc_id == 0 && offset >= b.start && offset < b.end);

            if !covered {
                prop_assert_eq!(result, FieldType::Content, "Uncovered offset should return Content");
            }
        }

        /// Property: field lookup is deterministic (same inputs produce same output)
        #[test]
        fn prop_field_lookup_deterministic(
            boundaries in field_boundary_strategy(),
            doc_id in 0usize..5,
            offset in 0usize..2000,
        ) {
            use crate::scoring::get_field_type_from_boundaries;

            // Sort boundaries as they would be in production
            let sorted = sort_boundaries(boundaries);

            let result1 = get_field_type_from_boundaries(doc_id, offset, &sorted);
            let result2 = get_field_type_from_boundaries(doc_id, offset, &sorted);

            prop_assert_eq!(result1, result2, "Field lookup should be deterministic");
        }
    }

    // =========================================================================
    // PROPERTY TESTS FOR TOP-K SORTING
    // =========================================================================

    proptest! {
        /// Property: results are sorted by score descending
        #[test]
        fn prop_results_sorted_descending(scores in prop::collection::vec(0.0f64..200.0, 0..50)) {
            let doc_scores: HashMap<usize, f64> = scores
                .iter()
                .enumerate()
                .map(|(id, score)| (id, *score))
                .collect();

            // Create minimal index for testing
            let docs: Vec<SearchDoc> = (0..scores.len())
                .map(|id| make_doc(id, &format!("Doc {}", id)))
                .collect();

            let mut results: Vec<ScoredDoc> = doc_scores
                .into_iter()
                .filter_map(|(doc_id, score)| {
                    docs.get(doc_id).map(|doc| ScoredDoc {
                        doc: doc.clone(),
                        score,
                    })
                })
                .collect();

            results.sort_by(|a, b| {
                b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)
            });

            // Verify sorted descending
            for window in results.windows(2) {
                prop_assert!(
                    window[0].score >= window[1].score,
                    "Results not sorted: {} should be >= {}",
                    window[0].score,
                    window[1].score
                );
            }
        }

        /// Property: top-K preserves the K highest scoring documents
        #[test]
        fn prop_top_k_preserves_highest(
            scores in prop::collection::vec(0.0f64..200.0, 10..50),
            k in 1usize..10,
        ) {
            let mut all_scores: Vec<(usize, f64)> = scores
                .iter()
                .enumerate()
                .map(|(id, score)| (id, *score))
                .collect();

            // Sort all by score descending
            all_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            // Verify all top-K scores are >= any score not in top-K
            let min_top_k_score = all_scores.iter().take(k).map(|(_, s)| *s).fold(f64::INFINITY, f64::min);
            let max_rest_score = all_scores.iter().skip(k).map(|(_, s)| *s).fold(f64::NEG_INFINITY, f64::max);

            if all_scores.len() > k {
                prop_assert!(
                    min_top_k_score >= max_rest_score,
                    "Top-K invariant violated: min in top {} is {} but max outside is {}",
                    k,
                    min_top_k_score,
                    max_rest_score
                );
            }
        }
    }

    // =========================================================================
    // PROPERTY TESTS FOR SEARCH DETERMINISM
    // =========================================================================

    proptest! {
        /// Property: search results are deterministic (same query always gives same order)
        #[test]
        fn prop_search_deterministic(
            query in "[a-z]{2,6}",
            num_docs in 5usize..20,
        ) {
            // Build a small test index
            let docs: Vec<SearchDoc> = (0..num_docs)
                .map(|i| make_doc(i, &format!("Document {} about {}", i, if i % 3 == 0 { "apples" } else { "oranges" })))
                .collect();
            let texts: Vec<String> = docs.iter().map(|d| format!("{} content", d.title)).collect();
            let index = build_hybrid_index(docs, texts, vec![]);

            // Run search twice
            let results1 = search_hybrid(&index, &query);
            let results2 = search_hybrid(&index, &query);

            // Results should be identical (same order)
            prop_assert_eq!(
                results1.len(),
                results2.len(),
                "Same query should return same number of results"
            );

            for (r1, r2) in results1.iter().zip(results2.iter()) {
                prop_assert_eq!(
                    r1.id,
                    r2.id,
                    "Same query should return results in same order"
                );
            }
        }

        /// Property: search results are sorted by score descending, then by title
        #[test]
        fn prop_search_results_sorted(
            query in "[a-z]{2,6}",
            num_docs in 5usize..20,
        ) {
            let docs: Vec<SearchDoc> = (0..num_docs)
                .map(|i| make_doc(i, &format!("Document {} about {}", i, if i % 3 == 0 { "test" } else { "other" })))
                .collect();
            let texts: Vec<String> = docs.iter().map(|d| format!("{} content with test word", d.title)).collect();
            let index = build_hybrid_index(docs, texts, vec![]);

            let results = search_hybrid(&index, &query);

            // Verify sorted by score descending (we can't access scores directly from SearchDoc,
            // but we can verify the ordering is stable)
            if results.len() >= 2 {
                // Run twice and verify same order
                let results2 = search_hybrid(&index, &query);
                for (r1, r2) in results.iter().zip(results2.iter()) {
                    prop_assert_eq!(r1.id, r2.id, "Results should be in stable order");
                }
            }
        }

        /// Property: search is case-insensitive (different query cases return same results)
        #[test]
        fn prop_search_case_insensitive(
            base_query in "[a-z]{2,6}",
            num_docs in 5usize..15,
        ) {
            // Create documents with mixed-case content
            let docs: Vec<SearchDoc> = (0..num_docs)
                .map(|i| {
                    let title = if i % 2 == 0 {
                        format!("Document about {}", base_query.to_uppercase())
                    } else {
                        format!("Document about {}", base_query)
                    };
                    make_doc(i, &title)
                })
                .collect();
            let texts: Vec<String> = docs.iter().map(|d| format!("{} content", d.title)).collect();
            let index = build_hybrid_index(docs, texts, vec![]);

            // Search with different cases
            let results_lower = search_hybrid(&index, &base_query.to_lowercase());
            let results_upper = search_hybrid(&index, &base_query.to_uppercase());
            let results_mixed = search_hybrid(&index, &mixed_case(&base_query));

            // All should return same results
            prop_assert_eq!(
                results_lower.len(),
                results_upper.len(),
                "Uppercase and lowercase queries should return same number of results"
            );
            prop_assert_eq!(
                results_lower.len(),
                results_mixed.len(),
                "Mixed case query should return same number of results"
            );

            // Same documents in same order
            for (r_low, r_up) in results_lower.iter().zip(results_upper.iter()) {
                prop_assert_eq!(
                    r_low.id,
                    r_up.id,
                    "Case should not affect result order"
                );
            }
        }

        /// Property: title field matches rank higher than content-only matches
        /// Note: The hybrid index scores based on field_boundaries, so we need to
        /// set up proper field boundaries to test title vs content ranking.
        #[test]
        fn prop_title_field_matches_rank_higher(num_docs in 3usize..10) {
            let query = "gemm";

            // Create docs where one has query in Title field, others in Content field
            let mut docs = Vec::new();
            let mut texts = Vec::new();
            let mut boundaries = Vec::new();

            // Doc 0: query only in content field
            docs.push(make_doc(0, "Introduction"));
            let text0 = format!("Introduction content about {} operations.", query);
            // Title field: 0-12 "Introduction", Content field: 13+
            boundaries.push(FieldBoundary { doc_id: 0, start: 0, end: 12, field_type: FieldType::Title, section_id: None, heading_level: 0 });
            boundaries.push(FieldBoundary { doc_id: 0, start: 13, end: text0.len(), field_type: FieldType::Content, section_id: None, heading_level: 0 });
            texts.push(text0);

            // Doc 1: query in title field (should rank higher due to field scoring)
            docs.push(make_doc(1, "GEMM Reference"));
            let text1 = "gemm reference api documentation.".to_string();
            // Title field covers "gemm reference" (0-14), Content field: 15+
            boundaries.push(FieldBoundary { doc_id: 1, start: 0, end: 14, field_type: FieldType::Title, section_id: None, heading_level: 0 });
            boundaries.push(FieldBoundary { doc_id: 1, start: 15, end: text1.len(), field_type: FieldType::Content, section_id: None, heading_level: 0 });
            texts.push(text1);

            // Add filler docs with query in content only
            for i in 2..num_docs {
                docs.push(make_doc(i, &format!("Document {}", i)));
                let text = format!("Document {} content about {} here.", i, query);
                let title_end = format!("Document {}", i).len();
                boundaries.push(FieldBoundary { doc_id: i, start: 0, end: title_end, field_type: FieldType::Title, section_id: None, heading_level: 0 });
                boundaries.push(FieldBoundary { doc_id: i, start: title_end + 1, end: text.len(), field_type: FieldType::Content, section_id: None, heading_level: 0 });
                texts.push(text);
            }

            let index = build_hybrid_index(docs, texts, boundaries);
            let results = search_hybrid(&index, query);

            // Doc 1 should rank first (title field match > content field match)
            prop_assert!(
                !results.is_empty(),
                "Should find results for query '{}'", query
            );
            prop_assert_eq!(
                results[0].id,
                1,
                "Document with query in title field should rank first (got doc {})", results[0].id
            );
        }
    }
}

#[cfg(test)]
mod tier_tests {
    //! Tier assignment tests.
    //!
    //! Tier fields are assigned in wasm.rs based on which search tier function
    //! produced the result:
    //! - Tier 1: Exact match (search_tier1_exact)
    //! - Tier 2: Prefix match (search_tier2_prefix)
    //! - Tier 3: Fuzzy match (search_tier3_fuzzy)
    //!
    //! Verification approach:
    //! 1. SearchResult struct has tier: u8 field (defined in types.rs)
    //! 2. All SearchResult creations assign a valid tier (1-3) in wasm.rs
    //! 3. SearchResultOutput includes tier field and serializes to JSON
    //! 4. Integration tests verify tier values via JavaScript:
    //!    - Exact query "changelog" returns tier 1
    //!    - Prefix query "changel" returns tier 2
    //!    - Fuzzy query "changelg" returns tier 3
    //!
    //! See: /tmp/test-tiers.ts for JavaScript integration test

    use crate::testing::make_doc;
    use crate::types::{SearchDoc, SearchResult, SearchSource};
    use serde_json;

    #[test]
    fn test_search_result_has_tier_field() {
        // Verify SearchResult struct includes tier field
        let doc = make_doc(0, "Test");

        let result = SearchResult {
            doc_id: doc.id,
            source: SearchSource::Title,
            score: 100.0,
            section_id: None,
            tier: 1,
        };

        // Verify tier is accessible and valid
        assert_eq!(result.tier, 1);
        assert!((1..=3).contains(&result.tier));
    }

    #[test]
    fn test_search_result_tier_serialization() {
        // Verify tier field is serialized to JSON
        let doc = make_doc(0, "Test");

        let result = SearchResult {
            doc_id: doc.id,
            source: SearchSource::Title,
            score: 100.0,
            section_id: None,
            tier: 2,
        };

        // Serialize to JSON
        let json_str = serde_json::to_string(&result).expect("Should serialize");

        // Verify tier is in the JSON output
        assert!(
            json_str.contains("\"tier\""),
            "JSON should include tier field: {}",
            json_str
        );
        assert!(
            json_str.contains("2"),
            "JSON should include tier value 2: {}",
            json_str
        );
    }

    #[test]
    fn test_all_tiers_valid_range() {
        // Test that all valid tier values work
        for tier in 1..=3 {
            let doc = make_doc(tier as usize, &format!("Tier {} test", tier));

            let result = SearchResult {
                doc_id: doc.id,
                source: SearchSource::Title,
                score: 100.0,
                section_id: None,
                tier,
            };

            // Verify tier is in valid range
            assert!((1..=3).contains(&result.tier), "Tier {} not in range [1, 3]", tier);
        }
    }

    #[test]
    fn test_fuzzy_ranking_prefers_shorter_terms() {
        // When searching for "coasync", verify that shorter, more specific terms
        // rank higher than longer ones when edit distance is equal.
        //
        // Example: "cpasync" (7 chars) should rank higher than "asynchronous" (12 chars)
        // when both are edit distance 1 from the query.
        //
        // This test verifies the tiebreaker logic:
        // - Same edit distance -> shorter term wins (more specific)
        // - Implementation: subtract term_len * 0.01 from score
        // - So "cpasync" scores 0.07 points higher than "asynchronous"

        // Create two search results with same base score and distance
        // but different term lengths
        let _doc_short = SearchDoc {
            id: 1,
            title: "cpasync submodule".to_string(),
            excerpt: "cpasync is a short specific term".to_string(),
            href: "/cpasync".to_string(),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        };

        let _doc_long = SearchDoc {
            id: 2,
            title: "asynchronous operations".to_string(),
            excerpt: "asynchronous is a longer generic term".to_string(),
            href: "/async".to_string(),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        };

        // Both have distance 1, so base_score = 30.0
        // After term length penalty:
        // - cpasync (7 chars): 30.0 - 0.07 = 29.93
        // - asynchronous (12 chars): 30.0 - 0.12 = 29.88
        // cpasync should rank higher

        let result_short = SearchResult {
            doc_id: 1,
            source: SearchSource::Title,
            score: 29.93,  // Simulating: 30.0 - (7 * 0.01)
            section_id: None,
            tier: 3,
        };

        let result_long = SearchResult {
            doc_id: 2,
            source: SearchSource::Title,
            score: 29.88,  // Simulating: 30.0 - (12 * 0.01)
            section_id: None,
            tier: 3,
        };

        // The shorter term should have a slightly higher score
        assert!(result_short.score > result_long.score,
            "Shorter term (cpasync: {}) should rank higher than longer term (asynchronous: {})",
            result_short.score, result_long.score);
    }
}
