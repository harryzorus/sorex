// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! The math behind search ranking.
//!
//! Field type dominates position. A title match with the worst possible position bonus
//! still beats a heading match with the best position bonus. This sounds obvious, but
//! getting the constants right is fiddly - hence the Lean proofs.
//!
//! # Lean Correspondence
//!
//! These functions correspond to specifications in `SearchVerified/Scoring.lean`:
//! - `field_type_score` → `baseScore`
//! - `position_bonus` → `positionBoost`
//! - Field hierarchy → `title_beats_heading`, `heading_beats_content`
//!
//! # Key Invariant: Field Type Dominance
//!
//! The scoring constants satisfy (proven in Lean):
//!
//! ```text
//! Title - MaxBoost > Heading + MaxBoost   (99.5 > 10.5)
//! Heading - MaxBoost > Content + MaxBoost (9.5 > 1.5)
//! ```
//!
//! # Constants (DO NOT CHANGE WITHOUT LEAN PROOF UPDATE)
//!
//! | Field   | Score | Why this value |
//! |---------|-------|----------------|
//! | Title   | 100.0 | High enough to dominate heading even with max position penalty |
//! | Heading | 10.0  | High enough to dominate content even with max position penalty |
//! | Content | 1.0   | Baseline - position bonus matters more within content |
//! | MaxBoost| 0.5   | Small relative to field gaps - can't invert hierarchy |
//!
//! Changing these requires updating `Scoring.lean` and verifying the theorems rebuild.

use crate::types::{FieldBoundary, FieldType, SearchIndex};

#[cfg(feature = "lean")]
use sorex_lean_macros::lean_property;

// =============================================================================
// SCORING CONSTANTS
// =============================================================================
// DO NOT CHANGE without updating Lean proofs in Scoring.lean and verifying
// title_beats_heading and heading_beats_content theorems still build.
// Run: cd lean && lake build

/// Base score for Title field matches.
pub const TITLE_BASE_SCORE: f64 = 100.0;

/// Base score for Heading field matches.
pub const HEADING_BASE_SCORE: f64 = 10.0;

/// Base score for Content field matches.
pub const CONTENT_BASE_SCORE: f64 = 1.0;

/// Maximum position bonus (matches at start of text get this bonus).
pub const MAX_POSITION_BONUS: f64 = 0.5;

// =============================================================================
// TIERED SEARCH SCORING CONSTANTS
// =============================================================================
// These define the base scores for each tier of the three-tier search:
// Note: Tier scoring constants have been removed.
// Scores are now pre-computed at index time and stored in PostingEntry.
// T2/T3 apply penalties to these stored scores at search time:
// - T2 (prefix): score * (query.len / term.len)
// - T3 (fuzzy): score * (1 - edit_distance / MAX_EDIT_DISTANCE)

/// Which field type contains this offset? Title, heading, or content?
///
/// Looks up the position in the field boundaries table to determine what kind
/// of text the match landed in.
pub fn get_field_type(index: &SearchIndex, doc_id: usize, offset: usize) -> FieldType {
    get_field_type_from_boundaries(doc_id, offset, &index.field_boundaries)
}

/// The actual lookup: finds which field boundary (if any) contains this offset.
///
/// Binary search to find the first boundary for this doc_id, then linear scan
/// through that doc's boundaries. Typically docs have <10 boundaries, so the
/// linear part is fast. The binary search handles the case where you have
/// thousands of documents.
///
/// Returns `Content` if no boundary matches - the safe default.
pub fn get_field_type_from_boundaries(
    doc_id: usize,
    offset: usize,
    boundaries: &[FieldBoundary],
) -> FieldType {
    if boundaries.is_empty() {
        return FieldType::Content;
    }

    // OPTIMIZATION: Binary search to find the first boundary for this doc_id
    // Assumes boundaries are sorted by (doc_id, start)
    let first_for_doc = boundaries.partition_point(|b| b.doc_id < doc_id);

    // Scan through boundaries for this doc (typically small number per doc)
    for boundary in boundaries[first_for_doc..].iter() {
        // Stop if we've passed this doc
        if boundary.doc_id > doc_id {
            break;
        }

        // Check if offset falls within this boundary
        if offset >= boundary.start && offset < boundary.end {
            return boundary.field_type;
        }

        // OPTIMIZATION: If boundaries are sorted by start within a doc,
        // we can break early if we've passed the offset
        if boundary.start > offset {
            break;
        }
    }

    // Default to content if no boundary found
    FieldType::Content
}

/// Base score by field type: Title (100) > Heading (10) > Content (1).
///
/// These values are intentionally far apart. The gap between adjacent tiers
/// is larger than the maximum position bonus, so field type always wins.
///
/// # Lean Specification
///
/// The hierarchy is proven in `Scoring.lean`:
///
/// ```lean
/// theorem title_beats_heading :
///     baseScore .title - maxPositionBoost > baseScore .heading + maxPositionBoost
///
/// theorem heading_beats_content :
///     baseScore .heading - maxPositionBoost > baseScore .content + maxPositionBoost
/// ```
#[cfg_attr(
    feature = "lean",
    lean_property("title_beats_heading: 100.0 - 0.5 > 10.0 + 0.5")
)]
#[cfg_attr(
    feature = "lean",
    lean_property("heading_beats_content: 10.0 - 0.5 > 1.0 + 0.5")
)]
pub fn field_type_score(field_type: &FieldType) -> f64 {
    // INVARIANT: FIELD_TYPE_DOMINANCE
    // These values are proven correct in Lean. DO NOT CHANGE without:
    // 1. Updating baseScore in Scoring.lean
    // 2. Verifying title_beats_heading theorem builds
    // 3. Verifying heading_beats_content theorem builds
    // 4. Running: cd lean && lake build
    match field_type {
        FieldType::Title => TITLE_BASE_SCORE,
        FieldType::Heading => HEADING_BASE_SCORE,
        FieldType::Content => CONTENT_BASE_SCORE,
    }
}

/// Position bonus: matches near the start of text score slightly higher.
///
/// This is the "tiebreaker within field type" - a title match at offset 0 beats
/// a title match at offset 100. The bonus is capped at 0.5, so it can never
/// overcome the field type hierarchy.
///
/// # Lean Specification
///
/// From `Scoring.lean`:
/// - Range: `[0, maxPositionBoost]` where `maxPositionBoost = 0.5`
/// - Monotonic: earlier positions get higher or equal boost
pub fn position_bonus(offset: usize, text_len: usize) -> f64 {
    if text_len > 0 {
        MAX_POSITION_BONUS * (1.0 - (offset as f64 / text_len as f64))
    } else {
        0.0
    }
}

/// Combine field type base score with position bonus.
pub fn final_score(field_type: &FieldType, offset: usize, text_len: usize) -> f64 {
    field_type_score(field_type) + position_bonus(offset, text_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_type_hierarchy() {
        let title = field_type_score(&FieldType::Title);
        let heading = field_type_score(&FieldType::Heading);
        let content = field_type_score(&FieldType::Content);

        assert!(title > heading);
        assert!(heading > content);
    }

    #[test]
    fn test_field_type_dominance() {
        // Worst title should beat best heading
        let worst_title = field_type_score(&FieldType::Title) - MAX_POSITION_BONUS;
        let best_heading = field_type_score(&FieldType::Heading) + MAX_POSITION_BONUS;
        assert!(worst_title > best_heading);

        // Worst heading should beat best content
        let worst_heading = field_type_score(&FieldType::Heading) - MAX_POSITION_BONUS;
        let best_content = field_type_score(&FieldType::Content) + MAX_POSITION_BONUS;
        assert!(worst_heading > best_content);
    }

    #[test]
    fn test_position_bonus() {
        // Start of text gets maximum bonus
        assert!((position_bonus(0, 100) - MAX_POSITION_BONUS).abs() < 0.01);

        // End of text gets minimum bonus
        assert!((position_bonus(100, 100) - 0.0).abs() < 0.01);

        // Middle gets half of max bonus
        assert!((position_bonus(50, 100) - MAX_POSITION_BONUS / 2.0).abs() < 0.01);
    }
}
