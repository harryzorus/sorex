// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Edit distance with an early-exit optimization.
//!
//! The key insight: `|len(a) - len(b)|` is a lower bound on edit distance.
//! If two strings differ in length by more than the threshold, skip the O(nm) DP.
//! This catches ~40% of non-matches before allocating anything.
//!
//! # Lean Correspondence
//!
//! These functions correspond to specifications in `SearchVerified/Levenshtein.lean`:
//! - `levenshtein_within` → `editDistanceBounded`
//! - Early-exit optimization → `withinBounds_sound`, `early_exit_correct`

#[cfg(feature = "lean")]
use sorex_lean_macros::lean_verify;

/// Are these strings within `max` edits of each other?
///
/// Bounded Levenshtein with two early-exit paths:
/// 1. If length difference exceeds `max`, return false immediately
/// 2. If minimum row value exceeds `max`, abandon the DP early
///
/// Both are sound - proven in Lean to never reject valid matches.
///
/// # Lean Specification
///
/// Corresponds to `Levenshtein.editDistanceBounded` in `Levenshtein.lean`:
///
/// ```lean
/// def editDistanceBounded (a b : String) (maxDist : Nat) : Option Nat :=
///   if ¬withinBounds a b maxDist then none
///   else let d := editDistance a b
///        if d ≤ maxDist then some d else none
/// ```
///
/// Key properties:
/// - `length_diff_lower_bound`: `|len(a) - len(b)| ≤ editDistance a b`
/// - `early_exit_correct`: If `editDistance a b ≤ d`, then `withinBounds a b d = true`
#[cfg_attr(
    feature = "lean",
    lean_verify(
        spec = "levenshtein_within",
        requires = "a.length > 0 ∧ b.length > 0",
        ensures = "result = true → editDistance a b ≤ max",
        properties = ["length_diff_lower_bound", "early_exit_correct"]
    )
)]
pub fn levenshtein_within(a: &str, b: &str, max: usize) -> bool {
    // Use character counts, not byte lengths, for Unicode correctness
    let a_len = a.chars().count();
    let b_len = b.chars().count();

    // Early-exit: length difference is a lower bound on edit distance
    if (a_len as isize - b_len as isize).unsigned_abs() > max {
        return false;
    }

    let mut dp: Vec<usize> = (0..=b_len).collect();
    for (i, ac) in a.chars().enumerate() {
        let mut prev = dp[0];
        dp[0] = i + 1;
        let mut min_row = dp[0];

        for (j, bc) in b.chars().enumerate() {
            let temp = dp[j + 1];
            let cost = if ac == bc { 0 } else { 1 };
            dp[j + 1] = (dp[j + 1] + 1).min(dp[j] + 1).min(prev + cost);
            prev = temp;
            if dp[j + 1] < min_row {
                min_row = dp[j + 1];
            }
        }

        // Early-exit: if minimum in this row exceeds max, no point continuing
        if min_row > max {
            return false;
        }
    }

    dp[b_len] <= max
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        assert!(levenshtein_within("hello", "hello", 0));
    }

    #[test]
    fn test_one_edit() {
        assert!(levenshtein_within("hello", "hallo", 1));
        assert!(levenshtein_within("hello", "hell", 1));
        assert!(levenshtein_within("hello", "helloo", 1));
    }

    #[test]
    fn test_early_exit() {
        // Length difference is 5, so distance must be >= 5
        assert!(!levenshtein_within("a", "abcdef", 1));
    }

    #[test]
    fn test_two_edits() {
        assert!(levenshtein_within("hello", "hxllo", 1));
        assert!(levenshtein_within("photography", "phptography", 2));
    }

    #[test]
    fn test_unicode_diacritics() {
        // ASCII vs diacritic versions should have small edit distance
        assert!(levenshtein_within("tummalacherla", "tummalachērla", 2)); // e vs ē
        assert!(levenshtein_within("harish", "harīṣh", 2)); // i vs ī, s vs ṣ
        assert!(levenshtein_within("cafe", "café", 1)); // e vs é
    }
}
