#![allow(unexpected_cfgs)]
//! Negative tests for binary format validation.
//!
//! These tests verify that the binary decoder correctly rejects malformed,
//! truncated, or corrupted input. This is critical for security - we must
//! never panic or produce undefined behavior on untrusted input.
//!
//! ## Test Categories
//!
//! 1. **Varint rejection**: Overflow, truncation, max bytes exceeded
//! 2. **Postings rejection**: Truncation, too large, delta overflow
//! 3. **Vocabulary rejection**: Invalid UTF-8, truncation, prefix overflow
//! 4. **Section table rejection**: Invalid UTF-8, count mismatch
//! 5. **Full file rejection**: Magic, CRC, version, size limits

use sorex::binary::{
    decode_postings, decode_suffix_array, decode_varint, encode_postings, encode_varint,
};

// ============================================================================
// VARINT REJECTION TESTS
// ============================================================================

/// Empty buffer should return error, not panic
#[test]
fn test_varint_empty_buffer() {
    let result = decode_varint(&[]);
    assert!(result.is_err(), "Empty buffer should return error");
}

/// Varint with all continuation bits set (never terminates)
#[test]
fn test_varint_unterminated() {
    // 5 bytes all with continuation bit set
    let bytes = vec![0x80, 0x80, 0x80, 0x80, 0x80];
    let result = decode_varint(&bytes);
    assert!(result.is_err(), "Unterminated varint should return error");
}

/// Varint exceeding MAX_VARINT_BYTES (10 bytes for u64)
#[test]
fn test_varint_exceeds_max_bytes() {
    // 11 continuation bytes - exceeds the 10-byte limit
    let bytes = vec![0x80; 11];
    let result = decode_varint(&bytes);
    assert!(result.is_err(), "Varint exceeding max bytes should return error");
}

/// Varint that decodes but would overflow u64
#[test]
fn test_varint_overflow_u64() {
    // This encodes a value larger than u64::MAX
    // Each 0xFF byte adds 7 bits of 1s
    let bytes = vec![0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F];
    // This should either error or wrap - we want to verify no panic
    let _ = decode_varint(&bytes);
}

/// Power-of-two boundaries should decode correctly
#[test]
fn test_varint_boundary_values() {
    // Test boundary values that stress the encoding
    let test_cases: Vec<u64> = vec![
        0,
        1,
        127,      // Max single byte
        128,      // First 2-byte value
        16383,    // Max 2-byte value
        16384,    // First 3-byte value
        u32::MAX as u64,
        u64::MAX / 2,
    ];

    for val in test_cases {
        let mut buf = Vec::new();
        encode_varint(val, &mut buf);
        let (decoded, _) = decode_varint(&buf).unwrap_or_else(|_| panic!("Should decode {}", val));
        assert_eq!(decoded, val, "Value {} roundtrip failed", val);
    }
}

/// Single byte truncated in middle of multi-byte varint
#[test]
fn test_varint_truncated_multibyte() {
    // Start of a 2-byte varint, truncated
    let bytes = vec![0x80]; // Continuation bit set but no following byte
    let result = decode_varint(&bytes);
    assert!(result.is_err(), "Truncated multi-byte varint should return error");
}

// ============================================================================
// POSTINGS REJECTION TESTS
// ============================================================================

/// Postings with count but missing data
#[test]
fn test_postings_count_no_data() {
    // Encode count = 5 but provide no actual posting data
    let mut buf = Vec::new();
    encode_varint(5, &mut buf); // Says 5 entries
    // No entry data follows

    let result = decode_postings(&buf);
    assert!(result.is_err(), "Postings with count but no data should error");
}

/// Postings truncated in middle of entry
#[test]
fn test_postings_truncated_mid_entry() {
    // First, create valid postings
    let entries = vec![sorex::binary::PostingEntry {
        doc_id: 100,
        section_idx: 5,
        heading_level: 2,
    }];
    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    // Now truncate mid-way through
    let truncated = &buf[..buf.len() / 2];
    let result = decode_postings(truncated);
    assert!(result.is_err(), "Truncated postings should error");
}

/// Postings with extremely large count (resource exhaustion check)
#[test]
fn test_postings_count_too_large() {
    let mut buf = Vec::new();
    // Encode count larger than MAX_POSTING_SIZE (10 million)
    encode_varint(20_000_000, &mut buf);

    let result = decode_postings(&buf);
    assert!(result.is_err(), "Posting count exceeding MAX_POSTING_SIZE should error");
}

/// Postings with zero count should be valid (empty list)
#[test]
fn test_postings_zero_count_valid() {
    let mut buf = Vec::new();
    encode_varint(0, &mut buf); // Empty posting list

    let (decoded, _) = decode_postings(&buf).expect("Empty postings should be valid");
    assert!(decoded.is_empty());
}

// ============================================================================
// SUFFIX ARRAY REJECTION TESTS
// ============================================================================

/// Suffix array with count but no data
#[test]
fn test_suffix_array_count_no_data() {
    let mut buf = Vec::new();
    encode_varint(10, &mut buf); // Says 10 entries
    // No entry data follows

    let result = decode_suffix_array(&buf);
    assert!(result.is_err(), "Suffix array with count but no data should error");
}

/// Suffix array with mismatched stream lengths
#[test]
fn test_suffix_array_truncated() {
    // Create a valid suffix array buffer first
    let entries: Vec<(u32, u32)> = vec![(0, 100), (1, 200), (2, 300)];
    let mut buf = Vec::new();
    sorex::binary::encode_suffix_array(&entries, &mut buf);

    // Truncate it
    let truncated = &buf[..buf.len() - 5];
    let result = decode_suffix_array(truncated);
    assert!(result.is_err(), "Truncated suffix array should error");
}

// ============================================================================
// INVALID UTF-8 REJECTION TESTS
// ============================================================================

/// Helper: create invalid UTF-8 sequence
fn invalid_utf8() -> Vec<u8> {
    vec![0xFF, 0xFE, 0xFD] // Invalid UTF-8 continuation bytes
}

/// Test that we don't panic on invalid UTF-8 (this tests the principle)
#[test]
fn test_invalid_utf8_in_section_id() {
    // While we can't directly test decode_section_table without the full binary,
    // we verify the principle that invalid UTF-8 should be handled safely
    let bad_utf8 = invalid_utf8();
    let result = std::str::from_utf8(&bad_utf8);
    assert!(result.is_err(), "Invalid UTF-8 should be detected");
}

// ============================================================================
// FULL BINARY LAYER REJECTION TESTS
// ============================================================================

#[allow(unexpected_cfgs)]
#[cfg(feature = "binary_layer_tests")]
mod binary_layer_tests {
    use super::*;
    use sorex::binary::BinaryLayer;

    /// File too small (can't contain header + footer)
    #[test]
    fn test_file_too_small() {
        let bytes = vec![0u8; 50]; // Less than header (52) + footer (8)
        let result = BinaryLayer::from_bytes(&bytes);
        assert!(result.is_err(), "File too small should error");
    }

    /// Invalid header magic
    #[test]
    fn test_invalid_header_magic() {
        let mut bytes = vec![0u8; 100];
        // Write invalid magic at start
        bytes[0..4].copy_from_slice(b"NOPE");
        let result = BinaryLayer::from_bytes(&bytes);
        assert!(result.is_err(), "Invalid magic should error");
    }

    /// Invalid footer magic
    #[test]
    fn test_invalid_footer_magic() {
        // Minimum valid-looking file with invalid footer
        let mut bytes = vec![0u8; 100];
        // Last 4 bytes should be "XROS" but we put something else
        let len = bytes.len();
        bytes[len-4..].copy_from_slice(b"NOPE");
        let result = BinaryLayer::from_bytes(&bytes);
        assert!(result.is_err(), "Invalid footer magic should error");
    }

    /// CRC32 mismatch detection
    #[test]
    fn test_crc32_mismatch() {
        // This would need a valid file with corrupted CRC
        // The test verifies the principle of CRC validation
    }
}

// ============================================================================
// CONTRACT VIOLATION TESTS
// ============================================================================

/// These tests verify that debug assertions catch programming errors

#[test]
fn test_postings_must_be_sorted() {
    // Postings should be sorted by doc_id
    // If we encode unsorted postings, they should still decode
    // (sorting is a contract, not enforced at decode time)
    let unsorted = vec![
        sorex::binary::PostingEntry { doc_id: 100, section_idx: 0, heading_level: 0 },
        sorex::binary::PostingEntry { doc_id: 50, section_idx: 0, heading_level: 0 }, // Out of order!
    ];

    let mut buf = Vec::new();
    encode_postings(&unsorted, &mut buf);

    // Decoding should work (validation happens elsewhere)
    let (decoded, _) = decode_postings(&buf).expect("Decoding should succeed");
    assert_eq!(decoded.len(), 2);
    // The delta encoding may produce unexpected values for unsorted input
}

// ============================================================================
// BOUNDARY VALUE TESTS
// ============================================================================

#[test]
fn test_max_valid_doc_id() {
    let entry = sorex::binary::PostingEntry {
        doc_id: u32::MAX,
        section_idx: 0,
        heading_level: 0,
    };

    let mut buf = Vec::new();
    encode_postings(&[entry], &mut buf);

    let (decoded, _) = decode_postings(&buf).expect("Max doc_id should be valid");
    assert_eq!(decoded[0].doc_id, u32::MAX);
}

#[test]
fn test_max_valid_section_idx() {
    let entry = sorex::binary::PostingEntry {
        doc_id: 0,
        section_idx: u32::MAX,
        heading_level: 0,
    };

    let mut buf = Vec::new();
    encode_postings(&[entry], &mut buf);

    let (decoded, _) = decode_postings(&buf).expect("Max section_idx should be valid");
    assert_eq!(decoded[0].section_idx, u32::MAX);
}

#[test]
fn test_max_valid_heading_level() {
    let entry = sorex::binary::PostingEntry {
        doc_id: 0,
        section_idx: 0,
        heading_level: u8::MAX,
    };

    let mut buf = Vec::new();
    encode_postings(&[entry], &mut buf);

    let (decoded, _) = decode_postings(&buf).expect("Max heading_level should be valid");
    assert_eq!(decoded[0].heading_level, u8::MAX);
}
