//! Suffix array property tests.
//!
//! These tests verify suffix array invariants from SuffixArray.lean:
//! - Sortedness: suffixes are in lexicographic order
//! - Completeness: every position is represented
//! - LCP correctness: longest common prefix values are accurate
//! - Unicode: multi-byte UTF-8 characters are handled correctly

use super::common::{assert_index_well_formed, build_test_index};
use proptest::prelude::*;

// ============================================================================
// STRATEGIES
// ============================================================================

/// Generate random word-like strings.
fn word_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z0-9]{2,8}").unwrap()
}

/// Generate random document text (multiple words).
fn document_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(word_strategy(), 1..10).prop_map(|words| words.join(" "))
}

/// Generate a corpus of documents.
fn corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(document_strategy(), 1..5)
}

/// Generate Unicode words with diacritics and multi-byte characters.
fn unicode_word_strategy() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        // Latin with diacritics
        "café".to_string(),
        "naïve".to_string(),
        "résumé".to_string(),
        "über".to_string(),
        "tōkyō".to_string(),
        // Names with special characters
        "harīṣh".to_string(),
        "tummalachērla".to_string(),
        "māori".to_string(),
        // Telugu script
        "తెలుగు".to_string(),
        "హరీష్".to_string(),
        // Mixed ASCII
        "hello".to_string(),
        "world".to_string(),
        "test".to_string(),
        "search".to_string(),
    ])
}

/// Generate documents containing Unicode text.
fn unicode_document_strategy() -> impl Strategy<Value = String> {
    prop::collection::vec(unicode_word_strategy(), 2..6).prop_map(|words| words.join(" "))
}

/// Generate a corpus with Unicode content.
fn unicode_corpus_strategy() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(unicode_document_strategy(), 1..4)
}

// ============================================================================
// ASCII SUFFIX ARRAY PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Property: For any corpus, the resulting suffix array is sorted.
    ///
    /// Lean spec: SuffixArray.Sorted
    #[test]
    fn prop_suffix_array_always_sorted(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for i in 1..index.suffix_array.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            let prev_suffix = index.texts.get(prev.doc_id)
                .and_then(|t| t.get(prev.offset..))
                .unwrap_or("");
            let curr_suffix = index.texts.get(curr.doc_id)
                .and_then(|t| t.get(curr.offset..))
                .unwrap_or("");

            prop_assert!(
                prev_suffix <= curr_suffix,
                "Suffix array not sorted at {}: '{}' > '{}'",
                i, prev_suffix, curr_suffix
            );
        }
    }

    /// Property: For any corpus, all invariants hold.
    ///
    /// Lean spec: WellFormedIndex
    #[test]
    fn prop_index_always_well_formed(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);
        assert_index_well_formed(&index);
    }

    /// Property: Suffix array is complete (contains all suffixes).
    ///
    /// Lean spec: SuffixArray.Complete
    #[test]
    fn prop_suffix_array_complete(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for (doc_id, text) in index.texts.iter().enumerate() {
            for offset in 0..text.len() {
                let found = index.suffix_array.iter()
                    .any(|e| e.doc_id == doc_id && e.offset == offset);
                prop_assert!(
                    found,
                    "Missing suffix for doc_id={}, offset={}",
                    doc_id, offset
                );
            }
        }
    }

    /// Property: LCP values are correct.
    ///
    /// Lean spec: LCP correctness (lcp[0] = 0, lcp[i] = common prefix length)
    #[test]
    fn prop_lcp_correct(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        prop_assert_eq!(index.lcp.len(), index.suffix_array.len());

        if !index.lcp.is_empty() {
            prop_assert_eq!(index.lcp[0], 0);
        }

        for i in 1..index.lcp.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            let prev_suffix = &index.texts[prev.doc_id][prev.offset..];
            let curr_suffix = &index.texts[curr.doc_id][curr.offset..];

            let expected = prev_suffix.chars()
                .zip(curr_suffix.chars())
                .take_while(|(a, b)| a == b)
                .count();

            prop_assert_eq!(
                index.lcp[i], expected,
                "LCP[{}] = {} but expected {}",
                i, index.lcp[i], expected
            );
        }
    }
}

// ============================================================================
// UNICODE SUFFIX ARRAY PROPERTIES
// ============================================================================
//
// These ensure multi-byte UTF-8 characters don't break the suffix array.
// Critical for international text search.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property: Suffix array with Unicode text is still sorted.
    #[test]
    fn prop_suffix_array_unicode_sorted(corpus in unicode_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for i in 1..index.suffix_array.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            // Use character-based slicing for proper Unicode comparison
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
                "Unicode suffix array not sorted at {}: '{}' > '{}'",
                i, prev_suffix, curr_suffix
            );
        }
    }

    /// Property: Suffix array offsets are character offsets, not byte offsets.
    #[test]
    fn prop_suffix_array_char_offsets(corpus in unicode_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for entry in &index.suffix_array {
            let text = &index.texts[entry.doc_id];
            let char_count = text.chars().count();

            // Offset should be < char_count (not byte count)
            prop_assert!(
                entry.offset < char_count,
                "Suffix entry offset {} >= char_count {} for text '{}' (byte_len={})",
                entry.offset, char_count, text, text.len()
            );
        }
    }

    /// Property: LCP is correct for Unicode text.
    #[test]
    fn prop_lcp_unicode_correct(corpus in unicode_corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for i in 1..index.lcp.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            // Get suffixes using character offsets
            let prev_suffix: String = index.texts[prev.doc_id]
                .chars()
                .skip(prev.offset)
                .collect();
            let curr_suffix: String = index.texts[curr.doc_id]
                .chars()
                .skip(curr.offset)
                .collect();

            // LCP should be in characters, not bytes
            let expected = prev_suffix.chars()
                .zip(curr_suffix.chars())
                .take_while(|(a, b)| a == b)
                .count();

            prop_assert_eq!(
                index.lcp[i], expected,
                "Unicode LCP[{}] = {} but expected {} for '{}' vs '{}'",
                i, index.lcp[i], expected, prev_suffix, curr_suffix
            );
        }
    }
}
