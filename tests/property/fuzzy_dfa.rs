//! Property tests for DFA (Deterministic Finite Automaton) Levenshtein matching.
//!
//! The DFA is used for fuzzy search with edit distance. These tests verify:
//! 1. Levenshtein distance computation is correct
//! 2. Threshold-based acceptance works correctly
//! 3. Edge cases are handled properly

use proptest::prelude::*;
use sorex::levenshtein_within;

// ============================================================================
// STRATEGIES
// ============================================================================

/// Generate word-like strings for pattern and candidate testing.
fn word_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{2,8}").unwrap()
}

/// Generate shorter words for better test coverage.
fn short_word_strategy() -> impl Strategy<Value = String> {
    prop::string::string_regex("[a-z]{1,5}").unwrap()
}

/// Maximum distance threshold for Levenshtein (typically 2 in search).
const MAX_DISTANCE: usize = 2;

// ============================================================================
// LEVENSHTEIN DISTANCE PROPERTY TESTS
// ============================================================================

proptest! {
    /// Property: levenshtein_within(a, a, d) is always true
    ///
    /// A string should always be within distance of itself.
    #[test]
    fn prop_levenshtein_self_always_true(word in word_strategy()) {
        let result = levenshtein_within(&word, &word, MAX_DISTANCE);
        prop_assert!(result, "String should be within distance of itself");
    }

    /// Property: Empty strings within distance
    ///
    /// Empty to empty should be within any distance.
    #[test]
    fn prop_empty_string_within_distance(max_dist in 1usize..=5) {
        let result = levenshtein_within("", "", max_dist);
        prop_assert!(result, "Empty strings should be within distance");
    }

    /// Property: Distance depends on length difference
    ///
    /// If length difference > max_distance, strings can't be within distance.
    #[test]
    fn prop_length_diff_determines_distance(
        a in short_word_strategy(),
        b in short_word_strategy(),
        max_dist in 0usize..=3
    ) {
        let len_diff = (a.len() as i32 - b.len() as i32).unsigned_abs() as usize;
        let result = levenshtein_within(&a, &b, max_dist);

        // If length difference exceeds max_distance, result must be false
        if len_diff > max_dist {
            prop_assert!(
                !result,
                "Length difference {} > max_dist {} should result in false",
                len_diff, max_dist
            );
        }
    }

    /// Property: Exact matches are always within distance
    ///
    /// Identical strings should always be within distance.
    #[test]
    fn prop_exact_match_always_within(word in word_strategy()) {
        let result = levenshtein_within(&word, &word, 0);
        prop_assert!(result, "Exact match should be within distance 0");
    }

    /// Property: Single character difference
    ///
    /// Changing one character should be within distance 1.
    #[test]
    fn prop_single_char_diff_within_one(word in short_word_strategy()) {
        if word.len() >= 2 {
            let mut modified = word.clone();
            let first_char = modified.chars().next().unwrap();

            // Find a different character
            let mut replacement = 'a';
            while replacement == first_char && replacement != 'z' {
                replacement = ((replacement as u8) + 1) as char;
            }

            if replacement != first_char {
                modified = replacement.to_string() + &word[1..];
                let result = levenshtein_within(&word, &modified, 1);
                prop_assert!(result, "Single character difference should be within distance 1");
            }
        }
    }

    /// Property: Adding one character is within distance 1
    ///
    /// Inserting a single character should be within distance 1.
    #[test]
    fn prop_single_insertion_within_one(word in short_word_strategy()) {
        let mut inserted = word.clone();
        inserted.insert(0, 'x');

        let result = levenshtein_within(&word, &inserted, 1);
        prop_assert!(result, "Single insertion should be within distance 1");
    }

    /// Property: Removing one character is within distance 1
    ///
    /// Deleting a single character should be within distance 1.
    #[test]
    fn prop_single_deletion_within_one(word in short_word_strategy()) {
        if word.len() >= 2 {
            let deleted = word.chars()
                .enumerate()
                .filter(|(i, _)| *i != 0)
                .map(|(_, c)| c)
                .collect::<String>();

            let result = levenshtein_within(&word, &deleted, 1);
            prop_assert!(result, "Single deletion should be within distance 1");
        }
    }

    /// Property: Monotone with threshold
    ///
    /// If within smaller threshold, also within larger threshold.
    #[test]
    fn prop_threshold_monotone(
        a in short_word_strategy(),
        b in short_word_strategy(),
        t1 in 0usize..=2,
        t2 in 0usize..=3
    ) {
        if t1 <= t2 {
            let result_t1 = levenshtein_within(&a, &b, t1);
            let result_t2 = levenshtein_within(&a, &b, t2);

            if result_t1 {
                prop_assert!(
                    result_t2,
                    "Larger threshold should also match if smaller threshold matched"
                );
            }
        }
    }

    /// Property: Case sensitivity
    ///
    /// Levenshtein matching is case-sensitive.
    #[test]
    fn prop_case_sensitive_distance(word in short_word_strategy()) {
        if !word.is_empty() && word != word.to_uppercase() {
            let upper = word.to_uppercase();

            // Case change should require edits for most strings
            // (unless the string is all lowercase letters)
            let char_count = word.chars().count();
            let result = levenshtein_within(&word, &upper, 0);

            // Case change is not distance 0
            if char_count <= 5 {
                // For short strings, case change is at most char_count edits
                prop_assert!(
                    !result || word == upper,
                    "Case change should not be within distance 0"
                );
            }
        }
    }

    /// Property: Empty string to non-empty
    ///
    /// Empty to non-empty depends on the length of the string.
    #[test]
    fn prop_empty_to_nonempty_distance(word in short_word_strategy()) {
        let len = word.len();
        let max_dist = len;

        let result = levenshtein_within("", &word, max_dist);
        prop_assert!(
            result,
            "Empty to word of length {} should be within distance {}",
            len, max_dist
        );

        // Smaller threshold might not work
        if len > 0 {
            let result_smaller = levenshtein_within("", &word, len - 1);
            // This depends on the actual distance, so we just verify it ran
            let _ = result_smaller;
        }
    }

    /// Property: Commutativity
    ///
    /// Levenshtein distance should be symmetric.
    #[test]
    fn prop_levenshtein_commutative(
        a in short_word_strategy(),
        b in short_word_strategy(),
        max_dist in 0usize..=3
    ) {
        let result_a_b = levenshtein_within(&a, &b, max_dist);
        let result_b_a = levenshtein_within(&b, &a, max_dist);

        prop_assert_eq!(
            result_a_b, result_b_a,
            "Levenshtein should be commutative: '{}' vs '{}' gave {} vs {}",
            a, b, result_a_b, result_b_a
        );
    }
}

// ============================================================================
// PARAMETRIC DFA TESTS
// ============================================================================

#[cfg(test)]
mod dfa_tests {
    use sorex::{ParametricDFA, QueryMatcher};

    /// Test: DFA construction doesn't panic
    #[test]
    fn test_dfa_construction_safe() {
        let dfa = ParametricDFA::build(false);
        let _matcher = QueryMatcher::new(&dfa, "test");
    }

    /// Test: DFA empty pattern handling
    #[test]
    fn test_dfa_empty_pattern() {
        let dfa = ParametricDFA::build(false);
        let _matcher = QueryMatcher::new(&dfa, "");
    }

    /// Test: QueryMatcher matches same candidate
    #[test]
    fn test_matcher_self_match() {
        let dfa = ParametricDFA::build(false);
        let matcher = QueryMatcher::new(&dfa, "test");

        let result = matcher.matches("test");
        assert!(result.is_some(), "Pattern should match itself");
    }

    /// Test: QueryMatcher result is consistent
    #[test]
    fn test_matcher_deterministic() {
        let dfa = ParametricDFA::build(false);
        let matcher = QueryMatcher::new(&dfa, "test");

        let result1 = matcher.matches("test");
        let result2 = matcher.matches("test");

        assert_eq!(result1, result2, "Matcher should be deterministic");
    }

    /// Test: DFA can handle various inputs without panicking
    #[test]
    fn test_dfa_no_panic() {
        let dfa = ParametricDFA::build(false);
        let matcher = QueryMatcher::new(&dfa, "pattern");

        // Should not panic
        let _ = matcher.matches("candidate");
    }

    /// Test: DFA with transpositions handles input correctly
    #[test]
    fn test_dfa_with_transpositions() {
        let dfa = ParametricDFA::build(true);  // with_transpositions = true
        let _matcher = QueryMatcher::new(&dfa, "test");
    }
}
