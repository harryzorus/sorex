//! Comprehensive tests covering edge cases and thorough coverage.
//!
//! These tests complement the property tests by targeting specific scenarios
//! that might be missed by random generation.

mod common;

use common::{assert_index_well_formed, build_test_index, build_test_index_with_fields, make_doc};
use sieve::{build_index, search, FieldBoundary, FieldType};

// ============================================================================
// BINARY SEARCH EDGE CASES
// ============================================================================

#[test]
fn binary_search_single_element() {
    let index = build_test_index(&["x"]);
    assert_index_well_formed(&index);

    // Should find the single character
    let results = search(&index, "x");
    assert_eq!(results.len(), 1);
}

#[test]
fn binary_search_first_element() {
    let index = build_test_index(&["aaa bbb ccc"]);

    // Search for prefix that would be first lexicographically
    let results = search(&index, "a");
    assert!(!results.is_empty());
}

#[test]
fn binary_search_last_element() {
    let index = build_test_index(&["aaa bbb zzz"]);

    // Search for prefix that would be last lexicographically
    let results = search(&index, "z");
    assert!(!results.is_empty());
}

#[test]
fn binary_search_middle_element() {
    let index = build_test_index(&["aaa mmm zzz"]);

    // Search for prefix in the middle
    let results = search(&index, "m");
    assert!(!results.is_empty());
}

#[test]
fn binary_search_nonexistent_before_all() {
    let index = build_test_index(&["bbb ccc ddd"]);

    // Search for something lexicographically before all elements
    let results = search(&index, "aaa");
    assert!(results.is_empty());
}

#[test]
fn binary_search_nonexistent_after_all() {
    let index = build_test_index(&["aaa bbb ccc"]);

    // Search for something lexicographically after all elements
    let results = search(&index, "zzz");
    assert!(results.is_empty());
}

#[test]
fn binary_search_nonexistent_between() {
    let index = build_test_index(&["aaa zzz"]);

    // Search for something between existing elements
    let results = search(&index, "mmm");
    assert!(results.is_empty());
}

// ============================================================================
// OFFSET BOUND INVARIANT TESTS
// ============================================================================

#[test]
fn offset_bound_strictly_less_than_length() {
    let index = build_test_index(&["abc"]);

    // Verify no suffix entry has offset == length (would be empty suffix)
    for entry in &index.suffix_array {
        let text_len = index.texts[entry.doc_id].len();
        assert!(
            entry.offset < text_len,
            "Found offset {} >= text length {} - empty suffix not allowed",
            entry.offset,
            text_len
        );
    }
}

#[test]
fn suffix_array_excludes_empty_suffix() {
    let index = build_test_index(&["ab"]);

    // "ab" has 2 characters, so valid suffixes are at offset 0 ("ab") and 1 ("b")
    // Offset 2 would be empty string - should not exist
    assert_eq!(index.suffix_array.len(), 2);

    let offsets: Vec<usize> = index.suffix_array.iter().map(|e| e.offset).collect();
    assert!(offsets.contains(&0));
    assert!(offsets.contains(&1));
    assert!(!offsets.contains(&2)); // No empty suffix
}

// ============================================================================
// SEARCH RESULT DEDUPLICATION
// ============================================================================

#[test]
fn search_deduplicates_results() {
    // Document with multiple occurrences of same substring
    let docs = vec![make_doc(0, "Test")];
    let texts = vec!["test test test".to_string()];
    let index = build_index(docs, texts, vec![]);

    let results = search(&index, "test");

    // Should return the document once, not three times
    assert_eq!(results.len(), 1);
}

#[test]
fn search_deduplicates_across_fields() {
    let docs = vec![make_doc(0, "Test Title")];
    let texts = vec!["test in title test in content".to_string()];
    let boundaries = vec![
        FieldBoundary {
            doc_id: 0,
            start: 0,
            end: 13,
            field_type: FieldType::Title,
            section_id: None,
        },
        FieldBoundary {
            doc_id: 0,
            start: 14,
            end: 29,
            field_type: FieldType::Content,
            section_id: None,
        },
    ];
    let index = build_index(docs, texts, boundaries);

    let results = search(&index, "test");

    // Document should appear once (with best score)
    assert_eq!(results.len(), 1);
}

// ============================================================================
// EMPTY AND EDGE CASE HANDLING
// ============================================================================

#[test]
fn empty_text_produces_no_suffixes() {
    let index = build_test_index(&[""]);
    assert!(index.suffix_array.is_empty());
    assert!(index.lcp.is_empty());
}

#[test]
fn mixed_empty_and_nonempty_texts() {
    let docs = vec![make_doc(0, "Empty"), make_doc(1, "HasContent")];
    let texts = vec!["".to_string(), "content".to_string()];
    let index = build_index(docs, texts, vec![]);

    assert_index_well_formed(&index);

    // Only non-empty text should have suffixes
    for entry in &index.suffix_array {
        assert_eq!(entry.doc_id, 1, "Only doc 1 should have suffixes");
    }
}

#[test]
fn whitespace_only_text() {
    let index = build_test_index(&["   "]);

    // Whitespace characters should still create suffixes
    assert!(!index.suffix_array.is_empty());
    assert_index_well_formed(&index);
}

#[test]
fn single_character_documents() {
    let index = build_test_index(&["a", "b", "c"]);
    assert_index_well_formed(&index);

    // Should have 3 suffix entries (one per doc)
    assert_eq!(index.suffix_array.len(), 3);
}

// ============================================================================
// FIELD BOUNDARY EDGE CASES
// ============================================================================

#[test]
fn field_boundary_at_offset_zero() {
    let docs = vec![make_doc(0, "Test")];
    let texts = vec!["title content".to_string()];
    let boundaries = vec![FieldBoundary {
        doc_id: 0,
        start: 0,
        end: 5,
        field_type: FieldType::Title,
        section_id: None,
    }];
    let index = build_index(docs, texts, boundaries);

    assert_index_well_formed(&index);
}

#[test]
fn contiguous_field_boundaries() {
    let docs = vec![make_doc(0, "Test")];
    let texts = vec!["title heading content".to_string()];
    let boundaries = vec![
        FieldBoundary {
            doc_id: 0,
            start: 0,
            end: 5,
            field_type: FieldType::Title,
            section_id: None,
        },
        FieldBoundary {
            doc_id: 0,
            start: 6,
            end: 13,
            field_type: FieldType::Heading,
            section_id: None,
        },
        FieldBoundary {
            doc_id: 0,
            start: 14,
            end: 21,
            field_type: FieldType::Content,
            section_id: None,
        },
    ];
    let index = build_index(docs, texts, boundaries);

    assert_index_well_formed(&index);
}

#[test]
fn field_boundaries_with_gaps() {
    // Text has gaps between field regions
    let docs = vec![make_doc(0, "Test")];
    let texts = vec!["title ... content".to_string()];
    let boundaries = vec![
        FieldBoundary {
            doc_id: 0,
            start: 0,
            end: 5,
            field_type: FieldType::Title,
            section_id: None,
        },
        // Gap from 5-10 (defaults to Content)
        FieldBoundary {
            doc_id: 0,
            start: 10,
            end: 17,
            field_type: FieldType::Content,
            section_id: None,
        },
    ];
    let index = build_index(docs, texts, boundaries);

    assert_index_well_formed(&index);
}

#[test]
fn no_field_boundaries_defaults_to_content() {
    let docs = vec![make_doc(0, "Test")];
    let texts = vec!["some content".to_string()];
    // No boundaries provided
    let index = build_index(docs, texts, vec![]);

    assert_index_well_formed(&index);

    // Search should still work
    let results = search(&index, "content");
    assert_eq!(results.len(), 1);
}

// ============================================================================
// SCORING EDGE CASES
// ============================================================================

#[test]
fn position_bonus_at_start_vs_end() {
    let docs_data = vec![(
        "Doc".to_string(),
        vec![("target at start then other words at end with target".to_string(), FieldType::Content)],
    )];
    let index = build_test_index_with_fields(&docs_data);

    // Should find the document
    let results = search(&index, "target");
    assert!(!results.is_empty());
}

#[test]
fn title_match_beats_content_match_regardless_of_position() {
    let docs = vec![make_doc(0, "Content First"), make_doc(1, "Title Match")];
    let texts = vec![
        "rust is mentioned early here".to_string(),
        "something else rust".to_string(), // rust at end
    ];
    let boundaries = vec![
        FieldBoundary {
            doc_id: 0,
            start: 0,
            end: 28,
            field_type: FieldType::Content,
            section_id: None,
        },
        FieldBoundary {
            doc_id: 1,
            start: 0,
            end: 19,
            field_type: FieldType::Title,
            section_id: None,
        },
    ];
    let index = build_index(docs, texts, boundaries);

    let results = search(&index, "rust");
    assert!(results.len() >= 2);

    // Title match (doc 1) should rank first despite position
    assert_eq!(results[0].id, 1, "Title match should rank first");
}

#[test]
fn heading_match_beats_content_match() {
    let docs = vec![make_doc(0, "Content"), make_doc(1, "Heading")];
    let texts = vec!["rust content".to_string(), "rust heading".to_string()];
    let boundaries = vec![
        FieldBoundary {
            doc_id: 0,
            start: 0,
            end: 12,
            field_type: FieldType::Content,
            section_id: None,
        },
        FieldBoundary {
            doc_id: 1,
            start: 0,
            end: 12,
            field_type: FieldType::Heading,
            section_id: None,
        },
    ];
    let index = build_index(docs, texts, boundaries);

    let results = search(&index, "rust");
    assert!(results.len() >= 2);

    // Heading match should rank first
    assert_eq!(results[0].id, 1, "Heading match should rank first");
}

// ============================================================================
// LARGE CORPUS STRESS TESTS
// ============================================================================

#[test]
fn many_documents() {
    let count = 100;
    let texts: Vec<String> = (0..count)
        .map(|i| format!("document number {} with some content", i))
        .collect();
    let text_refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
    let index = build_test_index(&text_refs);

    assert_index_well_formed(&index);
    assert_eq!(index.docs.len(), count);

    // Search should work
    let results = search(&index, "document");
    assert_eq!(results.len(), count);
}

#[test]
fn long_document() {
    let long_text = "word ".repeat(1000);
    let index = build_test_index(&[&long_text]);

    assert_index_well_formed(&index);

    // Should have many suffixes
    assert!(index.suffix_array.len() > 1000);

    // Search should still work
    let results = search(&index, "word");
    assert_eq!(results.len(), 1);
}

#[test]
fn many_matches_in_single_document() {
    // Document with repeated pattern
    let repeated = "pattern ".repeat(50);
    let index = build_test_index(&[&repeated]);

    let results = search(&index, "pattern");

    // Should return document once (deduplicated)
    assert_eq!(results.len(), 1);
}

// ============================================================================
// SPECIAL CHARACTER HANDLING
// ============================================================================

#[test]
fn numbers_in_text() {
    let index = build_test_index(&["document 123 with numbers 456"]);

    let results = search(&index, "123");
    assert!(!results.is_empty());

    let results = search(&index, "456");
    assert!(!results.is_empty());
}

#[test]
fn punctuation_in_text() {
    let index = build_test_index(&["hello, world! how are you?"]);

    // Search for word with punctuation stripped (via normalization)
    let results = search(&index, "hello");
    assert!(!results.is_empty());

    let results = search(&index, "world");
    assert!(!results.is_empty());
}

#[test]
fn mixed_case_handling() {
    // Note: Index stores normalized (lowercase) text in production
    // The search function normalizes queries before matching
    // Test helper build_test_index doesn't normalize, so we use lowercase input
    let index = build_test_index(&["camelcase mixedcase uppercase lowercase"]);

    // All query cases should find matches (queries are normalized)
    assert!(!search(&index, "camelcase").is_empty());
    assert!(!search(&index, "MIXEDCASE").is_empty());
    assert!(!search(&index, "UpperCase").is_empty());
    assert!(!search(&index, "LOWERCASE").is_empty());
}

// ============================================================================
// MULTI-TERM SEARCH EDGE CASES
// ============================================================================

#[test]
fn multi_term_all_match() {
    let index = build_test_index(&["rust programming language", "python programming"]);

    // Both terms in same doc
    let results = search(&index, "rust language");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 0);
}

#[test]
fn multi_term_partial_match() {
    let index = build_test_index(&["rust programming", "python programming"]);

    // Only first term matches doc 0, only second matches both
    let results = search(&index, "rust python");
    assert!(results.is_empty()); // No doc has both
}

#[test]
fn multi_term_with_common_word() {
    let index = build_test_index(&[
        "the quick brown fox",
        "the lazy brown dog",
        "a red fox",
    ]);

    // "brown fox" should match only doc 0
    let results = search(&index, "brown fox");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 0);
}

// ============================================================================
// LCP ARRAY CORRECTNESS
// ============================================================================

#[test]
fn lcp_with_common_prefixes() {
    let index = build_test_index(&["banana"]);

    // Sorted suffixes for "banana":
    // "a", "ana", "anana", "banana", "na", "nana"
    // LCP: 0, 1, 3, 0, 0, 2

    assert_eq!(index.lcp[0], 0); // First element always 0

    // Verify LCP values match common prefix lengths
    for i in 1..index.lcp.len() {
        let prev = &index.suffix_array[i - 1];
        let curr = &index.suffix_array[i];
        let prev_suffix = &index.texts[prev.doc_id][prev.offset..];
        let curr_suffix = &index.texts[curr.doc_id][curr.offset..];

        let expected = prev_suffix
            .chars()
            .zip(curr_suffix.chars())
            .take_while(|(a, b)| a == b)
            .count();

        assert_eq!(
            index.lcp[i], expected,
            "LCP[{}] should be {} (common prefix of '{}' and '{}')",
            i, expected, prev_suffix, curr_suffix
        );
    }
}

#[test]
fn lcp_with_no_common_prefixes() {
    let index = build_test_index(&["abc xyz"]);

    // All LCP values after 0 should be 0 or small since prefixes differ
    for lcp_val in &index.lcp {
        assert!(*lcp_val < 10, "Unexpected large LCP value");
    }
}

// ============================================================================
// REGRESSION TESTS
// ============================================================================

#[test]
fn regression_empty_query_parts() {
    let index = build_test_index(&["test content"]);

    // Multiple spaces shouldn't cause issues
    let results = search(&index, "test  content");
    assert!(!results.is_empty());

    // Leading/trailing spaces
    let results = search(&index, "  test  ");
    assert!(!results.is_empty());
}

#[test]
fn regression_very_short_query() {
    let index = build_test_index(&["a b c d e f"]);

    // Single character queries
    let results = search(&index, "a");
    assert!(!results.is_empty());

    let results = search(&index, "f");
    assert!(!results.is_empty());
}

#[test]
fn regression_query_longer_than_text() {
    let index = build_test_index(&["hi"]);

    // Query longer than any text
    let results = search(&index, "hello world this is a very long query");
    assert!(results.is_empty());
}
