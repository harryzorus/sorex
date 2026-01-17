// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Binary format parsing under adversarial input.
//!
//! If someone uploads a crafted `.sorex` file, the worst case should be an
//! error message, not a DoS. This fuzz target hammers the parser with garbage
//! bytes, including varints that decode to usize::MAX, truncated headers, and
//! CRCs that lie about the data they protect. None of it should crash.

#![no_main]

use libfuzzer_sys::fuzz_target;
use sorex::binary::LoadedLayer;

/// Every path through `from_bytes` must terminate safely.
///
/// The fuzzer will find the edge cases you didn't think about:
/// truncated varints, self-referential offsets, headers claiming more
/// data than exists. If any of these panic instead of returning Err,
/// that's a bug worth fixing before production.
fuzz_target!(|data: &[u8]| {
    // The from_bytes function should never panic on any input.
    // It should return Err for malformed input.
    let result = LoadedLayer::from_bytes(data);

    // If parsing succeeded, verify invariants hold
    if let Ok(layer) = result {
        // INVARIANT 1: All vocabulary entries should be valid UTF-8
        // (This is guaranteed by the parser, verify it holds)
        for term in &layer.vocabulary {
            assert!(
                term.is_ascii() || term.chars().all(|c| !c.is_control()),
                "Vocabulary term contains control characters"
            );
        }

        // INVARIANT 2: All doc IDs in postings should be in bounds
        for postings in &layer.postings {
            for posting in postings {
                assert!(
                    (posting.doc_id as usize) < layer.docs.len(),
                    "Posting doc_id {} out of bounds (docs.len = {})",
                    posting.doc_id,
                    layer.docs.len()
                );
            }
        }

        // INVARIANT 3: Suffix array entries should be valid
        // suffix_array is Vec<(u32, u32)> = (term_idx, offset)
        for &(term_idx, offset) in &layer.suffix_array {
            assert!(
                (term_idx as usize) < layer.vocabulary.len(),
                "Suffix array term_idx {} out of bounds (vocab.len = {})",
                term_idx,
                layer.vocabulary.len()
            );
            let term = &layer.vocabulary[term_idx as usize];
            assert!(
                (offset as usize) <= term.len(),
                "Suffix array offset {} out of bounds for term '{}' (len = {})",
                offset,
                term,
                term.len()
            );
        }

        // INVARIANT 4: Docs should have valid hrefs (non-empty)
        for (i, doc) in layer.docs.iter().enumerate() {
            assert!(
                !doc.href.is_empty(),
                "Doc {} has empty href",
                i
            );
        }

        // INVARIANT 5: doc_count should match docs.len()
        assert_eq!(
            layer.doc_count,
            layer.docs.len(),
            "doc_count mismatch: {} vs {}",
            layer.doc_count,
            layer.docs.len()
        );

        // INVARIANT 6: vocabulary.len() should match postings.len()
        assert_eq!(
            layer.vocabulary.len(),
            layer.postings.len(),
            "vocabulary/postings count mismatch: {} vs {}",
            layer.vocabulary.len(),
            layer.postings.len()
        );
    }
});
