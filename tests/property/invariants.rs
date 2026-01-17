//! Tests that verify Lean specification invariants.
//!
//! These tests correspond directly to theorems in the Lean project.
//! If any of these fail, the Rust implementation has diverged from the formal spec.

use super::common::{assert_index_well_formed, assert_suffix_array_complete, build_test_index};
use sorex::{field_type_score, FieldType};

// ============================================================================
// SCORING INVARIANTS (Scoring.lean)
// ============================================================================

/// Lean theorem: `title_beats_heading`
/// worst_title > best_heading (even with max position bonus difference)
#[test]
fn lean_theorem_title_beats_heading() {
    let title_base = field_type_score(&FieldType::Title);
    let heading_base = field_type_score(&FieldType::Heading);
    let max_position_boost = 0.5;

    let worst_title = title_base - max_position_boost;
    let best_heading = heading_base + max_position_boost;

    assert!(
        worst_title > best_heading,
        "LEAN THEOREM VIOLATED: title_beats_heading\n\
         worst_title ({}) must be > best_heading ({})",
        worst_title,
        best_heading
    );
}

/// Lean theorem: `heading_beats_content`
/// worst_heading > best_content
#[test]
fn lean_theorem_heading_beats_content() {
    let heading_base = field_type_score(&FieldType::Heading);
    let content_base = field_type_score(&FieldType::Content);
    let max_position_boost = 0.5;

    let worst_heading = heading_base - max_position_boost;
    let best_content = content_base + max_position_boost;

    assert!(
        worst_heading > best_content,
        "LEAN THEOREM VIOLATED: heading_beats_content\n\
         worst_heading ({}) must be > best_content ({})",
        worst_heading,
        best_content
    );
}

/// Lean theorem: `field_type_dominance` (combined)
#[test]
fn lean_theorem_field_type_dominance() {
    let title = field_type_score(&FieldType::Title);
    let heading = field_type_score(&FieldType::Heading);
    let content = field_type_score(&FieldType::Content);

    // Strict hierarchy
    assert!(title > heading, "Title must score higher than Heading");
    assert!(heading > content, "Heading must score higher than Content");

    // Gap must be sufficient (> 2 * max_boost)
    let max_boost = 0.5;
    let min_gap = 2.0 * max_boost;

    assert!(
        title - heading > min_gap,
        "Gap between Title and Heading ({}) must be > {}",
        title - heading,
        min_gap
    );
    assert!(
        heading - content > min_gap,
        "Gap between Heading and Content ({}) must be > {}",
        heading - content,
        min_gap
    );
}

// ============================================================================
// SUFFIX ARRAY INVARIANTS (SuffixArray.lean)
// ============================================================================

/// Lean axiom: `suffix_array_sorted`
#[test]
fn lean_axiom_suffix_array_sorted() {
    let index = build_test_index(&["banana", "apple", "cherry"]);

    for i in 1..index.suffix_array.len() {
        let prev = &index.suffix_array[i - 1];
        let curr = &index.suffix_array[i];

        let prev_suffix = &index.texts[prev.doc_id][prev.offset..];
        let curr_suffix = &index.texts[curr.doc_id][curr.offset..];

        assert!(
            prev_suffix <= curr_suffix,
            "LEAN AXIOM VIOLATED: suffix_array_sorted at position {}\n\
             prev='{}' > curr='{}'",
            i,
            prev_suffix,
            curr_suffix
        );
    }
}

/// Lean axiom: `suffix_array_complete`
#[test]
fn lean_axiom_suffix_array_complete() {
    let index = build_test_index(&["hello", "world"]);
    assert_suffix_array_complete(&index);
}

/// Lean axiom: `lcp_correct`
#[test]
fn lean_axiom_lcp_correct() {
    let index = build_test_index(&["banana", "bandana"]);

    // lcp.len() == suffix_array.len()
    assert_eq!(index.lcp.len(), index.suffix_array.len());

    // lcp[0] == 0
    if !index.lcp.is_empty() {
        assert_eq!(index.lcp[0], 0, "LCP[0] must be 0");
    }

    // lcp[i] = common prefix length of sa[i-1] and sa[i]
    for i in 1..index.lcp.len() {
        let prev = &index.suffix_array[i - 1];
        let curr = &index.suffix_array[i];

        let prev_suffix = &index.texts[prev.doc_id][prev.offset..];
        let curr_suffix = &index.texts[curr.doc_id][curr.offset..];

        let expected_lcp = prev_suffix
            .chars()
            .zip(curr_suffix.chars())
            .take_while(|(a, b)| a == b)
            .count();

        assert_eq!(
            index.lcp[i], expected_lcp,
            "LEAN AXIOM VIOLATED: lcp_correct at position {}\n\
             lcp[{}] = {} but expected {}",
            i, i, index.lcp[i], expected_lcp
        );
    }
}

// ============================================================================
// INDEX WELL-FORMEDNESS (Types.lean)
// ============================================================================

/// Lean definition: `SearchIndex.WellFormed`
#[test]
fn lean_def_search_index_well_formed() {
    let index = build_test_index(&["test document", "another document"]);
    assert_index_well_formed(&index);
}

/// Lean definition: `SuffixEntry.WellFormed`
/// Note: Uses strict inequality (offset < length) because suffix arrays index non-empty suffixes
#[test]
fn lean_def_suffix_entry_well_formed() {
    let index = build_test_index(&["hello"]);

    for (i, entry) in index.suffix_array.iter().enumerate() {
        // doc_id < texts.size
        assert!(
            entry.doc_id < index.texts.len(),
            "SuffixEntry[{}].doc_id {} >= texts.len() {}",
            i,
            entry.doc_id,
            index.texts.len()
        );

        // offset < texts[doc_id].length (strict inequality - no empty suffixes)
        let text_len = index.texts[entry.doc_id].len();
        assert!(
            entry.offset < text_len,
            "SuffixEntry[{}].offset {} >= texts[{}].len() {}",
            i,
            entry.offset,
            entry.doc_id,
            text_len
        );
    }
}

// ============================================================================
// EDGE CASES
// ============================================================================

#[test]
fn empty_texts_produces_empty_suffix_array() {
    let index = build_test_index(&[""]);
    assert!(index.suffix_array.is_empty());
    assert!(index.lcp.is_empty());
}

#[test]
fn single_character_text() {
    let index = build_test_index(&["a"]);
    assert_eq!(index.suffix_array.len(), 1);
    assert_index_well_formed(&index);
}

/// Note: Current implementation uses byte offsets in suffix array.
/// For multibyte UTF-8 characters, some byte offsets are invalid char boundaries.
/// The implementation is designed for ASCII or pre-normalized text.
/// This test verifies ASCII handling works correctly.
#[test]
fn ascii_text_preserves_invariants() {
    let index = build_test_index(&["hello world", "search algorithms"]);
    assert_index_well_formed(&index);
}

/// Test with normalized unicode (after diacritics are stripped)
#[test]
fn normalized_unicode_preserves_invariants() {
    // In real usage, normalize() strips diacritics: "héllo" -> "hello"
    let index = build_test_index(&["hello world", "cafe au lait"]);
    assert_index_well_formed(&index);
}

#[test]
fn repeated_characters() {
    let index = build_test_index(&["aaaaaaa"]);
    assert_index_well_formed(&index);

    // All suffixes should still be sorted
    for i in 1..index.suffix_array.len() {
        let prev = &index.suffix_array[i - 1];
        let curr = &index.suffix_array[i];
        let prev_suffix = &index.texts[prev.doc_id][prev.offset..];
        let curr_suffix = &index.texts[curr.doc_id][curr.offset..];
        assert!(prev_suffix <= curr_suffix);
    }
}
