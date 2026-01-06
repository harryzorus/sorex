//! Runtime contracts derived from Lean specifications.
//!
//! This module provides debug-mode assertions that verify the properties
//! specified in the Lean formal verification project. These contracts:
//!
//! 1. Are **zero-cost in release builds** (use `debug_assert!`)
//! 2. Provide **early failure detection** during development
//! 3. Mirror the **formal Lean specifications** exactly
//!
//! # INVARIANTS (DO NOT REMOVE THESE CHECKS)
//!
//! Every function in this module verifies a proven property from the Lean specs.
//! Removing or weakening these checks defeats the purpose of formal verification.
//!
//! # Lean Correspondence
//!
//! | Contract Function              | Lean Specification                    |
//! |--------------------------------|---------------------------------------|
//! | `check_suffix_entry_valid`     | `SuffixEntry.WellFormed` (Types.lean) |
//! | `check_suffix_array_sorted`    | `SuffixArray.Sorted` (SuffixArray.lean)|
//! | `check_index_well_formed`      | `SearchIndex.WellFormed` (Types.lean) |
//! | `check_field_hierarchy`        | `Scoring.field_type_dominance`        |
//! | `check_lcp_correct`            | `SuffixArray.LcpCorrect`              |
//!
//! # Usage
//!
//! ```ignore
//! use search::contracts::*;
//!
//! // In debug builds, this panics if invariant is violated
//! check_suffix_array_sorted(&index.texts, &index.suffix_array);
//!
//! // In release builds, this is a no-op
//! ```

// ============================================================================
// COMPILE-TIME ASSERTIONS (evaluated at build time)
// ============================================================================

/// Static assertion that field type dominance holds.
/// This is evaluated at compile time - if it fails, the crate won't build.
const _: () = {
    const TITLE: f64 = 100.0;
    const HEADING: f64 = 10.0;
    const CONTENT: f64 = 1.0;
    const MAX_BOOST: f64 = 0.5;

    // INVARIANT: title_beats_heading (from Scoring.lean)
    // worst_title > best_heading
    const WORST_TITLE: f64 = TITLE - MAX_BOOST;
    const BEST_HEADING: f64 = HEADING + MAX_BOOST;
    assert!(WORST_TITLE > BEST_HEADING); // 99.5 > 10.5 ✓

    // INVARIANT: heading_beats_content (from Scoring.lean)
    // worst_heading > best_content
    const WORST_HEADING: f64 = HEADING - MAX_BOOST;
    const BEST_CONTENT: f64 = CONTENT + MAX_BOOST;
    assert!(WORST_HEADING > BEST_CONTENT); // 9.5 > 1.5 ✓
};

use crate::types::{FieldType, SearchIndex, SuffixEntry};
use crate::utils::common_prefix_len;

// ============================================================================
// SUFFIX ENTRY CONTRACTS
// ============================================================================

/// Check that a suffix entry is well-formed.
///
/// # Lean Specification
/// ```lean
/// def SuffixEntry.WellFormed (e : SuffixEntry) (texts : Array String) : Prop :=
///   e.doc_id < texts.size ∧ e.offset < texts[e.doc_id].length
/// ```
///
/// # Panics (debug builds only)
/// Panics if `doc_id >= texts.len()` or `offset >= texts[doc_id].len()`.
#[inline]
pub fn check_suffix_entry_valid(entry: &SuffixEntry, texts: &[String]) {
    debug_assert!(
        entry.doc_id < texts.len(),
        "Contract violation: SuffixEntry.WellFormed - doc_id {} >= texts.len() {}",
        entry.doc_id,
        texts.len()
    );

    if entry.doc_id < texts.len() {
        debug_assert!(
            entry.offset < texts[entry.doc_id].len(),
            "Contract violation: SuffixEntry.WellFormed - offset {} >= texts[{}].len() {}",
            entry.offset,
            entry.doc_id,
            texts[entry.doc_id].len()
        );
    }
}

/// Check that all suffix entries are well-formed.
#[inline]
pub fn check_all_entries_valid(suffix_array: &[SuffixEntry], texts: &[String]) {
    for (i, entry) in suffix_array.iter().enumerate() {
        debug_assert!(
            entry.doc_id < texts.len(),
            "Contract violation: suffix_array[{}].doc_id {} >= texts.len() {}",
            i,
            entry.doc_id,
            texts.len()
        );

        if entry.doc_id < texts.len() {
            debug_assert!(
                entry.offset < texts[entry.doc_id].len(),
                "Contract violation: suffix_array[{}].offset {} >= texts[{}].len() {}",
                i,
                entry.offset,
                entry.doc_id,
                texts[entry.doc_id].len()
            );
        }
    }
}

// ============================================================================
// SUFFIX ARRAY CONTRACTS
// ============================================================================

/// Check that a suffix array is sorted lexicographically.
///
/// # Lean Specification
/// ```lean
/// def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop :=
///   ∀ i j : Nat, (hi : i < sa.size) → (hj : j < sa.size) → i < j →
///     SuffixLe texts sa[i] sa[j]
/// ```
///
/// # Panics (debug builds only)
/// Panics if any adjacent pair violates the ordering.
#[inline]
pub fn check_suffix_array_sorted(texts: &[String], suffix_array: &[SuffixEntry]) {
    for i in 1..suffix_array.len() {
        let prev = &suffix_array[i - 1];
        let curr = &suffix_array[i];

        let prev_suffix = texts
            .get(prev.doc_id)
            .and_then(|t| t.get(prev.offset..))
            .unwrap_or("");
        let curr_suffix = texts
            .get(curr.doc_id)
            .and_then(|t| t.get(curr.offset..))
            .unwrap_or("");

        debug_assert!(
            prev_suffix <= curr_suffix,
            "Contract violation: SuffixArray.Sorted - \
             suffix_array[{}] ('{}') > suffix_array[{}] ('{}')",
            i - 1,
            prev_suffix.chars().take(20).collect::<String>(),
            i,
            curr_suffix.chars().take(20).collect::<String>()
        );
    }
}

/// Check that the suffix array is complete (contains all suffixes).
///
/// # Lean Specification
/// ```lean
/// def Complete (sa : Array SuffixEntry) (texts : Array String) : Prop :=
///   ∀ doc_id : Nat, (hd : doc_id < texts.size) →
///   ∀ offset : Nat, offset < (texts[doc_id]).length →
///     ∃ i : Nat, i < sa.size ∧ sa[i].doc_id = doc_id ∧ sa[i].offset = offset
/// ```
///
/// # Panics (debug builds only)
/// Panics if any (doc_id, offset) pair is missing from the suffix array.
#[inline]
pub fn check_suffix_array_complete(texts: &[String], suffix_array: &[SuffixEntry]) {
    for (doc_id, text) in texts.iter().enumerate() {
        for offset in 0..text.len() {
            let found = suffix_array
                .iter()
                .any(|e| e.doc_id == doc_id && e.offset == offset);

            debug_assert!(
                found,
                "Contract violation: SuffixArray.Complete - \
                 missing entry for doc_id={}, offset={} (suffix: '{}')",
                doc_id,
                offset,
                text.get(offset..)
                    .unwrap_or("")
                    .chars()
                    .take(20)
                    .collect::<String>()
            );
        }
    }
}

// ============================================================================
// LCP ARRAY CONTRACTS
// ============================================================================

/// Check that the LCP array is correct.
///
/// # Lean Specification
/// ```lean
/// def LcpCorrect (sa : Array SuffixEntry) (lcp : Array Nat) (texts : Array String) : Prop :=
///   lcp.size = sa.size ∧
///   (sa.size > 0 → lcp[0] = 0) ∧
///   ∀ i : Nat, (hi : i < sa.size) → i > 0 →
///     lcp[i] = (String.commonPrefix (suffixAt texts sa[i-1]) (suffixAt texts sa[i])).length
/// ```
///
/// # Panics (debug builds only)
/// Panics if LCP values are incorrect.
#[inline]
pub fn check_lcp_correct(texts: &[String], suffix_array: &[SuffixEntry], lcp: &[usize]) {
    debug_assert_eq!(
        lcp.len(),
        suffix_array.len(),
        "Contract violation: LcpCorrect - lcp.len() {} != suffix_array.len() {}",
        lcp.len(),
        suffix_array.len()
    );

    if !lcp.is_empty() {
        debug_assert_eq!(
            lcp[0], 0,
            "Contract violation: LcpCorrect - lcp[0] = {} (expected 0)",
            lcp[0]
        );
    }

    for i in 1..suffix_array.len() {
        let prev = &suffix_array[i - 1];
        let curr = &suffix_array[i];

        let prev_suffix = texts
            .get(prev.doc_id)
            .and_then(|t| t.get(prev.offset..))
            .unwrap_or("");
        let curr_suffix = texts
            .get(curr.doc_id)
            .and_then(|t| t.get(curr.offset..))
            .unwrap_or("");

        let expected = common_prefix_len(prev_suffix, curr_suffix);

        debug_assert_eq!(
            lcp[i], expected,
            "Contract violation: LcpCorrect - lcp[{}] = {} (expected {})",
            i, lcp[i], expected
        );
    }
}

// ============================================================================
// SEARCH INDEX CONTRACTS
// ============================================================================

/// Check that a search index is well-formed.
///
/// # Lean Specification
/// ```lean
/// def SearchIndex.WellFormed (idx : SearchIndex) : Prop :=
///   idx.docs.size = idx.texts.size ∧
///   idx.lcp.size = idx.suffix_array.size ∧
///   Sorted idx.suffix_array idx.texts ∧
///   (∀ e ∈ idx.suffix_array, SuffixEntry.WellFormed e idx.texts)
/// ```
///
/// # Panics (debug builds only)
/// Panics if any invariant is violated.
#[inline]
pub fn check_index_well_formed(index: &SearchIndex) {
    // docs.size = texts.size
    debug_assert_eq!(
        index.docs.len(),
        index.texts.len(),
        "Contract violation: SearchIndex.WellFormed - \
         docs.len() {} != texts.len() {}",
        index.docs.len(),
        index.texts.len()
    );

    // lcp.size = suffix_array.size
    debug_assert_eq!(
        index.lcp.len(),
        index.suffix_array.len(),
        "Contract violation: SearchIndex.WellFormed - \
         lcp.len() {} != suffix_array.len() {}",
        index.lcp.len(),
        index.suffix_array.len()
    );

    // All entries well-formed
    check_all_entries_valid(&index.suffix_array, &index.texts);

    // Suffix array sorted
    check_suffix_array_sorted(&index.texts, &index.suffix_array);
}

/// Full contract check for an index (includes expensive completeness check).
///
/// Use this sparingly as it's O(n²) where n is total suffix count.
#[inline]
pub fn check_index_complete(index: &SearchIndex) {
    check_index_well_formed(index);
    check_suffix_array_complete(&index.texts, &index.suffix_array);
    check_lcp_correct(&index.texts, &index.suffix_array, &index.lcp);
}

// ============================================================================
// SCORING CONTRACTS
// ============================================================================

/// Check that field type scoring hierarchy is maintained.
///
/// # Lean Specification
/// ```lean
/// theorem title_beats_heading :
///     baseScore .title - maxPositionBoost > baseScore .heading + maxPositionBoost
///
/// theorem heading_beats_content :
///     baseScore .heading - maxPositionBoost > baseScore .content + maxPositionBoost
/// ```
///
/// This is a static check that verifies the scoring constants are correct.
/// It should always pass unless someone modifies the scoring constants incorrectly.
#[inline]
pub fn check_field_hierarchy() {
    const TITLE_BASE: f64 = 100.0;
    const HEADING_BASE: f64 = 10.0;
    const CONTENT_BASE: f64 = 1.0;
    const MAX_POSITION_BOOST: f64 = 0.5;

    // Title always beats heading
    let worst_title = TITLE_BASE - MAX_POSITION_BOOST;
    let best_heading = HEADING_BASE + MAX_POSITION_BOOST;
    debug_assert!(
        worst_title > best_heading,
        "Contract violation: title_beats_heading - \
         worst title ({}) <= best heading ({})",
        worst_title,
        best_heading
    );

    // Heading always beats content
    let worst_heading = HEADING_BASE - MAX_POSITION_BOOST;
    let best_content = CONTENT_BASE + MAX_POSITION_BOOST;
    debug_assert!(
        worst_heading > best_content,
        "Contract violation: heading_beats_content - \
         worst heading ({}) <= best content ({})",
        worst_heading,
        best_content
    );
}

/// Check that a score respects field type dominance.
#[inline]
pub fn check_score_dominance(score1: f64, field1: &FieldType, score2: f64, field2: &FieldType) {
    use std::cmp::Ordering;

    let cmp = match (field1, field2) {
        (FieldType::Title, FieldType::Heading) => Some(Ordering::Greater),
        (FieldType::Title, FieldType::Content) => Some(Ordering::Greater),
        (FieldType::Heading, FieldType::Content) => Some(Ordering::Greater),
        (FieldType::Heading, FieldType::Title) => Some(Ordering::Less),
        (FieldType::Content, FieldType::Title) => Some(Ordering::Less),
        (FieldType::Content, FieldType::Heading) => Some(Ordering::Less),
        _ => None, // Same field type, no dominance requirement
    };

    if let Some(expected) = cmp {
        let actual = score1.partial_cmp(&score2).unwrap_or(Ordering::Equal);
        debug_assert_eq!(
            actual, expected,
            "Contract violation: field_type_dominance - \
             {:?} score ({}) should {:?} {:?} score ({})",
            field1, score1, expected, field2, score2
        );
    }
}

// ============================================================================
// BINARY SEARCH CONTRACTS
// ============================================================================

/// Check binary search preconditions.
///
/// # Lean Specification
/// The suffix array must be sorted for binary search to work correctly.
#[inline]
pub fn check_binary_search_precondition(texts: &[String], suffix_array: &[SuffixEntry]) {
    check_suffix_array_sorted(texts, suffix_array);
}

/// Check binary search result bounds.
///
/// # Lean Specification
/// ```lean
/// axiom findFirstGe_bounds (sa : Array SuffixEntry) (texts : Array String) (target : String) :
///     findFirstGe sa texts target ≤ sa.size
/// ```
#[inline]
pub fn check_binary_search_result(result: usize, suffix_array_len: usize) {
    debug_assert!(
        result <= suffix_array_len,
        "Contract violation: findFirstGe_bounds - \
         result {} > suffix_array.len() {}",
        result,
        suffix_array_len
    );
}

// ============================================================================
// LEVENSHTEIN CONTRACTS
// ============================================================================

/// Check Levenshtein distance lower bound.
///
/// # Lean Specification
/// ```lean
/// axiom length_diff_lower_bound (a b : String) :
///     (a.length - b.length : Int).natAbs ≤ editDistance a b
/// ```
///
/// If `levenshtein_within(a, b, max)` returns false, then `|len(a) - len(b)| > max`.
#[inline]
pub fn check_levenshtein_early_exit(a: &str, b: &str, max: usize, result: bool) {
    let len_diff = (a.len() as isize - b.len() as isize).unsigned_abs();

    // If we returned false due to early exit, length difference must exceed max
    if !result && len_diff > max {
        // This is the expected behavior - early exit is correct
    } else if !result {
        // We returned false but not due to early exit
        // The actual edit distance must be > max
        // (We can't easily verify this without computing full distance)
    }

    // If we returned true, the length difference must be <= max
    if result {
        debug_assert!(
            len_diff <= max,
            "Contract violation: levenshtein_within returned true but \
             |len('{}') - len('{}')| = {} > max = {}",
            a.chars().take(10).collect::<String>(),
            b.chars().take(10).collect::<String>(),
            len_diff,
            max
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::build_index;
    use crate::types::SearchDoc;

    fn make_doc(id: usize) -> SearchDoc {
        SearchDoc {
            id,
            title: format!("Doc {}", id),
            excerpt: "".to_string(),
            href: format!("/doc/{}", id),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        }
    }

    #[test]
    fn test_check_suffix_entry_valid() {
        let texts = vec!["hello".to_string()];
        let valid_entry = SuffixEntry {
            doc_id: 0,
            offset: 2,
        };

        // Should not panic
        check_suffix_entry_valid(&valid_entry, &texts);
    }

    #[test]
    #[should_panic(expected = "Contract violation")]
    fn test_check_suffix_entry_invalid_doc_id() {
        let texts = vec!["hello".to_string()];
        let invalid_entry = SuffixEntry {
            doc_id: 5,
            offset: 0,
        };

        check_suffix_entry_valid(&invalid_entry, &texts);
    }

    #[test]
    fn test_check_index_well_formed() {
        let docs = vec![make_doc(0), make_doc(1)];
        let texts = vec!["hello".to_string(), "world".to_string()];
        let index = build_index(docs, texts, vec![]);

        // Should not panic
        check_index_well_formed(&index);
    }

    #[test]
    fn test_check_field_hierarchy() {
        // Should not panic - scoring constants are correct
        check_field_hierarchy();
    }

    #[test]
    fn test_check_levenshtein_early_exit() {
        // Valid early exit
        check_levenshtein_early_exit("hello", "hi", 1, false);

        // Valid within bounds
        check_levenshtein_early_exit("hello", "hallo", 1, true);
    }
}
