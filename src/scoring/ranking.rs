// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Result ranking: how search results get sorted.
//!
//! The ranking is bucketed by match type, not by raw score. A title match with
//! score 50 beats a content match with score 100. Numeric scores only matter
//! as tiebreakers within each bucket.
//!
//! Bucket hierarchy: Title > Section > Subsection > Subsubsection > Content
//!
//! # Lean Specification
//!
//! Theorems in `SearchVerified/MatchType.lean`:
//! - `title_beats_section`
//! - `section_beats_subsection`
//! - `subsection_beats_subsubsection`
//! - `subsubsection_beats_content`

use crate::search::tiered::SearchResult;
use crate::types::SearchDoc;
use std::cmp::Ordering;

/// Compare two search results for ranking.
///
/// Sort order:
/// 1. **Match type** - bucket hierarchy dominates (Title > Section > ... > Content)
/// 2. **Score** - only within the same bucket (higher wins)
/// 3. **Title** - alphabetical tiebreaker for determinism
/// 4. **Doc ID** - final tiebreaker when everything else is equal
///
/// The key insight: a title match at score 50 beats a content match at score 100.
/// Buckets are impermeable - scores can't cross bucket boundaries.
///
/// # Example
///
/// ```ignore
/// // Title with low score beats content with high score
/// let title_result = SearchResult { match_type: MatchType::Title, score: 50.0, .. };
/// let content_result = SearchResult { match_type: MatchType::Content, score: 100.0, .. };
///
/// assert_eq!(compare_results(&title_result, &content_result, &docs), Ordering::Less);
/// ```
pub fn compare_results(a: &SearchResult, b: &SearchResult, docs: &[SearchDoc]) -> Ordering {
    // Primary: match_type (smaller enum value = better rank)
    // Enum ordering: Title(0) < Section(1) < Subsection(2) < Subsubsection(3) < Content(4)
    match a.match_type.cmp(&b.match_type) {
        Ordering::Equal => {
            // Secondary: score (descending - higher score wins)
            match b.score.partial_cmp(&a.score) {
                Some(ord) if ord != Ordering::Equal => ord,
                _ => {
                    // Tertiary: title (ascending - alphabetical)
                    let a_title = docs.get(a.doc_id).map(|d| d.title.as_str()).unwrap_or("");
                    let b_title = docs.get(b.doc_id).map(|d| d.title.as_str()).unwrap_or("");
                    match a_title.cmp(b_title) {
                        Ordering::Equal => {
                            // Final tie-breaker: doc_id for absolute determinism
                            a.doc_id.cmp(&b.doc_id)
                        }
                        ord => ord,
                    }
                }
            }
        }
        ord => ord, // match_type order determines ranking
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MatchType;

    #[test]
    fn test_compare_results_title_beats_section() {
        let title = SearchResult {
            doc_id: 0,
            score: 50.0,
            section_idx: 0, // 0 = no section
            tier: 1,
            match_type: MatchType::Title,
            matched_term: None,
        };

        let section = SearchResult {
            doc_id: 1,
            score: 100.0,   // Higher score
            section_idx: 1, // 1 = first section in table
            tier: 1,
            match_type: MatchType::Section,
            matched_term: None,
        };

        let docs = vec![];

        // Title should win despite lower score
        assert_eq!(compare_results(&title, &section, &docs), Ordering::Less);
    }

    #[test]
    fn test_compare_results_within_bucket_uses_score() {
        let high_score = SearchResult {
            doc_id: 0,
            score: 100.0,
            section_idx: 0,
            tier: 1,
            match_type: MatchType::Section,
            matched_term: None,
        };

        let low_score = SearchResult {
            doc_id: 1,
            score: 50.0,
            section_idx: 1,
            tier: 1,
            match_type: MatchType::Section, // Same bucket
            matched_term: None,
        };

        let docs = vec![];

        // Same bucket, so score matters
        assert_eq!(
            compare_results(&high_score, &low_score, &docs),
            Ordering::Less
        );
    }
}
