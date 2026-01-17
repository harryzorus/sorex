// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Vocabulary index: the "FST" that isn't actually an FST.
//!
//! The name is historical. FST (Finite State Transducer) is overkill for a blog's
//! ~150-term vocabulary. Linear scan with a precomputed Levenshtein DFA beats FST
//! automaton intersection when n is small. Plus, `Vec<String>` is debuggable.
//!
//! # Performance
//!
//! | Operation     | Complexity | Notes |
//! |---------------|------------|-------|
//! | Exact lookup  | O(1)       | Hash into inverted index |
//! | Prefix search | O(log k)   | Binary search vocabulary suffix array |
//! | Fuzzy search  | O(vocab)   | ~8ns/term with Levenshtein DFA |
//!
//! For huge vocabularies (>10k terms), consider `fst` crate. For typical blogs,
//! this is faster and simpler.

use super::inverted::build_inverted_index_parallel;
use super::sais::build_vocab_suffix_array_sais;
use crate::types::{FieldBoundary, InvertedIndex, SearchDoc, VocabSuffixEntry};

/// Index with vocabulary for efficient search.
///
/// Contains:
/// - Sorted vocabulary (simple Vec<String>)
/// - Vocabulary suffix array (for prefix search)
/// - Inverted index (for exact lookup)
#[derive(Debug, Clone)]
pub struct FstIndex {
    /// Document metadata
    pub docs: Vec<SearchDoc>,
    /// Document texts (for snippet extraction)
    pub texts: Vec<String>,
    /// Field boundaries for scoring
    pub field_boundaries: Vec<FieldBoundary>,
    /// Inverted index: term → posting list
    pub inverted_index: InvertedIndex,
    /// Sorted vocabulary terms
    pub vocabulary: Vec<String>,
    /// Suffix array over vocabulary
    pub vocab_suffix_array: Vec<VocabSuffixEntry>,
}

/// Build vocabulary index with parallel construction.
///
/// ```text
/// 1. Build inverted index (parallel map-reduce)
/// 2. Extract and sort vocabulary
/// 3. Build vocabulary suffix array (SA-IS)
/// ```
pub fn build_fst_index(
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    field_boundaries: Vec<FieldBoundary>,
) -> FstIndex {
    // Step 1: Build inverted index in parallel
    let inverted_index = build_inverted_index_parallel(&texts, &field_boundaries);

    // Step 2: Extract vocabulary (sorted for binary search and suffix array)
    let mut vocabulary: Vec<String> = inverted_index.terms.keys().cloned().collect();
    vocabulary.sort();

    // Step 3: Build vocabulary suffix array
    let vocab_suffix_array = build_vocab_suffix_array_sais(&vocabulary);

    FstIndex {
        docs,
        texts,
        field_boundaries,
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
    fn test_build_fst_index() {
        let docs = vec![make_doc(0, "Rust Guide"), make_doc(1, "Python Guide")];
        let texts = vec![
            "rust programming language".to_string(),
            "python programming language".to_string(),
        ];
        let index = build_fst_index(docs, texts, vec![]);

        // Verify vocabulary contains all terms
        assert!(index.vocabulary.contains(&"rust".to_string()));
        assert!(index.vocabulary.contains(&"python".to_string()));
        assert!(index.vocabulary.contains(&"programming".to_string()));
        assert!(index.vocabulary.contains(&"language".to_string()));
        assert!(!index.vocabulary.contains(&"notinvocab".to_string()));
    }

    #[test]
    fn test_vocabulary_is_sorted() {
        let docs = vec![make_doc(0, "Test")];
        let texts = vec!["zebra apple mango banana".to_string()];
        let index = build_fst_index(docs, texts, vec![]);

        // Vocabulary should be sorted
        let mut sorted = index.vocabulary.clone();
        sorted.sort();
        assert_eq!(index.vocabulary, sorted);
    }

    #[test]
    fn test_deterministic_construction() {
        let docs = vec![make_doc(0, "Test"), make_doc(1, "Other")];
        let texts = vec![
            "rust programming systems".to_string(),
            "python scripting language".to_string(),
        ];

        // Build twice and compare
        let index1 = build_fst_index(docs.clone(), texts.clone(), vec![]);
        let index2 = build_fst_index(docs, texts, vec![]);

        // Vocabulary should be identical
        assert_eq!(index1.vocabulary, index2.vocabulary);

        // Suffix array should be identical
        assert_eq!(index1.vocab_suffix_array, index2.vocab_suffix_array);
    }
}
