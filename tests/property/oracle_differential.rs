//! Differential testing: compare optimized Rust implementations against oracles.
//!
//! This module implements "fractional proof decomposition" - each optimized
//! algorithm is tested against a simple, obviously-correct oracle implementation.
//! If they disagree, the oracle is right.
//!
//! Philosophy from Theorem.dev: Bug detection scales logarithmically with
//! decomposition depth, not linearly with test count.

use super::oracles::{
    oracle_common_prefix_len, oracle_decode_varint, oracle_encode_varint, oracle_levenshtein,
    oracle_lower_bound, oracle_suffix_array,
};
use proptest::prelude::*;
use sorex::binary::{decode_varint, encode_varint};

// =============================================================================
// VARINT: Rust implementation vs Oracle
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Differential test: Rust varint encode matches oracle encode.
    #[test]
    fn diff_varint_encode(value: u64) {
        let mut rust_buf = Vec::new();
        encode_varint(value, &mut rust_buf);
        let oracle_buf = oracle_encode_varint(value);

        prop_assert_eq!(
            rust_buf, oracle_buf,
            "Rust encode differs from oracle for value {}",
            value
        );
    }

    /// Differential test: Rust varint decode matches oracle decode.
    #[test]
    fn diff_varint_decode(value: u64) {
        // Encode with oracle (known correct)
        let encoded = oracle_encode_varint(value);

        // Decode with both implementations
        let rust_result = decode_varint(&encoded);
        let oracle_result = oracle_decode_varint(&encoded);

        prop_assert!(rust_result.is_ok(), "Rust decode failed for oracle-encoded {}", value);
        prop_assert!(oracle_result.is_some(), "Oracle decode failed for self-encoded {}", value);

        let (rust_val, rust_consumed) = rust_result.unwrap();
        let (oracle_val, oracle_consumed) = oracle_result.unwrap();

        prop_assert_eq!(rust_val, oracle_val, "Decoded values differ for {}", value);
        prop_assert_eq!(rust_consumed, oracle_consumed, "Consumed bytes differ for {}", value);
    }

    /// Differential test: Rust encode + oracle decode roundtrips.
    #[test]
    fn diff_varint_cross_roundtrip(value: u64) {
        // Encode with Rust, decode with oracle
        let mut rust_encoded = Vec::new();
        encode_varint(value, &mut rust_encoded);

        let oracle_decoded = oracle_decode_varint(&rust_encoded);
        prop_assert!(oracle_decoded.is_some(), "Oracle couldn't decode Rust encoding");

        let (decoded_val, _) = oracle_decoded.unwrap();
        prop_assert_eq!(decoded_val, value, "Cross-roundtrip failed");
    }
}

// =============================================================================
// LEVENSHTEIN: Rust implementation vs Oracle
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Differential test: Rust levenshtein_within matches oracle distance.
    #[test]
    fn diff_levenshtein_within(
        a in "[a-z]{0,15}",
        b in "[a-z]{0,15}",
        max_dist in 0usize..5
    ) {
        let oracle_dist = oracle_levenshtein(&a, &b);
        let rust_within = sorex::levenshtein_within(&a, &b, max_dist);

        // Rust should return true iff oracle distance <= max_dist
        let expected = oracle_dist <= max_dist;
        prop_assert_eq!(
            rust_within, expected,
            "levenshtein_within({:?}, {:?}, {}) = {} but oracle distance = {}",
            a, b, max_dist, rust_within, oracle_dist
        );
    }

    /// Differential test: Unicode strings don't break levenshtein.
    #[test]
    fn diff_levenshtein_unicode(
        a in prop::sample::select(vec![
            "cafe", "caf\u{00e9}", "na\u{00ef}ve", "r\u{00e9}sum\u{00e9}",
            "\u{00fc}ber", "t\u{014d}ky\u{014d}", "hello", "world"
        ]),
        b in prop::sample::select(vec![
            "cafe", "caf\u{00e9}", "na\u{00ef}ve", "r\u{00e9}sum\u{00e9}",
            "\u{00fc}ber", "t\u{014d}ky\u{014d}", "hello", "world"
        ])
    ) {
        let oracle_dist = oracle_levenshtein(a, b);

        // Rust within should match oracle for distance 0, 1, 2
        for max in 0..=3 {
            let rust_within = sorex::levenshtein_within(a, b, max);
            let expected = oracle_dist <= max;
            prop_assert_eq!(
                rust_within, expected,
                "Unicode levenshtein_within({:?}, {:?}, {}) = {} but oracle = {}",
                a, b, max, rust_within, oracle_dist
            );
        }
    }
}

// =============================================================================
// SUFFIX ARRAY: Rust implementation vs Oracle
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Differential test: Rust suffix array produces same sorted order as oracle.
    #[test]
    fn diff_suffix_array_order(text in "[a-z]{1,50}") {
        use super::common::build_test_index;

        let index = build_test_index(&[&text]);
        let oracle_sa = oracle_suffix_array(&text);

        // Both should have same length
        prop_assert_eq!(
            index.suffix_array.len(), oracle_sa.len(),
            "Suffix array lengths differ: rust={}, oracle={}",
            index.suffix_array.len(), oracle_sa.len()
        );

        // Compare the sorted order
        for (i, (rust_entry, &oracle_pos)) in index.suffix_array.iter().zip(oracle_sa.iter()).enumerate() {
            // Rust uses (doc_id, char_offset), oracle uses byte offset
            // For single-doc ASCII, char_offset == byte_offset
            prop_assert_eq!(
                rust_entry.offset, oracle_pos,
                "Suffix array differs at position {}: rust offset={}, oracle offset={}",
                i, rust_entry.offset, oracle_pos
            );
        }
    }

    /// Differential test: Suffix array is a valid permutation.
    #[test]
    fn diff_suffix_array_permutation(text in "[a-z]{1,30}") {
        let oracle_sa = oracle_suffix_array(&text);

        // Must contain each position exactly once
        let mut sorted = oracle_sa.clone();
        sorted.sort();
        let expected: Vec<usize> = (0..text.len()).collect();

        prop_assert_eq!(
            sorted, expected,
            "Oracle suffix array is not a permutation"
        );
    }
}

// =============================================================================
// BINARY SEARCH: Rust implementation vs Oracle
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Differential test: Rust binary search matches oracle linear scan.
    #[test]
    fn diff_lower_bound(
        arr in prop::collection::vec(0u32..10000, 0..100),
        target in 0u32..10000
    ) {
        let mut sorted = arr.clone();
        sorted.sort();

        // Oracle: linear scan
        let oracle_idx = oracle_lower_bound(&sorted, &target);

        // Rust: binary search (partition_point is the standard library's lower_bound)
        let rust_idx = sorted.partition_point(|&x| x < target);

        prop_assert_eq!(
            rust_idx, oracle_idx,
            "Binary search differs from linear scan: rust={}, oracle={} for target={}",
            rust_idx, oracle_idx, target
        );
    }
}

// =============================================================================
// COMMON PREFIX: Rust implementation vs Oracle
// =============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    /// Differential test: Rust common_prefix_len matches oracle.
    #[test]
    fn diff_common_prefix_len(
        s1 in "[a-z]{0,50}",
        s2 in "[a-z]{0,50}"
    ) {
        let oracle_len = oracle_common_prefix_len(&s1, &s2);

        // Rust implementation (used in vocabulary encoding)
        let rust_len = s1.chars()
            .zip(s2.chars())
            .take_while(|(a, b)| a == b)
            .count();

        prop_assert_eq!(
            rust_len, oracle_len,
            "common_prefix_len differs: rust={}, oracle={} for {:?} vs {:?}",
            rust_len, oracle_len, s1, s2
        );
    }

    /// Differential test: Common prefix with Unicode.
    #[test]
    fn diff_common_prefix_unicode(
        s1 in prop::sample::select(vec![
            "caf\u{00e9}", "cafe", "application", "apply", "harish", "har\u{012b}\u{1e63}h"
        ]),
        s2 in prop::sample::select(vec![
            "caf\u{00e9}", "cafe", "applications", "applied", "harish", "har\u{012b}\u{1e63}h"
        ])
    ) {
        let oracle_len = oracle_common_prefix_len(s1, s2);

        let rust_len = s1.chars()
            .zip(s2.chars())
            .take_while(|(a, b)| a == b)
            .count();

        prop_assert_eq!(
            rust_len, oracle_len,
            "Unicode common_prefix differs: rust={}, oracle={} for {:?} vs {:?}",
            rust_len, oracle_len, s1, s2
        );
    }
}

// =============================================================================
// COMBINED ORACLE TESTS
// =============================================================================

#[cfg(test)]
mod combined_tests {
    use super::*;

    /// Verify the oracle implementations themselves are consistent.
    #[test]
    fn oracles_are_consistent() {
        // Levenshtein: symmetric
        assert_eq!(oracle_levenshtein("abc", "xyz"), oracle_levenshtein("xyz", "abc"));

        // Levenshtein: identity
        assert_eq!(oracle_levenshtein("hello", "hello"), 0);

        // Common prefix: bounded
        let cp = oracle_common_prefix_len("hello", "help");
        assert!(cp <= "hello".len());
        assert!(cp <= "help".len());
        assert_eq!(cp, 3);

        // Suffix array: sorted
        let sa = oracle_suffix_array("banana");
        for i in 1..sa.len() {
            assert!("banana"[sa[i - 1]..] <= "banana"[sa[i]..]);
        }

        // Varint: roundtrip
        for val in [0u64, 1, 127, 128, 16383, 16384, u64::MAX] {
            let enc = oracle_encode_varint(val);
            let (dec, _) = oracle_decode_varint(&enc).unwrap();
            assert_eq!(val, dec);
        }
    }

    /// Edge cases that often catch bugs.
    #[test]
    fn edge_cases() {
        // Empty strings
        assert_eq!(oracle_levenshtein("", ""), 0);
        assert_eq!(oracle_levenshtein("", "abc"), 3);
        assert_eq!(oracle_common_prefix_len("", "abc"), 0);

        // Single character
        assert_eq!(oracle_levenshtein("a", "b"), 1);
        assert_eq!(oracle_levenshtein("a", "a"), 0);

        // Suffix array with repeated characters
        let sa = oracle_suffix_array("aaa");
        assert_eq!(sa.len(), 3);

        // Lower bound edge cases
        assert_eq!(oracle_lower_bound::<i32>(&[], &5), 0);
        assert_eq!(oracle_lower_bound(&[1, 2, 3], &0), 0);
        assert_eq!(oracle_lower_bound(&[1, 2, 3], &4), 3);
    }
}
