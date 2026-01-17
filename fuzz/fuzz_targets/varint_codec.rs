// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for varint (LEB128) encoding/decoding.
//!
//! Varints are the foundation of the binary format. If roundtrip fails or
//! decode panics on malformed input, everything built on top breaks.

#![no_main]

use libfuzzer_sys::fuzz_target;
use sorex::binary::{decode_varint, encode_varint};

/// Varints are the foundation. If they break, everything breaks.
///
/// The fuzzer throws garbage bytes at the decoder. It should return Err,
/// not panic. For valid decodes, roundtrip must preserve the value exactly.
fuzz_target!(|data: &[u8]| {
    // Property 1: decode_varint should never panic
    // It should return Ok or Err, but never crash
    if let Ok((value, consumed)) = decode_varint(data) {
        // Property 2: Roundtrip check
        // If we successfully decoded, re-encoding should produce
        // equivalent bytes (when decoded again)
        let mut reencoded = Vec::new();
        encode_varint(value, &mut reencoded);

        let (redecoded, reconsumed) = decode_varint(&reencoded)
            .expect("Re-encoding of valid value should always decode");

        assert_eq!(
            value, redecoded,
            "Roundtrip failed: {} != {}",
            value, redecoded
        );

        // The re-encoded form should be canonical (minimal)
        // and reconsumed should equal the buffer length
        assert_eq!(
            reconsumed,
            reencoded.len(),
            "Re-encoded varint should be fully consumed"
        );

        // Property 3: Check that consumed is reasonable
        // Varint for u64 should be at most 10 bytes
        assert!(
            consumed <= 10,
            "Varint consumed {} bytes, max should be 10",
            consumed
        );

        // Verify consumed doesn't exceed input length
        assert!(
            consumed <= data.len(),
            "Consumed {} bytes but input only had {}",
            consumed,
            data.len()
        );
    }
});
