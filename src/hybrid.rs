//! Hybrid index: inverted index + suffix array over vocabulary.
//!
//! # Architecture
//!
//! The hybrid index combines two data structures:
//! 1. **Inverted Index**: Hash map from term → posting list (O(1) exact lookup)
//! 2. **Vocabulary Suffix Array**: Suffix array over unique terms (O(log k) prefix search)
//!
//! The key insight is that we don't need a suffix array over the full text.
//! Building a suffix array over just the vocabulary is:
//! - ~100× smaller (vocabulary vs full text)
//! - O(n) construction via SA-IS algorithm (not O(n log n) naive sort)
//! - Still enables prefix search via posting list union
//!
//! # Complexity
//!
//! | Operation | Time | Space |
//! |-----------|------|-------|
//! | Build | O(n + k) | O(k + m) |
//! | Exact word | O(1) | - |
//! | Prefix search | O(log k + p) | - |
//! | Multi-word AND | O(min posting list) | - |
//!
//! Where:
//! - n = total characters in corpus
//! - k = vocabulary size (unique terms)
//! - m = total postings
//! - p = number of prefix-matching terms
//!
//! # SA-IS Algorithm
//!
//! The suffix array is built using the SA-IS (Suffix Array by Induced Sorting)
//! algorithm, which runs in O(n) time. See `sais.rs` for implementation details.

use crate::inverted::{build_inverted_index, build_inverted_index_parallel};
use crate::levenshtein::levenshtein_within;
use crate::sais::build_vocab_suffix_array_sais;
use crate::scoring::{final_score, get_field_type_from_boundaries};
use crate::types::{FieldBoundary, HybridIndex, PostingList, ScoredDoc, SearchDoc};
use crate::utils::normalize;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::collections::HashMap;

/// Build a hybrid index from documents.
///
/// This creates:
/// 1. An inverted index for O(1) exact word lookup
/// 2. A vocabulary suffix array for O(log k) prefix search
///
/// # Lean Specification
///
/// ```lean
/// def build_hybrid_index (docs : Array SearchDoc) (texts : Array String)
///     (boundaries : Array FieldBoundary) : HybridIndex :=
///   let inverted := build_inverted_index texts boundaries
///   let vocabulary := inverted.terms.keys.toArray.qsort (· < ·)
///   let vocab_suffix_array := build_vocab_suffix_array vocabulary
///   { docs, texts, field_boundaries := boundaries,
///     inverted_index := inverted,
///     vocabulary,
///     vocab_suffix_array }
/// ```
pub fn build_hybrid_index(
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    field_boundaries: Vec<FieldBoundary>,
) -> HybridIndex {
    // Step 1: Build inverted index (reuse existing implementation)
    let inverted_index = build_inverted_index(&texts, &field_boundaries);

    // Step 2: Extract vocabulary (sorted)
    let mut vocabulary: Vec<String> = inverted_index.terms.keys().cloned().collect();
    vocabulary.sort();

    // Step 3: Build suffix array over vocabulary using SA-IS (O(n) linear time)
    let vocab_suffix_array = build_vocab_suffix_array_sais(&vocabulary);

    HybridIndex {
        docs,
        texts,
        field_boundaries,
        inverted_index,
        vocabulary,
        vocab_suffix_array,
    }
}

/// Build a hybrid index using parallel construction.
///
/// Faster for large corpora (100+ documents):
/// 1. Parallel inverted index construction (map-reduce over documents)
/// 2. Parallel suffix array sorting
///
/// For small corpora, use `build_hybrid_index` instead.
pub fn build_hybrid_index_parallel(
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    field_boundaries: Vec<FieldBoundary>,
) -> HybridIndex {
    // Step 1: Build inverted index in parallel
    let inverted_index = build_inverted_index_parallel(&texts, &field_boundaries);

    // Step 2: Extract vocabulary (parallel sort for large vocabularies)
    let mut vocabulary: Vec<String> = inverted_index.terms.keys().cloned().collect();
    #[cfg(feature = "parallel")]
    vocabulary.par_sort();
    #[cfg(not(feature = "parallel"))]
    vocabulary.sort();

    // Step 3: Build suffix array over vocabulary using SA-IS (O(n) linear time)
    // SA-IS is already O(n), so no need for parallel version
    let vocab_suffix_array = build_vocab_suffix_array_sais(&vocabulary);

    HybridIndex {
        docs,
        texts,
        field_boundaries,
        inverted_index,
        vocabulary,
        vocab_suffix_array,
    }
}

/// Search the hybrid index.
///
/// Strategy:
/// 1. Try exact word lookup first (O(1))
/// 2. If no exact match, use prefix search via vocabulary suffix array (O(log k))
/// 3. For multi-word queries, intersect posting lists
pub fn search_hybrid(index: &HybridIndex, query: &str) -> Vec<SearchDoc> {
    let parts: Vec<String> = normalize(query)
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();

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

    // Aggregate scores (AND semantics: only docs matching all terms)
    let mut doc_scores: HashMap<usize, f64> = HashMap::new();

    // Initialize with first term's scores (take max per doc)
    for (doc_id, score) in &score_sets[0] {
        doc_scores
            .entry(*doc_id)
            .and_modify(|s| *s = s.max(*score))
            .or_insert(*score);
    }

    // Intersect with remaining terms
    for score_set in &score_sets[1..] {
        let mut term_scores: HashMap<usize, f64> = HashMap::new();
        for (doc_id, score) in score_set {
            term_scores
                .entry(*doc_id)
                .and_modify(|s| *s = s.max(*score))
                .or_insert(*score);
        }

        doc_scores.retain(|doc_id, score| {
            if let Some(additional) = term_scores.get(doc_id) {
                *score += additional;
                true
            } else {
                false
            }
        });

        if doc_scores.is_empty() {
            break;
        }
    }

    // Convert to sorted results
    let mut results: Vec<ScoredDoc> = doc_scores
        .into_iter()
        .filter_map(|(doc_id, score)| {
            index
                .docs
                .iter()
                .find(|d| d.id == doc_id)
                .map(|doc| ScoredDoc {
                    doc: doc.clone(),
                    score,
                })
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.into_iter().map(|sd| sd.doc).collect()
}

// =============================================================================
// STREAMING SEARCH API
// =============================================================================
// These functions support two-phase streaming search:
// 1. search_exact() - O(1) inverted index lookup (returns first results fast)
// 2. search_expanded() - O(log k) suffix array search (additional matches)
//
// Lean Specification: StreamingSearch.lean
// Key Invariants:
// - exact_subset_full: exact results ⊆ full results
// - expanded_disjoint_exact: expanded ∩ exact = ∅
// - union_complete: exact ∪ expanded = full results

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
    let parts: Vec<String> = normalize(query)
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();

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
    let parts: Vec<String> = normalize(query)
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();

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
    let parts: Vec<String> = normalize(query)
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect();

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
fn score_term_exact(index: &HybridIndex, term: &str) -> Vec<(usize, f64)> {
    if let Some(posting_list) = index.inverted_index.terms.get(term) {
        return score_posting_list(posting_list, &index.texts, &index.field_boundaries);
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
            all_scores.extend(score_posting_list(
                posting_list,
                &index.texts,
                &index.field_boundaries,
            ));
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

/// Aggregate score sets with AND semantics and sort by score.
fn aggregate_and_sort(index: &HybridIndex, score_sets: Vec<Vec<(usize, f64)>>) -> Vec<SearchDoc> {
    let mut doc_scores: HashMap<usize, f64> = HashMap::new();

    // Initialize with first term's scores (take max per doc)
    for (doc_id, score) in &score_sets[0] {
        doc_scores
            .entry(*doc_id)
            .and_modify(|s| *s = s.max(*score))
            .or_insert(*score);
    }

    // Intersect with remaining terms
    for score_set in &score_sets[1..] {
        let mut term_scores: HashMap<usize, f64> = HashMap::new();
        for (doc_id, score) in score_set {
            term_scores
                .entry(*doc_id)
                .and_modify(|s| *s = s.max(*score))
                .or_insert(*score);
        }

        doc_scores.retain(|doc_id, score| {
            if let Some(additional) = term_scores.get(doc_id) {
                *score += additional;
                true
            } else {
                false
            }
        });

        if doc_scores.is_empty() {
            break;
        }
    }

    // Convert to sorted results
    let mut results: Vec<ScoredDoc> = doc_scores
        .into_iter()
        .filter_map(|(doc_id, score)| {
            index
                .docs
                .iter()
                .find(|d| d.id == doc_id)
                .map(|doc| ScoredDoc {
                    doc: doc.clone(),
                    score,
                })
        })
        .collect();

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.into_iter().map(|sd| sd.doc).collect()
}

/// Score a single search term.
///
/// Three-tier search strategy:
/// 1. Exact lookup in inverted index (O(1))
/// 2. Prefix/substring search via vocabulary suffix array (O(log k))
/// 3. Fuzzy search via Levenshtein distance over vocabulary (O(vocab))
fn score_term(index: &HybridIndex, term: &str) -> Vec<(usize, f64)> {
    // Tier 1: Try exact match first (O(1))
    if let Some(posting_list) = index.inverted_index.terms.get(term) {
        return score_posting_list(posting_list, &index.texts, &index.field_boundaries);
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
            all_scores.extend(score_posting_list(
                posting_list,
                &index.texts,
                &index.field_boundaries,
            ));
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
fn score_posting_list(
    posting_list: &PostingList,
    texts: &[String],
    boundaries: &[FieldBoundary],
) -> Vec<(usize, f64)> {
    posting_list
        .postings
        .iter()
        .map(|posting| {
            let text_len = texts.get(posting.doc_id).map(|t| t.len()).unwrap_or(0);
            let field_type =
                get_field_type_from_boundaries(posting.doc_id, posting.offset, boundaries);
            let score = final_score(&field_type, posting.offset, text_len);
            (posting.doc_id, score)
        })
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

    fn make_doc(id: usize, title: &str) -> SearchDoc {
        SearchDoc {
            id,
            title: title.to_string(),
            excerpt: String::new(),
            href: format!("/doc/{}", id),
            kind: "post".to_string(),
        }
    }

    #[test]
    fn test_build_hybrid_index() {
        let docs = vec![make_doc(0, "Rust Guide"), make_doc(1, "Python Guide")];
        let texts = vec![
            "rust programming language".to_string(),
            "python programming language".to_string(),
        ];
        let index = build_hybrid_index(docs, texts, vec![]);

        // Check vocabulary is sorted
        assert!(index.vocabulary.windows(2).all(|w| w[0] <= w[1]));

        // Check all vocabulary terms have posting lists
        for term in &index.vocabulary {
            assert!(index.inverted_index.terms.contains_key(term));
        }
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

    #[test]
    fn test_vocabulary_size() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["the the the rust rust".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        // Vocabulary should have 2 unique terms: "the", "rust"
        assert_eq!(index.vocabulary.len(), 2);
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
}
