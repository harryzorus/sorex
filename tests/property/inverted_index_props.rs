//! Inverted index property tests.
//!
//! These tests verify inverted index invariants:
//! - Posting lists are sorted by (score DESC, doc_id ASC)
//! - Document frequency is accurate
//! - All postings point to valid locations
//! - All terms in documents appear in the index
//! - Heading levels are correctly assigned from field boundaries

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

// ============================================================================
// INVERTED INDEX PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

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
// HEADING LEVEL PROPERTIES
// ============================================================================
//
// These tests verify that the inverted index correctly assigns heading levels
// based on field boundaries, and uses the correct default for unmapped positions.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

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
