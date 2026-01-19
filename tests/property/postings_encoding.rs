//! Property tests for postings encoding/decoding.
//!
//! Verifies:
//! 1. Roundtrip encoding/decoding is lossless
//! 2. Delta encoding handles edge cases (zero deltas, large gaps)
//! 3. Skip list construction and search correctness
//! 4. Robustness against malformed input

use proptest::prelude::*;
use sorex::binary::{decode_postings, encode_postings, PostingEntry, SkipList};
use sorex::binary::{BLOCK_SIZE, SKIP_INTERVAL, SKIP_LIST_THRESHOLD};

// ============================================================================
// STRATEGIES
// ============================================================================

/// Generate a single posting entry.
fn posting_entry_strategy() -> impl Strategy<Value = PostingEntry> {
    (0u32..10000, 0u32..100, 0u8..10, 1u32..10000).prop_map(
        |(doc_id, section_idx, heading_level, score)| PostingEntry {
            doc_id,
            section_idx,
            heading_level,
            score,
        },
    )
}

/// Generate a vector of posting entries (unsorted).
fn postings_strategy() -> impl Strategy<Value = Vec<PostingEntry>> {
    prop::collection::vec(posting_entry_strategy(), 1..100)
}

/// Generate a sorted vector of posting entries by score descending.
#[allow(dead_code)]
fn sorted_postings_strategy() -> impl Strategy<Value = Vec<PostingEntry>> {
    postings_strategy().prop_map(|mut postings: Vec<PostingEntry>| {
        postings.sort_by(|a, b| b.score.cmp(&a.score).then(a.doc_id.cmp(&b.doc_id)));
        postings
    })
}

/// Generate postings with specific doc_id patterns.
fn sequential_doc_ids_strategy(count: usize) -> impl Strategy<Value = Vec<PostingEntry>> {
    (0u32..1000, 100u32..10000).prop_map(move |(start, base_score)| {
        (0..count)
            .map(|i| PostingEntry {
                doc_id: start + i as u32,
                section_idx: i as u32 % 10,
                heading_level: (i % 5) as u8,
                score: base_score.saturating_sub(i as u32), // Descending scores
            })
            .collect()
    })
}

/// Generate postings with large gaps between doc_ids.
fn sparse_doc_ids_strategy() -> impl Strategy<Value = Vec<PostingEntry>> {
    prop::collection::vec((1000u32..100000, 0u32..50, 0u8..10, 1u32..10000), 1..20).prop_map(
        |tuples| {
            let mut postings: Vec<PostingEntry> = tuples
                .into_iter()
                .map(|(doc_id, section_idx, heading_level, score)| PostingEntry {
                    doc_id,
                    section_idx,
                    heading_level,
                    score,
                })
                .collect();
            postings.sort_by(|a, b| b.score.cmp(&a.score)); // Sort by score descending
                                                            // Remove duplicates by doc_id
            postings.dedup_by_key(|e| e.doc_id);
            postings
        },
    )
}

// ============================================================================
// ROUNDTRIP PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(100))]

    /// Property: Encode/decode roundtrip is lossless.
    ///
    /// Encoding and then decoding postings should produce identical entries.
    /// Postings are sorted by score descending internally.
    #[test]
    fn prop_roundtrip_lossless(postings in postings_strategy()) {
        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, bytes_read) = decode_postings(&buf).expect("Decode should succeed");

        // Bytes read should equal buffer size
        prop_assert_eq!(
            bytes_read, buf.len(),
            "Should read all {} encoded bytes, read {}",
            buf.len(), bytes_read
        );

        // Count should match
        prop_assert_eq!(
            decoded.len(), postings.len(),
            "Decoded {} entries but encoded {}",
            decoded.len(), postings.len()
        );

        // Sort both for comparison (encode_postings sorts by score desc, then doc_id asc)
        let mut sorted_original: Vec<_> = postings.iter().collect();
        sorted_original.sort_by(|a, b| b.score.cmp(&a.score).then(a.doc_id.cmp(&b.doc_id)));

        for (orig, dec) in sorted_original.iter().zip(decoded.iter()) {
            prop_assert_eq!(orig.doc_id, dec.doc_id, "doc_id mismatch");
            prop_assert_eq!(orig.section_idx, dec.section_idx, "section_idx mismatch");
            prop_assert_eq!(orig.heading_level, dec.heading_level, "heading_level mismatch");
            prop_assert_eq!(orig.score, dec.score, "score mismatch");
        }
    }

    /// Property: Sequential doc_ids roundtrip correctly.
    #[test]
    fn prop_sequential_roundtrip(postings in sequential_doc_ids_strategy(50)) {
        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

        prop_assert_eq!(decoded.len(), postings.len());
        for (orig, dec) in postings.iter().zip(decoded.iter()) {
            prop_assert_eq!(orig.doc_id, dec.doc_id);
            prop_assert_eq!(orig.section_idx, dec.section_idx);
            prop_assert_eq!(orig.heading_level, dec.heading_level);
        }
    }

    /// Property: Sparse doc_ids (large gaps) roundtrip correctly.
    #[test]
    fn prop_sparse_roundtrip(postings in sparse_doc_ids_strategy()) {
        if postings.is_empty() {
            return Ok(());
        }

        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

        // Sort original by score desc for comparison (encode_postings sorts internally)
        let mut sorted_original = postings.clone();
        sorted_original.sort_by(|a, b| b.score.cmp(&a.score).then(a.doc_id.cmp(&b.doc_id)));

        prop_assert_eq!(decoded.len(), sorted_original.len());
        for (orig, dec) in sorted_original.iter().zip(decoded.iter()) {
            prop_assert_eq!(orig.doc_id, dec.doc_id);
            prop_assert_eq!(orig.score, dec.score);
        }
    }
}

// ============================================================================
// DELTA ENCODING PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    /// Property: Decoded scores are sorted descending.
    ///
    /// After decode, scores should be in descending order (sorted by score).
    #[test]
    fn prop_decoded_scores_sorted_descending(postings in postings_strategy()) {
        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

        for i in 1..decoded.len() {
            prop_assert!(
                decoded[i].score <= decoded[i - 1].score,
                "Decoded scores should be descending: {} <= {}",
                decoded[i].score, decoded[i - 1].score
            );
        }
    }
}

/// Test: Zero-delta (same doc_id) handled correctly.
#[test]
fn test_zero_delta_handled() {
    // Create postings with same doc_id, different sections, descending scores
    let postings = vec![
        PostingEntry {
            doc_id: 100,
            section_idx: 0,
            heading_level: 0,
            score: 1000,
        },
        PostingEntry {
            doc_id: 100,
            section_idx: 1,
            heading_level: 1,
            score: 900,
        },
        PostingEntry {
            doc_id: 100,
            section_idx: 2,
            heading_level: 2,
            score: 800,
        },
    ];

    let mut buf = Vec::new();
    encode_postings(&postings, &mut buf);

    let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

    assert_eq!(decoded.len(), 3);
    for (i, entry) in decoded.iter().enumerate() {
        assert_eq!(entry.doc_id, 100, "All doc_ids should be 100");
        assert_eq!(entry.section_idx, i as u32, "section_idx should match");
    }
}

// ============================================================================
// EMPTY AND EDGE CASE PROPERTIES
// ============================================================================

/// Test: Empty postings list encodes/decodes correctly.
#[test]
fn test_empty_postings() {
    let postings: Vec<PostingEntry> = vec![];

    let mut buf = Vec::new();
    encode_postings(&postings, &mut buf);

    let (decoded, bytes_read) = decode_postings(&buf).expect("Decode should succeed");

    assert!(decoded.is_empty(), "Empty postings should decode to empty");
    assert!(bytes_read > 0, "Should read at least the count varint");
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Single posting entry roundtrips correctly.
    #[test]
    fn prop_single_posting(entry in posting_entry_strategy()) {
        let postings = vec![entry.clone()];

        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

        prop_assert_eq!(decoded.len(), 1);
        prop_assert_eq!(decoded[0].doc_id, entry.doc_id);
        prop_assert_eq!(decoded[0].section_idx, entry.section_idx);
        prop_assert_eq!(decoded[0].heading_level, entry.heading_level);
    }
}

// ============================================================================
// EXTREME VALUES
// ============================================================================

/// Generate posting entry with large doc_id (near u32::MAX).
fn large_doc_id_strategy() -> impl Strategy<Value = PostingEntry> {
    (u32::MAX - 1000..=u32::MAX, 0u32..100, 0u8..10, 1u32..10000).prop_map(
        |(doc_id, section_idx, heading_level, score)| PostingEntry {
            doc_id,
            section_idx,
            heading_level,
            score,
        },
    )
}

/// Generate posting entry with large section_idx (near u32::MAX).
fn large_section_idx_strategy() -> impl Strategy<Value = PostingEntry> {
    (0u32..1000, u32::MAX - 100..=u32::MAX, 0u8..10, 1u32..10000).prop_map(
        |(doc_id, section_idx, heading_level, score)| PostingEntry {
            doc_id,
            section_idx,
            heading_level,
            score,
        },
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Large doc_id values (near u32::MAX) handled correctly.
    #[test]
    fn prop_large_doc_ids(entry in large_doc_id_strategy()) {
        let postings = vec![entry.clone()];

        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

        prop_assert_eq!(decoded.len(), 1);
        prop_assert_eq!(decoded[0].doc_id, entry.doc_id, "Large doc_id should roundtrip");
    }

    /// Property: Large section_idx values handled correctly.
    #[test]
    fn prop_large_section_idx(entry in large_section_idx_strategy()) {
        let postings = vec![entry.clone()];

        let mut buf = Vec::new();
        encode_postings(&postings, &mut buf);

        let (decoded, _) = decode_postings(&buf).expect("Decode should succeed");

        prop_assert_eq!(decoded.len(), 1);
        prop_assert_eq!(decoded[0].section_idx, entry.section_idx, "Large section_idx should roundtrip");
    }
}

// ============================================================================
// SKIP LIST PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Skip list is only built for large posting lists.
    #[test]
    fn prop_skip_list_threshold(count in 1usize..SKIP_LIST_THRESHOLD + 100) {
        let doc_ids: Vec<u32> = (0..count as u32).collect();
        let skip_list = SkipList::build(&doc_ids);

        if count < SKIP_LIST_THRESHOLD {
            prop_assert!(
                skip_list.is_none(),
                "Skip list should not be built for {} < {} docs",
                count, SKIP_LIST_THRESHOLD
            );
        }
        // Note: Even above threshold, skip list may not be built if num_blocks < SKIP_INTERVAL
    }

    /// Property: Skip list levels are properly decimated.
    #[test]
    fn prop_skip_list_decimation(count in SKIP_LIST_THRESHOLD * 2..SKIP_LIST_THRESHOLD * 10) {
        let doc_ids: Vec<u32> = (0..count as u32).collect();

        if let Some(skip_list) = SkipList::build(&doc_ids) {
            // Each level should be approximately SKIP_INTERVAL smaller than previous
            for level_idx in 0..skip_list.levels.len() - 1 {
                let curr_len = skip_list.levels[level_idx].len();
                let next_len = skip_list.levels[level_idx + 1].len();

                // next level should be ~curr/SKIP_INTERVAL
                let expected_max = curr_len.div_ceil(SKIP_INTERVAL);
                prop_assert!(
                    next_len <= expected_max + 1,
                    "Level {} has {} entries, level {} has {} (expected <= {})",
                    level_idx, curr_len, level_idx + 1, next_len, expected_max
                );
            }
        }
    }

    /// Property: Skip list doc_ids are monotonically increasing at each level.
    #[test]
    fn prop_skip_list_monotonic(count in SKIP_LIST_THRESHOLD * 2..SKIP_LIST_THRESHOLD * 5) {
        let doc_ids: Vec<u32> = (0..count as u32).collect();

        if let Some(skip_list) = SkipList::build(&doc_ids) {
            for (level_idx, level) in skip_list.levels.iter().enumerate() {
                for i in 1..level.len() {
                    prop_assert!(
                        level[i].doc_id > level[i - 1].doc_id,
                        "Skip list level {} not monotonic: {} <= {}",
                        level_idx, level[i].doc_id, level[i - 1].doc_id
                    );
                }
            }
        }
    }
}

// ============================================================================
// SKIP LIST SEARCH PROPERTIES
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Skip list search returns valid block offset.
    #[test]
    fn prop_skip_list_search_valid(
        count in SKIP_LIST_THRESHOLD * 2..SKIP_LIST_THRESHOLD * 5,
        target in 0u32..SKIP_LIST_THRESHOLD as u32 * 5
    ) {
        let doc_ids: Vec<u32> = (0..count as u32).collect();

        if let Some(skip_list) = SkipList::build(&doc_ids) {
            if let Some(block_offset) = skip_list.skip_to(target) {
                // Block offset should be within valid range
                let num_blocks = count / BLOCK_SIZE;
                prop_assert!(
                    (block_offset as usize) < num_blocks || num_blocks == 0,
                    "Block offset {} should be < num_blocks {}",
                    block_offset, num_blocks
                );
            }
        }
    }

    /// Property: Skip list search is consistent.
    #[test]
    fn prop_skip_list_search_consistent(count in SKIP_LIST_THRESHOLD * 2..SKIP_LIST_THRESHOLD * 4) {
        let doc_ids: Vec<u32> = (0..count as u32).collect();

        if let Some(skip_list) = SkipList::build(&doc_ids) {
            // Search for various targets
            for target in [0, count as u32 / 4, count as u32 / 2, count as u32 - 1] {
                let result1 = skip_list.skip_to(target);
                let result2 = skip_list.skip_to(target);
                prop_assert_eq!(
                    result1, result2,
                    "Skip list search should be deterministic"
                );
            }
        }
    }
}

// ============================================================================
// SKIP LIST ENCODING/DECODING
// ============================================================================

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    /// Property: Skip list encode/decode roundtrip is lossless.
    #[test]
    fn prop_skip_list_roundtrip(count in SKIP_LIST_THRESHOLD * 2..SKIP_LIST_THRESHOLD * 5) {
        let doc_ids: Vec<u32> = (0..count as u32).collect();

        if let Some(skip_list) = SkipList::build(&doc_ids) {
            let mut buf = Vec::new();
            skip_list.encode(&mut buf);

            let (decoded, bytes_read) = SkipList::decode(&buf).expect("Decode should succeed");

            prop_assert_eq!(
                bytes_read, buf.len(),
                "Should read all {} bytes",
                buf.len()
            );

            prop_assert_eq!(
                decoded.levels.len(), skip_list.levels.len(),
                "Level count mismatch"
            );

            for (level_idx, (orig_level, dec_level)) in
                skip_list.levels.iter().zip(decoded.levels.iter()).enumerate()
            {
                prop_assert_eq!(
                    orig_level.len(), dec_level.len(),
                    "Level {} entry count mismatch",
                    level_idx
                );

                for (orig, dec) in orig_level.iter().zip(dec_level.iter()) {
                    prop_assert_eq!(
                        orig.doc_id, dec.doc_id,
                        "Level {} doc_id mismatch",
                        level_idx
                    );
                    prop_assert_eq!(
                        orig.block_offset, dec.block_offset,
                        "Level {} block_offset mismatch",
                        level_idx
                    );
                }
            }
        }
    }
}

// ============================================================================
// UNIT TESTS FOR ERROR HANDLING
// ============================================================================

#[test]
fn test_decode_truncated_postings() {
    // Create valid postings
    let postings = vec![
        PostingEntry {
            doc_id: 1,
            section_idx: 0,
            heading_level: 0,
            score: 1000,
        },
        PostingEntry {
            doc_id: 2,
            section_idx: 1,
            heading_level: 1,
            score: 900,
        },
    ];

    let mut buf = Vec::new();
    encode_postings(&postings, &mut buf);

    // Truncate buffer at various points
    for truncate_at in 1..buf.len() {
        let truncated = &buf[..truncate_at];
        let result = decode_postings(truncated);
        // Should either succeed (if we got lucky) or return an error
        // but should NEVER panic
        if let Ok((decoded, _)) = result {
            // If decode succeeded, it should have fewer entries
            assert!(decoded.len() <= postings.len());
        }
    }
}

#[test]
fn test_decode_empty_buffer() {
    let result = decode_postings(&[]);
    assert!(result.is_err(), "Empty buffer should error");
}

#[test]
fn test_skip_list_decode_empty() {
    let result = SkipList::decode(&[]);
    assert!(result.is_err(), "Empty buffer should error");
}

#[test]
fn test_skip_list_decode_truncated() {
    let doc_ids: Vec<u32> = (0..SKIP_LIST_THRESHOLD as u32 * 3).collect();

    if let Some(skip_list) = SkipList::build(&doc_ids) {
        let mut buf = Vec::new();
        skip_list.encode(&mut buf);

        // Truncate at various points
        for truncate_at in 1..buf.len().min(20) {
            let truncated = &buf[..truncate_at];
            let result = SkipList::decode(truncated);
            // Should either succeed partially or error, but never panic
            if let Err(e) = result {
                // Expected - truncated data should produce an error
                // Error messages vary, just check we got an error
                let _msg = e.to_string();
            }
        }
    }
}

#[test]
fn test_postings_with_all_same_doc_id() {
    // Regression test: all same doc_id should work (zero deltas for scores)
    let postings: Vec<PostingEntry> = (0..10)
        .map(|i| PostingEntry {
            doc_id: 42,
            section_idx: i,
            heading_level: (i % 5) as u8,
            score: 1000 - i, // Descending scores
        })
        .collect();

    let mut buf = Vec::new();
    encode_postings(&postings, &mut buf);

    let (decoded, _) = decode_postings(&buf).expect("Should decode");

    assert_eq!(decoded.len(), 10);
    for entry in &decoded {
        assert_eq!(entry.doc_id, 42);
    }
}
