//! Property-based tests using proptest.
//!
//! These tests verify that invariants hold for randomly generated inputs,
//! providing high confidence that the implementation matches the Lean specs.

mod common;

use common::{assert_index_well_formed, build_test_index};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use sorex::{field_type_score, levenshtein_within, search, FieldType};

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
// SUFFIX ARRAY PROPERTIES
// ============================================================================

proptest! {
    /// Property: For any corpus, the resulting suffix array is sorted.
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
    #[test]
    fn prop_index_always_well_formed(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);
        assert_index_well_formed(&index);
    }

    /// Property: Suffix array is complete (contains all suffixes).
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

    // ========================================================================
    // UNICODE SUFFIX ARRAY PROPERTIES
    // These ensure multi-byte UTF-8 characters don't break the suffix array
    // ========================================================================

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

// ============================================================================
// SEARCH PROPERTIES
// ============================================================================

proptest! {
    /// Property: Searching for a substring that exists finds the document.
    #[test]
    fn prop_search_finds_substrings(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        for (doc_id, text) in index.texts.iter().enumerate() {
            // Skip empty or very short texts
            if text.len() < 3 {
                continue;
            }

            // Pick a substring to search for
            let start = 0;
            let end = text.len().min(5);
            let query = &text[start..end];

            let results = search(&index, query);

            // The document containing this text should be in results
            prop_assert!(
                results.iter().any(|doc| doc.id == doc_id),
                "Search for '{}' did not find doc {} containing '{}'",
                query, doc_id, text
            );
        }
    }

    /// Property: Empty search returns no results.
    #[test]
    fn prop_empty_search_returns_empty(corpus in corpus_strategy()) {
        let texts: Vec<&str> = corpus.iter().map(|s| s.as_str()).collect();
        let index = build_test_index(&texts);

        let results = search(&index, "");
        prop_assert!(results.is_empty(), "Empty search should return no results");

        let results = search(&index, "   ");
        prop_assert!(results.is_empty(), "Whitespace search should return no results");
    }
}

// ============================================================================
// SCORING PROPERTIES
// ============================================================================

proptest! {
    /// Property: Field type dominance holds for any position combination.
    #[test]
    fn prop_field_hierarchy_preserved(
        title_offset in 0usize..1000,
        content_offset in 0usize..1000,
        text_len in 100usize..2000
    ) {
        let title_base = field_type_score(&FieldType::Title);
        let content_base = field_type_score(&FieldType::Content);
        let max_boost = 0.5;

        // Position bonus: 0 to max_boost based on position
        let title_bonus = max_boost * (1.0 - (title_offset.min(text_len) as f64 / text_len as f64));
        let content_bonus = max_boost * (1.0 - (content_offset.min(text_len) as f64 / text_len as f64));

        let title_score = title_base + title_bonus;
        let content_score = content_base + content_bonus;

        prop_assert!(
            title_score > content_score,
            "Title score ({}) should always beat content score ({})",
            title_score, content_score
        );
    }
}

// ============================================================================
// LEVENSHTEIN PROPERTIES
// ============================================================================

/// Generate strings with Unicode diacritics (multi-byte UTF-8 characters).
/// These test the byte-vs-char length bug that was fixed.
fn unicode_diacritic_strategy() -> impl Strategy<Value = String> {
    // Mix of ASCII and diacritic variants
    prop::sample::select(vec![
        // Latin with diacritics
        "café".to_string(),
        "naïve".to_string(),
        "résumé".to_string(),
        "tête-à-tête".to_string(),
        // Names with macrons/dots
        "harīṣh".to_string(),
        "tummalachērla".to_string(),
        "māori".to_string(),
        "tōkyō".to_string(),
        // Mixed
        "hello".to_string(),
        "world".to_string(),
        "café au lait".to_string(),
        "über".to_string(),
        // Telugu (multi-byte)
        "తెలుగు".to_string(),
        "హరీష్".to_string(),
        // Various combining characters
        "e\u{0301}".to_string(), // é as e + combining acute
        "n\u{0303}".to_string(), // ñ as n + combining tilde
    ])
}

/// Generate pairs of ASCII and diacritic versions of similar words.
fn ascii_diacritic_pair_strategy() -> impl Strategy<Value = (String, String)> {
    prop::sample::select(vec![
        ("cafe".to_string(), "café".to_string()),
        ("naive".to_string(), "naïve".to_string()),
        ("resume".to_string(), "résumé".to_string()),
        ("tete".to_string(), "tête".to_string()),
        ("harish".to_string(), "harīṣh".to_string()),
        ("tummalacherla".to_string(), "tummalachērla".to_string()),
        ("uber".to_string(), "über".to_string()),
        ("tokyo".to_string(), "tōkyō".to_string()),
    ])
}

proptest! {
    /// Property: Levenshtein distance lower bound is length difference.
    #[test]
    fn prop_levenshtein_length_bound(
        a in "[a-z]{0,20}",
        b in "[a-z]{0,20}"
    ) {
        // Use CHAR count, not byte count
        let len_diff = (a.chars().count() as isize - b.chars().count() as isize).unsigned_abs();

        // If length difference > max, should return false
        if len_diff > 2 {
            prop_assert!(
                !levenshtein_within(&a, &b, 1),
                "levenshtein_within should return false when char_len_diff ({}) > max (1)",
                len_diff
            );
        }
    }

    /// Property: Identical strings have distance 0.
    #[test]
    fn prop_levenshtein_identical(s in "[a-z]{1,20}") {
        prop_assert!(
            levenshtein_within(&s, &s, 0),
            "Identical strings should have distance 0"
        );
    }

    /// Property: Single character difference has distance 1.
    #[test]
    fn prop_levenshtein_single_char(s in "[a-z]{2,20}") {
        // Swap first two characters
        let mut chars: Vec<char> = s.chars().collect();
        if chars.len() >= 2 && chars[0] != chars[1] {
            chars.swap(0, 1);
            let modified: String = chars.into_iter().collect();
            prop_assert!(
                levenshtein_within(&s, &modified, 2),
                "Swapped string should be within distance 2"
            );
        }
    }

    // ========================================================================
    // UNICODE-SPECIFIC LEVENSHTEIN PROPERTIES
    // These tests specifically catch the byte-vs-char bug
    // ========================================================================

    /// Property: Levenshtein works correctly with multi-byte Unicode characters.
    /// This is the regression test for the byte-vs-char length bug.
    #[test]
    fn prop_levenshtein_unicode_identical(s in unicode_diacritic_strategy()) {
        prop_assert!(
            levenshtein_within(&s, &s, 0),
            "Identical Unicode string '{}' should have distance 0",
            s
        );
    }

    /// Property: ASCII and diacritic versions of similar words have small edit distance.
    /// This tests fuzzy matching across diacritics.
    #[test]
    fn prop_levenshtein_ascii_diacritic_close((ascii, diacritic) in ascii_diacritic_pair_strategy()) {
        // Distance should be small (number of diacritic substitutions)
        // Most pairs differ by 1-3 characters
        prop_assert!(
            levenshtein_within(&ascii, &diacritic, 3),
            "ASCII '{}' and diacritic '{}' should be within distance 3",
            ascii, diacritic
        );
    }

    /// Property: Levenshtein with Unicode uses character count, not byte count.
    /// Explicitly tests that byte length != char length doesn't break the algorithm.
    #[test]
    fn prop_levenshtein_byte_char_invariant(s in unicode_diacritic_strategy()) {
        let char_count = s.chars().count();
        let byte_count = s.len();

        // For strings where byte_count != char_count (multi-byte UTF-8),
        // the algorithm should still work correctly
        if byte_count != char_count {
            // Adding one character should have distance 1
            let extended = format!("{}x", s);
            prop_assert!(
                levenshtein_within(&s, &extended, 1),
                "Adding one char to '{}' (chars={}, bytes={}) should be distance 1",
                s, char_count, byte_count
            );

            // Identical strings should still match
            prop_assert!(
                levenshtein_within(&s, &s, 0),
                "Identical Unicode '{}' (chars={}, bytes={}) should be distance 0",
                s, char_count, byte_count
            );
        }
    }

    /// Property: Early-exit optimization uses character length, not byte length.
    #[test]
    fn prop_levenshtein_early_exit_unicode((ascii, diacritic) in ascii_diacritic_pair_strategy()) {
        let ascii_chars = ascii.chars().count();
        let diacritic_chars = diacritic.chars().count();
        let char_diff = (ascii_chars as isize - diacritic_chars as isize).unsigned_abs();

        // If char lengths are equal, strings might still match with fuzzy
        // The early-exit should NOT reject based on byte length difference
        if char_diff == 0 {
            // Strings with same char count but different bytes (due to diacritics)
            // should NOT be rejected by early-exit
            let result = levenshtein_within(&ascii, &diacritic, ascii_chars);
            prop_assert!(
                result,
                "Same char count strings '{}' and '{}' should not be rejected by early-exit",
                ascii, diacritic
            );
        }
    }
}

// ============================================================================
// INVERTED INDEX PROPERTIES
// ============================================================================

proptest! {
    /// Property: Inverted index posting lists are always sorted.
    #[test]
    fn prop_inverted_index_sorted(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);

        for (term, list) in &index.terms {
            for i in 1..list.postings.len() {
                let prev = &list.postings[i - 1];
                let curr = &list.postings[i];

                prop_assert!(
                    (prev.doc_id, prev.offset) < (curr.doc_id, curr.offset),
                    "Posting list for '{}' not sorted at {}: ({}, {}) >= ({}, {})",
                    term, i, prev.doc_id, prev.offset, curr.doc_id, curr.offset
                );
            }
        }
    }

    /// Property: Document frequency is correct.
    #[test]
    fn prop_inverted_index_doc_freq(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);

        for (term, list) in &index.terms {
            let mut unique_docs: Vec<usize> = list.postings.iter()
                .map(|p| p.doc_id)
                .collect();
            unique_docs.sort();
            unique_docs.dedup();

            prop_assert_eq!(
                list.doc_freq, unique_docs.len(),
                "doc_freq for '{}' is {} but {} unique docs found",
                term, list.doc_freq, unique_docs.len()
            );
        }
    }

    /// Property: All postings point to valid locations.
    #[test]
    fn prop_inverted_index_postings_valid(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);

        for (term, list) in &index.terms {
            let term_len = term.len();

            for posting in &list.postings {
                prop_assert!(
                    posting.doc_id < corpus.len(),
                    "Posting for '{}' has invalid doc_id {}",
                    term, posting.doc_id
                );

                let text = &corpus[posting.doc_id];
                prop_assert!(
                    posting.offset + term_len <= text.len(),
                    "Posting for '{}' has invalid offset {} in doc of len {}",
                    term, posting.offset, text.len()
                );
            }
        }
    }

    /// Property: Total docs matches corpus size.
    #[test]
    fn prop_inverted_index_total_docs(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);
        prop_assert_eq!(index.total_docs, corpus.len());
    }

    /// Property: Every non-stop word in corpus appears in index.
    #[test]
    fn prop_inverted_index_complete(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);

        for (doc_id, text) in corpus.iter().enumerate() {
            for word in text.split_whitespace() {
                let normalized = sorex::normalize(word);
                if normalized.is_empty() {
                    continue;
                }

                // Stop words are intentionally filtered from the index
                if sorex::is_stop_word(&normalized) {
                    continue;
                }

                let list = index.terms.get(&normalized);
                prop_assert!(
                    list.is_some(),
                    "Word '{}' not found in inverted index",
                    normalized
                );

                let list = list.unwrap();
                prop_assert!(
                    list.postings.iter().any(|p| p.doc_id == doc_id),
                    "Word '{}' in doc {} not found in posting list",
                    normalized, doc_id
                );
            }
        }
    }

    /// Property: Validated inverted index accepts well-formed index.
    #[test]
    fn prop_validated_inverted_index(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);
        let result = sorex::ValidatedInvertedIndex::new(index, &corpus);

        prop_assert!(
            result.is_ok(),
            "ValidatedInvertedIndex should accept build output: {:?}",
            result.err()
        );
    }

    /// Property: Index mode selection is deterministic.
    #[test]
    fn prop_index_mode_deterministic(
        doc_count in 1usize..2000,
        bytes in 1000usize..5_000_000,
        prefix in proptest::bool::ANY,
        fuzzy in proptest::bool::ANY
    ) {
        let thresholds = sorex::IndexThresholds::default();

        let mode1 = sorex::select_index_mode(doc_count, bytes, prefix, fuzzy, &thresholds);
        let mode2 = sorex::select_index_mode(doc_count, bytes, prefix, fuzzy, &thresholds);

        prop_assert_eq!(mode1, mode2, "Index mode selection should be deterministic");
    }
}

// ============================================================================
// STREAMING SEARCH PROPERTIES
// ============================================================================
// These tests verify the streaming search invariants from StreamingSearch.lean:
// - exact_subset_full: exact results ⊆ full results
// - expanded_disjoint_exact: expanded ∩ exact = ∅
// - streaming_preserves_ranking: score ordering is maintained

proptest! {
    /// Property: Exact search results are always a subset of full search results.
    ///
    /// Lean spec: exact_subset_full
    #[test]
    fn prop_streaming_exact_subset_full(corpus in corpus_strategy(), query in word_strategy()) {
        let index = common::build_hybrid_test_index(&corpus);

        let exact_results = sorex::search_exact(&index, &query);
        let full_results = sorex::search_hybrid(&index, &query);

        // Every exact result should be in full results
        for exact in &exact_results {
            prop_assert!(
                full_results.iter().any(|f| f.id == exact.id),
                "Exact result {} for query '{}' not found in full results",
                exact.id, query
            );
        }
    }

    /// Property: Expanded search results are disjoint from exact results.
    ///
    /// Lean spec: expanded_disjoint_exact
    #[test]
    fn prop_streaming_expanded_disjoint(corpus in corpus_strategy(), query in word_strategy()) {
        let index = common::build_hybrid_test_index(&corpus);

        let exact_results = sorex::search_exact(&index, &query);
        let exact_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();
        let expanded_results = sorex::search_expanded(&index, &query, &exact_ids);

        // No expanded result should be in exact results
        for expanded in &expanded_results {
            prop_assert!(
                !exact_ids.contains(&expanded.id),
                "Expanded result {} for query '{}' duplicates exact result",
                expanded.id, query
            );
        }
    }

    /// Property: Full search results are covered by streaming union.
    ///
    /// Lean spec: union_complete (full ⊆ exact ∪ expanded ∪ fuzzy)
    #[test]
    fn prop_streaming_covers_full(corpus in corpus_strategy(), query in word_strategy()) {
        let index = common::build_hybrid_test_index(&corpus);

        // Phase 1: Exact match (inverted index)
        let exact_results = sorex::search_exact(&index, &query);
        let exact_ids: Vec<usize> = exact_results.iter().map(|r| r.id).collect();

        // Phase 2: Expanded match (suffix array prefix/substring)
        let expanded_results = sorex::search_expanded(&index, &query, &exact_ids);
        let mut exclude_ids = exact_ids.clone();
        exclude_ids.extend(expanded_results.iter().map(|r| r.id));

        // Phase 3: Fuzzy match (Levenshtein distance)
        let fuzzy_results = sorex::search_fuzzy(&index, &query, &exclude_ids);

        // Full search (all 3 tiers)
        let full_results = sorex::search_hybrid(&index, &query);

        // Union of exact, expanded, and fuzzy
        let mut union_ids: Vec<usize> = exact_ids.clone();
        union_ids.extend(expanded_results.iter().map(|r| r.id));
        union_ids.extend(fuzzy_results.iter().map(|r| r.id));

        // Every full result should be in the union
        for full in &full_results {
            prop_assert!(
                union_ids.contains(&full.id),
                "Full result {} for query '{}' not covered by streaming union",
                full.id, query
            );
        }
    }

    /// Property: Empty query returns empty results for all search phases.
    #[test]
    fn prop_streaming_empty_query(corpus in corpus_strategy()) {
        let index = common::build_hybrid_test_index(&corpus);

        prop_assert!(sorex::search_exact(&index, "").is_empty());
        prop_assert!(sorex::search_expanded(&index, "", &[]).is_empty());
        prop_assert!(sorex::search_hybrid(&index, "").is_empty());
    }
}

// ============================================================================
// SECTION NAVIGATION PROPERTIES
// ============================================================================
//
// These tests verify the section invariants specified in `lean/SearchVerified/Section.lean`:
// - offset_maps_to_unique_section: Every offset maps to at most one section
// - title_has_no_section_id: Title fields have section_id = None
// - content_inherits_section: Content fields inherit section_id from parent heading
// ============================================================================

/// Strategy for generating valid section IDs (like rehype-slug output).
fn section_id_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z][a-z0-9-]{0,20}").unwrap()
}

/// Strategy for generating a list of non-overlapping sections.
fn sections_strategy(doc_length: usize) -> impl Strategy<Value = Vec<sorex::Section>> {
    if doc_length == 0 {
        return Just(vec![]).boxed();
    }

    // Generate 1-5 section boundaries
    prop::collection::vec(0usize..doc_length, 0..5)
        .prop_flat_map(move |mut boundaries| {
            // Sort and dedupe boundaries
            boundaries.sort();
            boundaries.dedup();

            // Add start (0) and end (doc_length) if not present
            if boundaries.first() != Some(&0) {
                boundaries.insert(0, 0);
            }
            if boundaries.last() != Some(&doc_length) {
                boundaries.push(doc_length);
            }

            // Generate IDs for each section
            let num_sections = boundaries.len().saturating_sub(1);
            prop::collection::vec(section_id_strategy(), num_sections).prop_map(move |ids| {
                ids.into_iter()
                    .enumerate()
                    .map(|(i, id)| sorex::Section {
                        id,
                        start_offset: boundaries[i],
                        end_offset: boundaries[i + 1],
                        level: ((i % 6) + 1) as u8,
                    })
                    .collect()
            })
        })
        .boxed()
}

proptest! {
    /// Property: Generated sections are always non-overlapping.
    ///
    /// Lean spec: Section.NonOverlapping
    #[test]
    fn prop_sections_non_overlapping(
        doc_length in 10usize..1000,
    ) {
        let sections = sections_strategy(doc_length)
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();

        for i in 0..sections.len() {
            for j in (i + 1)..sections.len() {
                prop_assert!(
                    sections[i].non_overlapping(&sections[j]),
                    "Sections {} and {} overlap: [{}, {}) and [{}, {})",
                    i, j,
                    sections[i].start_offset, sections[i].end_offset,
                    sections[j].start_offset, sections[j].end_offset
                );
            }
        }
    }

    /// Property: Every offset maps to at most one section.
    ///
    /// Lean spec: offset_maps_to_unique_section (proven in Section.lean)
    #[test]
    fn prop_offset_unique_section(
        doc_length in 10usize..500,
    ) {
        let sections = sections_strategy(doc_length)
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();

        // For each offset in the document, check it's in at most one section
        for offset in 0..doc_length {
            let containing: Vec<_> = sections.iter()
                .filter(|s| s.contains(offset))
                .collect();

            prop_assert!(
                containing.len() <= 1,
                "Offset {} contained in {} sections (expected at most 1)",
                offset, containing.len()
            );
        }
    }

    /// Property: Section IDs are valid URL anchors.
    ///
    /// Lean spec: Section.validId / validSectionIdChar
    #[test]
    fn prop_section_ids_valid(id in section_id_strategy()) {
        // Create a section with this ID
        let section = sorex::Section {
            id: id.clone(),
            start_offset: 0,
            end_offset: 10,
            level: 2,
        };

        prop_assert!(
            section.is_valid_id(),
            "Section ID '{}' is not valid for URL anchors",
            id
        );
    }

    /// Property: find_section_at_offset finds the correct section.
    ///
    /// Lean spec: Follows from offset_maps_to_unique_section
    #[test]
    fn prop_find_section_correct(
        doc_length in 10usize..500,
    ) {
        let sections = sections_strategy(doc_length)
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();

        // For each section, verify find_section_at_offset returns it
        for section in &sections {
            // Check a point in the middle of the section
            let mid = (section.start_offset + section.end_offset) / 2;
            let found = sorex::find_section_at_offset(&sections, mid);

            prop_assert_eq!(
                found, Some(section.id.as_str()),
                "find_section_at_offset({}) returned {:?} but expected {:?}",
                mid, found, Some(&section.id)
            );
        }
    }

    /// Property: validate_sections accepts well-formed section lists.
    ///
    /// Lean spec: validSectionList
    #[test]
    fn prop_validate_sections_accepts_valid(
        doc_length in 10usize..500,
    ) {
        let sections = sections_strategy(doc_length)
            .new_tree(&mut proptest::test_runner::TestRunner::default())
            .unwrap()
            .current();

        let result = sorex::validate_sections(&sections, doc_length);
        prop_assert!(
            result.is_ok(),
            "validate_sections rejected valid sections: {:?}",
            result.err()
        );
    }

    /// Property: Section well-formedness (start < end).
    ///
    /// Lean spec: Section.WellFormed
    #[test]
    fn prop_section_well_formed(
        start in 0usize..1000,
        length in 1usize..100,
    ) {
        let section = sorex::Section {
            id: "test".to_string(),
            start_offset: start,
            end_offset: start + length,
            level: 2,
        };

        prop_assert!(
            section.is_well_formed(),
            "Section [{}, {}) should be well-formed",
            section.start_offset, section.end_offset
        );
    }

    /// Property: Empty section ID is invalid.
    #[test]
    fn prop_empty_section_id_invalid(_dummy in 0..1i32) {
        let section = sorex::Section {
            id: "".to_string(),
            start_offset: 0,
            end_offset: 10,
            level: 2,
        };

        prop_assert!(
            !section.is_valid_id(),
            "Empty section ID should be invalid"
        );
    }
}
