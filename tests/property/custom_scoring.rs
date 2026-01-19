//! Property tests for custom scoring: delta encoding, tier penalties, multi-term aggregation.
//!
//! These tests verify invariants from Scoring.lean for custom ranking functions:
//! - Delta encoding roundtrip: decode(encode(score)) = score
//! - Delta encoding monotonicity: descending scores → ascending encoded values
//! - Tier penalties preserve relative ordering
//! - Tier penalties don't increase scores
//! - Multi-term aggregation is commutative and monotone

use proptest::prelude::*;

// ============================================================================
// DELTA ENCODING FUNCTIONS (mirrors Lean spec)
// ============================================================================

/// Encode score using (max - score) transformation.
/// This converts descending scores to ascending deltas for efficient varint compression.
fn encode_score_delta(max_score: u32, score: u32) -> u32 {
    debug_assert!(score <= max_score, "score must be <= max_score");
    max_score - score
}

/// Decode delta-encoded score back to original.
fn decode_score_delta(max_score: u32, encoded: u32) -> u32 {
    debug_assert!(encoded <= max_score, "encoded must be <= max_score");
    max_score - encoded
}

/// Apply tier penalty (percentage-based, e.g., 50 = 50%).
fn apply_tier_penalty(score: u32, penalty_percent: u32) -> u32 {
    (score as u64 * penalty_percent as u64 / 100) as u32
}

/// Sum scores for multi-term aggregation.
fn multi_term_score(scores: &[u32]) -> u32 {
    scores.iter().fold(0u32, |acc, &s| acc.saturating_add(s))
}

// ============================================================================
// DELTA ENCODING PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Lean theorem: score_delta_roundtrip
    /// decode(encode(score)) = score when score <= max_score
    #[test]
    fn lean_theorem_score_delta_roundtrip(
        max_score in 1u32..10000,
        score_ratio in 0.0..=1.0f64
    ) {
        let score = (max_score as f64 * score_ratio) as u32;
        prop_assert!(score <= max_score);

        let encoded = encode_score_delta(max_score, score);
        let decoded = decode_score_delta(max_score, encoded);

        prop_assert_eq!(
            decoded, score,
            "Roundtrip failed: encode({}, {}) = {}, decode({}, {}) = {}",
            max_score, score, encoded, max_score, encoded, decoded
        );
    }

    /// Lean theorem: score_delta_monotone
    /// When s1 >= s2 (descending), encode(s1) <= encode(s2) (ascending)
    #[test]
    fn lean_theorem_score_delta_monotone(
        max_score in 1u32..10000,
        s1_ratio in 0.0..=1.0f64,
        s2_ratio in 0.0..=1.0f64
    ) {
        let s1 = (max_score as f64 * s1_ratio) as u32;
        let s2 = (max_score as f64 * s2_ratio) as u32;

        // Only test when s1 >= s2 (descending original scores)
        if s1 >= s2 {
            let enc1 = encode_score_delta(max_score, s1);
            let enc2 = encode_score_delta(max_score, s2);

            prop_assert!(
                enc1 <= enc2,
                "Monotonicity failed: s1={} >= s2={}, but enc1={} > enc2={}",
                s1, s2, enc1, enc2
            );
        }
    }

    /// Property: Encoded values are valid (within [0, max_score])
    #[test]
    fn prop_encoded_in_range(
        max_score in 1u32..10000,
        score_ratio in 0.0..=1.0f64
    ) {
        let score = (max_score as f64 * score_ratio) as u32;
        let encoded = encode_score_delta(max_score, score);

        prop_assert!(
            encoded <= max_score,
            "Encoded {} should be <= max_score {}",
            encoded, max_score
        );
    }

    /// Property: Descending score sequence produces ascending encoded sequence
    #[test]
    fn prop_descending_to_ascending(
        max_score in 100u32..10000,
        count in 2usize..20
    ) {
        // Generate descending scores
        let scores: Vec<u32> = (0..count)
            .map(|i| max_score - (i as u32 * max_score / count as u32))
            .collect();

        // Verify descending
        for i in 1..scores.len() {
            prop_assert!(scores[i] <= scores[i - 1], "Scores not descending");
        }

        // Encode and verify ascending
        let encoded: Vec<u32> = scores
            .iter()
            .map(|&s| encode_score_delta(max_score, s))
            .collect();

        for i in 1..encoded.len() {
            prop_assert!(
                encoded[i] >= encoded[i - 1],
                "Encoded values should be ascending: {} >= {} failed",
                encoded[i], encoded[i - 1]
            );
        }
    }
}

// ============================================================================
// TIER PENALTY PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Lean theorem: tier_penalty_monotone
    /// When s1 >= s2, apply_penalty(s1) >= apply_penalty(s2)
    #[test]
    fn lean_theorem_tier_penalty_monotone(
        s1 in 0u32..10000,
        s2 in 0u32..10000,
        penalty in 0u32..=100
    ) {
        if s1 >= s2 {
            let p1 = apply_tier_penalty(s1, penalty);
            let p2 = apply_tier_penalty(s2, penalty);

            prop_assert!(
                p1 >= p2,
                "Tier penalty not monotone: s1={} >= s2={}, penalty={}%, but p1={} < p2={}",
                s1, s2, penalty, p1, p2
            );
        }
    }

    /// Lean theorem: tier_penalty_decreasing
    /// apply_penalty(score, penalty) <= score when penalty <= 100
    #[test]
    fn lean_theorem_tier_penalty_decreasing(
        score in 0u32..10000,
        penalty in 0u32..=100
    ) {
        let penalized = apply_tier_penalty(score, penalty);

        prop_assert!(
            penalized <= score,
            "Tier penalty increased score: {} with {}% penalty = {} > {}",
            score, penalty, penalized, score
        );
    }

    /// Property: 100% penalty returns original score
    #[test]
    fn prop_full_penalty_identity(score in 0u32..10000) {
        let penalized = apply_tier_penalty(score, 100);
        prop_assert_eq!(penalized, score, "100% penalty should return original score");
    }

    /// Property: 0% penalty returns zero
    #[test]
    fn prop_zero_penalty_zeroes(score in 0u32..10000) {
        let penalized = apply_tier_penalty(score, 0);
        prop_assert_eq!(penalized, 0, "0% penalty should return 0");
    }

    /// Property: Typical tier penalties (T2 prefix ~75%, T3 fuzzy ~50%)
    #[test]
    fn prop_typical_tier_penalties(
        score in 100u32..10000
    ) {
        let t2_penalty = 75; // prefix match penalty
        let t3_penalty = 50; // fuzzy match penalty

        let t2_score = apply_tier_penalty(score, t2_penalty);
        let t3_score = apply_tier_penalty(score, t3_penalty);

        // T2 should score higher than T3
        prop_assert!(
            t2_score >= t3_score,
            "T2 ({} @ {}%) = {} should >= T3 ({} @ {}%) = {}",
            score, t2_penalty, t2_score, score, t3_penalty, t3_score
        );

        // Both should be less than original
        prop_assert!(t2_score <= score);
        prop_assert!(t3_score <= score);
    }
}

// ============================================================================
// MULTI-TERM AGGREGATION PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Lean theorem: multiTermScore_comm
    /// Summing scores is commutative (order doesn't matter)
    #[test]
    fn lean_theorem_multi_term_comm(
        s1 in 0u32..1000,
        s2 in 0u32..1000,
        rest in prop::collection::vec(0u32..1000, 0..10)
    ) {
        let mut order1 = vec![s1, s2];
        order1.extend(&rest);

        let mut order2 = vec![s2, s1];
        order2.extend(&rest);

        let sum1 = multi_term_score(&order1);
        let sum2 = multi_term_score(&order2);

        prop_assert_eq!(
            sum1, sum2,
            "Multi-term should be commutative: {:?} vs {:?}",
            order1, order2
        );
    }

    /// Lean theorem: multiTermScore_monotone
    /// Adding a positive score increases the total
    #[test]
    fn lean_theorem_multi_term_monotone(
        scores in prop::collection::vec(0u32..1000, 0..10),
        extra in 1u32..1000
    ) {
        let base_sum = multi_term_score(&scores);

        let mut with_extra = scores.clone();
        with_extra.push(extra);
        let extra_sum = multi_term_score(&with_extra);

        prop_assert!(
            extra_sum > base_sum,
            "Adding {} should increase sum: {} + {} = {} should > {}",
            extra, base_sum, extra, extra_sum, base_sum
        );
    }

    /// Property: Empty scores list sums to zero
    #[test]
    fn prop_empty_sum_zero(_dummy in 0..1i32) {
        let sum = multi_term_score(&[]);
        prop_assert_eq!(sum, 0, "Empty list should sum to 0");
    }

    /// Property: Single score returns itself
    #[test]
    fn prop_single_score_identity(score in 0u32..10000) {
        let sum = multi_term_score(&[score]);
        prop_assert_eq!(sum, score, "Single score should return itself");
    }

    /// Property: Sum is associative
    #[test]
    fn prop_sum_associative(
        a in 0u32..1000,
        b in 0u32..1000,
        c in 0u32..1000
    ) {
        // (a + b) + c == a + (b + c)
        let left = multi_term_score(&[a, b]) + c;
        let right = a + multi_term_score(&[b, c]);

        // Use multi_term_score for all
        let all = multi_term_score(&[a, b, c]);

        prop_assert_eq!(left, all);
        prop_assert_eq!(right, all);
    }
}

// ============================================================================
// INTEGRATION: DELTA ENCODING WITH TIER PENALTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property: Tier penalties preserve delta encoding monotonicity
    ///
    /// If we have descending scores, apply the same penalty, encode them,
    /// the encoded values should still be ascending.
    #[test]
    fn prop_penalty_preserves_delta_monotone(
        max_score in 100u32..10000,
        penalty in 1u32..=100,
        count in 2usize..10
    ) {
        // Generate descending scores
        let scores: Vec<u32> = (0..count)
            .map(|i| max_score - (i as u32 * max_score / count as u32))
            .collect();

        // Apply penalty to all (still descending)
        let penalized: Vec<u32> = scores
            .iter()
            .map(|&s| apply_tier_penalty(s, penalty))
            .collect();

        // Penalized should still be descending
        for i in 1..penalized.len() {
            prop_assert!(
                penalized[i] <= penalized[i - 1],
                "Penalized scores not descending"
            );
        }

        // Find max for encoding
        let pen_max = penalized.iter().copied().max().unwrap_or(0);

        // Encode (should produce ascending)
        let encoded: Vec<u32> = penalized
            .iter()
            .map(|&s| encode_score_delta(pen_max, s))
            .collect();

        for i in 1..encoded.len() {
            prop_assert!(
                encoded[i] >= encoded[i - 1],
                "Encoded penalized values should be ascending"
            );
        }
    }

    /// Property: Multi-term then delta encode preserves ordering
    #[test]
    fn prop_multiterm_then_delta(
        doc1_scores in prop::collection::vec(1u32..100, 1..5),
        doc2_scores in prop::collection::vec(1u32..100, 1..5)
    ) {
        let sum1 = multi_term_score(&doc1_scores);
        let sum2 = multi_term_score(&doc2_scores);

        let max_score = sum1.max(sum2);

        // Encode both
        let enc1 = encode_score_delta(max_score, sum1);
        let enc2 = encode_score_delta(max_score, sum2);

        // If sum1 > sum2, then enc1 < enc2 (lower encoded = higher score)
        if sum1 > sum2 {
            prop_assert!(
                enc1 < enc2,
                "Higher sum {} should have lower encoded {} vs {}",
                sum1, enc1, enc2
            );
        } else if sum1 < sum2 {
            prop_assert!(
                enc1 > enc2,
                "Lower sum {} should have higher encoded {} vs {}",
                sum1, enc1, enc2
            );
        } else {
            prop_assert_eq!(enc1, enc2, "Equal sums should have equal encoding");
        }
    }
}

// ============================================================================
// UNIT TESTS
// ============================================================================

#[test]
fn test_delta_encoding_edge_cases() {
    // Max score with score = 0
    assert_eq!(encode_score_delta(100, 0), 100);
    assert_eq!(decode_score_delta(100, 100), 0);

    // Max score with score = max
    assert_eq!(encode_score_delta(100, 100), 0);
    assert_eq!(decode_score_delta(100, 0), 100);

    // Roundtrip at boundaries
    for score in [0, 1, 50, 99, 100] {
        let encoded = encode_score_delta(100, score);
        let decoded = decode_score_delta(100, encoded);
        assert_eq!(decoded, score, "Roundtrip failed for {}", score);
    }
}

#[test]
fn test_tier_penalty_edge_cases() {
    // 100% penalty = identity
    assert_eq!(apply_tier_penalty(500, 100), 500);

    // 0% penalty = zero
    assert_eq!(apply_tier_penalty(500, 0), 0);

    // 50% penalty = half
    assert_eq!(apply_tier_penalty(100, 50), 50);

    // Rounding: 33% of 100 = 33
    assert_eq!(apply_tier_penalty(100, 33), 33);
}

#[test]
fn test_multi_term_score_edge_cases() {
    // Empty
    assert_eq!(multi_term_score(&[]), 0);

    // Single
    assert_eq!(multi_term_score(&[42]), 42);

    // Multiple
    assert_eq!(multi_term_score(&[10, 20, 30]), 60);

    // Saturating addition (no overflow)
    assert_eq!(multi_term_score(&[u32::MAX, 1]), u32::MAX);
}

#[test]
fn test_descending_scores_to_ascending_deltas() {
    let max = 1000;
    let descending = vec![1000, 800, 600, 400, 200, 0];

    let encoded: Vec<u32> = descending
        .iter()
        .map(|&s| encode_score_delta(max, s))
        .collect();

    assert_eq!(encoded, vec![0, 200, 400, 600, 800, 1000]);

    // Verify ascending
    for i in 1..encoded.len() {
        assert!(encoded[i] >= encoded[i - 1], "Should be ascending");
    }

    // Verify roundtrip
    let decoded: Vec<u32> = encoded
        .iter()
        .map(|&e| decode_score_delta(max, e))
        .collect();

    assert_eq!(decoded, descending);
}
