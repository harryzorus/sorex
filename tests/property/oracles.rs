//! Reference oracles for differential testing.
//!
//! These are simple, obviously-correct implementations that mirror the Lean
//! specifications in `lean/SearchVerified/Oracle.lean`. They serve as ground
//! truth for verifying the optimized Rust implementations.
//!
//! Philosophy: Prove the simple implementation correct in Lean, then verify
//! Rust matches it via differential fuzzing.

use proptest::prelude::*;

// =============================================================================
// ORACLE IMPLEMENTATIONS
// =============================================================================

/// Simple O(n² log n) suffix array construction.
///
/// Mirrors `simpleSuffixArray` in Lean: sort positions by suffix string.
/// This is slow but obviously correct.
pub fn oracle_suffix_array(input: &str) -> Vec<usize> {
    let mut positions: Vec<usize> = (0..input.len()).collect();
    positions.sort_by(|&i, &j| input[i..].cmp(&input[j..]));
    positions
}

/// Linear scan for lower bound.
///
/// Mirrors `linearSearchFirstGe` in Lean: find first element >= target.
/// O(n) but trivially correct.
pub fn oracle_lower_bound<T: Ord>(arr: &[T], target: &T) -> usize {
    arr.iter().position(|x| x >= target).unwrap_or(arr.len())
}

/// Classic Levenshtein edit distance via dynamic programming.
///
/// Mirrors `levenshteinDistance` in Lean: standard Wagner-Fischer algorithm.
/// O(nm) time, O(n) space.
pub fn oracle_levenshtein(s1: &str, s2: &str) -> usize {
    let a: Vec<char> = s1.chars().collect();
    let b: Vec<char> = s2.chars().collect();
    let m = a.len();
    let n = b.len();

    if m == 0 {
        return n;
    }
    if n == 0 {
        return m;
    }

    // Two-row DP
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0; n + 1];

    for (i, c1) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, c2) in b.iter().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1) // deletion
                .min(curr[j] + 1) // insertion
                .min(prev[j] + cost); // substitution
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[n]
}

/// Common prefix length between two strings.
///
/// Mirrors `commonPrefixLen` in Lean: character-by-character comparison.
pub fn oracle_common_prefix_len(s1: &str, s2: &str) -> usize {
    s1.chars()
        .zip(s2.chars())
        .take_while(|(a, b)| a == b)
        .count()
}

/// Common prefix length for byte slices.
///
/// Used for vocabulary front compression.
#[allow(dead_code)]
pub fn oracle_common_prefix_bytes(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
}

/// Varint encode (LEB128).
///
/// Mirrors `encodeVarint` in Lean.
pub fn oracle_encode_varint(mut value: u64) -> Vec<u8> {
    let mut result = Vec::new();
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            result.push(byte);
            break;
        } else {
            result.push(byte | 0x80);
        }
    }
    result
}

/// Varint decode (LEB128).
///
/// Mirrors `decodeVarint` in Lean.
pub fn oracle_decode_varint(bytes: &[u8]) -> Option<(u64, usize)> {
    if bytes.is_empty() {
        return None;
    }

    let mut result: u64 = 0;
    let mut shift = 0;

    for (i, &byte) in bytes.iter().enumerate() {
        if i >= 10 {
            return None; // Too long
        }
        result |= ((byte & 0x7F) as u64) << shift;
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
        shift += 7;
    }

    None // Incomplete
}

// =============================================================================
// DIFFERENTIAL PROPERTY TESTS
// =============================================================================

proptest! {
    /// Verify varint roundtrip matches oracle.
    #[test]
    fn prop_varint_roundtrip_oracle(value: u64) {
        let encoded = oracle_encode_varint(value);
        let decoded = oracle_decode_varint(&encoded);
        prop_assert!(decoded.is_some());
        let (decoded_value, consumed) = decoded.unwrap();
        prop_assert_eq!(decoded_value, value);
        prop_assert_eq!(consumed, encoded.len());
    }

    /// Verify Levenshtein triangle inequality.
    #[test]
    fn prop_levenshtein_triangle(
        a in "[a-z]{0,10}",
        b in "[a-z]{0,10}",
        c in "[a-z]{0,10}"
    ) {
        let d_ac = oracle_levenshtein(&a, &c);
        let d_ab = oracle_levenshtein(&a, &b);
        let d_bc = oracle_levenshtein(&b, &c);
        prop_assert!(d_ac <= d_ab + d_bc, "Triangle inequality violated");
    }

    /// Verify Levenshtein length bound.
    #[test]
    fn prop_levenshtein_length_bound(
        s1 in "[a-z]{0,20}",
        s2 in "[a-z]{0,20}"
    ) {
        let dist = oracle_levenshtein(&s1, &s2);
        let len_diff = (s1.len() as isize - s2.len() as isize).unsigned_abs();
        prop_assert!(len_diff <= dist, "Length diff {} > distance {}", len_diff, dist);
    }

    /// Verify common prefix is bounded.
    #[test]
    fn prop_common_prefix_bounded(
        s1 in "[a-z]{0,30}",
        s2 in "[a-z]{0,30}"
    ) {
        let len = oracle_common_prefix_len(&s1, &s2);
        prop_assert!(len <= s1.len());
        prop_assert!(len <= s2.len());
    }

    /// Verify common prefix correctness.
    #[test]
    fn prop_common_prefix_correct(
        s1 in "[a-z]{0,30}",
        s2 in "[a-z]{0,30}"
    ) {
        let len = oracle_common_prefix_len(&s1, &s2);

        // All chars before len are equal
        let chars1: Vec<char> = s1.chars().collect();
        let chars2: Vec<char> = s2.chars().collect();
        for i in 0..len {
            prop_assert_eq!(chars1[i], chars2[i]);
        }

        // Char at len differs (or one string ended)
        if len < chars1.len() && len < chars2.len() {
            prop_assert_ne!(chars1[len], chars2[len]);
        }
    }

    /// Verify suffix array is a permutation.
    #[test]
    fn prop_suffix_array_permutation(s in "[a-z]{1,50}") {
        let sa = oracle_suffix_array(&s);

        // Must be same length as input
        prop_assert_eq!(sa.len(), s.len());

        // Must contain each index exactly once
        let mut sorted = sa.clone();
        sorted.sort();
        let expected: Vec<usize> = (0..s.len()).collect();
        prop_assert_eq!(sorted, expected);
    }

    /// Verify suffix array is sorted.
    #[test]
    fn prop_suffix_array_sorted(s in "[a-z]{1,50}") {
        let sa = oracle_suffix_array(&s);

        for i in 1..sa.len() {
            let suffix_prev = &s[sa[i-1]..];
            let suffix_curr = &s[sa[i]..];
            prop_assert!(suffix_prev <= suffix_curr,
                "Not sorted at {}: {:?} > {:?}", i, suffix_prev, suffix_curr);
        }
    }

    /// Verify lower bound correctness.
    #[test]
    fn prop_lower_bound_correct(
        arr in prop::collection::vec(0u32..1000, 0..50),
        target in 0u32..1000
    ) {
        let mut sorted = arr.clone();
        sorted.sort();

        let idx = oracle_lower_bound(&sorted, &target);

        // All elements before idx are < target
        for i in 0..idx {
            prop_assert!(sorted[i] < target);
        }

        // Element at idx (if exists) is >= target
        if idx < sorted.len() {
            prop_assert!(sorted[idx] >= target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_levenshtein_basic() {
        assert_eq!(oracle_levenshtein("", ""), 0);
        assert_eq!(oracle_levenshtein("a", ""), 1);
        assert_eq!(oracle_levenshtein("", "a"), 1);
        assert_eq!(oracle_levenshtein("kitten", "sitting"), 3);
        assert_eq!(oracle_levenshtein("saturday", "sunday"), 3);
    }

    #[test]
    fn test_common_prefix() {
        assert_eq!(oracle_common_prefix_len("hello", "help"), 3);
        assert_eq!(oracle_common_prefix_len("abc", "xyz"), 0);
        assert_eq!(oracle_common_prefix_len("abc", "abc"), 3);
        assert_eq!(oracle_common_prefix_len("", "abc"), 0);
    }

    #[test]
    fn test_varint_roundtrip() {
        for value in [0u64, 1, 127, 128, 255, 16383, 16384, u64::MAX] {
            let encoded = oracle_encode_varint(value);
            let (decoded, consumed) = oracle_decode_varint(&encoded).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, encoded.len());
        }
    }

    #[test]
    fn test_suffix_array() {
        let sa = oracle_suffix_array("banana");
        // Expected: [5, 3, 1, 0, 4, 2] for "a", "ana", "anana", "banana", "na", "nana"
        assert_eq!(sa, vec![5, 3, 1, 0, 4, 2]);
    }
}
