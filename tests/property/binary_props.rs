//! Binary encoding property tests.
//!
//! These tests verify binary encoding roundtrips and invariants:
//! - Varint encoding/decoding is reversible
//! - Suffix array encoding is reversible
//! - Vocabulary front compression is reversible
//! - Postings delta encoding is reversible
//! - Multi-byte varints have correct continuation bits

use proptest::prelude::*;
use sorex::binary::{
    decode_postings, decode_suffix_array, decode_varint, decode_vocabulary,
    encode_postings, encode_suffix_array, encode_varint, encode_vocabulary,
    PostingEntry, MAX_VARINT_BYTES,
};

// ============================================================================
// VARINT ENCODING PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// Property: Varint encoding is reversible for all u64 values.
    #[test]
    fn prop_varint_roundtrip(value: u64) {
        let mut buf = Vec::new();
        encode_varint(value, &mut buf);
        let (decoded, consumed) = decode_varint(&buf).unwrap();
        prop_assert_eq!(value, decoded);
        prop_assert_eq!(consumed, buf.len());
    }

    /// Property: Varint encoding uses at most MAX_VARINT_BYTES.
    #[test]
    fn prop_varint_size_bound(value: u64) {
        let mut buf = Vec::new();
        encode_varint(value, &mut buf);
        prop_assert!(
            buf.len() <= MAX_VARINT_BYTES,
            "Varint for {} used {} bytes (max {})",
            value, buf.len(), MAX_VARINT_BYTES
        );
    }

    /// Property: Multi-byte varints have continuation bit set on all but last byte.
    /// This catches mutations like `| 0x80` → `^ 0x80`.
    #[test]
    fn prop_varint_continuation_bit(value in 128u64..u64::MAX) {
        let mut buf = Vec::new();
        encode_varint(value, &mut buf);

        // Multi-byte varints: all bytes except last must have continuation bit
        prop_assert!(buf.len() > 1, "value {} should produce multi-byte varint", value);

        for (i, &byte) in buf.iter().enumerate() {
            if i < buf.len() - 1 {
                prop_assert!(
                    byte & 0x80 != 0,
                    "Byte {} should have continuation bit set for value {}",
                    i, value
                );
            } else {
                prop_assert!(
                    byte & 0x80 == 0,
                    "Last byte should NOT have continuation bit for value {}",
                    value
                );
            }
        }
    }
}

// ============================================================================
// SUFFIX ARRAY ENCODING PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Property: Suffix array encoding is reversible.
    #[test]
    fn prop_suffix_array_roundtrip(
        entries in prop::collection::vec((0u32..65535, 0u32..10000), 0..500)
    ) {
        let mut buf = Vec::new();
        encode_suffix_array(&entries, &mut buf);
        let (decoded, _) = decode_suffix_array(&buf).unwrap();
        prop_assert_eq!(entries, decoded);
    }
}

// ============================================================================
// VOCABULARY ENCODING PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Property: Vocabulary encoding (front compression) is reversible.
    #[test]
    fn prop_vocabulary_roundtrip(vocab in prop::collection::vec("[a-z]{1,20}", 1..100)) {
        let mut sorted = vocab.clone();
        sorted.sort();
        sorted.dedup(); // Must be unique and sorted
        if sorted.is_empty() {
            return Ok(());
        }
        let mut buf = Vec::new();
        encode_vocabulary(&sorted, &mut buf);
        let decoded = decode_vocabulary(&buf, sorted.len()).unwrap();
        prop_assert_eq!(sorted, decoded);
    }
}

// ============================================================================
// POSTINGS ENCODING PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Property: Postings encoding (delta+varint) is reversible.
    #[test]
    fn prop_postings_roundtrip(
        doc_ids in prop::collection::vec(0u32..10000, 1..500)
    ) {
        // Delta encoding requires sorted doc_ids
        let mut sorted: Vec<u32> = doc_ids.into_iter().collect();
        sorted.sort();
        sorted.dedup();
        if sorted.is_empty() {
            return Ok(());
        }

        let entries: Vec<PostingEntry> = sorted.iter().enumerate()
            .map(|(i, &doc_id)| PostingEntry {
                doc_id,
                section_idx: (i % 10) as u32,
                heading_level: (i % 6) as u8,
            })
            .collect();

        let mut buf = Vec::new();
        encode_postings(&entries, &mut buf);
        let (decoded, _) = decode_postings(&buf).unwrap();

        prop_assert_eq!(entries.len(), decoded.len());
        for (e, d) in entries.iter().zip(decoded.iter()) {
            prop_assert_eq!(e.doc_id, d.doc_id, "doc_id mismatch");
            prop_assert_eq!(e.section_idx, d.section_idx, "section_idx mismatch");
            prop_assert_eq!(e.heading_level, d.heading_level, "heading_level mismatch");
        }
    }
}

// ============================================================================
// EDGE CASE UNIT TESTS FOR MUTATION COVERAGE
// ============================================================================

#[cfg(test)]
mod edge_case_tests {
    use super::*;

    /// Test: Varint decoder rejects 11-byte sequences (max is 10 for u64).
    /// Closes mutation gap: `< MAX_VARINT_BYTES` → `<= MAX_VARINT_BYTES`
    #[test]
    fn test_varint_rejects_overlong() {
        // 11 continuation bytes (all with 0x80 set) should be rejected
        let overlong: Vec<u8> = std::iter::repeat_n(0x80, 11).collect();
        let result = decode_varint(&overlong);
        assert!(result.is_err(), "Should reject 11-byte varint");

        // 10 continuation bytes + terminator (0x01) should work
        let max_valid: Vec<u8> = std::iter::repeat_n(0x80, 9)
            .chain(std::iter::once(0x01))
            .collect();
        let result = decode_varint(&max_valid);
        assert!(result.is_ok(), "10-byte varint should be valid");
    }

    /// Test: Varint decoder handles boundary values correctly.
    #[test]
    fn test_varint_boundary_values() {
        // Single byte boundary (127 = max single byte)
        let mut buf = Vec::new();
        encode_varint(127, &mut buf);
        assert_eq!(buf.len(), 1);
        assert_eq!(buf[0] & 0x80, 0, "127 should not have continuation bit");

        // Two byte boundary (128 = min two byte)
        buf.clear();
        encode_varint(128, &mut buf);
        assert_eq!(buf.len(), 2);
        assert_ne!(buf[0] & 0x80, 0, "First byte of 128 must have continuation bit");
        assert_eq!(buf[1] & 0x80, 0, "Last byte of 128 must not have continuation bit");

        // Max u64 should be exactly 10 bytes
        buf.clear();
        encode_varint(u64::MAX, &mut buf);
        assert_eq!(buf.len(), MAX_VARINT_BYTES, "u64::MAX should be exactly {} bytes", MAX_VARINT_BYTES);
    }

    /// Test: Empty buffer returns error, not panic.
    #[test]
    fn test_varint_empty_buffer() {
        let result = decode_varint(&[]);
        assert!(result.is_err(), "Empty buffer should return error");
    }

    /// Test: Front compression actually compresses (catches `common_prefix_len -> 0`).
    /// If common_prefix_len always returns 0, this test fails because output is too large.
    #[test]
    fn test_vocabulary_front_compression_effective() {
        // Vocabulary with significant shared prefixes
        let vocab = vec![
            "application".to_string(),
            "applications".to_string(),
            "apply".to_string(),
            "applied".to_string(),
            "applying".to_string(),
        ];

        let mut compressed = Vec::new();
        encode_vocabulary(&vocab, &mut compressed);

        // Calculate naive encoding size (no compression)
        let naive_size: usize = vocab.iter()
            .map(|s| s.len() + 2) // string bytes + 2 varint bytes (shared=0, len)
            .sum();

        // Compressed size should be significantly smaller
        // "application" = 11 bytes, but subsequent terms share "appl" prefix
        assert!(
            compressed.len() < naive_size,
            "Front compression not effective: compressed {} >= naive {}",
            compressed.len(), naive_size
        );

        // Verify roundtrip still works
        let decoded = decode_vocabulary(&compressed, vocab.len()).unwrap();
        assert_eq!(vocab, decoded);
    }

    /// Test: Suffix array roundtrip preserves exact values.
    /// Closes mutation gap for arithmetic operations in delta decoding.
    #[test]
    fn test_suffix_array_roundtrip_exact() {
        // Test with specific values that exercise delta encoding
        let entries = vec![
            (0u32, 0u32),     // doc 0, offset 0
            (0, 100),         // same doc, different offset
            (1, 0),           // new doc
            (1, 50),          // same doc
            (100, 0),         // large doc_id jump
            (100, 1000),      // large offset
        ];

        let mut buf = Vec::new();
        encode_suffix_array(&entries, &mut buf);
        let (decoded, consumed) = decode_suffix_array(&buf).unwrap();

        assert_eq!(entries.len(), decoded.len());
        assert_eq!(consumed, buf.len());

        for (i, ((orig_doc, orig_off), (dec_doc, dec_off))) in
            entries.iter().zip(decoded.iter()).enumerate()
        {
            assert_eq!(
                *orig_doc, *dec_doc,
                "doc_id mismatch at index {}: expected {}, got {}",
                i, orig_doc, dec_doc
            );
            assert_eq!(
                *orig_off, *dec_off,
                "offset mismatch at index {}: expected {}, got {}",
                i, orig_off, dec_off
            );
        }
    }
}
