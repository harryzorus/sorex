// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Kani model checking proofs for sorex encoding primitives.
//!
//! This standalone crate extracts the critical varint encoding functions
//! and provides mathematical proofs of their correctness using Kani.
//!
//! Run with: `cargo kani`
//!
//! ## Verified Properties
//!
//! 1. **No panics**: encode_varint and decode_varint never panic
//! 2. **Roundtrip**: decode(encode(x)) == x for all x
//! 3. **Bounds**: Output never exceeds MAX_VARINT_BYTES

/// Maximum varint bytes (u64 needs at most 10 bytes in LEB128)
pub const MAX_VARINT_BYTES: usize = 10;

// ============================================================================
// VARINT ENCODING (copied from src/binary/encoding.rs)
// ============================================================================

/// Encode a varint to bytes (LEB128 format)
pub fn encode_varint(mut value: u64, buf: &mut Vec<u8>) {
    loop {
        let byte = (value & 0x7F) as u8;
        value >>= 7;
        if value == 0 {
            buf.push(byte);
            break;
        } else {
            buf.push(byte | 0x80);
        }
    }
}

/// Result type for decode_varint
#[derive(Debug, Clone, PartialEq)]
pub enum DecodeError {
    EmptyBuffer,
    Incomplete,
    TooLong,
}

/// Decode a varint from bytes, returning (value, bytes_consumed)
pub fn decode_varint(bytes: &[u8]) -> Result<(u64, usize), DecodeError> {
    if bytes.is_empty() {
        return Err(DecodeError::EmptyBuffer);
    }

    let mut result: u64 = 0;
    let mut shift = 0;
    let mut i = 0;

    while i < bytes.len() && i < MAX_VARINT_BYTES {
        let byte = bytes[i];
        result |= ((byte & 0x7F) as u64) << shift;
        i += 1;
        if byte & 0x80 == 0 {
            return Ok((result, i));
        }
        shift += 7;
    }

    // If we get here, either buffer ended mid-varint or varint is too long
    if i >= MAX_VARINT_BYTES {
        Err(DecodeError::TooLong)
    } else {
        Err(DecodeError::Incomplete)
    }
}

// ============================================================================
// KANI MODEL CHECKING PROOFS
// ============================================================================

#[cfg(kani)]
mod kani_proofs {
    use super::*;

    /// Verify encode_varint never panics and produces valid output.
    #[kani::proof]
    #[kani::unwind(11)] // Max 10 bytes for u64 + 1
    fn verify_encode_varint_no_panic() {
        let value: u64 = kani::any();
        let mut buf = Vec::new();

        // This must not panic
        encode_varint(value, &mut buf);

        // Output must be non-empty and bounded
        kani::assert(!buf.is_empty(), "encode_varint must produce at least 1 byte");
        kani::assert(
            buf.len() <= MAX_VARINT_BYTES,
            "encode_varint must produce at most MAX_VARINT_BYTES bytes",
        );

        // The last byte must NOT have the continuation bit set
        kani::assert(
            buf.last().map_or(false, |&b| b & 0x80 == 0),
            "Last byte must not have continuation bit",
        );
    }

    /// Verify decode_varint never panics for any byte sequence.
    #[kani::proof]
    #[kani::unwind(12)] // MAX_VARINT_BYTES + 2 for safety
    fn verify_decode_varint_no_panic() {
        // Create symbolic byte array up to max varint size + 1
        let len: usize = kani::any_where(|&n| n <= MAX_VARINT_BYTES + 1);
        let mut bytes = [0u8; 11]; // MAX_VARINT_BYTES + 1

        // Fill with symbolic values
        for i in 0..len {
            bytes[i] = kani::any();
        }

        let slice = &bytes[..len];

        // This must not panic (may return Err, that's fine)
        match decode_varint(slice) {
            Ok((_, consumed)) => {
                // If successful, verify properties
                kani::assert(consumed > 0, "Must consume at least 1 byte on success");
                kani::assert(
                    consumed <= slice.len(),
                    "Cannot consume more bytes than available",
                );
                kani::assert(
                    consumed <= MAX_VARINT_BYTES,
                    "Cannot consume more than MAX_VARINT_BYTES",
                );
            }
            Err(_) => {
                // Errors are acceptable - we're proving no panics
            }
        }
    }

    /// Verify roundtrip: decode(encode(x)) == x for all x.
    #[kani::proof]
    fn verify_varint_roundtrip() {
        let original: u64 = kani::any();
        let mut buf = Vec::new();

        // Encode
        encode_varint(original, &mut buf);

        // Decode
        let result = decode_varint(&buf);
        kani::assert(result.is_ok(), "Decoding encoded value must succeed");

        let (decoded, consumed) = result.unwrap();
        kani::assert(decoded == original, "Roundtrip must preserve value");
        kani::assert(
            consumed == buf.len(),
            "Must consume exactly the encoded bytes",
        );
    }

    /// Verify decode handles empty input gracefully.
    #[kani::proof]
    fn verify_decode_empty_input() {
        let empty: &[u8] = &[];
        let result = decode_varint(empty);
        kani::assert(
            matches!(result, Err(DecodeError::EmptyBuffer)),
            "Empty input must return EmptyBuffer error",
        );
    }

    /// Verify decode rejects overlong varints.
    #[kani::proof]
    fn verify_decode_rejects_overlong() {
        // Create a varint that's too long (all continuation bits set)
        let bytes = [0x80u8; 11]; // All continuation bits set for 11 bytes

        // This should fail because the varint is too long
        let result = decode_varint(&bytes);
        kani::assert(
            matches!(result, Err(DecodeError::TooLong)),
            "Overlong varint (11 bytes) must be rejected",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip() {
        for value in [0u64, 1, 127, 128, 255, 256, 16383, 16384, u64::MAX] {
            let mut buf = Vec::new();
            encode_varint(value, &mut buf);
            let (decoded, consumed) = decode_varint(&buf).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(consumed, buf.len());
        }
    }

    #[test]
    fn test_max_varint_size() {
        let mut buf = Vec::new();
        encode_varint(u64::MAX, &mut buf);
        assert_eq!(buf.len(), 10); // u64::MAX needs exactly 10 bytes
    }
}
