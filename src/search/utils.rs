// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Shared utilities for search modules.
//!
//! The boring-but-essential functions that every search path needs:
//! query parsing and multi-term score merging. Extracted here to avoid
//! four copies of the same logic.

use crate::util::normalize::normalize;
use std::collections::HashMap;

/// Parse a query string into normalized, whitespace-separated terms.
///
/// # Example
///
/// ```ignore
/// let terms = parse_query("Hello World");
/// assert_eq!(terms, vec!["hello", "world"]);
/// ```
pub fn parse_query(query: &str) -> Vec<String> {
    normalize(query)
        .split(' ')
        .filter(|p| !p.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Merge multiple score sets with AND semantics.
///
/// Documents must match ALL terms to be included. Scores are summed across terms,
/// with the maximum score taken when a document matches a term multiple times.
///
/// # Arguments
///
/// * `score_sets` - One `Vec<(doc_id, score)>` per query term
///
/// # Returns
///
/// HashMap of `doc_id -> total_score` for documents matching all terms.
pub fn merge_score_sets(score_sets: &[Vec<(usize, f64)>]) -> HashMap<usize, f64> {
    if score_sets.is_empty() {
        return HashMap::new();
    }

    // Pre-allocate with capacity hint from first term
    let first_set = &score_sets[0];
    let mut doc_scores: HashMap<usize, f64> = HashMap::with_capacity(first_set.len());

    // Initialize with first term's scores (take max per doc)
    for (doc_id, score) in first_set {
        doc_scores
            .entry(*doc_id)
            .and_modify(|s| *s = s.max(*score))
            .or_insert(*score);
    }

    // Reuse a single HashMap for term scores instead of allocating per-term
    let mut term_scores: HashMap<usize, f64> = HashMap::with_capacity(
        score_sets.get(1).map(|s| s.len()).unwrap_or(0),
    );

    // Intersect with remaining terms
    for score_set in &score_sets[1..] {
        term_scores.clear();

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

    doc_scores
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_basic() {
        let terms = parse_query("hello world");
        assert_eq!(terms, vec!["hello", "world"]);
    }

    #[test]
    fn test_parse_query_empty() {
        let terms = parse_query("");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_parse_query_whitespace_only() {
        let terms = parse_query("   ");
        assert!(terms.is_empty());
    }

    #[test]
    fn test_parse_query_normalizes() {
        let terms = parse_query("HELLO World");
        assert_eq!(terms, vec!["hello", "world"]);
    }

    #[test]
    fn test_parse_query_extra_spaces() {
        let terms = parse_query("  hello   world  ");
        assert_eq!(terms, vec!["hello", "world"]);
    }

    #[test]
    fn test_merge_score_sets_empty() {
        let result = merge_score_sets(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_merge_score_sets_single_term() {
        let scores = vec![(0, 1.0), (1, 2.0)];
        let result = merge_score_sets(&[scores]);
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&0), Some(&1.0));
        assert_eq!(result.get(&1), Some(&2.0));
    }

    #[test]
    fn test_merge_score_sets_and_semantics() {
        let term1 = vec![(0, 1.0), (1, 2.0), (2, 3.0)];
        let term2 = vec![(1, 1.5), (2, 2.5)]; // doc 0 doesn't match term2
        let result = merge_score_sets(&[term1, term2]);
        // Only docs 1 and 2 match both terms
        assert_eq!(result.len(), 2);
        assert_eq!(result.get(&0), None); // filtered out
        assert_eq!(result.get(&1), Some(&3.5)); // 2.0 + 1.5
        assert_eq!(result.get(&2), Some(&5.5)); // 3.0 + 2.5
    }

    #[test]
    fn test_merge_score_sets_takes_max_per_doc() {
        // Same doc appears twice in first term
        let term1 = vec![(0, 1.0), (0, 5.0)];
        let result = merge_score_sets(&[term1]);
        assert_eq!(result.get(&0), Some(&5.0)); // max of 1.0 and 5.0
    }
}
