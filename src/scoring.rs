//! Scoring functions for search results.
//!
//! # Lean Correspondence
//!
//! These functions correspond to specifications in `SearchVerified/Scoring.lean`:
//! - `field_type_score` → `baseScore`
//! - Position bonus → `positionBoost`
//! - Field hierarchy → `title_beats_heading`, `heading_beats_content`
//!
//! # INVARIANTS (DO NOT VIOLATE)
//!
//! ## FIELD_TYPE_DOMINANCE
//! The scoring constants MUST satisfy these inequalities (proven in Lean):
//!
//! ```text
//! Title - MaxBoost > Heading + MaxBoost
//! Heading - MaxBoost > Content + MaxBoost
//! ```
//!
//! With current values: `100 - 0.5 = 99.5 > 10 + 0.5 = 10.5` ✓
//!                      `10 - 0.5 = 9.5 > 1 + 0.5 = 1.5` ✓
//!
//! ## CONSTANTS (DO NOT CHANGE WITHOUT LEAN PROOF UPDATE)
//! - Title = 100.0
//! - Heading = 10.0
//! - Content = 1.0
//! - MaxBoost = 0.5
//!
//! Changing these requires updating `Scoring.lean` and verifying `title_beats_heading`
//! and `heading_beats_content` theorems still hold.

use crate::types::{FieldBoundary, FieldType, SearchIndex};

#[cfg(feature = "lean")]
use sieve_lean_macros::lean_property;

/// Get the field type for a given position in a document.
pub fn get_field_type(index: &SearchIndex, doc_id: usize, offset: usize) -> FieldType {
    get_field_type_from_boundaries(doc_id, offset, &index.field_boundaries)
}

/// Get the field type from a list of boundaries.
///
/// This is the core implementation used by both SearchIndex and HybridIndex.
pub fn get_field_type_from_boundaries(
    doc_id: usize,
    offset: usize,
    boundaries: &[FieldBoundary],
) -> FieldType {
    // Find the field boundary that contains this offset
    for boundary in boundaries {
        if boundary.doc_id == doc_id && offset >= boundary.start && offset < boundary.end {
            return boundary.field_type.clone();
        }
    }
    // Default to content if no boundary found
    FieldType::Content
}

/// Get the base score for a field type.
///
/// Scoring hierarchy: Title (100) > Heading (10) > Content (1).
///
/// # Lean Specification
///
/// Key invariant from `Scoring.lean`: differences are large enough that
/// position bonuses cannot invert ranking.
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
        FieldType::Title => 100.0,  // Lean: baseScore .title = 1000 (×10)
        FieldType::Heading => 10.0, // Lean: baseScore .heading = 100 (×10)
        FieldType::Content => 1.0,  // Lean: baseScore .content = 10 (×10)
    }
}

/// Calculate position bonus for a match.
///
/// Earlier positions get higher bonuses (up to 0.5 points).
///
/// # Lean Specification
///
/// Corresponds to `Scoring.positionBoost` in `Scoring.lean`:
/// - Range: `[0, maxPositionBoost]` (maxPositionBoost = 0.5)
/// - Monotonicity: earlier positions get higher or equal boost
pub fn position_bonus(offset: usize, text_len: usize) -> f64 {
    if text_len > 0 {
        0.5 * (1.0 - (offset as f64 / text_len as f64))
    } else {
        0.0
    }
}

/// Calculate final score for a match.
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
        let max_bonus = 0.5;

        // Worst title should beat best heading
        let worst_title = field_type_score(&FieldType::Title) - max_bonus;
        let best_heading = field_type_score(&FieldType::Heading) + max_bonus;
        assert!(worst_title > best_heading);

        // Worst heading should beat best content
        let worst_heading = field_type_score(&FieldType::Heading) - max_bonus;
        let best_content = field_type_score(&FieldType::Content) + max_bonus;
        assert!(worst_heading > best_content);
    }

    #[test]
    fn test_position_bonus() {
        // Start of text gets maximum bonus
        assert!((position_bonus(0, 100) - 0.5).abs() < 0.01);

        // End of text gets minimum bonus
        assert!((position_bonus(100, 100) - 0.0).abs() < 0.01);

        // Middle gets half
        assert!((position_bonus(50, 100) - 0.25).abs() < 0.01);
    }
}
