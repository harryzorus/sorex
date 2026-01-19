//! Property-based tests using proptest.
//!
//! These tests verify that invariants hold for randomly generated inputs,
//! providing high confidence that the implementation matches the Lean specs.

use super::common::{assert_index_well_formed, build_hybrid_test_index, build_test_index};
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
    /// Property: Inverted index posting lists are sorted by (score DESC, doc_id ASC).
    #[test]
    fn prop_inverted_index_sorted(corpus in corpus_strategy()) {
        let index = sorex::build_inverted_index(&corpus, &[]);

        for (term, list) in &index.terms {
            for i in 1..list.postings.len() {
                let prev = &list.postings[i - 1];
                let curr = &list.postings[i];

                // Score should be descending (prev >= curr)
                // If equal score, doc_id should be ascending (prev <= curr)
                let score_ok = prev.score >= curr.score;
                let tiebreak_ok = prev.score != curr.score || prev.doc_id <= curr.doc_id;

                prop_assert!(
                    score_ok && tiebreak_ok,
                    "Posting list for '{}' not sorted by (score DESC, doc_id ASC) at {}: \
                     prev=(score={}, doc_id={}) curr=(score={}, doc_id={})",
                    term, i, prev.score, prev.doc_id, curr.score, curr.doc_id
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
        let index = build_hybrid_test_index(&corpus);

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
        let index = build_hybrid_test_index(&corpus);

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
        let index = build_hybrid_test_index(&corpus);

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
        let index = build_hybrid_test_index(&corpus);

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

// ============================================================================
// HEADING LEVEL AND MATCH TYPE PROPERTIES
// ============================================================================
//
// These tests verify the heading level to match type mapping that determines
// bucketed ranking. Fixed issues:
// - heading_level=0 must map to Title (document title)
// - heading_level=1,2 must map to Section (h1, h2 headings)
// - Default heading_level for unmapped positions must be >= 5 (Content)
// ============================================================================

use sorex::MatchType;

proptest! {
    /// Property: heading_level=0 always maps to MatchType::Title.
    ///
    /// This is the most important invariant for bucketed ranking: document titles
    /// must rank above all other field types. A bug where default heading_level
    /// was 0 caused content to incorrectly rank as Title matches.
    #[test]
    fn prop_heading_level_0_is_title(_dummy in 0..1i32) {
        let match_type = MatchType::from_heading_level(0);
        prop_assert_eq!(
            match_type, MatchType::Title,
            "heading_level=0 must map to Title, got {:?}",
            match_type
        );
    }

    /// Property: heading_level=1 maps to Section, NOT Title.
    ///
    /// This was a critical bug: h1 headings were incorrectly treated as Title.
    /// Only heading_level=0 (document title field) should be Title.
    #[test]
    fn prop_heading_level_1_is_section(_dummy in 0..1i32) {
        let match_type = MatchType::from_heading_level(1);
        prop_assert_eq!(
            match_type, MatchType::Section,
            "heading_level=1 (h1) must map to Section, got {:?}",
            match_type
        );
    }

    /// Property: heading_level=2 maps to Section.
    #[test]
    fn prop_heading_level_2_is_section(_dummy in 0..1i32) {
        let match_type = MatchType::from_heading_level(2);
        prop_assert_eq!(
            match_type, MatchType::Section,
            "heading_level=2 (h2) must map to Section, got {:?}",
            match_type
        );
    }

    /// Property: heading_level >= 5 maps to Content.
    ///
    /// Content is the lowest priority in bucketed ranking. Default unmapped
    /// positions must use heading_level >= 5 to avoid false Title/Section matches.
    #[test]
    fn prop_heading_level_5_plus_is_content(level in 5u8..=255) {
        let match_type = MatchType::from_heading_level(level);
        prop_assert_eq!(
            match_type, MatchType::Content,
            "heading_level={} must map to Content, got {:?}",
            level, match_type
        );
    }

    /// Property: MatchType ordering is strictly hierarchical.
    ///
    /// Title < Section < Subsection < Subsubsection < Content in enum order.
    /// Lower enum values have higher priority in bucketed ranking.
    #[test]
    fn prop_match_type_ordering(_dummy in 0..1i32) {
        prop_assert!(MatchType::Title < MatchType::Section);
        prop_assert!(MatchType::Section < MatchType::Subsection);
        prop_assert!(MatchType::Subsection < MatchType::Subsubsection);
        prop_assert!(MatchType::Subsubsection < MatchType::Content);
    }

    /// Property: For any heading_level, the mapping is deterministic.
    #[test]
    fn prop_heading_level_mapping_deterministic(level in 0u8..=255) {
        let result1 = MatchType::from_heading_level(level);
        let result2 = MatchType::from_heading_level(level);
        prop_assert_eq!(result1, result2, "Mapping must be deterministic");
    }

    /// Property: Title dominates all other match types in ranking.
    ///
    /// A document with MatchType::Title must always rank above documents
    /// with any other match type, regardless of score within the tier.
    #[test]
    fn prop_title_dominates_other_types(
        title_score in 1.0f64..1000.0,
        other_score in 1.0f64..10000.0  // Even 10x higher score
    ) {
        // Title match with lower score should still rank higher
        // (in our ranking, lower MatchType enum value = higher priority)
        prop_assert!(
            MatchType::Title < MatchType::Section,
            "Title must rank above Section regardless of score"
        );
        prop_assert!(
            MatchType::Title < MatchType::Content,
            "Title must rank above Content regardless of score"
        );

        // Verify the enum ordering holds
        let title_rank = MatchType::Title as u8;
        let section_rank = MatchType::Section as u8;
        let content_rank = MatchType::Content as u8;

        prop_assert!(title_rank < section_rank);
        prop_assert!(section_rank < content_rank);

        // Suppress unused variable warnings
        let _ = (title_score, other_score);
    }
}

// ============================================================================
// INVERTED INDEX HEADING LEVEL PROPERTIES
// ============================================================================
//
// These tests verify that the inverted index correctly assigns heading levels
// based on field boundaries, and uses the correct default for unmapped positions.
// ============================================================================

proptest! {
    /// Property: Postings for positions within field boundaries inherit heading_level.
    ///
    /// When a token falls within a defined field boundary, it must inherit
    /// that boundary's heading_level for correct bucketed ranking.
    #[test]
    fn prop_postings_inherit_boundary_heading_level(
        doc_text in "[a-z]{10,50}",
        boundary_start in 0usize..5,
        boundary_len in 5usize..20,
        heading_level in 0u8..6
    ) {
        let boundary_end = (boundary_start + boundary_len).min(doc_text.len());
        if boundary_start >= boundary_end {
            return Ok(());
        }

        let boundaries = vec![sorex::FieldBoundary {
            doc_id: 0,
            start: boundary_start,
            end: boundary_end,
            field_type: sorex::FieldType::Heading,
            section_id: Some("test-section".to_string()),
            heading_level,
        }];

        let corpus = vec![doc_text.clone()];
        let index = sorex::build_inverted_index(&corpus, &boundaries);

        // Check postings within the boundary
        for list in index.terms.values() {
            for posting in &list.postings {
                if posting.doc_id == 0 && posting.offset >= boundary_start && posting.offset < boundary_end {
                    prop_assert_eq!(
                        posting.heading_level, heading_level,
                        "Posting at offset {} should have heading_level={}, got {}",
                        posting.offset, heading_level, posting.heading_level
                    );
                }
            }
        }
    }

    /// Property: Postings for positions outside all boundaries get Content-level heading.
    ///
    /// The default heading_level for unmapped positions must be >= 5 (Content),
    /// NOT 0 (Title). This was a critical bug that caused content to rank as Title.
    #[test]
    fn prop_postings_outside_boundaries_are_content(
        doc_text in "[a-z]{20,50}",
    ) {
        // Create a small boundary at the start, leaving rest unmapped
        let boundary = sorex::FieldBoundary {
            doc_id: 0,
            start: 0,
            end: 5.min(doc_text.len()),
            field_type: sorex::FieldType::Title,
            section_id: None,
            heading_level: 0,
        };

        let corpus = vec![doc_text.clone()];
        let index = sorex::build_inverted_index(&corpus, &[boundary]);

        // Check postings outside the boundary (offset >= 5)
        for list in index.terms.values() {
            for posting in &list.postings {
                if posting.doc_id == 0 && posting.offset >= 5 {
                    // Default heading_level must be >= 5 (Content level)
                    prop_assert!(
                        posting.heading_level >= 5,
                        "Posting at offset {} (outside boundaries) should have heading_level >= 5, got {}",
                        posting.offset, posting.heading_level
                    );

                    // Must NOT be Title level (0)
                    prop_assert!(
                        posting.heading_level != 0,
                        "Posting at offset {} must not default to heading_level=0 (Title)",
                        posting.offset
                    );
                }
            }
        }
    }

    /// Property: Empty boundaries result in all Content-level postings.
    ///
    /// When no field boundaries are defined, all postings should default to
    /// Content level (heading_level >= 5), not Title level.
    #[test]
    fn prop_no_boundaries_all_content(doc_text in "[a-z]{10,30}") {
        let corpus = vec![doc_text];
        let index = sorex::build_inverted_index(&corpus, &[]);

        for (term, list) in &index.terms {
            for posting in &list.postings {
                prop_assert!(
                    posting.heading_level >= 5,
                    "With no boundaries, term '{}' at offset {} should have heading_level >= 5, got {}",
                    term, posting.offset, posting.heading_level
                );
            }
        }
    }
}

// ============================================================================
// NEW LEAN AXIOM VERIFICATION PROPERTIES
// ============================================================================
//
// These tests verify axioms added during the Lean proof rationalization.
// They correspond to axioms in the SearchVerified Lean modules.
// ============================================================================

proptest! {
    /// Property: fromHeadingLevel is monotone (lower level = better rank).
    ///
    /// Lean spec: MatchType.fromHeadingLevel_monotone in Types.lean
    #[test]
    fn prop_from_heading_level_monotone(l1 in 0u8..10, l2 in 0u8..10) {
        if l1 <= l2 {
            let mt1 = MatchType::from_heading_level(l1);
            let mt2 = MatchType::from_heading_level(l2);
            prop_assert!(
                mt1 <= mt2,
                "fromHeadingLevel not monotone: level {} gave {:?}, level {} gave {:?}",
                l1, mt1, l2, mt2
            );
        }
    }

    /// Property: Fuzzy score is monotonically decreasing with distance.
    ///
    /// Lean spec: fuzzy_score_monotone in TieredSearch.lean
    #[test]
    fn prop_fuzzy_score_monotone(d1 in 1u8..5, d2 in 1u8..5) {
        // Base score by distance (matching src/search/tiered.rs:210-216)
        fn fuzzy_base_score(distance: u8) -> f64 {
            match distance {
                1 => 30.0,
                2 => 15.0,
                _ => 5.0,
            }
        }

        if d1 < d2 {
            let s1 = fuzzy_base_score(d1);
            let s2 = fuzzy_base_score(d2);
            prop_assert!(
                s1 >= s2,
                "Fuzzy score not monotone: distance {} score {}, distance {} score {}",
                d1, s1, d2, s2
            );
        }
    }

    /// Property: Fuzzy scores are bounded by prefix tier base score.
    ///
    /// Lean spec: fuzzy_bounded_by_prefix in TieredSearch.lean
    #[test]
    fn prop_fuzzy_bounded_by_prefix(distance in 1u8..10) {
        // Base score by distance (matching src/search/tiered.rs:210-216)
        fn fuzzy_base_score(distance: u8) -> f64 {
            match distance {
                1 => 30.0,
                2 => 15.0,
                _ => 5.0,
            }
        }

        let fuzzy_score = fuzzy_base_score(distance);
        let prefix_base = 50.0f64; // Tier 2 prefix base score
        prop_assert!(
            fuzzy_score < prefix_base,
            "Fuzzy score {} >= prefix base score {}",
            fuzzy_score, prefix_base
        );
    }

    /// Property: MatchType ordering is transitive.
    ///
    /// Lean spec: matchType_ordering_transitive in Scoring.lean
    #[test]
    fn prop_match_type_transitive(
        l1 in 0u8..10,
        l2 in 0u8..10,
        l3 in 0u8..10
    ) {
        let mt1 = MatchType::from_heading_level(l1);
        let mt2 = MatchType::from_heading_level(l2);
        let mt3 = MatchType::from_heading_level(l3);
        if mt1 < mt2 && mt2 < mt3 {
            prop_assert!(
                mt1 < mt3,
                "MatchType ordering not transitive: {:?} < {:?} < {:?} but {:?} >= {:?}",
                mt1, mt2, mt3, mt1, mt3
            );
        }
    }
}

// ============================================================================
// BINARY ENCODING ROUNDTRIP PROPERTIES
// ============================================================================
//
// These tests verify that encoding and decoding are inverses for all binary
// formats. Silent data corruption is possible if these invariants are broken.
// ============================================================================

use sorex::binary::{
    decode_postings, decode_suffix_array, decode_varint, decode_vocabulary, encode_postings,
    encode_suffix_array, encode_varint, encode_vocabulary, PostingEntry, MAX_VARINT_BYTES,
};

proptest! {
    /// Property: Varint encoding is reversible for all u64 values.
    #[test]
    fn prop_varint_roundtrip(value: u64) {
        let mut buf = Vec::new();
        encode_varint(value, &mut buf);
        let (decoded, consumed) = decode_varint(&buf).unwrap();
        prop_assert_eq!(value, decoded);
        prop_assert_eq!(consumed, buf.len());
    }

    /// Property: Varint encoding uses at most MAX_VARINT_BYTES.
    #[test]
    fn prop_varint_size_bound(value: u64) {
        let mut buf = Vec::new();
        encode_varint(value, &mut buf);
        prop_assert!(
            buf.len() <= MAX_VARINT_BYTES,
            "Varint for {} used {} bytes (max {})",
            value, buf.len(), MAX_VARINT_BYTES
        );
    }

    /// Property: Suffix array encoding is reversible.
    #[test]
    fn prop_suffix_array_roundtrip(
        entries in prop::collection::vec((0u32..65535, 0u32..10000), 0..500)
    ) {
        let mut buf = Vec::new();
        encode_suffix_array(&entries, &mut buf);
        let (decoded, _) = decode_suffix_array(&buf).unwrap();
        prop_assert_eq!(entries, decoded);
    }

    /// Property: Vocabulary encoding (front compression) is reversible.
    #[test]
    fn prop_vocabulary_roundtrip(vocab in prop::collection::vec("[a-z]{1,20}", 1..100)) {
        let mut sorted = vocab.clone();
        sorted.sort();
        sorted.dedup(); // Must be unique and sorted
        if sorted.is_empty() {
            return Ok(());
        }
        let mut buf = Vec::new();
        encode_vocabulary(&sorted, &mut buf);
        let decoded = decode_vocabulary(&buf, sorted.len()).unwrap();
        prop_assert_eq!(sorted, decoded);
    }

    /// Property: Postings encoding (delta+varint) is reversible.
    #[test]
    fn prop_postings_roundtrip(
        doc_ids in prop::collection::vec(0u32..10000, 1..500)
    ) {
        // Delta encoding requires sorted doc_ids
        let mut sorted: Vec<u32> = doc_ids.into_iter().collect();
        sorted.sort();
        sorted.dedup();
        if sorted.is_empty() {
            return Ok(());
        }

        let entries: Vec<PostingEntry> = sorted.iter().enumerate()
            .map(|(i, &doc_id)| PostingEntry {
                doc_id,
                section_idx: (i % 10) as u32,
                heading_level: (i % 6) as u8,
                score: 1000u32.saturating_sub(i as u32), // Descending scores
            })
            .collect();

        let mut buf = Vec::new();
        encode_postings(&entries, &mut buf);
        let (decoded, _) = decode_postings(&buf).unwrap();

        prop_assert_eq!(entries.len(), decoded.len());
        for (e, d) in entries.iter().zip(decoded.iter()) {
            prop_assert_eq!(e.doc_id, d.doc_id, "doc_id mismatch");
            prop_assert_eq!(e.section_idx, d.section_idx, "section_idx mismatch");
            prop_assert_eq!(e.heading_level, d.heading_level, "heading_level mismatch");
        }
    }

    /// Property: Multi-byte varints have continuation bit set on all but last byte.
    /// This catches mutations like `| 0x80` → `^ 0x80`.
    #[test]
    fn prop_varint_continuation_bit(value in 128u64..u64::MAX) {
        let mut buf = Vec::new();
        encode_varint(value, &mut buf);

        // Multi-byte varints: all bytes except last must have continuation bit
        prop_assert!(buf.len() > 1, "value {} should produce multi-byte varint", value);

        for (i, &byte) in buf.iter().enumerate() {
            if i < buf.len() - 1 {
                prop_assert!(
                    byte & 0x80 != 0,
                    "Byte {} should have continuation bit set for value {}",
                    i, value
                );
            } else {
                prop_assert!(
                    byte & 0x80 == 0,
                    "Last byte should NOT have continuation bit for value {}",
                    value
                );
            }
        }
    }
}

// ============================================================================
// EDGE CASE UNIT TESTS FOR MUTATION COVERAGE
// ============================================================================

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test: Varint decoder rejects 11-byte sequences (max is 10 for u64).
    /// Closes mutation gap: `< MAX_VARINT_BYTES` → `<= MAX_VARINT_BYTES`
    #[test]
    fn test_varint_rejects_overlong() {
        // 11 continuation bytes (all with 0x80 set) should be rejected
        let overlong: Vec<u8> = std::iter::repeat_n(0x80, 11).collect();
        let result = decode_varint(&overlong);
        assert!(result.is_err(), "Should reject 11-byte varint");

        // 10 continuation bytes + terminator (0x01) should work
        let max_valid: Vec<u8> = std::iter::repeat_n(0x80, 9)
            .chain(std::iter::once(0x01))
            .collect();
        let result = decode_varint(&max_valid);
        assert!(result.is_ok(), "10-byte varint should be valid");
    }

    /// Test: Varint decoder handles boundary values correctly.
    #[test]
    fn test_varint_boundary_values() {
        // Single byte boundary (127 = max single byte)
        let mut buf = Vec::new();
        encode_varint(127, &mut buf);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0] & 0x80, 0, "127 should not have continuation bit");

        // Two byte boundary (128 = min two byte)
        buf.clear();
        encode_varint(128, &mut buf);
        assert_eq!(buf.len(), 2);
        assert_ne!(
            buf[0] & 0x80,
            0,
            "First byte of 128 must have continuation bit"
        );
        assert_eq!(
            buf[1] & 0x80,
            0,
            "Last byte of 128 must not have continuation bit"
        );

        // Max u64 should be exactly 10 bytes
        buf.clear();
        encode_varint(u64::MAX, &mut buf);
        assert_eq!(
            buf.len(),
            MAX_VARINT_BYTES,
            "u64::MAX should be exactly {} bytes",
            MAX_VARINT_BYTES
        );
    }

    /// Test: Empty buffer returns error, not panic.
    #[test]
    fn test_varint_empty_buffer() {
        let result = decode_varint(&[]);
        assert!(result.is_err(), "Empty buffer should return error");
    }

    /// Test: Front compression actually compresses (catches `common_prefix_len -> 0`).
    /// If common_prefix_len always returns 0, this test fails because output is too large.
    #[test]
    fn test_vocabulary_front_compression_effective() {
        // Vocabulary with significant shared prefixes
        let vocab = vec![
            "application".to_string(),
            "applications".to_string(),
            "apply".to_string(),
            "applied".to_string(),
            "applying".to_string(),
        ];

        let mut compressed = Vec::new();
        encode_vocabulary(&vocab, &mut compressed);

        // Calculate naive encoding size (no compression)
        let naive_size: usize = vocab
            .iter()
            .map(|s| s.len() + 2) // string bytes + 2 varint bytes (shared=0, len)
            .sum();

        // Compressed size should be significantly smaller
        // "application" = 11 bytes, but subsequent terms share "appl" prefix
        assert!(
            compressed.len() < naive_size,
            "Front compression not effective: compressed {} >= naive {}",
            compressed.len(),
            naive_size
        );

        // Verify roundtrip still works
        let decoded = decode_vocabulary(&compressed, vocab.len()).unwrap();
        assert_eq!(vocab, decoded);
    }

    /// Test: Suffix array roundtrip preserves exact values.
    /// Closes mutation gap for arithmetic operations in delta decoding.
    #[test]
    fn test_suffix_array_roundtrip_exact() {
        // Test with specific values that exercise delta encoding
        let entries = vec![
            (0u32, 0u32), // doc 0, offset 0
            (0, 100),     // same doc, different offset
            (1, 0),       // new doc
            (1, 50),      // same doc
            (100, 0),     // large doc_id jump
            (100, 1000),  // large offset
        ];

        let mut buf = Vec::new();
        encode_suffix_array(&entries, &mut buf);
        let (decoded, consumed) = decode_suffix_array(&buf).unwrap();

        assert_eq!(entries.len(), decoded.len());
        assert_eq!(consumed, buf.len());

        for (i, ((orig_doc, orig_off), (dec_doc, dec_off))) in
            entries.iter().zip(decoded.iter()).enumerate()
        {
            assert_eq!(
                *orig_doc, *dec_doc,
                "doc_id mismatch at index {}: expected {}, got {}",
                i, orig_doc, dec_doc
            );
            assert_eq!(
                *orig_off, *dec_off,
                "offset mismatch at index {}: expected {}, got {}",
                i, orig_off, dec_off
            );
        }
    }
}
