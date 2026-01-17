// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Postings list encoding/decoding and skip lists.
//!
//! Postings are the heart of an inverted index: for each term, which documents
//! contain it? Delta encoding is the obvious optimization since doc_ids tend to
//! cluster. If documents 100, 102, 105, 110 all contain "rust", we store
//! [100, 2, 3, 5] instead of [100, 102, 105, 110]. Small deltas compress well.
//!
//! Skip lists are for when you have a huge posting list and need to jump to
//! doc_id 50000 without scanning 49999 entries. We build them automatically
//! for posting lists over 1024 entries.
//!
//! # References
//!
//! - **Delta Encoding for Postings**: Classic Information Retrieval technique.
//!   See Croft, Metzler, Strohman (2009): "Search Engines: Information Retrieval
//!   in Practice", Chapter 5 "Ranking with Indexes". Also Zobel & Moffat (2006):
//!   "Inverted Files for Text Search Engines", ACM Computing Surveys.
//!
//! - **Skip Lists**: Pugh (1990): "Skip Lists: A Probabilistic Alternative to
//!   Balanced Trees", Communications of the ACM 33(6).

use std::io;

use super::encoding::{decode_varint, encode_varint};
use super::header::{BLOCK_SIZE, MAX_POSTING_SIZE, MAX_SKIP_LEVELS, SKIP_INTERVAL, SKIP_LIST_THRESHOLD};

// ============================================================================
// POSTING ENTRY
// ============================================================================

/// Posting entry with doc_id, section_id index, and heading level
#[derive(Debug, Clone)]
pub struct PostingEntry {
    pub doc_id: u32,
    /// Index into section table (0 = no section_id, 1+ = table index + 1)
    pub section_idx: u32,
    /// Heading level (0=title, 2=h2, 3=h3, 4=h4, etc.) - used for bucketed ranking
    pub heading_level: u8,
}

// ============================================================================
// POSTINGS ENCODING (Delta+Varint - optimized for brotli compression)
// ============================================================================

/// Encode posting list with delta+varint compression
///
/// This format is optimized for compact file size. Files are served with
/// external brotli compression. Decoding fully materializes PostingEntry
/// vectors for fast in-memory search.
///
/// Format:
/// - doc_freq: varint
/// - For each entry (sorted by doc_id):
///   - doc_id_delta: varint (delta from previous doc_id)
///   - section_idx: varint
///   - heading_level: u8
///
/// This simple format compresses ~45% better with brotli than complex schemes.
pub fn encode_postings(entries: &[PostingEntry], buf: &mut Vec<u8>) {
    let doc_freq = entries.len();
    encode_varint(doc_freq as u64, buf);

    if doc_freq == 0 {
        return;
    }

    // Sort by doc_id for optimal delta encoding (small deltas compress well)
    let mut sorted: Vec<&PostingEntry> = entries.iter().collect();
    sorted.sort_by_key(|e| e.doc_id);

    // Delta-encode doc_ids, varint for section_idx, raw byte for heading_level
    let mut prev_doc_id = 0u32;
    for entry in sorted {
        let delta = entry.doc_id - prev_doc_id;
        prev_doc_id = entry.doc_id;
        encode_varint(delta as u64, buf);
        encode_varint(entry.section_idx as u64, buf);
        buf.push(entry.heading_level);
    }
}

/// Decode posting list with delta+varint compression
///
/// Fully materializes PostingEntry vectors for fast in-memory search.
pub fn decode_postings(bytes: &[u8]) -> io::Result<(Vec<PostingEntry>, usize)> {
    let (doc_freq, mut pos) = decode_varint(bytes)?;
    let doc_freq = doc_freq as usize;

    if doc_freq > MAX_POSTING_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Posting list too large: {} (max {})", doc_freq, MAX_POSTING_SIZE),
        ));
    }

    if doc_freq == 0 {
        return Ok((Vec::new(), pos));
    }

    // Decode entries: delta doc_id (varint), section_idx (varint), heading_level (u8)
    let mut entries = Vec::with_capacity(doc_freq);
    let mut prev_doc_id = 0u32;

    for _ in 0..doc_freq {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated posting entry",
            ));
        }

        let (delta, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;
        let doc_id = prev_doc_id + delta as u32;
        prev_doc_id = doc_id;

        let (section_idx, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;

        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated heading_level",
            ));
        }
        let heading_level = bytes[pos];
        pos += 1;

        entries.push(PostingEntry {
            doc_id,
            section_idx: section_idx as u32,
            heading_level,
        });
    }

    Ok((entries, pos))
}

// ============================================================================
// SKIP LIST
// ============================================================================

/// Skip list entry
#[derive(Debug, Clone)]
pub struct SkipEntry {
    pub doc_id: u32,
    pub block_offset: u32,
}

/// Skip list for a single term
#[derive(Debug, Clone)]
pub struct SkipList {
    /// Skip entries at each level (level 0 = finest granularity)
    pub levels: Vec<Vec<SkipEntry>>,
}

impl SkipList {
    /// Build skip list from doc IDs
    pub fn build(doc_ids: &[u32]) -> Option<Self> {
        if doc_ids.len() < SKIP_LIST_THRESHOLD {
            return None;
        }

        let num_blocks = doc_ids.len() / BLOCK_SIZE;
        if num_blocks < SKIP_INTERVAL {
            return None;
        }

        // Level 0: every SKIP_INTERVAL blocks
        let mut level0 = Vec::new();
        for block_idx in (0..num_blocks).step_by(SKIP_INTERVAL) {
            let doc_idx = block_idx * BLOCK_SIZE;
            level0.push(SkipEntry {
                doc_id: doc_ids[doc_idx],
                block_offset: block_idx as u32,
            });
        }

        let mut levels = vec![level0];

        // Higher levels: every SKIP_INTERVAL entries from previous level
        while levels.last().unwrap().len() >= SKIP_INTERVAL {
            let prev = levels.last().unwrap();
            let next: Vec<SkipEntry> = prev.iter().step_by(SKIP_INTERVAL).cloned().collect();
            if next.len() < 2 {
                break;
            }
            levels.push(next);
        }

        Some(SkipList { levels })
    }

    /// Find the block containing doc_id or the closest preceding block
    pub fn skip_to(&self, target_doc: u32) -> Option<u32> {
        let mut block_offset = 0u32;

        // Search from highest level down
        for level in self.levels.iter().rev() {
            for entry in level {
                if entry.doc_id <= target_doc {
                    block_offset = entry.block_offset;
                } else {
                    break;
                }
            }
        }

        Some(block_offset)
    }

    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(self.levels.len() as u8);

        for level in &self.levels {
            encode_varint(level.len() as u64, buf);
            for entry in level {
                encode_varint(entry.doc_id as u64, buf);
                encode_varint(entry.block_offset as u64, buf);
            }
        }
    }

    pub fn decode(bytes: &[u8]) -> io::Result<(Self, usize)> {
        if bytes.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Empty skip list data",
            ));
        }

        let num_levels = bytes[0] as usize;
        let mut pos = 1;

        // Validate num_levels
        if num_levels > MAX_SKIP_LEVELS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Too many skip levels: {} (max {})",
                    num_levels, MAX_SKIP_LEVELS
                ),
            ));
        }

        let mut levels = Vec::with_capacity(num_levels);

        for _ in 0..num_levels {
            if pos >= bytes.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Truncated skip list level",
                ));
            }
            let (count, consumed) = decode_varint(&bytes[pos..])?;
            pos += consumed;
            let count = count as usize;

            let mut level = Vec::with_capacity(count);
            for _ in 0..count {
                if pos >= bytes.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Truncated skip list entry",
                    ));
                }
                let (doc_id, consumed) = decode_varint(&bytes[pos..])?;
                pos += consumed;

                if pos >= bytes.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "Truncated skip list block offset",
                    ));
                }
                let (block_offset, consumed) = decode_varint(&bytes[pos..])?;
                pos += consumed;

                level.push(SkipEntry {
                    doc_id: doc_id as u32,
                    block_offset: block_offset as u32,
                });
            }
            levels.push(level);
        }

        Ok((SkipList { levels }, pos))
    }
}
