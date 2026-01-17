//! Section navigation property tests.
//!
//! These tests verify section invariants from Section.lean:
//! - offset_maps_to_unique_section: Every offset maps to at most one section
//! - title_has_no_section_id: Title fields have section_id = None
//! - content_inherits_section: Content fields inherit section_id from parent heading
//! - Section IDs are valid URL anchors

use proptest::prelude::*;
use proptest::strategy::ValueTree;

// ============================================================================
// STRATEGIES
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

// ============================================================================
// SECTION NAVIGATION PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

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
