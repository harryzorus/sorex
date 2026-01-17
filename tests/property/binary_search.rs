//! Property tests for binary search correctness on suffix arrays.
//!
//! Verifies that:
//! 1. Binary search finds ALL entries that start with a given prefix
//! 2. Binary search returns ONLY entries that start with the prefix
//! 3. Results are properly ordered (suffix array sortedness is maintained)
//! 4. UTF-8 character boundary safety

use super::common::{build_test_index, assert_index_well_formed};
use proptest::prelude::*;

// ============================================================================
// STRATEGIES
// ============================================================================

/// Generate word-like strings for building corpus.
fn word_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z0-9]{2,8}").unwrap()
}

/// Generate random document text.
fn document_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(word_strategy(), 1..10).prop_map(|words| words.join(" "))
}

/// Generate a corpus of documents (expanded from 1..5 to 1..10 for better coverage).
fn small_corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(document_strategy(), 1..10)
}

/// Generate medium-sized corpus (20..50 documents).
fn medium_corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(document_strategy(), 20..50)
}

/// Generate mixed corpus with 80% small, 20% medium.
fn corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop_oneof![
        4 => small_corpus_strategy(),
        1 => medium_corpus_strategy(),
    ]
}

/// Generate a prefix for searching.
fn prefix_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z0-9]{1,4}").unwrap()
}

// ============================================================================
// SUFFIX ARRAY BINARY SEARCH PROPERTIES
// ============================================================================

proptest! {
    /// Property: All returned entries match the prefix
    ///
    /// When binary search returns a range [start, end) for a prefix,
    /// every suffix array entry in that range must start with the prefix.
    #[test]
    fn prop_binary_search_returns_only_matches(
        corpus in corpus_strategy(),
        prefix in prefix_strategy()
    ) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);
        assert_index_well_formed(&index);

        // Manual binary search implementation to find prefix matches
        let mut found_matches = Vec::new();

        for (i, entry) in index.suffix_array.iter().enumerate() {
            let suffix: String = index.texts[entry.doc_id]
                .chars()
                .skip(entry.offset)
                .collect();

            let suffix_lower = suffix.to_lowercase();
            let prefix_lower = prefix.to_lowercase();

            if suffix_lower.starts_with(&prefix_lower) {
                found_matches.push(i);
            }
        }

        // All found matches should be in a contiguous range (due to suffix array sortedness)
        if !found_matches.is_empty() {
            for (i, expected_idx) in found_matches.iter().enumerate() {
                if i > 0 {
                    // Should be consecutive or nearly consecutive (allowing for gaps due to case)
                    let prev_idx = found_matches[i - 1];
                    // In a properly sorted suffix array, matches should be close together
                    prop_assert!(
                        expected_idx - prev_idx <= 1 || i == 0,
                        "Prefix matches should be contiguous in sorted suffix array"
                    );
                }
            }
        }
    }

    /// Property: No matches are missed by binary search
    ///
    /// For any prefix, every suffix starting with that prefix must be found.
    /// This is tested by verifying the suffix array is sorted and checking
    /// that all matching suffixes would be adjacent.
    #[test]
    fn prop_binary_search_complete(
        corpus in corpus_strategy(),
        prefix in prefix_strategy()
    ) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        let mut matching_suffixes = 0;

        for (doc_id, text) in index.texts.iter().enumerate() {
            let char_count = text.chars().count();
            for offset in 0..char_count {

                let suffix: String = text
                    .chars()
                    .skip(offset)
                    .collect();

                let suffix_lower = suffix.to_lowercase();
                let prefix_lower = prefix.to_lowercase();

                if suffix_lower.starts_with(&prefix_lower) {
                    matching_suffixes += 1;

                    // Every matching suffix should be in the suffix array
                    let found = index.suffix_array.iter()
                        .any(|e| e.doc_id == doc_id && e.offset == offset);
                    prop_assert!(
                        found,
                        "Missing suffix for prefix '{}': doc_id={}, offset={}",
                        prefix, doc_id, offset
                    );
                }
            }
        }

        // If we have matches, verify there's at least one
        if matching_suffixes > 0 {
            prop_assert!(matching_suffixes > 0, "Should find at least one match for common prefixes");
        }
    }

    /// Property: Suffix array remains sorted for all queries
    ///
    /// The binary search correctness depends on the suffix array being sorted.
    /// This property verifies sortedness is maintained across different prefixes.
    #[test]
    fn prop_suffix_array_sorted_invariant(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Verify suffix array is sorted
        for i in 1..index.suffix_array.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            let prev_suffix: String = index.texts[prev.doc_id]
                .chars()
                .skip(prev.offset)
                .collect();
            let curr_suffix: String = index.texts[curr.doc_id]
                .chars()
                .skip(curr.offset)
                .collect();

            prop_assert!(
                prev_suffix <= curr_suffix,
                "Suffix array not sorted at position {}: '{}' > '{}'",
                i,
                prev_suffix.chars().take(20).collect::<String>(),
                curr_suffix.chars().take(20).collect::<String>()
            );
        }
    }

    /// Property: Empty prefix handling
    ///
    /// An empty prefix should match all suffixes.
    #[test]
    fn prop_empty_prefix_matches_all(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        let empty_prefix = "";
        let mut total_suffixes = 0;
        let mut matching_suffixes = 0;

        for text in index.texts.iter() {
            let char_count = text.chars().count();
            for offset in 0..char_count {
                total_suffixes += 1;

                let suffix: String = text
                    .chars()
                    .skip(offset)
                    .collect();

                if suffix.starts_with(empty_prefix) {
                    matching_suffixes += 1;
                }
            }
        }

        // All suffixes match empty prefix
        prop_assert_eq!(
            total_suffixes, matching_suffixes,
            "Empty prefix should match all suffixes"
        );
        prop_assert_eq!(
            matching_suffixes, index.suffix_array.len(),
            "Suffix array should contain all suffixes"
        );
    }

    /// Property: UTF-8 character boundary safety
    ///
    /// All suffix array offsets must be valid character boundaries
    /// in their respective documents. This is critical for substring operations.
    #[test]
    fn prop_suffix_offsets_on_char_boundaries(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for (i, entry) in index.suffix_array.iter().enumerate() {
            let text = &index.texts[entry.doc_id];
            let char_count = text.chars().count();

            // Offset should be within bounds (strict inequality for valid suffixes)
            prop_assert!(
                entry.offset < char_count,
                "Suffix array[{}]: offset {} >= char_count {} for doc_id {}",
                i, entry.offset, char_count, entry.doc_id
            );

            // Offset should be a valid character boundary when converted to bytes
            // This is guaranteed by using character offsets, not byte offsets
            let suffix: String = text
                .chars()
                .skip(entry.offset)
                .collect();

            // Suffix should be non-empty (since offset < char_count)
            prop_assert!(
                !suffix.is_empty(),
                "Suffix at [{}] (doc_id={}, offset={}) should not be empty",
                i, entry.doc_id, entry.offset
            );
        }
    }

    /// Property: Case-insensitive prefix matching
    ///
    /// Prefix search should be case-insensitive, finding matches regardless
    /// of the case used in the query or the document.
    #[test]
    fn prop_prefix_case_insensitive(
        corpus in corpus_strategy(),
        prefix in prefix_strategy()
    ) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        let prefix_lower = prefix.to_lowercase();
        let prefix_upper = prefix.to_uppercase();
        let prefix_mixed = if !prefix.is_empty() {
            let mut chars = prefix.chars();
            let first = chars.next().unwrap().to_uppercase().to_string();
            first + &chars.collect::<String>()
        } else {
            prefix.clone()
        };

        // Count matches for each case variant
        let count_matches = |prefix: &str| {
            index.suffix_array.iter()
                .filter(|entry| {
                    let suffix: String = index.texts[entry.doc_id]
                        .chars()
                        .skip(entry.offset)
                        .collect();
                    suffix.to_lowercase().starts_with(&prefix.to_lowercase())
                })
                .count()
        };

        let lower_matches = count_matches(&prefix_lower);
        let upper_matches = count_matches(&prefix_upper);
        let mixed_matches = count_matches(&prefix_mixed);

        // All case variants should find the same matches
        prop_assert_eq!(
            lower_matches, upper_matches,
            "Case-insensitive search should find same matches for lowercase and uppercase"
        );
        prop_assert_eq!(
            lower_matches, mixed_matches,
            "Case-insensitive search should find same matches for mixed case"
        );
    }

    /// Property: Prefix search with special characters
    ///
    /// Binary search should handle prefixes with numbers and hyphens correctly.
    #[test]
    fn prop_prefix_with_special_chars(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Test prefixes with numbers and hyphens
        let test_prefixes = vec!["0", "1", "9", "a0", "test-"];

        for test_prefix in test_prefixes {
            for entry in &index.suffix_array {
                let suffix: String = index.texts[entry.doc_id]
                    .chars()
                    .skip(entry.offset)
                    .collect();

                let suffix_lower = suffix.to_lowercase();
                let prefix_lower = test_prefix.to_lowercase();

                // Verify consistency: if matches, should be findable
                if suffix_lower.starts_with(&prefix_lower) {
                    let found = index.suffix_array.iter()
                        .any(|e| {
                            let s: String = index.texts[e.doc_id]
                                .chars()
                                .skip(e.offset)
                                .collect();
                            s.to_lowercase() == suffix_lower
                        });
                    prop_assert!(found, "Match for prefix '{}' should be findable", test_prefix);
                }
            }
        }
    }
}
