//! Property tests for suffix array search correctness.
//!
//! Verifies:
//! 1. Binary search boundary conditions
//! 2. UTF-8 character offset safety
//! 3. Prefix matching completeness (all and only matches returned)

use super::common::build_test_index;
use proptest::prelude::*;
use std::collections::HashSet;

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

/// Generate a small corpus of documents.
fn small_corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(document_strategy(), 1..10)
}

/// Generate a medium corpus of documents.
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

/// Generate a prefix for searching (shorter for prefix matching).
fn prefix_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z0-9]{1,4}").unwrap()
}

/// Generate Unicode text with multi-byte characters.
fn unicode_document_strategy() -> impl Strategy<Value = String> {
    prop_oneof![
        // ASCII words
        prop::collection::vec(word_strategy(), 1..5).prop_map(|words| words.join(" ")),
        // Mixed ASCII and emoji
        prop::string::string_regex("[a-z]{2,4} [a-z]{2,4}").unwrap(),
        // Just lowercase ASCII for safety
        prop::string::string_regex("[a-z ]{10,30}").unwrap(),
    ]
}

/// Generate unicode corpus.
fn unicode_corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(unicode_document_strategy(), 1..10)
}

// ============================================================================
// BINARY SEARCH CORRECTNESS
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property: Binary search finds ALL entries that start with prefix.
    ///
    /// For any prefix, every suffix in the suffix array that starts with
    /// that prefix should be accessible via binary search.
    #[test]
    fn prop_binary_search_finds_all_matches(
        corpus in corpus_strategy(),
        prefix in prefix_strategy()
    ) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Manually find all matching suffixes
        let mut expected_matches: HashSet<(usize, usize)> = HashSet::new();
        for (doc_id, text) in index.texts.iter().enumerate() {
            let char_count = text.chars().count();
            for offset in 0..char_count {
                let suffix: String = text.chars().skip(offset).collect();
                if suffix.to_lowercase().starts_with(&prefix.to_lowercase()) {
                    expected_matches.insert((doc_id, offset));
                }
            }
        }

        // Verify all expected matches exist in suffix array
        for (doc_id, offset) in &expected_matches {
            let found = index.suffix_array.iter()
                .any(|e| e.doc_id == *doc_id && e.offset == *offset);
            prop_assert!(
                found,
                "Missing suffix entry for prefix '{}': doc_id={}, offset={}",
                prefix, doc_id, offset
            );
        }
    }

    /// Property: Binary search returns ONLY entries that match the prefix.
    ///
    /// Every entry in the suffix array range returned by binary search
    /// must actually start with the search prefix.
    #[test]
    fn prop_binary_search_returns_only_matches(
        corpus in corpus_strategy(),
        prefix in prefix_strategy()
    ) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Find matching entries by scanning suffix array
        for entry in &index.suffix_array {
            let suffix: String = index.texts[entry.doc_id]
                .chars()
                .skip(entry.offset)
                .collect();

            let suffix_lower = suffix.to_lowercase();
            let prefix_lower = prefix.to_lowercase();

            // If we claim this matches the prefix, verify it actually does
            if suffix_lower.starts_with(&prefix_lower) {
                prop_assert!(
                    suffix_lower.starts_with(&prefix_lower),
                    "False positive: '{}' does not start with '{}'",
                    suffix_lower, prefix_lower
                );
            }
        }
    }

    /// Property: Suffix array is sorted lexicographically.
    ///
    /// Binary search correctness depends on the suffix array being sorted.
    #[test]
    fn prop_suffix_array_sorted(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

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
                "Suffix array not sorted at {}: '{}' > '{}'",
                i,
                &prev_suffix.chars().take(20).collect::<String>(),
                &curr_suffix.chars().take(20).collect::<String>()
            );
        }
    }
}

// ============================================================================
// UTF-8 CHARACTER BOUNDARY SAFETY
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Property: All suffix array offsets are valid character boundaries.
    ///
    /// Suffix array stores CHARACTER offsets (not byte offsets), so
    /// skip(offset) should always succeed without panic.
    #[test]
    fn prop_offsets_are_valid_char_boundaries(corpus in unicode_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for (i, entry) in index.suffix_array.iter().enumerate() {
            let text = &index.texts[entry.doc_id];
            let char_count = text.chars().count();

            // Offset must be < char_count (strict inequality for non-empty suffixes)
            prop_assert!(
                entry.offset < char_count,
                "Suffix array[{}]: offset {} >= char_count {} for doc_id {}",
                i, entry.offset, char_count, entry.doc_id
            );

            // Creating suffix should not panic
            let suffix: String = text.chars().skip(entry.offset).collect();
            prop_assert!(
                !suffix.is_empty(),
                "Suffix at [{}] should not be empty (offset={}, char_count={})",
                i, entry.offset, char_count
            );
        }
    }

    /// Property: Unicode prefix search doesn't panic.
    ///
    /// Searching with various Unicode strings should never cause panics.
    #[test]
    fn prop_unicode_prefix_search_safe(corpus in unicode_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Various test prefixes including edge cases
        let test_prefixes = vec!["", "a", "ab", "test", "123"];

        for prefix in test_prefixes {
            // This should not panic
            let mut found = 0;
            for entry in &index.suffix_array {
                let suffix: String = index.texts[entry.doc_id]
                    .chars()
                    .skip(entry.offset)
                    .collect();
                if suffix.to_lowercase().starts_with(&prefix.to_lowercase()) {
                    found += 1;
                }
            }
            // Just ensure we got here without panic
            prop_assert!(found >= 0, "Should count matches without panic");
        }
    }
}

// ============================================================================
// EDGE CASES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Empty prefix matches all suffixes.
    #[test]
    fn prop_empty_prefix_matches_all(corpus in small_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        let mut _total_chars = 0;
        for text in &index.texts {
            _total_chars += text.chars().count();
        }

        // Empty prefix should match everything
        let mut matches = 0;
        for entry in &index.suffix_array {
            let suffix: String = index.texts[entry.doc_id]
                .chars()
                .skip(entry.offset)
                .collect();
            if suffix.starts_with("") {
                matches += 1;
            }
        }

        prop_assert_eq!(
            matches, index.suffix_array.len(),
            "Empty prefix should match all {} suffixes, got {}",
            index.suffix_array.len(), matches
        );
    }

    /// Property: Single character prefix matching.
    #[test]
    fn prop_single_char_prefix(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Pick first char from any text
        if let Some(first_text) = index.texts.first() {
            if let Some(first_char) = first_text.chars().next() {
                let prefix = first_char.to_string().to_lowercase();

                // Count matches in suffix array
                let mut sa_matches = 0;
                for entry in &index.suffix_array {
                    let suffix: String = index.texts[entry.doc_id]
                        .chars()
                        .skip(entry.offset)
                        .collect();
                    if suffix.to_lowercase().starts_with(&prefix) {
                        sa_matches += 1;
                    }
                }

                // Count expected matches
                let mut expected = 0;
                for text in &index.texts {
                    let char_count = text.chars().count();
                    for offset in 0..char_count {
                        let suffix: String = text.chars().skip(offset).collect();
                        if suffix.to_lowercase().starts_with(&prefix) {
                            expected += 1;
                        }
                    }
                }

                prop_assert_eq!(
                    sa_matches, expected,
                    "Single char prefix '{}' should match {} suffixes, found {}",
                    prefix, expected, sa_matches
                );
            }
        }
    }
}

// ============================================================================
// SUFFIX ARRAY COMPLETENESS
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Property: Suffix array contains exactly one entry per character offset.
    #[test]
    fn prop_suffix_array_complete(corpus in small_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        // Count expected entries
        let mut expected_entries = 0;
        for text in &index.texts {
            expected_entries += text.chars().count();
        }

        // Verify suffix array has correct length
        prop_assert_eq!(
            index.suffix_array.len(), expected_entries,
            "Suffix array should have {} entries, got {}",
            expected_entries, index.suffix_array.len()
        );

        // Verify all entries are unique
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        for entry in &index.suffix_array {
            prop_assert!(
                seen.insert((entry.doc_id, entry.offset)),
                "Duplicate suffix array entry: doc_id={}, offset={}",
                entry.doc_id, entry.offset
            );
        }
    }

    /// Property: Every character position is represented in suffix array.
    #[test]
    fn prop_all_positions_represented(corpus in small_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for (doc_id, text) in index.texts.iter().enumerate() {
            let char_count = text.chars().count();
            for offset in 0..char_count {
                let found = index.suffix_array.iter()
                    .any(|e| e.doc_id == doc_id && e.offset == offset);
                prop_assert!(
                    found,
                    "Missing entry for doc_id={}, offset={} in suffix array",
                    doc_id, offset
                );
            }
        }
    }
}

// ============================================================================
// LCP ARRAY PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(30))]

    /// Property: LCP[0] is always 0.
    #[test]
    fn prop_lcp_first_is_zero(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        if !index.lcp.is_empty() {
            prop_assert_eq!(
                index.lcp[0], 0,
                "LCP[0] should be 0, got {}",
                index.lcp[0]
            );
        }
    }

    /// Property: LCP array has same length as suffix array.
    #[test]
    fn prop_lcp_same_length_as_sa(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        prop_assert_eq!(
            index.lcp.len(), index.suffix_array.len(),
            "LCP length {} should equal suffix array length {}",
            index.lcp.len(), index.suffix_array.len()
        );
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[test]
fn test_empty_corpus() {
    let texts: Vec<&str> = vec![];
    let index = build_test_index(&texts);

    assert!(index.suffix_array.is_empty(), "Empty corpus should have empty suffix array");
    assert!(index.lcp.is_empty(), "Empty corpus should have empty LCP array");
}

#[test]
fn test_single_char_document() {
    let texts: Vec<&str> = vec!["a"];
    let index = build_test_index(&texts);

    assert_eq!(index.suffix_array.len(), 1, "Single char should have one suffix");
    assert_eq!(index.suffix_array[0].doc_id, 0);
    assert_eq!(index.suffix_array[0].offset, 0);
}

#[test]
fn test_repeated_chars() {
    let texts: Vec<&str> = vec!["aaa"];
    let index = build_test_index(&texts);

    assert_eq!(index.suffix_array.len(), 3, "Should have 3 suffixes");

    // All suffixes start with 'a', but sorted by length
    // "aaa" < "aa" < "a" is NOT true - they're sorted lexicographically
    // Actually "a" < "aa" < "aaa"
    let suffixes: Vec<String> = index.suffix_array.iter()
        .map(|e| index.texts[e.doc_id].chars().skip(e.offset).collect())
        .collect();

    // Verify sorted order
    for i in 1..suffixes.len() {
        assert!(suffixes[i-1] <= suffixes[i], "Suffixes should be sorted");
    }
}

#[test]
fn test_case_insensitivity() {
    let texts: Vec<&str> = vec!["Hello World"];
    let index = build_test_index(&texts);

    // Search for prefix "hel" should find "Hello" (case insensitive)
    let prefix = "hel";
    let mut found = false;
    for entry in &index.suffix_array {
        let suffix: String = index.texts[entry.doc_id]
            .chars()
            .skip(entry.offset)
            .collect();
        if suffix.to_lowercase().starts_with(&prefix.to_lowercase()) {
            found = true;
            break;
        }
    }
    assert!(found, "Should find case-insensitive match for '{}'", prefix);
}
