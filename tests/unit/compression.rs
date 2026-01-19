//! Tests for v12 compression (delta+varint postings, separated suffix array streams)

use sorex::binary::{
    decode_postings, decode_suffix_array, encode_postings, encode_suffix_array, PostingEntry,
};

// ============================================================================
// POSTINGS TESTS
// ============================================================================

#[test]
fn test_postings_empty() {
    let entries: Vec<PostingEntry> = vec![];

    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    let (decoded, _) = decode_postings(&buf).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn test_postings_single() {
    let entries = vec![PostingEntry {
        doc_id: 42,
        section_idx: 1,
        heading_level: 2,
        score: 500,
    }];

    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    let (decoded, _) = decode_postings(&buf).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].doc_id, 42);
    assert_eq!(decoded[0].section_idx, 1);
    assert_eq!(decoded[0].heading_level, 2);
    assert_eq!(decoded[0].score, 500);
}

#[test]
fn test_postings_roundtrip() {
    let entries = vec![
        PostingEntry {
            doc_id: 0,
            section_idx: 0,
            heading_level: 0,
            score: 1000,
        },
        PostingEntry {
            doc_id: 5,
            section_idx: 1,
            heading_level: 2,
            score: 800,
        },
        PostingEntry {
            doc_id: 100,
            section_idx: 0,
            heading_level: 3,
            score: 600,
        },
        PostingEntry {
            doc_id: 200,
            section_idx: 2,
            heading_level: 4,
            score: 400,
        },
    ];

    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    let (decoded, _) = decode_postings(&buf).unwrap();
    assert_eq!(decoded.len(), entries.len());

    for (orig, dec) in entries.iter().zip(decoded.iter()) {
        assert_eq!(orig.doc_id, dec.doc_id);
        assert_eq!(orig.section_idx, dec.section_idx);
        assert_eq!(orig.heading_level, dec.heading_level);
    }
}

#[test]
fn test_postings_large_clustered() {
    // Generate 1000 clustered postings (10 clusters of 100)
    let entries: Vec<PostingEntry> = (0..1000)
        .map(|i| PostingEntry {
            doc_id: (i / 100) * 1000 + (i % 100), // clustered: 0-99, 1000-1099, etc.
            section_idx: if i % 10 == 0 { i / 10 } else { 0 },
            heading_level: (i % 6) as u8,
            score: 10000u32.saturating_sub(i), // Descending scores
        })
        .collect();

    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    let (decoded, _) = decode_postings(&buf).unwrap();
    assert_eq!(decoded.len(), entries.len());

    for (orig, dec) in entries.iter().zip(decoded.iter()) {
        assert_eq!(orig.doc_id, dec.doc_id);
        assert_eq!(orig.section_idx, dec.section_idx);
        assert_eq!(orig.heading_level, dec.heading_level);
    }
}

// ============================================================================
// SUFFIX ARRAY TESTS
// ============================================================================

#[test]
fn test_suffix_array_empty() {
    let entries: Vec<(u32, u32)> = vec![];

    let mut buf = Vec::new();
    encode_suffix_array(&entries, &mut buf);

    let (decoded, _) = decode_suffix_array(&buf).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn test_suffix_array_roundtrip() {
    let entries = vec![(0, 5), (100, 20), (50, 100), (0, 0)];

    let mut buf = Vec::new();
    encode_suffix_array(&entries, &mut buf);

    let (decoded, _) = decode_suffix_array(&buf).unwrap();
    assert_eq!(decoded, entries);
}

#[test]
fn test_suffix_array_large() {
    // 10K entries
    let entries: Vec<(u32, u32)> = (0..10_000).map(|i| ((i % 1000), i * 5)).collect();

    let mut buf = Vec::new();
    encode_suffix_array(&entries, &mut buf);

    let (decoded, _) = decode_suffix_array(&buf).unwrap();
    assert_eq!(decoded, entries);
}

#[test]
fn test_suffix_array_max_term_ord() {
    // Test near 16-bit boundary
    let entries = vec![(0, 0), (65535, 100), (32768, 50)];

    let mut buf = Vec::new();
    encode_suffix_array(&entries, &mut buf);

    let (decoded, _) = decode_suffix_array(&buf).unwrap();
    assert_eq!(decoded, entries);
}

// ============================================================================
// SIZE TESTS
// ============================================================================

#[test]
fn test_postings_sparse() {
    // Generate sparse postings (large gaps)
    let entries: Vec<PostingEntry> = (0..500)
        .map(|i| PostingEntry {
            doc_id: i * 1000, // large gaps
            section_idx: 0,
            heading_level: 0,
            score: 5000u32.saturating_sub(i), // Descending scores
        })
        .collect();

    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    let (decoded, _) = decode_postings(&buf).unwrap();
    assert_eq!(decoded.len(), entries.len());

    println!(
        "Sparse postings (500 entries, gap=1000): {} bytes",
        buf.len()
    );
}

#[test]
fn test_postings_dense() {
    // Generate dense postings (consecutive doc_ids)
    let entries: Vec<PostingEntry> = (0..1000)
        .map(|i| PostingEntry {
            doc_id: i as u32, // consecutive
            section_idx: if i % 50 == 0 { (i / 50) as u32 } else { 0 },
            heading_level: (i % 4) as u8,
            score: 10000u32.saturating_sub(i as u32), // Descending scores
        })
        .collect();

    let mut buf = Vec::new();
    encode_postings(&entries, &mut buf);

    let (decoded, _) = decode_postings(&buf).unwrap();
    assert_eq!(decoded.len(), entries.len());

    println!("Dense postings (1000 consecutive): {} bytes", buf.len());
}
