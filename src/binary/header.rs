// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Binary format header and footer structures.
//!
//! The header is 52 bytes of fixed-size fields, designed to be parsed in one
//! read before anything else. It tells you exactly where every section lives,
//! so you can seek directly to what you need or dispatch parallel decodes.
//!
//! The footer is 8 bytes: a CRC32 checksum over everything before it, plus a
//! magic number ("XROS", the header magic reversed). If the footer is wrong,
//! something got corrupted or truncated. Don't trust the data.
//!
//! `SectionOffsets` is the single source of truth for v12 file layout. Every
//! piece of code that reads or writes sections MUST use it. This prevents the
//! "I updated the write path but forgot the read path" class of bugs.

use std::io::{self, Read, Write};

use crc32fast::Hasher as Crc32Hasher;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Magic bytes: "SORX" in ASCII (header)
pub const MAGIC: [u8; 4] = [0x53, 0x4F, 0x52, 0x58];

/// Footer magic: "XROS" (reversed, marks valid file end)
pub const FOOTER_MAGIC: [u8; 4] = [0x58, 0x52, 0x4F, 0x53];

/// Current format version (v12: WASM first for streaming, v10+ encoding only)
pub const VERSION: u8 = 12;

/// Block size for PFOR encoding (Lucene uses 128)
pub const BLOCK_SIZE: usize = 128;

/// Minimum docs for skip list
pub const SKIP_LIST_THRESHOLD: usize = 1024;

/// Skip interval (every N blocks)
pub const SKIP_INTERVAL: usize = 8;

// ============================================================================
// SECURITY LIMITS (prevent resource exhaustion from malicious input)
// ============================================================================

/// Maximum file size: 100 MB (prevents huge allocations)
pub const MAX_FILE_SIZE: usize = 100 * 1024 * 1024;

/// Maximum number of documents
pub const MAX_DOC_COUNT: u32 = 10_000_000;

/// Maximum number of terms
pub const MAX_TERM_COUNT: u32 = 10_000_000;

/// Maximum posting list size per term
pub const MAX_POSTING_SIZE: usize = 10_000_000;

/// Maximum varint bytes (u64 needs at most 10 bytes)
pub const MAX_VARINT_BYTES: usize = 10;

/// Maximum skip list levels (log8 of MAX_DOC_COUNT)
pub const MAX_SKIP_LEVELS: usize = 8;

// ============================================================================
// FLAGS
// ============================================================================

/// Format flags
#[derive(Debug, Clone, Copy, Default)]
pub struct FormatFlags(pub(crate) u8);

impl FormatFlags {
    pub const HAS_SKIP_LISTS: u8 = 0b0000_0001;
    pub const HAS_POSITIONS: u8 = 0b0000_0010;
    pub const HAS_PAYLOADS: u8 = 0b0000_0100;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn with_skip_lists(mut self) -> Self {
        self.0 |= Self::HAS_SKIP_LISTS;
        self
    }

    pub fn has_skip_lists(self) -> bool {
        self.0 & Self::HAS_SKIP_LISTS != 0
    }
}

// ============================================================================
// HEADER
// ============================================================================

/// Binary format header (52 bytes fixed size, v7)
#[derive(Debug, Clone)]
pub struct SorexHeader {
    pub version: u8,
    pub flags: FormatFlags,
    pub doc_count: u32,
    pub term_count: u32,
    pub vocab_len: u32,
    pub sa_len: u32,
    pub postings_len: u32,
    pub skip_len: u32,
    /// Section ID string table length (v6+, was fst_len in v2-v3)
    /// Used for deep linking - stores unique section_id strings
    pub section_table_len: u32,
    /// Levenshtein DFA bytes length (new in v3)
    pub lev_dfa_len: u32,
    /// Docs binary section length (new in v5)
    pub docs_len: u32,
    /// WASM binary length (new in v7)
    /// Embedded WASM for self-contained search runtime
    pub wasm_len: u32,
    /// Dictionary tables length (new in v7)
    /// Parquet-style compression for category, author, tags, href_prefix
    pub dict_table_len: u32,
}

impl SorexHeader {
    // 4 (magic) + 1 (version) + 1 (flags) + 11*4 (u32s) + 2 (reserved) = 52
    pub const SIZE: usize = 52;

    /// Compute section byte offsets for this header.
    /// This is the SINGLE SOURCE OF TRUTH for the v12 file layout.
    pub fn section_offsets(&self) -> SectionOffsets {
        SectionOffsets::from_header(self)
    }

    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(&MAGIC)?;
        w.write_all(&[self.version])?;
        w.write_all(&[self.flags.0])?;
        w.write_all(&self.doc_count.to_le_bytes())?;
        w.write_all(&self.term_count.to_le_bytes())?;
        w.write_all(&self.vocab_len.to_le_bytes())?;
        w.write_all(&self.sa_len.to_le_bytes())?;
        w.write_all(&self.postings_len.to_le_bytes())?;
        w.write_all(&self.skip_len.to_le_bytes())?;
        w.write_all(&self.section_table_len.to_le_bytes())?; // v6: section_id table
        w.write_all(&self.lev_dfa_len.to_le_bytes())?;
        w.write_all(&self.docs_len.to_le_bytes())?;
        w.write_all(&self.wasm_len.to_le_bytes())?; // v7: embedded WASM
        w.write_all(&self.dict_table_len.to_le_bytes())?; // v7: dictionary tables
        w.write_all(&[0u8; 2])?; // reserved (2 bytes for alignment)
        Ok(())
    }

    pub fn read<R: Read>(r: &mut R) -> io::Result<Self> {
        let mut magic = [0u8; 4];
        r.read_exact(&mut magic)?;
        if magic != MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid magic: expected SORX, got {:?}", magic),
            ));
        }

        let mut buf = [0u8; 48]; // 52 - 4 (magic) = 48
        r.read_exact(&mut buf)?;

        Ok(Self {
            version: buf[0],
            flags: FormatFlags(buf[1]),
            doc_count: u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]),
            term_count: u32::from_le_bytes([buf[6], buf[7], buf[8], buf[9]]),
            vocab_len: u32::from_le_bytes([buf[10], buf[11], buf[12], buf[13]]),
            sa_len: u32::from_le_bytes([buf[14], buf[15], buf[16], buf[17]]),
            postings_len: u32::from_le_bytes([buf[18], buf[19], buf[20], buf[21]]),
            skip_len: u32::from_le_bytes([buf[22], buf[23], buf[24], buf[25]]),
            section_table_len: u32::from_le_bytes([buf[26], buf[27], buf[28], buf[29]]), // v6: section_id table
            lev_dfa_len: u32::from_le_bytes([buf[30], buf[31], buf[32], buf[33]]),
            docs_len: u32::from_le_bytes([buf[34], buf[35], buf[36], buf[37]]),
            wasm_len: u32::from_le_bytes([buf[38], buf[39], buf[40], buf[41]]), // v7: embedded WASM
            dict_table_len: u32::from_le_bytes([buf[42], buf[43], buf[44], buf[45]]), // v7: dictionary tables
            // buf[46..48] is reserved
        })
    }
}

// ============================================================================
// FOOTER (8 bytes)
// ============================================================================

/// Footer with CRC32 checksum and magic number
#[derive(Debug, Clone)]
pub struct SorexFooter {
    /// CRC32 checksum of header + all sections (everything before footer)
    pub crc32: u32,
}

// ============================================================================
// SECTION OFFSETS (SINGLE SOURCE OF TRUTH for v12 layout)
// ============================================================================

/// Section byte offsets for the v12 file layout.
///
/// This is the SINGLE SOURCE OF TRUTH for file layout. All code that reads
/// or writes section data MUST use this struct to compute offsets.
///
/// v12 layout is optimized for streaming decode based on dependency analysis:
/// - WASM first for `WebAssembly.compileStreaming()` async
/// - Dependencies ordered: VOCAB before SA, DICT_TABLES before DOCS
/// - LEV_DFA last (only needed for T3 fuzzy search)
#[derive(Debug, Clone, Copy)]
pub struct SectionOffsets {
    // Start and end offsets for each section
    pub wasm: (usize, usize),
    pub vocabulary: (usize, usize),
    pub dict_tables: (usize, usize),
    pub postings: (usize, usize),
    pub suffix_array: (usize, usize),
    pub docs: (usize, usize),
    pub section_table: (usize, usize),
    pub skip_lists: (usize, usize),
    pub lev_dfa: (usize, usize),
    pub footer: (usize, usize),
}

impl SectionOffsets {
    /// Compute section offsets from header lengths.
    ///
    /// v12 layout order (dependency-optimized):
    /// 1. HEADER        [52B]     - Parse first to get section lengths
    /// 2. WASM          [wasm_len]    - Start async compile immediately
    /// 3. VOCABULARY    [vocab_len]   - Decode, needed by SUFFIX_ARRAY
    /// 4. DICT_TABLES   [dict_table_len] - Decode, needed by DOCS
    /// 5. POSTINGS      [postings_len]- Decode, independent
    /// 6. SUFFIX_ARRAY  [sa_len]      - Decode after VOCABULARY
    /// 7. DOCS          [docs_len]    - Decode after DICT_TABLES
    /// 8. SECTION_TABLE [section_table_len] - For deep links
    /// 9. SKIP_LISTS    [skip_len]    - For fast postings access
    /// 10. LEV_DFA      [lev_dfa_len] - Only for T3 fuzzy search
    /// 11. FOOTER       [8B]          - CRC32 validation
    pub fn from_header(h: &SorexHeader) -> Self {
        let mut pos = SorexHeader::SIZE;

        // 1. WASM (async compile)
        let wasm_start = pos;
        pos += h.wasm_len as usize;
        let wasm_end = pos;

        // 2. VOCABULARY (needed by SA)
        let vocab_start = pos;
        pos += h.vocab_len as usize;
        let vocab_end = pos;

        // 3. DICT_TABLES (needed by DOCS)
        let dict_start = pos;
        pos += h.dict_table_len as usize;
        let dict_end = pos;

        // 4. POSTINGS (independent)
        let postings_start = pos;
        pos += h.postings_len as usize;
        let postings_end = pos;

        // 5. SUFFIX_ARRAY (after VOCABULARY)
        let sa_start = pos;
        pos += h.sa_len as usize;
        let sa_end = pos;

        // 6. DOCS (after DICT_TABLES)
        let docs_start = pos;
        pos += h.docs_len as usize;
        let docs_end = pos;

        // 7. SECTION_TABLE (for deep links)
        let section_start = pos;
        pos += h.section_table_len as usize;
        let section_end = pos;

        // 8. SKIP_LISTS (for fast postings access)
        let skip_start = pos;
        pos += h.skip_len as usize;
        let skip_end = pos;

        // 9. LEV_DFA (only for T3 fuzzy)
        let lev_start = pos;
        pos += h.lev_dfa_len as usize;
        let lev_end = pos;

        // 10. FOOTER
        let footer_start = pos;
        let footer_end = pos + SorexFooter::SIZE;

        Self {
            wasm: (wasm_start, wasm_end),
            vocabulary: (vocab_start, vocab_end),
            dict_tables: (dict_start, dict_end),
            postings: (postings_start, postings_end),
            suffix_array: (sa_start, sa_end),
            docs: (docs_start, docs_end),
            section_table: (section_start, section_end),
            skip_lists: (skip_start, skip_end),
            lev_dfa: (lev_start, lev_end),
            footer: (footer_start, footer_end),
        }
    }

    /// Expected content size (everything before footer)
    pub fn content_size(&self) -> usize {
        self.footer.0
    }

    /// Total file size including footer
    pub fn total_size(&self) -> usize {
        self.footer.1
    }

    /// Get a slice for a section from the bytes
    #[inline]
    pub fn slice<'a>(&self, bytes: &'a [u8], section: (usize, usize)) -> Option<&'a [u8]> {
        bytes.get(section.0..section.1)
    }
}

impl SorexFooter {
    pub const SIZE: usize = 8; // 4 bytes CRC32 + 4 bytes magic

    pub fn write<W: Write>(&self, w: &mut W) -> io::Result<()> {
        w.write_all(&self.crc32.to_le_bytes())?;
        w.write_all(&FOOTER_MAGIC)?;
        Ok(())
    }

    pub fn read(bytes: &[u8]) -> io::Result<Self> {
        if bytes.len() < Self::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "File too short for footer",
            ));
        }

        let footer_start = bytes.len() - Self::SIZE;

        // Verify footer magic
        let magic = &bytes[footer_start + 4..];
        if magic != FOOTER_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid footer magic: expected TFIS, got {:?}", magic),
            ));
        }

        let crc32 = u32::from_le_bytes([
            bytes[footer_start],
            bytes[footer_start + 1],
            bytes[footer_start + 2],
            bytes[footer_start + 3],
        ]);

        Ok(Self { crc32 })
    }

    /// Compute CRC32 over the given bytes
    pub fn compute_crc32(data: &[u8]) -> u32 {
        let mut hasher = Crc32Hasher::new();
        hasher.update(data);
        hasher.finalize()
    }
}
