// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Hybrid index: inverted index + vocabulary suffix array.
//!
//! Why not suffix array over full text? Because the vocabulary is ~100x smaller.
//! For exact matches, the inverted index gives O(1) lookup. For prefix matches,
//! the vocabulary suffix array gives O(log k) binary search, then we union the
//! posting lists.
//!
//! # Data Structures
//!
//! | Structure               | Query Type | Complexity |
//! |-------------------------|------------|------------|
//! | Inverted Index          | Exact      | O(1)       |
//! | Vocabulary Suffix Array | Prefix     | O(log k)   |
//!
//! Fuzzy search iterates all vocabulary terms with the Levenshtein DFA.

use super::{build_inverted_index, build_inverted_index_parallel, build_vocab_suffix_array_sais};
use crate::types::{FieldBoundary, HybridIndex, SearchDoc};
#[cfg(feature = "parallel")]
use rayon::prelude::*;

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

    // Step 4: Sort field boundaries by (doc_id, start) for binary search lookups
    // OPTIMIZATION: Enables O(log n) field type lookups instead of O(n)
    let mut sorted_boundaries = field_boundaries;
    sorted_boundaries.sort_by(|a, b| {
        a.doc_id.cmp(&b.doc_id).then_with(|| a.start.cmp(&b.start))
    });

    HybridIndex {
        docs,
        texts,
        field_boundaries: sorted_boundaries,
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

    // Step 4: Sort field boundaries by (doc_id, start) for binary search lookups
    // OPTIMIZATION: Enables O(log n) field type lookups instead of O(n)
    let mut sorted_boundaries = field_boundaries;
    #[cfg(feature = "parallel")]
    sorted_boundaries.par_sort_by(|a, b| {
        a.doc_id.cmp(&b.doc_id).then_with(|| a.start.cmp(&b.start))
    });
    #[cfg(not(feature = "parallel"))]
    sorted_boundaries.sort_by(|a, b| {
        a.doc_id.cmp(&b.doc_id).then_with(|| a.start.cmp(&b.start))
    });

    HybridIndex {
        docs,
        texts,
        field_boundaries: sorted_boundaries,
        inverted_index,
        vocabulary,
        vocab_suffix_array,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::make_doc;

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
    fn test_vocabulary_size() {
        let docs = vec![make_doc(0, "Test")];
        // Note: "the" is a stop word and gets filtered out
        let texts = vec!["apple apple apple rust rust".to_string()];
        let index = build_hybrid_index(docs, texts, vec![]);

        // Vocabulary should have 2 unique terms: "apple", "rust"
        assert_eq!(index.vocabulary.len(), 2);
    }
}
