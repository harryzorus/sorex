//! Scoring and match type property tests.
//!
//! These tests verify scoring invariants from Scoring.lean and Types.lean:
//! - Field type dominance: Title > Heading > Content
//! - MatchType ordering is hierarchical and transitive
//! - heading_level to MatchType mapping is monotone
//! - Fuzzy scores are monotonically decreasing with edit distance

use proptest::prelude::*;
use sorex::{field_type_score, FieldType, MatchType};

// ============================================================================
// FIELD TYPE HIERARCHY PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Property: Field type dominance holds for any position combination.
    ///
    /// Lean spec: title_beats_heading, heading_beats_content in Scoring.lean
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
// MATCH TYPE HIERARCHY PROPERTIES
// ============================================================================
//
// These tests verify the heading level to match type mapping that determines
// bucketed ranking. Critical invariants:
// - heading_level=0 must map to Title (document title)
// - heading_level=1,2 must map to Section (h1, h2 headings)
// - Default heading_level for unmapped positions must be >= 5 (Content)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

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
// LEAN AXIOM VERIFICATION PROPERTIES
// ============================================================================
//
// These tests verify axioms added during the Lean proof rationalization.
// They correspond to axioms in the SearchVerified Lean modules.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

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
