// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for section table encoding/decoding.
//!
//! The section table maps section indices to their string IDs for deep linking.
//! Roundtrip must preserve exact IDs, and the decoder must handle truncated or
//! corrupted input gracefully.

#![no_main]

use libfuzzer_sys::fuzz_target;
use sorex::binary::{decode_section_table, encode_section_table};

/// Section IDs become URL anchors. Corrupt them and deep links break.
///
/// The decoder must handle truncated strings, invalid UTF-8, and counts
/// that lie about how many entries follow. None of it should panic.
fuzz_target!(|data: &[u8]| {
    // Property 1: decode_section_table should never panic
    // It should return Ok or Err, but never crash
    if let Ok((sections, consumed)) = decode_section_table(data) {
        // Property 4: Verify consumed is reasonable
        assert!(
            consumed <= data.len(),
            "Consumed {} bytes but input only had {}",
            consumed,
            data.len()
        );

        // Property 3: All section IDs must be valid UTF-8
        // (String type guarantees this, but verify anyway)
        for section_id in &sections {
            assert!(
                section_id.is_ascii() || std::str::from_utf8(section_id.as_bytes()).is_ok(),
                "Section ID is not valid UTF-8: {:?}",
                section_id.as_bytes()
            );
        }

        // Property 2: Roundtrip check
        // If we successfully decoded, re-encoding should produce
        // equivalent output (when decoded again)
        let mut reencoded = Vec::new();
        encode_section_table(&sections, &mut reencoded);

        let (redecoded, _) = decode_section_table(&reencoded)
            .expect("Re-encoding of valid sections should always decode");

        assert_eq!(
            sections, redecoded,
            "Roundtrip failed: {} sections vs {} sections",
            sections.len(),
            redecoded.len()
        );

        // Verify each section matches
        for (i, (orig, decoded)) in sections.iter().zip(redecoded.iter()).enumerate() {
            assert_eq!(
                orig, decoded,
                "Section {} mismatch: '{}' vs '{}'",
                i, orig, decoded
            );
        }
    }
});
