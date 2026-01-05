//! Search function and match scoring.
//!
//! # Lean Correspondence
//!
//! The search function corresponds to specifications in:
//! - `SearchVerified/BinarySearch.lean` - Binary search correctness
//! - `SearchVerified/Scoring.lean` - Score calculation
//! - `SearchVerified/InvertedIndex.lean` - Inverted index search
//!
//! # Unicode Support
//!
//! The suffix array uses **character offsets** (not byte offsets).
//! All slicing operations must use character-based indexing to match
//! JavaScript's UTF-16 string semantics.

use crate::scoring::{final_score, get_field_type};
use crate::types::{
    FieldBoundary, IndexMode, InvertedIndex, ScoredDoc, SearchDoc, SearchIndex, UnifiedIndex,
};
use crate::utils::normalize;
use std::collections::HashMap;

/// Get the suffix at a character offset (for searching).
///
/// Since the suffix array stores character offsets (not byte offsets),
/// we need to convert to byte offsets for Rust string slicing.
fn suffix_at_char_offset(text: &str, char_offset: usize) -> &str {
    // Find byte index of the nth character
    text.char_indices()
        .nth(char_offset)
        .map(|(byte_idx, _)| &text[byte_idx..])
        .unwrap_or("")
}

/// Calculate scores for a search term using binary search.
///
/// # Lean Specification
///
/// This uses binary search to find matching suffixes, corresponding to
/// `BinarySearch.findFirstGe` and `BinarySearch.collectMatches` in `BinarySearch.lean`.
///
/// # Unicode Support
///
/// The suffix array uses character offsets. All slicing uses `suffix_at_char_offset`
/// to correctly handle multi-byte UTF-8 characters.
fn score_matches(index: &SearchIndex, part: &str) -> Vec<(usize, f64)> {
    let mut scores: Vec<(usize, f64)> = Vec::new();
    let suffixes = &index.suffix_array;
    let target = part.to_string();

    // Binary search: find first position where suffix >= target
    let mut lo = 0usize;
    let mut hi = suffixes.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let hay = suffix_at_char_offset(&index.texts[suffixes[mid].doc_id], suffixes[mid].offset);
        if hay < target.as_str() {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }

    // Walk forward and score exact prefix matches
    for i in lo..suffixes.len() {
        let entry = &suffixes[i];
        let s = suffix_at_char_offset(&index.texts[entry.doc_id], entry.offset);
        if !s.starts_with(part) {
            if s > &target {
                break;
            }
        } else {
            let field_type = get_field_type(index, entry.doc_id, entry.offset);
            let text_len = index.texts[entry.doc_id].chars().count();
            let score = final_score(&field_type, entry.offset, text_len);
            scores.push((entry.doc_id, score));
        }
    }

    // NOTE: Fuzzy matching is handled by HybridIndex with Levenshtein automaton.
    // The old O(n) fuzzy fallback was removed because it caused 450ms+ latency.
    // For fuzzy search, use HybridIndex.search_fuzzy() instead.

    scores
}

/// Search the index for documents matching the query.
///
/// Returns documents ranked by relevance score.
///
/// # Lean Specification
///
/// The search function satisfies:
/// - Soundness: All returned documents match the query
/// - Ranking: Higher-scored documents appear first
pub fn search(index: &SearchIndex, term: &str) -> Vec<SearchDoc> {
    let parts: Vec<String> = normalize(term)
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
        .map(|p| score_matches(index, p))
        .filter(|v| !v.is_empty())
        .collect();

    if score_sets.is_empty() {
        return Vec::new();
    }

    // Calculate aggregate scores for documents that match all terms
    let mut doc_scores: HashMap<usize, f64> = HashMap::new();

    // For the first term, take the MAXIMUM score for each document
    for (doc_id, score) in &score_sets[0] {
        doc_scores
            .entry(*doc_id)
            .and_modify(|existing| *existing = existing.max(*score))
            .or_insert(*score);
    }

    // For each additional term, keep only docs that match all terms
    for score_set in &score_sets[1..] {
        let mut term_scores: HashMap<usize, f64> = HashMap::new();
        for (doc_id, score) in score_set {
            term_scores
                .entry(*doc_id)
                .and_modify(|existing| *existing = existing.max(*score))
                .or_insert(*score);
        }

        doc_scores.retain(|doc_id, score| {
            if let Some(additional_score) = term_scores.get(doc_id) {
                *score += additional_score;
                true
            } else {
                false
            }
        });

        if doc_scores.is_empty() {
            break;
        }
    }

    // Convert to scored docs and sort by score (descending)
    let mut scored_docs: Vec<ScoredDoc> = doc_scores
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

    scored_docs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    scored_docs.into_iter().map(|sd| sd.doc).collect()
}

// =============================================================================
// INVERTED INDEX SEARCH
// =============================================================================

/// Calculate scores using the inverted index for exact word matches.
///
/// # Lean Specification
///
/// Uses O(1) term lookup from `InvertedIndex.lean`:
/// - Each term maps to a sorted posting list
/// - Postings contain field_type for scoring
fn score_matches_inverted(
    inverted: &InvertedIndex,
    texts: &[String],
    _boundaries: &[FieldBoundary],
    term: &str,
) -> Vec<(usize, f64)> {
    let mut scores: Vec<(usize, f64)> = Vec::new();

    if let Some(posting_list) = inverted.terms.get(term) {
        for posting in &posting_list.postings {
            let text_len = texts.get(posting.doc_id).map(|t| t.len()).unwrap_or(0);
            let score = final_score(&posting.field_type, posting.offset, text_len);
            scores.push((posting.doc_id, score));
        }
    }

    scores
}

/// Search using the inverted index only.
///
/// Best for exact word matching on large corpora. O(1) per term lookup.
fn search_inverted_only(
    inverted: &InvertedIndex,
    docs: &[SearchDoc],
    texts: &[String],
    boundaries: &[FieldBoundary],
    term: &str,
) -> Vec<SearchDoc> {
    let parts: Vec<String> = normalize(term)
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
        .map(|p| score_matches_inverted(inverted, texts, boundaries, p))
        .filter(|v| !v.is_empty())
        .collect();

    if score_sets.is_empty() {
        return Vec::new();
    }

    // Calculate aggregate scores for documents that match all terms
    let mut doc_scores: HashMap<usize, f64> = HashMap::new();

    for (doc_id, score) in &score_sets[0] {
        doc_scores
            .entry(*doc_id)
            .and_modify(|existing| *existing = existing.max(*score))
            .or_insert(*score);
    }

    for score_set in &score_sets[1..] {
        let mut term_scores: HashMap<usize, f64> = HashMap::new();
        for (doc_id, score) in score_set {
            term_scores
                .entry(*doc_id)
                .and_modify(|existing| *existing = existing.max(*score))
                .or_insert(*score);
        }

        doc_scores.retain(|doc_id, score| {
            if let Some(additional_score) = term_scores.get(doc_id) {
                *score += additional_score;
                true
            } else {
                false
            }
        });

        if doc_scores.is_empty() {
            break;
        }
    }

    // Convert to scored docs and sort
    let mut scored_docs: Vec<ScoredDoc> = doc_scores
        .into_iter()
        .filter_map(|(doc_id, score)| {
            docs.iter().find(|d| d.id == doc_id).map(|doc| ScoredDoc {
                doc: doc.clone(),
                score,
            })
        })
        .collect();

    scored_docs.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    scored_docs.into_iter().map(|sd| sd.doc).collect()
}

// =============================================================================
// UNIFIED INDEX SEARCH
// =============================================================================

/// Search the unified index using the appropriate mode.
///
/// Dispatches to suffix array or inverted index based on the index mode
/// selected at build time.
///
/// # Lean Specification
///
/// From `InvertedIndex.lean`:
/// ```lean
/// axiom hybrid_search_consistent :
///     (∃ pl, idx.inverted_index.lookup word = some pl ∧ doc_id ∈ pl.docIds) →
///     ∃ entry : SuffixEntry, entry ∈ idx.suffix_index.suffix_array.toList ∧ ...
/// ```
pub fn search_unified(index: &UnifiedIndex, term: &str) -> Vec<SearchDoc> {
    match index.mode {
        IndexMode::SuffixArrayOnly => {
            // Use suffix array search
            if let (Some(suffix_array), Some(lcp)) = (&index.suffix_array, &index.lcp) {
                let temp_index = SearchIndex {
                    docs: index.docs.clone(),
                    texts: index.texts.clone(),
                    suffix_array: suffix_array.clone(),
                    lcp: lcp.clone(),
                    field_boundaries: index.field_boundaries.clone(),
                    version: 4,
                };
                search(&temp_index, term)
            } else {
                Vec::new()
            }
        }
        IndexMode::InvertedIndexOnly => {
            // Use inverted index search
            if let Some(inverted) = &index.inverted_index {
                search_inverted_only(
                    inverted,
                    &index.docs,
                    &index.texts,
                    &index.field_boundaries,
                    term,
                )
            } else {
                Vec::new()
            }
        }
        IndexMode::Hybrid => {
            // Hybrid: use inverted index first, fall back to suffix array for fuzzy
            let mut results = if let Some(inverted) = &index.inverted_index {
                search_inverted_only(
                    inverted,
                    &index.docs,
                    &index.texts,
                    &index.field_boundaries,
                    term,
                )
            } else {
                Vec::new()
            };

            // If no exact matches, try suffix array for prefix/fuzzy
            if results.is_empty() {
                if let (Some(suffix_array), Some(lcp)) = (&index.suffix_array, &index.lcp) {
                    let temp_index = SearchIndex {
                        docs: index.docs.clone(),
                        texts: index.texts.clone(),
                        suffix_array: suffix_array.clone(),
                        lcp: lcp.clone(),
                        field_boundaries: index.field_boundaries.clone(),
                        version: 4,
                    };
                    results = search(&temp_index, term);
                }
            }

            results
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::build_index;
    use crate::types::{FieldBoundary, FieldType};

    fn make_doc(id: usize, title: &str) -> SearchDoc {
        SearchDoc {
            id,
            title: title.to_string(),
            excerpt: "".to_string(),
            href: format!("/doc/{}", id),
            kind: "post".to_string(),
        }
    }

    #[test]
    fn test_search_finds_matches() {
        let docs = vec![make_doc(0, "Hello World")];
        let texts = vec!["hello world".to_string()];
        let index = build_index(docs, texts, vec![]);

        let results = search(&index, "hello");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Hello World");
    }

    #[test]
    fn test_search_empty_query() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["test".to_string()];
        let index = build_index(docs, texts, vec![]);

        let results = search(&index, "");
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_no_matches() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["test".to_string()];
        let index = build_index(docs, texts, vec![]);

        let results = search(&index, "xyz");
        assert!(results.is_empty());
    }

    #[test]
    fn test_title_ranks_higher() {
        let docs = vec![make_doc(0, "Rust Guide"), make_doc(1, "Other")];
        let texts = vec!["rust guide".to_string(), "learn rust".to_string()];
        let boundaries = vec![
            FieldBoundary {
                doc_id: 0,
                start: 0,
                end: 10,
                field_type: FieldType::Title,
                section_id: None,
            },
            FieldBoundary {
                doc_id: 1,
                start: 0,
                end: 10,
                field_type: FieldType::Content,
                section_id: None,
            },
        ];
        let index = build_index(docs, texts, boundaries);

        let results = search(&index, "rust");
        assert!(results.len() >= 2);
        // Title match should rank first
        assert_eq!(results[0].id, 0);
    }
}
