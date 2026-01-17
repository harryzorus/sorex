// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Binary format for Sorex search indexes.
//!
//! The v12 format is designed for two conflicting goals: fast parsing and small
//! file size. The trick is to let brotli do the heavy lifting. Delta-encoded
//! postings and front-compressed vocabulary create repetitive patterns that
//! brotli loves. We get ~45% smaller files than naive varint encoding.
//!
//! The section ordering is carefully chosen based on dependency analysis.
//! WASM goes first so the browser can start `WebAssembly.compileStreaming()`
//! while we're still downloading. Vocabulary before suffix array. Dict tables
//! before docs. Levenshtein DFA last because fuzzy search is usually not the
//! first query.
//!
//! # Security Considerations
//!
//! This format is designed to be safely parsed from untrusted sources:
//! - All size fields are validated against MAX_* constants
//! - Bounds checking prevents buffer overreads
//! - CRC32 footer detects corruption/truncation
//! - Varint decoder has maximum iteration limits
//!
//! # Format Overview (v12)
//!
//! v12 layout is optimized for streaming decode based on dependency analysis.
//! Sections are ordered to minimize time-to-first-search:
//!
//! 1. WASM first: enables `WebAssembly.compileStreaming()` async
//! 2. VOCABULARY before SUFFIX_ARRAY (dependency)
//! 3. DICT_TABLES before DOCS (dependency)
//! 4. LEV_DFA last (only needed for T3 fuzzy search)
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │ HEADER (52 bytes)                                          │
//! │   magic: [u8; 4] = "SORX"                                  │
//! │   version: u8 = 12                                         │
//! │   flags: u8                                                │
//! │   doc_count: u32                                           │
//! │   term_count: u32                                          │
//! │   vocab_len: u32, sa_len: u32, postings_len: u32           │
//! │   skip_len: u32, section_table_len: u32, lev_dfa_len: u32  │
//! │   docs_len: u32, wasm_len: u32, dict_table_len: u32        │
//! │   reserved: [u8; 2]                                        │
//! ├────────────────────────────────────────────────────────────┤
//! │ 1. WASM (async compile, ~200KB)                            │
//! ├────────────────────────────────────────────────────────────┤
//! │ 2. VOCABULARY (front-compressed, needed by SA)             │
//! ├────────────────────────────────────────────────────────────┤
//! │ 3. DICT_TABLES (category/author/tags, needed by DOCS)      │
//! ├────────────────────────────────────────────────────────────┤
//! │ 4. POSTINGS (delta+varint, largest section)                │
//! ├────────────────────────────────────────────────────────────┤
//! │ 5. SUFFIX_ARRAY (separated streams, after VOCABULARY)      │
//! ├────────────────────────────────────────────────────────────┤
//! │ 6. DOCS (document metadata, after DICT_TABLES)             │
//! ├────────────────────────────────────────────────────────────┤
//! │ 7. SECTION_TABLE (deduplicated section_ids for deep links) │
//! ├────────────────────────────────────────────────────────────┤
//! │ 8. SKIP_LISTS (for fast postings access)                   │
//! ├────────────────────────────────────────────────────────────┤
//! │ 9. LEV_DFA (precomputed automaton, only for T3 fuzzy)      │
//! ├────────────────────────────────────────────────────────────┤
//! │ FOOTER (8 bytes): crc32 + magic "XROS"                     │
//! └────────────────────────────────────────────────────────────┘
//! ```

// Submodules
mod encoding;
mod header;
#[cfg(feature = "rayon")]
mod incremental;
mod postings;

// Re-export from submodules for public API
#[cfg(feature = "rayon")]
pub use incremental::IncrementalLoader;
pub use encoding::{
    decode_section_table, decode_suffix_array, decode_varint, decode_vocabulary,
    encode_section_table, encode_suffix_array, encode_varint, encode_vocabulary,
};
pub use header::{
    FormatFlags, SectionOffsets, SorexFooter, SorexHeader, BLOCK_SIZE, FOOTER_MAGIC, MAGIC,
    MAX_DOC_COUNT, MAX_FILE_SIZE, MAX_POSTING_SIZE, MAX_SKIP_LEVELS, MAX_TERM_COUNT,
    MAX_VARINT_BYTES, SKIP_INTERVAL, SKIP_LIST_THRESHOLD, VERSION,
};
pub use postings::{decode_postings, encode_postings, PostingEntry, SkipEntry, SkipList};

use std::collections::HashMap;
use std::io;

use crate::util::dict_table::DictTables;

// ============================================================================
// FULL LAYER ENCODING
// ============================================================================

/// A complete binary layer
#[derive(Debug)]
pub struct BinaryLayer {
    pub header: SorexHeader,
    pub vocab_bytes: Vec<u8>,
    pub sa_bytes: Vec<u8>,
    pub postings_bytes: Vec<u8>,
    pub skip_bytes: Vec<u8>,
    /// Section ID string table bytes (v6: deduplicated section_ids for deep linking)
    pub section_table_bytes: Vec<u8>,
    /// Levenshtein DFA bytes (precomputed parametric automaton)
    pub lev_dfa_bytes: Vec<u8>,
    /// Docs binary section (v5: length-prefixed strings, 1-bit type)
    pub docs_bytes: Vec<u8>,
    /// WASM binary (v7: embedded WebAssembly for self-contained runtime)
    pub wasm_bytes: Vec<u8>,
    /// Dictionary tables (v7: Parquet-style compression for category, author, tags, href_prefix)
    pub dict_table_bytes: Vec<u8>,
}

impl BinaryLayer {
    /// Build a binary layer (v12 format)
    ///
    /// v12 format optimized for brotli compression:
    /// - Front-compressed vocabulary
    /// - Delta+varint postings (~45% better compression)
    /// - Separated streams for suffix array
    /// - WASM first for streaming compilation
    #[allow(clippy::too_many_arguments)]
    pub fn build_v7(
        vocabulary: &[String],
        suffix_array: &[(u32, u32)],
        postings: &[Vec<PostingEntry>], // One posting list per term (in vocab order)
        section_table: &[String],       // Unique section_id strings
        doc_count: usize,
        lev_dfa_bytes: Vec<u8>,
        docs_bytes: Vec<u8>,
        wasm_bytes: Vec<u8>, // Embedded WASM binary
    ) -> io::Result<Self> {
        // Encode vocabulary with front compression
        let mut vocab_bytes = Vec::new();
        encode_vocabulary(vocabulary, &mut vocab_bytes);

        // Encode suffix array (separated streams for brotli compression)
        let mut sa_bytes = Vec::new();
        encode_suffix_array(suffix_array, &mut sa_bytes);

        // Encode postings (delta+varint for brotli compression)
        let mut postings_bytes = Vec::new();
        for posting_list in postings {
            encode_postings(posting_list, &mut postings_bytes);
        }

        // Build skip lists for large posting lists
        let mut skip_bytes = Vec::new();
        let mut has_skip_lists = false;

        for (term_ord, posting_list) in postings.iter().enumerate() {
            let doc_ids: Vec<u32> = posting_list.iter().map(|e| e.doc_id).collect();
            if let Some(skip_list) = SkipList::build(&doc_ids) {
                has_skip_lists = true;
                encode_varint(term_ord as u64, &mut skip_bytes);
                skip_list.encode(&mut skip_bytes);
            }
        }

        // Encode section table
        let mut section_table_bytes = Vec::new();
        encode_section_table(section_table, &mut section_table_bytes);

        let flags = if has_skip_lists {
            FormatFlags::new().with_skip_lists()
        } else {
            FormatFlags::new()
        };

        let header = SorexHeader {
            version: VERSION,
            flags,
            doc_count: doc_count as u32,
            term_count: vocabulary.len() as u32,
            vocab_len: vocab_bytes.len() as u32,
            sa_len: sa_bytes.len() as u32,
            postings_len: postings_bytes.len() as u32,
            skip_len: skip_bytes.len() as u32,
            section_table_len: section_table_bytes.len() as u32,
            lev_dfa_len: lev_dfa_bytes.len() as u32,
            docs_len: docs_bytes.len() as u32,
            wasm_len: wasm_bytes.len() as u32,
            dict_table_len: 0, // Caller sets this after build (see build/parallel.rs)
        };

        Ok(Self {
            header,
            vocab_bytes,
            sa_bytes,
            postings_bytes,
            skip_bytes,
            section_table_bytes,
            lev_dfa_bytes,
            docs_bytes,
            wasm_bytes,
            dict_table_bytes: Vec::new(), // Empty for now, populated via build_v7_with_dicts
        })
    }

    /// Build a binary layer with section_ids (v6-compatible, no WASM)
    ///
    /// Postings include section_id indices for deep linking.
    /// The section_table contains unique section_id strings.
    pub fn build_v6(
        vocabulary: &[String],
        suffix_array: &[(u32, u32)],
        postings: &[Vec<PostingEntry>], // One posting list per term (in vocab order)
        section_table: &[String],       // Unique section_id strings
        doc_count: usize,
        lev_dfa_bytes: Vec<u8>,
        docs_bytes: Vec<u8>,
    ) -> io::Result<Self> {
        Self::build_v7(
            vocabulary,
            suffix_array,
            postings,
            section_table,
            doc_count,
            lev_dfa_bytes,
            docs_bytes,
            Vec::new(), // Empty WASM for v6 compatibility
        )
    }

    /// Build a binary layer (legacy v5-compatible, no section_ids)
    pub fn build(
        vocabulary: &[String],
        suffix_array: &[(u32, u32)],
        postings: &[Vec<u32>], // One posting list per term (in vocab order)
        doc_count: usize,
        lev_dfa_bytes: Vec<u8>,
        docs_bytes: Vec<u8>,
    ) -> io::Result<Self> {
        // Convert to PostingEntry with no section_ids
        let postings_v6: Vec<Vec<PostingEntry>> = postings
            .iter()
            .map(|pl| {
                pl.iter()
                    .map(|&doc_id| PostingEntry {
                        doc_id,
                        section_idx: 0,
                        heading_level: 0, // Legacy v5 path has no heading levels (use build_v7)
                    })
                    .collect()
            })
            .collect();

        Self::build_v6(
            vocabulary,
            suffix_array,
            &postings_v6,
            &[], // No section table
            doc_count,
            lev_dfa_bytes,
            docs_bytes,
        )
    }

    /// Serialize to bytes (with CRC32 footer)
    ///
    /// v12 layout optimized for streaming decode (dependency-ordered):
    /// 1. HEADER      - Parse first to get section offsets
    /// 2. WASM        - Start WebAssembly.compile() async immediately
    /// 3. VOCABULARY  - Decode (expensive), needed by SUFFIX_ARRAY
    /// 4. DICT_TABLES - Decode (fast), needed by DOCS
    /// 5. POSTINGS    - Decode (expensive), independent
    /// 6. SUFFIX_ARRAY- Decode after VOCABULARY
    /// 7. DOCS        - Decode after DICT_TABLES
    /// 8. SECTION_TABLE- Decode (moderate), for deep links
    /// 9. SKIP_LISTS  - Decode, for fast postings access
    /// 10. LEV_DFA    - Memcpy, only for fuzzy search (T3)
    /// 11. FOOTER     - CRC32 validation
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let content_size = SorexHeader::SIZE
            + self.wasm_bytes.len()
            + self.vocab_bytes.len()
            + self.dict_table_bytes.len()
            + self.postings_bytes.len()
            + self.sa_bytes.len()
            + self.docs_bytes.len()
            + self.section_table_bytes.len()
            + self.skip_bytes.len()
            + self.lev_dfa_bytes.len();
        let total_size = content_size + SorexFooter::SIZE;

        let mut buf = Vec::with_capacity(total_size);
        self.header.write(&mut buf)?;
        // Optimal decode order based on dependency graph analysis:
        buf.extend_from_slice(&self.wasm_bytes);        // 1. WASM (async compile)
        buf.extend_from_slice(&self.vocab_bytes);       // 2. VOCABULARY (needed by SA)
        buf.extend_from_slice(&self.dict_table_bytes);  // 3. DICT_TABLES (needed by DOCS)
        buf.extend_from_slice(&self.postings_bytes);    // 4. POSTINGS (independent)
        buf.extend_from_slice(&self.sa_bytes);          // 5. SUFFIX_ARRAY (after VOCAB)
        buf.extend_from_slice(&self.docs_bytes);        // 6. DOCS (after DICT_TABLES)
        buf.extend_from_slice(&self.section_table_bytes); // 7. SECTION_TABLE
        buf.extend_from_slice(&self.skip_bytes);        // 8. SKIP_LISTS
        buf.extend_from_slice(&self.lev_dfa_bytes);     // 9. LEV_DFA (only for T3)

        // Compute CRC32 over everything written so far
        let crc32 = SorexFooter::compute_crc32(&buf);
        let footer = SorexFooter { crc32 };
        footer.write(&mut buf)?;

        Ok(buf)
    }

    /// Deserialize from bytes (with CRC32 validation)
    ///
    /// # Validation
    ///
    /// This method performs comprehensive validation:
    /// 1. File size is within limits (MAX_FILE_SIZE)
    /// 2. Header magic is valid ("SORX")
    /// 3. Version is supported
    /// 4. Section lengths don't exceed file size
    /// 5. Footer magic is valid ("XROS")
    /// 6. CRC32 checksum matches
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        // Security: Check file size limits
        if bytes.len() > MAX_FILE_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "File too large: {} bytes (max {})",
                    bytes.len(),
                    MAX_FILE_SIZE
                ),
            ));
        }

        // Minimum size: header + footer
        let min_size = SorexHeader::SIZE + SorexFooter::SIZE;
        if bytes.len() < min_size {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "File too small: {} bytes (minimum {})",
                    bytes.len(),
                    min_size
                ),
            ));
        }

        // Verify footer magic and read CRC32
        let footer = SorexFooter::read(bytes)?;
        let content = &bytes[..bytes.len() - SorexFooter::SIZE];
        let computed_crc32 = SorexFooter::compute_crc32(content);

        if footer.crc32 != computed_crc32 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "CRC32 mismatch: expected {:#010x}, got {:#010x} (file corrupted)",
                    footer.crc32, computed_crc32
                ),
            ));
        }

        // Parse header
        let mut cursor = io::Cursor::new(bytes);
        let header = SorexHeader::read(&mut cursor)?;

        // Validate version (v12 only)
        if header.version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unsupported version: {} (expected {})",
                    header.version, VERSION
                ),
            ));
        }

        // Validate doc_count and term_count
        if header.doc_count > MAX_DOC_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Too many documents: {} (max {})",
                    header.doc_count, MAX_DOC_COUNT
                ),
            ));
        }

        if header.term_count > MAX_TERM_COUNT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Too many terms: {} (max {})",
                    header.term_count, MAX_TERM_COUNT
                ),
            ));
        }

        // Get section offsets from SINGLE SOURCE OF TRUTH
        let offsets = header.section_offsets();

        // Validate content size matches expected
        if offsets.content_size() != content.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Section lengths mismatch: header claims {} bytes, got {} bytes",
                    offsets.content_size(),
                    content.len()
                ),
            ));
        }

        // Helper to extract section with proper error handling
        let extract_section = |section: (usize, usize), name: &str| -> io::Result<Vec<u8>> {
            bytes.get(section.0..section.1)
                .ok_or_else(|| io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("{} section truncated", name),
                ))
                .map(|s| s.to_vec())
        };

        // Extract sections using offsets from SectionOffsets (single source of truth)
        let wasm_bytes = extract_section(offsets.wasm, "WASM")?;
        let vocab_bytes = extract_section(offsets.vocabulary, "Vocabulary")?;
        let dict_table_bytes = extract_section(offsets.dict_tables, "Dict tables")?;
        let postings_bytes = extract_section(offsets.postings, "Postings")?;
        let sa_bytes = extract_section(offsets.suffix_array, "Suffix array")?;
        let docs_bytes = extract_section(offsets.docs, "Docs")?;
        let section_table_bytes = extract_section(offsets.section_table, "Section table")?;
        let skip_bytes = extract_section(offsets.skip_lists, "Skip lists")?;
        let lev_dfa_bytes = extract_section(offsets.lev_dfa, "Levenshtein DFA")?;
        // FOOTER is validated at the start of from_bytes()

        Ok(Self {
            header,
            vocab_bytes,
            sa_bytes,
            postings_bytes,
            skip_bytes,
            section_table_bytes,
            lev_dfa_bytes,
            docs_bytes,
            wasm_bytes,
            dict_table_bytes,
        })
    }
}

// ============================================================================
// DOCS ENCODING (length-prefixed strings, 1-bit type)
// ============================================================================

/// Document metadata for encoding (matches DocMeta fields)
pub struct DocMetaInput {
    pub title: String,
    pub excerpt: String,
    pub href: String,
    pub doc_type: String,
    /// Section ID for deep linking (None for titles, Some for headings/content)
    pub section_id: Option<String>,
    /// Category for client-side filtering (e.g., "engineering", "adventures")
    pub category: Option<String>,
    /// Author name (for multi-author blogs)
    pub author: Option<String>,
    /// Tags/labels for categorization
    pub tags: Vec<String>,
}

/// Magic byte indicating docs section v2+ format (includes author/tags).
/// Value 0xFE chosen because it's unlikely as first byte of varint doc_count
/// (would require >= 254 docs with continuation bit pattern).
const DOCS_V2_MAGIC: u8 = 0xFE;

/// Docs section version (v2 = includes author and tags)
const DOCS_VERSION: u8 = 2;

/// Encode docs to binary format (no JSON dependency)
///
/// Format (v2):
/// - magic: u8 (0xFE = v2+ format indicator)
/// - version: u8 (2)
/// - count: varint (number of docs)
/// - For each doc:
///   - type: u8 (0=page, 1=post)
///   - title: varint_len + utf8
///   - excerpt: varint_len + utf8
///   - href: varint_len + utf8
///   - has_section_id: u8 (0=None, 1=Some)
///   - section_id: varint_len + utf8 (only if has_section_id=1)
///   - category: varint_len + utf8 (empty string if None)
///   - author: varint_len + utf8 (empty string if None)
///   - tags_count: varint (number of tags)
///   - for each tag: varint_len + utf8
pub fn encode_docs_binary(docs: &[DocMetaInput]) -> Vec<u8> {
    let mut buf = Vec::new();

    // Magic byte + version (v2 includes author and tags)
    buf.push(DOCS_V2_MAGIC);
    buf.push(DOCS_VERSION);

    // Header: doc count
    encode_varint(docs.len() as u64, &mut buf);

    for doc in docs {
        // Type flag (1 byte)
        let type_byte = if doc.doc_type == "page" { 0u8 } else { 1u8 };
        buf.push(type_byte);

        // Length-prefixed strings
        let title_bytes = doc.title.as_bytes();
        encode_varint(title_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(title_bytes);

        let excerpt_bytes = doc.excerpt.as_bytes();
        encode_varint(excerpt_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(excerpt_bytes);

        let href_bytes = doc.href.as_bytes();
        encode_varint(href_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(href_bytes);

        // Section ID (optional)
        match &doc.section_id {
            Some(id) => {
                buf.push(1u8); // has_section_id = true
                let id_bytes = id.as_bytes();
                encode_varint(id_bytes.len() as u64, &mut buf);
                buf.extend_from_slice(id_bytes);
            }
            None => {
                buf.push(0u8); // has_section_id = false
            }
        }

        // Category (empty string if None)
        let category = doc.category.as_deref().unwrap_or("");
        let category_bytes = category.as_bytes();
        encode_varint(category_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(category_bytes);

        // Author (empty string if None)
        let author = doc.author.as_deref().unwrap_or("");
        let author_bytes = author.as_bytes();
        encode_varint(author_bytes.len() as u64, &mut buf);
        buf.extend_from_slice(author_bytes);

        // Tags (count + each tag as length-prefixed string)
        encode_varint(doc.tags.len() as u64, &mut buf);
        for tag in &doc.tags {
            let tag_bytes = tag.as_bytes();
            encode_varint(tag_bytes.len() as u64, &mut buf);
            buf.extend_from_slice(tag_bytes);
        }
    }

    buf
}

/// Decode docs from binary format (supports v1 and v2 formats)
///
/// - v1 (legacy): varint(count) + docs without author/tags
/// - v2: magic(0xFE) + version(2) + varint(count) + docs with author/tags
pub(crate) fn decode_docs_binary(bytes: &[u8]) -> io::Result<Vec<DocMeta>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut offset = 0;

    // Detect format version by checking for magic byte
    let is_v2 = bytes[0] == DOCS_V2_MAGIC;
    if is_v2 {
        offset += 1; // Skip magic byte
        if offset >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated docs section: missing version byte",
            ));
        }
        let version = bytes[offset];
        offset += 1;
        if version != DOCS_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Unsupported docs version: {} (expected {})", version, DOCS_VERSION),
            ));
        }
    }

    // Header: doc count
    let (doc_count, size) = decode_varint(&bytes[offset..])?;
    offset += size;

    let mut docs = Vec::with_capacity(doc_count as usize);

    for _ in 0..doc_count {
        if offset >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated docs section",
            ));
        }

        // Type flag
        let doc_type = if bytes[offset] == 0 { "page" } else { "post" };
        offset += 1;

        // Title
        let (len, size) = decode_varint(&bytes[offset..])?;
        offset += size;
        let title = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        offset += len as usize;

        // Excerpt
        let (len, size) = decode_varint(&bytes[offset..])?;
        offset += size;
        let excerpt = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        offset += len as usize;

        // Href
        let (len, size) = decode_varint(&bytes[offset..])?;
        offset += size;
        let href = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        offset += len as usize;

        // Section ID (optional string)
        if offset >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated docs section: missing section_id flag",
            ));
        }
        let has_section_id = bytes[offset];
        offset += 1;
        let section_id = if has_section_id == 1 {
            let (len, size) = decode_varint(&bytes[offset..])?;
            offset += size;
            let id = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            offset += len as usize;
            Some(id)
        } else {
            None
        };

        // Category (empty string = None)
        let (len, size) = decode_varint(&bytes[offset..])?;
        offset += size;
        let category = if len == 0 {
            None
        } else {
            let cat = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            offset += len as usize;
            Some(cat)
        };

        // v2: Author and tags
        let (author, tags) = if is_v2 {
            // Author (empty string = None)
            let (len, size) = decode_varint(&bytes[offset..])?;
            offset += size;
            let author = if len == 0 {
                None
            } else {
                let auth = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                offset += len as usize;
                Some(auth)
            };

            // Tags (count + each tag as length-prefixed string)
            let (tags_count, size) = decode_varint(&bytes[offset..])?;
            offset += size;
            let mut tags = Vec::with_capacity(tags_count as usize);
            for _ in 0..tags_count {
                let (len, size) = decode_varint(&bytes[offset..])?;
                offset += size;
                let tag = String::from_utf8(bytes[offset..offset + len as usize].to_vec())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                offset += len as usize;
                tags.push(tag);
            }

            (author, tags)
        } else {
            // v1: No author/tags
            (None, vec![])
        };

        docs.push(DocMeta {
            title,
            excerpt,
            href,
            doc_type: doc_type.to_string(),
            section_id,
            category,
            author,
            tags,
        });
    }

    Ok(docs)
}

// ============================================================================
// RUNTIME INDEX (for searching)
// ============================================================================

/// Document metadata (loaded from docs binary section)
#[derive(Debug, Clone)]
pub struct DocMeta {
    pub title: String,
    pub excerpt: String,
    pub href: String,
    /// "page" or "post"
    pub doc_type: String,
    /// Section ID for deep linking (e.g., "introduction", "performance-optimization")
    /// None for title matches (link to top of page), Some for heading/content matches
    pub section_id: Option<String>,
    /// Category for client-side filtering (e.g., "engineering", "adventures")
    pub category: Option<String>,
    /// Author name (for multi-author blogs)
    pub author: Option<String>,
    /// Tags/labels for categorization
    pub tags: Vec<String>,
}

/// Loaded binary layer ready for searching
#[derive(Debug)]
pub struct LoadedLayer {
    pub doc_count: usize,
    pub vocabulary: Vec<String>,
    pub suffix_array: Vec<(u32, u32)>,
    /// Posting lists with section_id indices (v6)
    pub postings: Vec<Vec<PostingEntry>>,
    /// Section ID string table (v6)
    pub section_table: Vec<String>,
    pub skip_lists: HashMap<usize, SkipList>,
    /// Levenshtein DFA bytes (precomputed parametric automaton)
    pub lev_dfa_bytes: Vec<u8>,
    /// Document metadata (embedded in binary)
    pub docs: Vec<DocMeta>,
    /// Dictionary tables for Parquet-style compression (v7)
    pub dict_tables: DictTables,
    /// Embedded WASM binary (v7)
    pub wasm_bytes: Vec<u8>,
}

impl LoadedLayer {
    /// Load from binary bytes (with full validation)
    ///
    /// Validates CRC32 checksum, header fields, and section boundaries.
    /// Returns detailed error messages for any corruption detected.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        let layer = BinaryLayer::from_bytes(bytes)?;

        // Decode vocabulary (front-compressed)
        let vocabulary = decode_vocabulary(&layer.vocab_bytes, layer.header.term_count as usize)?;

        // Decode suffix array (separated streams for brotli compression)
        let (suffix_array, _) = decode_suffix_array(&layer.sa_bytes)?;

        // Decode section table (v6)
        let (section_table, _) = decode_section_table(&layer.section_table_bytes)?;

        // Decode postings (delta+varint encoding)
        let mut postings = Vec::with_capacity(layer.header.term_count as usize);
        let mut pos = 0;
        let mut term_idx = 0;
        while pos < layer.postings_bytes.len() {
            let (posting_list, consumed) = decode_postings(&layer.postings_bytes[pos..])
                .map_err(|e| {
                    io::Error::new(
                        e.kind(),
                        format!("Error decoding term {} postings: {}", term_idx, e),
                    )
                })?;

            postings.push(posting_list);
            pos += consumed;
            term_idx += 1;
        }

        // Validate term count matches header
        if postings.len() != layer.header.term_count as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Term count mismatch: header says {}, got {} posting lists",
                    layer.header.term_count,
                    postings.len()
                ),
            ));
        }

        // Decode skip lists
        let mut skip_lists = HashMap::new();
        if layer.header.flags.has_skip_lists() {
            let mut pos = 0;
            while pos < layer.skip_bytes.len() {
                let (term_ord, consumed) = decode_varint(&layer.skip_bytes[pos..])?;
                pos += consumed;

                if pos >= layer.skip_bytes.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        format!("Truncated skip list for term {}", term_ord),
                    ));
                }

                let (skip_list, consumed) = SkipList::decode(&layer.skip_bytes[pos..])?;
                pos += consumed;
                skip_lists.insert(term_ord as usize, skip_list);
            }
        }

        // Decode docs
        let docs = decode_docs_binary(&layer.docs_bytes)?;

        // Decode dictionary tables (v7)
        let dict_tables = if !layer.dict_table_bytes.is_empty() {
            let (tables, _) = DictTables::decode(&layer.dict_table_bytes)?;
            tables
        } else {
            DictTables::default()
        };

        Ok(Self {
            doc_count: layer.header.doc_count as usize,
            vocabulary,
            suffix_array,
            postings,
            section_table,
            skip_lists,
            lev_dfa_bytes: layer.lev_dfa_bytes,
            docs,
            dict_tables,
            wasm_bytes: layer.wasm_bytes,
        })
    }

    /// Look up a term and get its posting list (uses binary search over vocabulary)
    pub fn get_postings(&self, term: &str) -> Option<&[PostingEntry]> {
        // Binary search since vocabulary is sorted
        let term_ord = self
            .vocabulary
            .binary_search_by(|t| t.as_str().cmp(term))
            .ok()?;
        self.postings.get(term_ord).map(|v| v.as_slice())
    }

    /// Look up a term and get just doc_ids (for skip list compatibility)
    pub fn get_doc_ids(&self, term: &str) -> Option<Vec<u32>> {
        self.get_postings(term)
            .map(|entries| entries.iter().map(|e| e.doc_id).collect())
    }

    /// Resolve section_idx to section_id string
    pub fn resolve_section_id(&self, section_idx: u32) -> Option<&str> {
        if section_idx == 0 {
            None // 0 means no section_id
        } else {
            // 1-indexed into section_table
            self.section_table
                .get((section_idx - 1) as usize)
                .map(|s| s.as_str())
        }
    }

    /// Get term by ordinal (for suffix array lookups)
    pub fn get_term(&self, ord: usize) -> Option<String> {
        self.vocabulary.get(ord).cloned()
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fuzzy::dfa::ParametricDFA;

    /// Helper to build Levenshtein DFA bytes (for tests)
    fn build_lev_dfa_bytes() -> Vec<u8> {
        ParametricDFA::build(true).to_bytes()
    }

    #[test]
    fn test_varint_roundtrip() {
        let values = [0, 1, 127, 128, 255, 256, 16383, 16384, u64::MAX];
        for &val in &values {
            let mut buf = Vec::new();
            encode_varint(val, &mut buf);
            let (decoded, len) = decode_varint(&buf).unwrap();
            assert_eq!(decoded, val);
            assert_eq!(len, buf.len());
        }
    }

    #[test]
    fn test_varint_error_on_empty() {
        let result = decode_varint(&[]);
        assert!(result.is_err());
    }

    #[test]
    fn test_varint_error_on_overflow() {
        // Create a malformed varint that never terminates (all continuation bits set)
        let malformed = [0xFF; 15];
        let result = decode_varint(&malformed);
        assert!(result.is_err());
    }

    #[test]
    fn test_postings_roundtrip() {
        // Create PostingEntry with unique doc_ids (delta encoding requires sorted)
        let entries: Vec<PostingEntry> = (0..500)
            .map(|i| PostingEntry {
                doc_id: i * 3,
                section_idx: if i % 10 == 0 { i / 10 } else { 0 },
                heading_level: (i % 6) as u8,
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

    #[test]
    fn test_suffix_array_roundtrip() {
        let entries: Vec<(u32, u32)> = vec![(0, 5), (0, 3), (1, 0), (2, 2), (5, 1)];
        let mut buf = Vec::new();
        encode_suffix_array(&entries, &mut buf);
        let (decoded, _) = decode_suffix_array(&buf).unwrap();
        assert_eq!(decoded, entries);
    }

    #[test]
    fn test_skip_list_build() {
        let doc_ids: Vec<u32> = (0..2000).collect();
        let skip_list = SkipList::build(&doc_ids).expect("Should build skip list");
        assert!(!skip_list.levels.is_empty());

        // Test skip_to
        let block = skip_list.skip_to(1500).expect("Should find block");
        assert!(block * BLOCK_SIZE as u32 <= 1500);
    }

    #[test]
    fn test_binary_layer_roundtrip() {
        let vocabulary = vec![
            "apple".to_string(),
            "banana".to_string(),
            "cherry".to_string(),
        ];
        let suffix_array = vec![(0, 0), (1, 0), (2, 0)];
        let postings = vec![vec![0, 5, 10], vec![1, 2, 3, 4], vec![0, 1, 2]];
        let lev_dfa_bytes = build_lev_dfa_bytes();

        // Create docs for embedding
        let docs = vec![
            DocMetaInput {
                title: "About Harry".to_string(),
                excerpt: "I'm a performance engineer...".to_string(),
                href: "/about".to_string(),
                doc_type: "page".to_string(),
                section_id: None,
                category: None,
                author: None,
                tags: vec![],
            },
            DocMetaInput {
                title: "Test Post".to_string(),
                excerpt: "A test post excerpt".to_string(),
                href: "/posts/2024/01/test".to_string(),
                doc_type: "post".to_string(),
                section_id: Some("introduction".to_string()),
                category: Some("engineering".to_string()),
                author: None,
                tags: vec![],
            },
        ];
        let docs_bytes = encode_docs_binary(&docs);

        let layer = BinaryLayer::build(
            &vocabulary,
            &suffix_array,
            &postings,
            20,
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();
        let bytes = layer.to_bytes().unwrap();

        let loaded = LoadedLayer::from_bytes(&bytes).unwrap();

        assert_eq!(loaded.doc_count, 20);
        assert_eq!(loaded.postings.len(), 3);
        // Use get_doc_ids since get_postings now returns PostingEntry
        assert_eq!(loaded.get_doc_ids("apple"), Some(vec![0, 5, 10]));
        assert_eq!(loaded.get_doc_ids("banana"), Some(vec![1, 2, 3, 4]));
        assert_eq!(loaded.get_doc_ids("cherry"), Some(vec![0, 1, 2]));
        assert_eq!(loaded.get_doc_ids("nonexistent"), None);
        assert!(
            !loaded.lev_dfa_bytes.is_empty(),
            "Levenshtein DFA bytes should be loaded"
        );

        // Verify docs roundtrip
        assert_eq!(loaded.docs.len(), 2);
        assert_eq!(loaded.docs[0].title, "About Harry");
        assert_eq!(loaded.docs[0].excerpt, "I'm a performance engineer...");
        assert_eq!(loaded.docs[0].href, "/about");
        assert_eq!(loaded.docs[0].doc_type, "page");
        assert_eq!(loaded.docs[1].title, "Test Post");
        assert_eq!(loaded.docs[1].doc_type, "post");
    }

    #[test]
    fn test_v6_section_table_roundtrip() {
        let vocabulary = vec!["test".to_string()];
        let suffix_array = vec![(0, 0), (0, 1), (0, 2)];
        let section_table = vec!["introduction".to_string(), "conclusion".to_string()];

        // Create postings with section_ids - each entry has unique doc_id
        // (v10 Elias-Fano requires strictly increasing doc_ids)
        let postings = vec![vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
                heading_level: 0,
            }, // No section (title)
            PostingEntry {
                doc_id: 1,
                section_idx: 1,
                heading_level: 1,
            }, // "introduction"
            PostingEntry {
                doc_id: 2,
                section_idx: 2,
                heading_level: 2,
            }, // "conclusion"
        ]];

        let lev_dfa_bytes = build_lev_dfa_bytes();
        let docs_bytes = encode_docs_binary(&[]);

        let layer = BinaryLayer::build_v6(
            &vocabulary,
            &suffix_array,
            &postings,
            &section_table,
            3, // 3 documents now
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();
        let bytes = layer.to_bytes().unwrap();

        let loaded = LoadedLayer::from_bytes(&bytes).unwrap();

        // Verify section table
        assert_eq!(loaded.section_table.len(), 2);
        assert_eq!(loaded.section_table[0], "introduction");
        assert_eq!(loaded.section_table[1], "conclusion");

        // Verify postings with section_ids (sorted by doc_id in v10)
        let entries = loaded.get_postings("test").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].doc_id, 0);
        assert_eq!(entries[0].section_idx, 0);
        assert_eq!(entries[1].doc_id, 1);
        assert_eq!(entries[1].section_idx, 1);
        assert_eq!(entries[2].doc_id, 2);
        assert_eq!(entries[2].section_idx, 2);

        // Verify section resolution
        assert_eq!(loaded.resolve_section_id(0), None);
        assert_eq!(loaded.resolve_section_id(1), Some("introduction"));
        assert_eq!(loaded.resolve_section_id(2), Some("conclusion"));
    }

    #[test]
    fn test_header_roundtrip() {
        let header = SorexHeader {
            version: VERSION,
            flags: FormatFlags::new().with_skip_lists(),
            doc_count: 1000,
            term_count: 500,
            vocab_len: 2048,
            sa_len: 4096,
            postings_len: 8192,
            skip_len: 512,
            section_table_len: 256, // v6: section_id table
            lev_dfa_len: 1200,
            docs_len: 5000,
            wasm_len: 50000,      // v7: embedded WASM
            dict_table_len: 1024, // v7: dictionary tables
        };

        let mut buf = Vec::new();
        header.write(&mut buf).unwrap();
        assert_eq!(buf.len(), SorexHeader::SIZE);

        let decoded = SorexHeader::read(&mut io::Cursor::new(&buf)).unwrap();
        assert_eq!(decoded.version, header.version);
        assert_eq!(decoded.doc_count, header.doc_count);
        assert_eq!(decoded.term_count, header.term_count);
        assert_eq!(decoded.section_table_len, header.section_table_len);
        assert_eq!(decoded.lev_dfa_len, header.lev_dfa_len);
        assert_eq!(decoded.docs_len, header.docs_len);
        assert_eq!(decoded.wasm_len, header.wasm_len);
        assert_eq!(decoded.dict_table_len, header.dict_table_len);
        assert!(decoded.flags.has_skip_lists());
    }

    #[test]
    fn test_crc32_detects_corruption() {
        // Create valid layer
        let vocabulary = vec!["test".to_string()];
        let suffix_array = vec![(0, 0)];
        let postings = vec![vec![0, 1, 2]];
        let lev_dfa_bytes = build_lev_dfa_bytes();
        let docs_bytes = encode_docs_binary(&[]);

        let layer = BinaryLayer::build(
            &vocabulary,
            &suffix_array,
            &postings,
            10,
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();
        let mut bytes = layer.to_bytes().unwrap();

        // Corrupt a byte in the middle (not the CRC itself)
        let corruption_idx = SorexHeader::SIZE + 5;
        bytes[corruption_idx] ^= 0xFF;

        // Should fail CRC check
        let result = LoadedLayer::from_bytes(&bytes);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("CRC32 mismatch"));
    }

    #[test]
    fn test_truncation_detected() {
        // Create valid layer
        let vocabulary = vec!["test".to_string()];
        let suffix_array = vec![(0, 0)];
        let postings = vec![vec![0, 1, 2]];
        let lev_dfa_bytes = build_lev_dfa_bytes();
        let docs_bytes = encode_docs_binary(&[]);

        let layer = BinaryLayer::build(
            &vocabulary,
            &suffix_array,
            &postings,
            10,
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();
        let bytes = layer.to_bytes().unwrap();

        // Truncate the file
        let truncated = &bytes[..bytes.len() - 10];

        // Should fail (either footer magic missing or CRC mismatch)
        let result = LoadedLayer::from_bytes(truncated);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_magic_rejected() {
        let mut bytes = vec![0u8; SorexHeader::SIZE + SorexFooter::SIZE];
        // Wrong header magic
        bytes[0..4].copy_from_slice(b"NOPE");
        // Valid footer magic at the end
        let footer_start = bytes.len() - 4;
        bytes[footer_start..].copy_from_slice(&FOOTER_MAGIC);

        let result = BinaryLayer::from_bytes(&bytes);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // The validation order is: footer magic → CRC32 → header magic
        // Since CRC32 will fail before we check header magic, just verify it errors
        assert!(
            err.to_string().contains("CRC32")
                || err.to_string().contains("Invalid magic")
                || err.to_string().contains("mismatch"),
            "Expected CRC32 or magic error, got: {}",
            err
        );
    }

    #[test]
    fn test_footer_magic_validated() {
        // Create bytes that are too small
        let small = vec![0u8; 4];
        let result = SorexFooter::read(&small);
        assert!(result.is_err());
    }

    #[test]
    fn test_footer_roundtrip() {
        let data = b"Hello, world!";
        let crc32 = SorexFooter::compute_crc32(data);

        let footer = SorexFooter { crc32 };
        let mut buf = Vec::new();
        footer.write(&mut buf).unwrap();
        assert_eq!(buf.len(), SorexFooter::SIZE);

        // Append footer to data and verify
        let mut full = data.to_vec();
        full.extend_from_slice(&buf);

        let parsed = SorexFooter::read(&full).unwrap();
        assert_eq!(parsed.crc32, crc32);
    }

    #[test]
    fn test_v7_wasm_embedding() {
        // Create a minimal index with embedded WASM
        let vocabulary = vec!["test".to_string()];
        let suffix_array = vec![(0, 0)];
        let postings = vec![vec![PostingEntry {
            doc_id: 0,
            section_idx: 0,
            heading_level: 0,
        }]];
        let lev_dfa_bytes = build_lev_dfa_bytes();
        let docs_bytes = encode_docs_binary(&[]);

        // Simulate WASM bytes (just some recognizable pattern)
        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]; // WASM magic

        let layer = BinaryLayer::build_v7(
            &vocabulary,
            &suffix_array,
            &postings,
            &[],
            1,
            lev_dfa_bytes,
            docs_bytes,
            wasm_bytes.clone(),
        )
        .unwrap();

        let bytes = layer.to_bytes().unwrap();

        // Verify header wasm_len is correct
        let header = SorexHeader::read(&mut &bytes[..]).unwrap();
        assert_eq!(header.version, VERSION);
        assert_eq!(header.wasm_len, 8); // Our test WASM bytes

        // Verify loaded layer has correct wasm_bytes
        let loaded = LoadedLayer::from_bytes(&bytes).unwrap();
        assert_eq!(loaded.wasm_bytes, wasm_bytes);
    }

    #[test]
    fn test_v7_wasm_offset_calculation() {
        // Test that WASM bytes are at the correct offset
        let vocabulary = vec!["apple".to_string(), "banana".to_string()];
        let suffix_array = vec![(0, 0), (1, 0)];
        let postings = vec![
            vec![
                PostingEntry { doc_id: 0, section_idx: 0, heading_level: 0 },
                PostingEntry { doc_id: 1, section_idx: 0, heading_level: 0 },
            ],
            vec![
                PostingEntry { doc_id: 1, section_idx: 0, heading_level: 0 },
                PostingEntry { doc_id: 2, section_idx: 0, heading_level: 0 },
            ],
        ];
        let lev_dfa_bytes = build_lev_dfa_bytes();

        let docs = vec![DocMetaInput {
            title: "Test".to_string(),
            excerpt: "Test excerpt".to_string(),
            href: "/test".to_string(),
            doc_type: "page".to_string(),
            section_id: None,
            category: None,
            author: None,
            tags: vec![],
        }];
        let docs_bytes = encode_docs_binary(&docs);

        // Create recognizable WASM bytes
        let wasm_bytes: Vec<u8> = (0..100).map(|i| (i * 7) as u8).collect();

        let layer = BinaryLayer::build_v7(
            &vocabulary,
            &suffix_array,
            &postings,
            &[],
            3,
            lev_dfa_bytes,
            docs_bytes,
            wasm_bytes.clone(),
        )
        .unwrap();

        let bytes = layer.to_bytes().unwrap();
        let header = SorexHeader::read(&mut &bytes[..]).unwrap();
        let offsets = header.section_offsets();

        // Verify WASM bytes are at the correct offset using SectionOffsets
        let extracted_wasm = &bytes[offsets.wasm.0..offsets.wasm.1];
        assert_eq!(extracted_wasm, &wasm_bytes[..]);
    }

    #[test]
    fn test_v7_dict_tables_roundtrip() {
        use crate::util::dict_table::DictTables;

        // Build dictionary tables with sample data
        let mut dict_tables = DictTables::new();
        dict_tables.category.insert("engineering");
        dict_tables.category.insert("adventures");
        dict_tables.author.insert("Harry");
        dict_tables.author.insert("Guest Author");
        dict_tables.tags.insert("rust");
        dict_tables.tags.insert("wasm");
        dict_tables.tags.insert("search");
        dict_tables.href_prefix.insert("/posts/2024/");
        dict_tables.href_prefix.insert("/posts/2025/");

        // Encode dict tables
        let mut dict_table_bytes = Vec::new();
        dict_tables.encode(&mut dict_table_bytes);

        // Create minimal index with dict_tables
        let vocabulary = vec!["test".to_string()];
        let suffix_array = vec![(0, 0)];
        let postings = vec![vec![PostingEntry {
            doc_id: 0,
            section_idx: 0,
            heading_level: 0,
        }]];
        let lev_dfa_bytes = build_lev_dfa_bytes();

        let docs = vec![DocMetaInput {
            title: "Test Doc".to_string(),
            excerpt: "Test excerpt".to_string(),
            href: "/posts/2024/test".to_string(),
            doc_type: "post".to_string(),
            section_id: None,
            category: Some("engineering".to_string()),
            author: Some("Harry".to_string()),
            tags: vec!["rust".to_string(), "wasm".to_string()],
        }];
        let docs_bytes = encode_docs_binary(&docs);

        // Build layer with dict_tables
        let mut layer = BinaryLayer::build_v6(
            &vocabulary,
            &suffix_array,
            &postings,
            &[],
            1,
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();

        // Add dict_tables to layer
        layer.header.dict_table_len = dict_table_bytes.len() as u32;
        layer.dict_table_bytes = dict_table_bytes;

        // Roundtrip through serialization
        let bytes = layer.to_bytes().unwrap();
        let loaded = LoadedLayer::from_bytes(&bytes).unwrap();

        // Verify dict_tables were loaded correctly
        assert_eq!(loaded.dict_tables.category.len(), 2);
        assert_eq!(loaded.dict_tables.category.get(0), Some("engineering"));
        assert_eq!(loaded.dict_tables.category.get(1), Some("adventures"));

        assert_eq!(loaded.dict_tables.author.len(), 2);
        assert_eq!(loaded.dict_tables.author.get(0), Some("Harry"));
        assert_eq!(loaded.dict_tables.author.get(1), Some("Guest Author"));

        assert_eq!(loaded.dict_tables.tags.len(), 3);
        assert_eq!(loaded.dict_tables.tags.get(0), Some("rust"));
        assert_eq!(loaded.dict_tables.tags.get(1), Some("wasm"));
        assert_eq!(loaded.dict_tables.tags.get(2), Some("search"));

        assert_eq!(loaded.dict_tables.href_prefix.len(), 2);
        assert_eq!(loaded.dict_tables.href_prefix.get(0), Some("/posts/2024/"));
        assert_eq!(loaded.dict_tables.href_prefix.get(1), Some("/posts/2025/"));
    }

    #[test]
    fn test_empty_dict_tables_roundtrip() {
        // Create index without dict_tables (empty)
        let vocabulary = vec!["test".to_string()];
        let suffix_array = vec![(0, 0)];
        let postings = vec![vec![PostingEntry {
            doc_id: 0,
            section_idx: 0,
            heading_level: 0,
        }]];
        let lev_dfa_bytes = build_lev_dfa_bytes();

        let docs = vec![DocMetaInput {
            title: "Test".to_string(),
            excerpt: "Test".to_string(),
            href: "/test".to_string(),
            doc_type: "page".to_string(),
            section_id: None,
            category: None,
            author: None,
            tags: vec![],
        }];
        let docs_bytes = encode_docs_binary(&docs);

        // Build layer with no dict_tables (empty bytes)
        let layer = BinaryLayer::build_v6(
            &vocabulary,
            &suffix_array,
            &postings,
            &[],
            1,
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();

        let bytes = layer.to_bytes().unwrap();
        let loaded = LoadedLayer::from_bytes(&bytes).unwrap();

        // Should have empty dict_tables
        assert!(loaded.dict_tables.is_empty());
        assert_eq!(loaded.dict_tables.total_entries(), 0);
    }
}
