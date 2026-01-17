// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Fuzz target for vocabulary (front-compressed) encoding/decoding.
//!
//! The vocabulary uses front compression: if consecutive terms share a prefix,
//! only store the difference. This fuzz target verifies roundtrip correctness
//! and that the decoder never panics on malformed prefix lengths.

#![no_main]

use libfuzzer_sys::fuzz_target;
use sorex::binary::{decode_vocabulary, encode_vocabulary};

/// Front compression saves space but creates attack surface.
///
/// A prefix length claiming to share 100 characters with a 3-character
/// predecessor shouldn't crash. It should return Err. The fuzzer will
/// find these malformed streams faster than you can construct them by hand.
fuzz_target!(|data: &[u8]| {
    // Property 1: decode_vocabulary should never panic
    // It should return Ok or Err, but never crash
    //
    // Note: decode_vocabulary requires term_count parameter, so we try
    // different reasonable counts based on data length
    let max_terms = data.len().min(1000);

    for term_count in [0, 1, 5, 10, max_terms] {
        if let Ok(vocab) = decode_vocabulary(data, term_count) {
            // Property 3: All strings must be valid UTF-8
            // (String type guarantees this, but verify anyway)
            for term in &vocab {
                assert!(term.is_ascii() || std::str::from_utf8(term.as_bytes()).is_ok());
            }

            // Property 2: Roundtrip check
            // If we successfully decoded, re-encoding should produce
            // equivalent output (when decoded again)
            if !vocab.is_empty() {
                let mut reencoded = Vec::new();
                encode_vocabulary(&vocab, &mut reencoded);

                let redecoded = decode_vocabulary(&reencoded, vocab.len())
                    .expect("Re-encoding of valid vocab should always decode");

                assert_eq!(
                    vocab, redecoded,
                    "Roundtrip failed: vocab lengths {} vs {}",
                    vocab.len(),
                    redecoded.len()
                );
            }
        }
    }
});
