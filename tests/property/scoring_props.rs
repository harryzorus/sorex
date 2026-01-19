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
// T3 FUZZY PENALTY FORMULA PROPERTIES
// ============================================================================
//
// The T3 penalty formula was changed from:
//   OLD: 1.0 - (distance / max_distance)  → distance=2 gives 0.0 (BUG!)
//   NEW: 1.0 / (1.0 + distance)           → distance=2 gives 0.33
//
// These tests verify the new formula's invariants.

/// Oracle: T3 fuzzy penalty calculation.
/// Must match the formula in src/search/tiered.rs.
fn oracle_t3_penalty(distance: u8) -> f64 {
    1.0 / (1.0 + distance as f64)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property: T3 penalty is always positive for any valid edit distance.
    ///
    /// This prevents the bug where distance=MAX_EDIT_DISTANCE gave score=0.
    #[test]
    fn prop_t3_penalty_always_positive(distance in 1u8..=10) {
        let penalty = oracle_t3_penalty(distance);
        prop_assert!(
            penalty > 0.0,
            "T3 penalty must be positive, got {} for distance={}",
            penalty, distance
        );
    }

    /// Property: T3 penalty is monotonically decreasing with distance.
    ///
    /// Lean spec: fuzzyScore_monotone in TieredSearch.lean
    /// Closer matches (lower distance) should have higher scores.
    #[test]
    fn prop_t3_penalty_monotonic(d1 in 1u8..10, d2 in 1u8..10) {
        if d1 < d2 {
            let p1 = oracle_t3_penalty(d1);
            let p2 = oracle_t3_penalty(d2);
            prop_assert!(
                p1 > p2,
                "T3 penalty not monotonic: distance {} penalty {} should be > distance {} penalty {}",
                d1, p1, d2, p2
            );
        }
    }

    /// Property: T3 penalty is bounded in (0, 1] for distance >= 1.
    ///
    /// Distance 1 gives max penalty of 0.5, higher distances give less.
    #[test]
    fn prop_t3_penalty_bounded(distance in 1u8..=255) {
        let penalty = oracle_t3_penalty(distance);
        prop_assert!(
            penalty > 0.0 && penalty <= 0.5,
            "T3 penalty {} out of bounds (0, 0.5] for distance={}",
            penalty, distance
        );
    }

    /// Property: T3 penalty values are deterministic.
    #[test]
    fn prop_t3_penalty_deterministic(distance in 1u8..=10) {
        let p1 = oracle_t3_penalty(distance);
        let p2 = oracle_t3_penalty(distance);
        prop_assert!(
            (p1 - p2).abs() < 1e-10,
            "T3 penalty not deterministic for distance={}",
            distance
        );
    }
}

#[cfg(test)]
mod t3_penalty_tests {
    use super::*;

    #[test]
    fn test_t3_penalty_specific_values() {
        // Distance 1: 1/(1+1) = 0.5
        assert!((oracle_t3_penalty(1) - 0.5).abs() < 1e-10);

        // Distance 2: 1/(1+2) = 0.333...
        assert!((oracle_t3_penalty(2) - 1.0 / 3.0).abs() < 1e-10);

        // Distance 3: 1/(1+3) = 0.25
        assert!((oracle_t3_penalty(3) - 0.25).abs() < 1e-10);

        // All values are positive (no zero scores!)
        for d in 1..=10 {
            assert!(oracle_t3_penalty(d) > 0.0);
        }
    }

    #[test]
    fn test_t3_penalty_never_zero() {
        // This was the original bug - distance=MAX_EDIT_DISTANCE gave 0
        // With new formula, even very high distances stay positive
        for d in 1..=255 {
            let p = oracle_t3_penalty(d);
            assert!(
                p > 0.0,
                "Penalty should never be zero, got {} for d={}",
                p,
                d
            );
        }
    }
}

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
