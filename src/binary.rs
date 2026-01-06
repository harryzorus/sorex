//! Binary format for Sieve search indexes.
//!
//! Lucene-inspired binary format with:
//! - Vocabulary (sorted, length-prefixed terms)
//! - Block PFOR postings (128-doc blocks)
//! - Skip lists for large posting lists
//! - CRC32 integrity validation
//! - Embedded WASM runtime (v7)
//!
//! # Security Considerations
//!
//! This format is designed to be safely parsed from untrusted sources:
//! - All size fields are validated against MAX_* constants
//! - Bounds checking prevents buffer overreads
//! - CRC32 footer detects corruption/truncation
//! - Varint decoder has maximum iteration limits
//!
//! # Format Overview (v7)
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────┐
//! │ HEADER (52 bytes)                                          │
//! │   magic: [u8; 4] = "SIFT"                                  │
//! │   version: u8 = 7                                          │
//! │   flags: u8                                                │
//! │   doc_count: u32                                           │
//! │   term_count: u32                                          │
//! │   vocab_len: u32                                           │
//! │   sa_len: u32                                              │
//! │   postings_len: u32                                        │
//! │   skip_len: u32                                            │
//! │   section_table_len: u32 (v6: section_id string table)     │
//! │   lev_dfa_len: u32                                         │
//! │   docs_len: u32 (embedded binary docs)                     │
//! │   wasm_len: u32 (v7: embedded WASM runtime)                │
//! │   dict_table_len: u32 (v7: dictionary tables)              │
//! │   reserved: [u8; 2]                                        │
//! ├────────────────────────────────────────────────────────────┤
//! │ VOCABULARY SECTION (sorted, length-prefixed terms)         │
//! │   For each term:                                           │
//! │     len: varint (byte length of term)                      │
//! │     term: [u8; len] (UTF-8 bytes)                          │
//! ├────────────────────────────────────────────────────────────┤
//! │ SUFFIX ARRAY SECTION                                       │
//! │   count: varint                                            │
//! │   FOR-encoded [term_ord, char_offset] pairs                │
//! ├────────────────────────────────────────────────────────────┤
//! │ POSTINGS SECTION (block PFOR, 128-doc blocks)              │
//! │   For each term (in vocab order):                          │
//! │     doc_freq: varint                                       │
//! │     num_blocks: varint                                     │
//! │     For each full block (128 docs):                        │
//! │       min_delta: varint (frame of reference)               │
//! │       bits_per_value: u8                                   │
//! │       packed_data: [u8; 128 * bits / 8]                    │
//! │     tail_count: varint                                     │
//! │     tail_docs: [varint; tail_count]                        │
//! ├────────────────────────────────────────────────────────────┤
//! │ SKIP LIST SECTION (for terms with >1024 docs)              │
//! │   For each term with skip list:                            │
//! │     term_ord: varint                                       │
//! │     num_levels: u8                                         │
//! │     For each level:                                        │
//! │       num_skips: varint                                    │
//! │       [doc_id, block_offset] pairs                         │
//! ├────────────────────────────────────────────────────────────┤
//! │ SECTION TABLE (v6: for deep linking)                       │
//! │   count: varint                                            │
//! │   For each section_id:                                     │
//! │     len: varint                                            │
//! │     id: [u8; len] (UTF-8 bytes)                            │
//! ├────────────────────────────────────────────────────────────┤
//! │ LEVENSHTEIN DFA SECTION (precomputed Schulz-Mihov tables)  │
//! │   Parametric automaton for fuzzy matching (k=2)            │
//! ├────────────────────────────────────────────────────────────┤
//! │ DOCS SECTION (embedded document metadata, binary)          │
//! │   count: varint                                            │
//! │   For each doc:                                            │
//! │     type: u8 (0=page, 1=post)                              │
//! │     title: varint_len + utf8                               │
//! │     excerpt: varint_len + utf8                             │
//! │     href: varint_len + utf8                                │
//! │     category: varint_len + utf8 (empty if none)            │
//! ├────────────────────────────────────────────────────────────┤
//! │ WASM SECTION (v7: embedded WebAssembly runtime)            │
//! │   Raw WASM binary (sieve_bg.wasm)                          │
//! ├────────────────────────────────────────────────────────────┤
//! │ DICTIONARY TABLES (v7: Parquet-style compression)          │
//! │   num_tables: u8 (4)                                       │
//! │   category_table: varint(count) + strings                  │
//! │   author_table: varint(count) + strings                    │
//! │   tags_table: varint(count) + strings                      │
//! │   href_prefix_table: varint(count) + strings               │
//! ├────────────────────────────────────────────────────────────┤
//! │ FOOTER (8 bytes)                                           │
//! │   crc32: u32 (over header + all sections)                  │
//! │   magic: [u8; 4] = "TFIS" (reversed, marks valid end)      │
//! └────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;
use std::io::{self, Read, Write};

use crc32fast::Hasher as Crc32Hasher;

use crate::dict_table::DictTables;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Magic bytes: "SIEVE" in ASCII (header)
pub const MAGIC: [u8; 4] = [0x53, 0x49, 0x46, 0x54];

/// Footer magic: "TFIS" (reversed, marks valid file end)
pub const FOOTER_MAGIC: [u8; 4] = [0x54, 0x46, 0x49, 0x53];

/// Current format version (v7 adds embedded WASM)
pub const VERSION: u8 = 7;

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
pub struct FormatFlags(u8);

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
pub struct SieveHeader {
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

impl SieveHeader {
    // 4 (magic) + 1 (version) + 1 (flags) + 11*4 (u32s) + 2 (reserved) = 52
    pub const SIZE: usize = 52;

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
                format!("Invalid magic: expected SIEVE, got {:?}", magic),
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
pub struct SieveFooter {
    /// CRC32 checksum of header + all sections (everything before footer)
    pub crc32: u32,
}

impl SieveFooter {
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

// ============================================================================
// VARINT ENCODING
// ============================================================================

/// Encode a varint to bytes
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

/// Decode a varint from bytes, returning (value, bytes_consumed)
///
/// Returns an error if:
/// - Buffer is empty
/// - Varint exceeds MAX_VARINT_BYTES (malformed/malicious input)
pub fn decode_varint(bytes: &[u8]) -> io::Result<(u64, usize)> {
    if bytes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Empty buffer for varint",
        ));
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
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Varint exceeds maximum length (possible corruption)",
        ))
    } else {
        Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Incomplete varint",
        ))
    }
}

// ============================================================================
// BLOCK PFOR ENCODING
// ============================================================================

/// Calculate bits needed to represent max value
fn bits_needed(max_value: u32) -> u8 {
    if max_value == 0 {
        return 0;
    }
    32 - max_value.leading_zeros() as u8
}

/// Encode a block of 128 deltas using PFOR
pub fn encode_block_pfor(deltas: &[u32; BLOCK_SIZE], buf: &mut Vec<u8>) {
    // Find min and max for frame of reference
    let min_delta = *deltas.iter().min().unwrap();
    let adjusted: Vec<u32> = deltas.iter().map(|&d| d - min_delta).collect();
    let max_adjusted = *adjusted.iter().max().unwrap();

    let bits = bits_needed(max_adjusted);

    // Write min delta (frame of reference)
    encode_varint(min_delta as u64, buf);

    // Write bits per value
    buf.push(bits);

    if bits == 0 {
        // All values are the same, nothing more to write
        return;
    }

    // Bit-pack the values
    let bytes_needed = (BLOCK_SIZE * bits as usize).div_ceil(8);
    let start = buf.len();
    buf.resize(start + bytes_needed, 0);

    let mut bit_pos = 0;
    for &val in &adjusted {
        let byte_idx = bit_pos / 8;
        let bit_offset = bit_pos % 8;

        // Write value across byte boundaries
        let mut remaining_bits = bits as usize;
        let mut remaining_val = val as u64;
        let mut current_byte = byte_idx;
        let mut current_offset = bit_offset;

        while remaining_bits > 0 {
            let bits_in_byte = (8 - current_offset).min(remaining_bits);
            let mask = ((1u64 << bits_in_byte) - 1) as u8;
            buf[start + current_byte] |= ((remaining_val as u8) & mask) << current_offset;
            remaining_val >>= bits_in_byte;
            remaining_bits -= bits_in_byte;
            current_byte += 1;
            current_offset = 0;
        }

        bit_pos += bits as usize;
    }
}

/// Decode a block of 128 deltas from PFOR
pub fn decode_block_pfor(bytes: &[u8]) -> io::Result<(Vec<u32>, usize)> {
    let (min_delta, varint_len) = decode_varint(bytes)?;
    let min_delta = min_delta as u32;
    let mut pos = varint_len;

    if pos >= bytes.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Truncated PFOR block",
        ));
    }

    let bits = bytes[pos];
    pos += 1;

    // Validate bits_per_value is reasonable (max 32 bits for u32)
    if bits > 32 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Invalid bits_per_value: {} (max 32)", bits),
        ));
    }

    if bits == 0 {
        // All values are min_delta
        return Ok((vec![min_delta; BLOCK_SIZE], pos));
    }

    // Check we have enough bytes for the packed data
    let bytes_needed = (BLOCK_SIZE * bits as usize).div_ceil(8);
    if pos + bytes_needed > bytes.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Truncated PFOR packed data",
        ));
    }

    let mut result = Vec::with_capacity(BLOCK_SIZE);
    let mut bit_pos = 0;
    let data = &bytes[pos..];

    for _ in 0..BLOCK_SIZE {
        let byte_idx = bit_pos / 8;
        let bit_offset = bit_pos % 8;

        // Read value across byte boundaries
        let mut val: u64 = 0;
        let mut remaining_bits = bits as usize;
        let mut current_byte = byte_idx;
        let mut current_offset = bit_offset;
        let mut val_offset = 0;

        while remaining_bits > 0 && current_byte < data.len() {
            let bits_in_byte = (8 - current_offset).min(remaining_bits);
            let mask = ((1u64 << bits_in_byte) - 1) as u8;
            val |= (((data[current_byte] >> current_offset) & mask) as u64) << val_offset;
            val_offset += bits_in_byte;
            remaining_bits -= bits_in_byte;
            current_byte += 1;
            current_offset = 0;
        }

        result.push(min_delta + val as u32);
        bit_pos += bits as usize;
    }

    let bytes_consumed = pos + bytes_needed;
    Ok((result, bytes_consumed))
}

// ============================================================================
// SECTION TABLE ENCODING (v6)
// ============================================================================

/// Encode section ID string table (deduplicated, length-prefixed)
pub fn encode_section_table(section_ids: &[String], buf: &mut Vec<u8>) {
    encode_varint(section_ids.len() as u64, buf);
    for id in section_ids {
        let bytes = id.as_bytes();
        encode_varint(bytes.len() as u64, buf);
        buf.extend_from_slice(bytes);
    }
}

/// Decode section ID string table
pub fn decode_section_table(bytes: &[u8]) -> io::Result<(Vec<String>, usize)> {
    if bytes.is_empty() {
        return Ok((Vec::new(), 0));
    }

    let (count, mut pos) = decode_varint(bytes)?;
    let count = count as usize;
    let mut table = Vec::with_capacity(count);

    for i in 0..count {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Truncated section table at entry {}", i),
            ));
        }
        let (len, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;

        let len = len as usize;
        if pos + len > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Truncated section ID {} (expected {} bytes)", i, len),
            ));
        }

        let id = String::from_utf8(bytes[pos..pos + len].to_vec()).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 in section ID {}: {}", i, e),
            )
        })?;
        table.push(id);
        pos += len;
    }

    Ok((table, pos))
}

// ============================================================================
// POSTINGS ENCODER
// ============================================================================

/// Posting entry with doc_id and optional section_id index
#[derive(Debug, Clone)]
pub struct PostingEntry {
    pub doc_id: u32,
    /// Index into section table (0 = no section_id, 1+ = table index + 1)
    pub section_idx: u32,
}

/// Encode posting list with block PFOR (v6: includes section_id indices)
pub fn encode_postings_v6(entries: &[PostingEntry], buf: &mut Vec<u8>) {
    let doc_freq = entries.len();
    encode_varint(doc_freq as u64, buf);

    if doc_freq == 0 {
        return;
    }

    // Extract doc_ids and convert to deltas
    let doc_ids: Vec<u32> = entries.iter().map(|e| e.doc_id).collect();
    let mut deltas: Vec<u32> = Vec::with_capacity(doc_freq);
    let mut prev = 0u32;
    for &doc_id in &doc_ids {
        deltas.push(doc_id - prev);
        prev = doc_id;
    }

    // Encode doc_id deltas using block PFOR
    let num_blocks = deltas.len() / BLOCK_SIZE;
    encode_varint(num_blocks as u64, buf);

    for block_idx in 0..num_blocks {
        let start = block_idx * BLOCK_SIZE;
        let block: [u32; BLOCK_SIZE] = deltas[start..start + BLOCK_SIZE].try_into().unwrap();
        encode_block_pfor(&block, buf);
    }

    // Encode tail (remaining docs as varints)
    let tail_start = num_blocks * BLOCK_SIZE;
    let tail_count = deltas.len() - tail_start;
    encode_varint(tail_count as u64, buf);

    for &delta in &deltas[tail_start..] {
        encode_varint(delta as u64, buf);
    }

    // v6: Encode section_id indices as varints (0 = none, 1+ = table index + 1)
    for entry in entries {
        encode_varint(entry.section_idx as u64, buf);
    }
}

/// Decode posting list with section_id indices (v6)
pub fn decode_postings_v6(bytes: &[u8]) -> io::Result<(Vec<PostingEntry>, usize)> {
    let (doc_freq, mut pos) = decode_varint(bytes)?;
    let doc_freq = doc_freq as usize;

    if doc_freq > MAX_POSTING_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Posting list too large: {} (max {})",
                doc_freq, MAX_POSTING_SIZE
            ),
        ));
    }

    if doc_freq == 0 {
        return Ok((Vec::new(), pos));
    }

    // Decode doc_ids
    let (num_blocks, consumed) = decode_varint(&bytes[pos..])?;
    pos += consumed;
    let num_blocks = num_blocks as usize;

    let mut doc_ids = Vec::with_capacity(doc_freq);
    let mut current_doc = 0u32;

    // Decode full blocks
    for _ in 0..num_blocks {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated postings blocks",
            ));
        }
        let (deltas, consumed) = decode_block_pfor(&bytes[pos..])?;
        pos += consumed;

        for delta in deltas {
            current_doc += delta;
            doc_ids.push(current_doc);
        }
    }

    // Decode tail
    let (tail_count, consumed) = decode_varint(&bytes[pos..])?;
    pos += consumed;

    for _ in 0..tail_count {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated postings tail values",
            ));
        }
        let (delta, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;
        current_doc += delta as u32;
        doc_ids.push(current_doc);
    }

    // v6: Decode section_id indices
    let mut entries = Vec::with_capacity(doc_freq);
    for (i, doc_id) in doc_ids.into_iter().enumerate() {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Truncated section_id index at posting {}", i),
            ));
        }
        let (section_idx, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;

        entries.push(PostingEntry {
            doc_id,
            section_idx: section_idx as u32,
        });
    }

    Ok((entries, pos))
}

/// Encode posting list with block PFOR (legacy v5 format - doc_ids only)
pub fn encode_postings(doc_ids: &[u32], buf: &mut Vec<u8>) {
    let doc_freq = doc_ids.len();
    encode_varint(doc_freq as u64, buf);

    if doc_freq == 0 {
        return;
    }

    // Convert to deltas
    let mut deltas: Vec<u32> = Vec::with_capacity(doc_freq);
    let mut prev = 0u32;
    for &doc_id in doc_ids {
        deltas.push(doc_id - prev);
        prev = doc_id;
    }

    // Encode full blocks
    let num_blocks = deltas.len() / BLOCK_SIZE;
    encode_varint(num_blocks as u64, buf);

    for block_idx in 0..num_blocks {
        let start = block_idx * BLOCK_SIZE;
        let block: [u32; BLOCK_SIZE] = deltas[start..start + BLOCK_SIZE].try_into().unwrap();
        encode_block_pfor(&block, buf);
    }

    // Encode tail (remaining docs as varints)
    let tail_start = num_blocks * BLOCK_SIZE;
    let tail_count = deltas.len() - tail_start;
    encode_varint(tail_count as u64, buf);

    for &delta in &deltas[tail_start..] {
        encode_varint(delta as u64, buf);
    }
}

/// Decode posting list from block PFOR
pub fn decode_postings(bytes: &[u8]) -> io::Result<(Vec<u32>, usize)> {
    let (doc_freq, mut pos) = decode_varint(bytes)?;
    let doc_freq = doc_freq as usize;

    // Validate doc_freq is reasonable
    if doc_freq > MAX_POSTING_SIZE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Posting list too large: {} (max {})",
                doc_freq, MAX_POSTING_SIZE
            ),
        ));
    }

    if doc_freq == 0 {
        return Ok((Vec::new(), pos));
    }

    if pos >= bytes.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Truncated postings",
        ));
    }

    let (num_blocks, consumed) = decode_varint(&bytes[pos..])?;
    pos += consumed;
    let num_blocks = num_blocks as usize;

    let mut doc_ids = Vec::with_capacity(doc_freq);
    let mut current_doc = 0u32;

    // Decode full blocks
    for _ in 0..num_blocks {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated postings blocks",
            ));
        }
        let (deltas, consumed) = decode_block_pfor(&bytes[pos..])?;
        pos += consumed;

        for delta in deltas {
            current_doc += delta;
            doc_ids.push(current_doc);
        }
    }

    // Decode tail
    if pos >= bytes.len() {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "Truncated postings tail",
        ));
    }
    let (tail_count, consumed) = decode_varint(&bytes[pos..])?;
    pos += consumed;
    let tail_count = tail_count as usize;

    for _ in 0..tail_count {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated postings tail values",
            ));
        }
        let (delta, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;
        current_doc += delta as u32;
        doc_ids.push(current_doc);
    }

    Ok((doc_ids, pos))
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

// ============================================================================
// SUFFIX ARRAY ENCODING
// ============================================================================

/// Encode suffix array (term_ord, offset pairs)
pub fn encode_suffix_array(entries: &[(u32, u32)], buf: &mut Vec<u8>) {
    encode_varint(entries.len() as u64, buf);

    // Interleave term_ord and offset, delta encode term_ord
    let mut prev_term = 0u32;
    for &(term_ord, offset) in entries {
        let delta = term_ord - prev_term;
        encode_varint(delta as u64, buf);
        encode_varint(offset as u64, buf);
        prev_term = term_ord;
    }
}

/// Decode suffix array
pub fn decode_suffix_array(bytes: &[u8]) -> io::Result<(Vec<(u32, u32)>, usize)> {
    let (count, mut pos) = decode_varint(bytes)?;
    let count = count as usize;

    // Validate count is reasonable
    if count > MAX_POSTING_SIZE * 10 {
        // Suffix array can be larger than postings (multiple suffixes per term)
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Suffix array too large: {}", count),
        ));
    }

    let mut result = Vec::with_capacity(count);
    let mut current_term = 0u32;

    for _ in 0..count {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated suffix array",
            ));
        }
        let (delta, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;
        current_term += delta as u32;

        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Truncated suffix array offset",
            ));
        }
        let (offset, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;

        result.push((current_term, offset as u32));
    }

    Ok((result, pos))
}

// ============================================================================
// FULL LAYER ENCODING
// ============================================================================

/// A complete binary layer
#[derive(Debug)]
pub struct BinaryLayer {
    pub header: SieveHeader,
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

/// Encode vocabulary as length-prefixed UTF-8 strings.
fn encode_vocabulary(vocabulary: &[String], out: &mut Vec<u8>) {
    for term in vocabulary {
        let bytes = term.as_bytes();
        encode_varint(bytes.len() as u64, out);
        out.extend_from_slice(bytes);
    }
}

/// Decode vocabulary from length-prefixed UTF-8 strings.
fn decode_vocabulary(bytes: &[u8], term_count: usize) -> io::Result<Vec<String>> {
    let mut terms = Vec::with_capacity(term_count);
    let mut pos = 0;

    for i in 0..term_count {
        if pos >= bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Truncated vocabulary at term {}", i),
            ));
        }

        let (len, consumed) = decode_varint(&bytes[pos..])?;
        pos += consumed;

        let len = len as usize;
        if pos + len > bytes.len() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!("Truncated term {} (expected {} bytes)", i, len),
            ));
        }

        let term = String::from_utf8(bytes[pos..pos + len].to_vec()).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 in term {}: {}", i, e),
            )
        })?;
        terms.push(term);
        pos += len;
    }

    Ok(terms)
}

impl BinaryLayer {
    /// Build a binary layer with section_ids and embedded WASM (v7 format)
    ///
    /// Postings include section_id indices for deep linking.
    /// The section_table contains unique section_id strings.
    /// The wasm_bytes contain the embedded WebAssembly runtime.
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
        // Encode vocabulary as length-prefixed strings
        let mut vocab_bytes = Vec::new();
        encode_vocabulary(vocabulary, &mut vocab_bytes);

        // Encode suffix array
        let mut sa_bytes = Vec::new();
        encode_suffix_array(suffix_array, &mut sa_bytes);

        // Encode postings with section_id indices (v6 format)
        let mut postings_bytes = Vec::new();
        for posting_list in postings {
            encode_postings_v6(posting_list, &mut postings_bytes);
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

        let header = SieveHeader {
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
            dict_table_len: 0, // TODO: Implement in Step 4
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
    pub fn to_bytes(&self) -> io::Result<Vec<u8>> {
        let content_size = SieveHeader::SIZE
            + self.vocab_bytes.len()
            + self.sa_bytes.len()
            + self.postings_bytes.len()
            + self.skip_bytes.len()
            + self.section_table_bytes.len()
            + self.lev_dfa_bytes.len()
            + self.docs_bytes.len()
            + self.wasm_bytes.len()
            + self.dict_table_bytes.len();
        let total_size = content_size + SieveFooter::SIZE;

        let mut buf = Vec::with_capacity(total_size);
        self.header.write(&mut buf)?;
        buf.extend_from_slice(&self.vocab_bytes);
        buf.extend_from_slice(&self.sa_bytes);
        buf.extend_from_slice(&self.postings_bytes);
        buf.extend_from_slice(&self.skip_bytes);
        buf.extend_from_slice(&self.section_table_bytes);
        buf.extend_from_slice(&self.lev_dfa_bytes);
        buf.extend_from_slice(&self.docs_bytes);
        buf.extend_from_slice(&self.wasm_bytes);
        buf.extend_from_slice(&self.dict_table_bytes);

        // Compute CRC32 over everything written so far
        let crc32 = SieveFooter::compute_crc32(&buf);
        let footer = SieveFooter { crc32 };
        footer.write(&mut buf)?;

        Ok(buf)
    }

    /// Deserialize from bytes (with CRC32 validation)
    ///
    /// # Validation
    ///
    /// This method performs comprehensive validation:
    /// 1. File size is within limits (MAX_FILE_SIZE)
    /// 2. Header magic is valid ("SIEVE")
    /// 3. Version is supported
    /// 4. Section lengths don't exceed file size
    /// 5. Footer magic is valid ("TFIS")
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
        let min_size = SieveHeader::SIZE + SieveFooter::SIZE;
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
        let footer = SieveFooter::read(bytes)?;
        let content = &bytes[..bytes.len() - SieveFooter::SIZE];
        let computed_crc32 = SieveFooter::compute_crc32(content);

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
        let header = SieveHeader::read(&mut cursor)?;

        // Validate version
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

        // Calculate expected content size and validate
        let expected_content_size = SieveHeader::SIZE
            + header.vocab_len as usize
            + header.sa_len as usize
            + header.postings_len as usize
            + header.skip_len as usize
            + header.section_table_len as usize // v6: section_id string table
            + header.lev_dfa_len as usize
            + header.docs_len as usize
            + header.wasm_len as usize // v7: embedded WASM
            + header.dict_table_len as usize; // v7: dictionary tables

        if expected_content_size != content.len() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Section lengths mismatch: header claims {} bytes, got {} bytes",
                    expected_content_size,
                    content.len()
                ),
            ));
        }

        // Extract sections with bounds checking
        let mut pos = SieveHeader::SIZE;

        let vocab_end = pos.checked_add(header.vocab_len as usize).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Vocabulary length overflow")
        })?;
        let vocab_bytes = bytes
            .get(pos..vocab_end)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "Vocabulary section truncated")
            })?
            .to_vec();
        pos = vocab_end;

        let sa_end = pos
            .checked_add(header.sa_len as usize)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "SA length overflow"))?;
        let sa_bytes = bytes
            .get(pos..sa_end)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Suffix array section truncated",
                )
            })?
            .to_vec();
        pos = sa_end;

        let postings_end = pos
            .checked_add(header.postings_len as usize)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Postings length overflow")
            })?;
        let postings_bytes = bytes
            .get(pos..postings_end)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "Postings section truncated")
            })?
            .to_vec();
        pos = postings_end;

        let skip_end = pos.checked_add(header.skip_len as usize).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Skip list length overflow")
        })?;
        let skip_bytes = bytes
            .get(pos..skip_end)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "Skip list section truncated")
            })?
            .to_vec();
        pos = skip_end;

        // v6: Section table (deduplicated section_id strings)
        let section_table_end = pos
            .checked_add(header.section_table_len as usize)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Section table length overflow")
            })?;
        let section_table_bytes = bytes
            .get(pos..section_table_end)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Section table section truncated",
                )
            })?
            .to_vec();
        pos = section_table_end;

        let lev_dfa_end = pos
            .checked_add(header.lev_dfa_len as usize)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "Levenshtein DFA length overflow",
                )
            })?;
        let lev_dfa_bytes = bytes
            .get(pos..lev_dfa_end)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Levenshtein DFA section truncated",
                )
            })?
            .to_vec();
        pos = lev_dfa_end;

        let docs_end = pos
            .checked_add(header.docs_len as usize)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Docs length overflow"))?;
        let docs_bytes = bytes
            .get(pos..docs_end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "Docs section truncated"))?
            .to_vec();
        pos = docs_end;

        // v7: Embedded WASM
        let wasm_end = pos
            .checked_add(header.wasm_len as usize)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "WASM length overflow"))?;
        let wasm_bytes = bytes
            .get(pos..wasm_end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "WASM section truncated"))?
            .to_vec();
        pos = wasm_end;

        // v7: Dictionary tables (Parquet-style compression)
        let dict_table_end = pos
            .checked_add(header.dict_table_len as usize)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "Dict table length overflow")
            })?;
        let dict_table_bytes = bytes
            .get(pos..dict_table_end)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Dict table section truncated",
                )
            })?
            .to_vec();

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

/// Encode docs to binary format (no JSON dependency)
///
/// Format:
/// - count: varint (number of docs)
/// - For each doc:
///   - type: u8 (0=page, 1=post)
///   - title: varint_len + utf8
///   - excerpt: varint_len + utf8
///   - href: varint_len + utf8
///   - has_section_id: u8 (0=None, 1=Some)
///   - section_id: varint_len + utf8 (only if has_section_id=1)
///   - category: varint_len + utf8 (empty string if None)
pub fn encode_docs_binary(docs: &[DocMetaInput]) -> Vec<u8> {
    let mut buf = Vec::new();

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
    }

    buf
}

/// Decode docs from binary format
fn decode_docs_binary(bytes: &[u8]) -> io::Result<Vec<DocMeta>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }

    let mut offset = 0;

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

        docs.push(DocMeta {
            title,
            excerpt,
            href,
            doc_type: doc_type.to_string(),
            section_id,
            category,
            // TODO: Add dictionary-based decoding in Step 5
            author: None,
            tags: vec![],
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

        // Decode vocabulary
        let vocabulary = decode_vocabulary(&layer.vocab_bytes, layer.header.term_count as usize)?;

        // Decode suffix array
        let (suffix_array, _) = decode_suffix_array(&layer.sa_bytes)?;

        // Decode section table (v6)
        let (section_table, _) = decode_section_table(&layer.section_table_bytes)?;

        // Decode postings with section_id indices (v6)
        let mut postings = Vec::with_capacity(layer.header.term_count as usize);
        let mut pos = 0;
        let mut term_idx = 0;
        while pos < layer.postings_bytes.len() {
            let (posting_list, consumed) = decode_postings_v6(&layer.postings_bytes[pos..])
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
    use crate::levenshtein_dfa::ParametricDFA;

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
    fn test_block_pfor_roundtrip() {
        let deltas: [u32; BLOCK_SIZE] = std::array::from_fn(|i| (i * 2 + 1) as u32);
        let mut buf = Vec::new();
        encode_block_pfor(&deltas, &mut buf);
        let (decoded, _) = decode_block_pfor(&buf).unwrap();
        assert_eq!(decoded.len(), BLOCK_SIZE);
        for i in 0..BLOCK_SIZE {
            assert_eq!(decoded[i], deltas[i]);
        }
    }

    #[test]
    fn test_block_pfor_uniform() {
        // All same values should use 0 bits
        let deltas: [u32; BLOCK_SIZE] = [42; BLOCK_SIZE];
        let mut buf = Vec::new();
        encode_block_pfor(&deltas, &mut buf);
        let (decoded, _) = decode_block_pfor(&buf).unwrap();
        assert_eq!(decoded, deltas.to_vec());
    }

    #[test]
    fn test_postings_roundtrip() {
        let doc_ids: Vec<u32> = (0..500).map(|i| i * 3).collect();
        let mut buf = Vec::new();
        encode_postings(&doc_ids, &mut buf);
        let (decoded, _) = decode_postings(&buf).unwrap();
        assert_eq!(decoded, doc_ids);
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
        let suffix_array = vec![(0, 0)];
        let section_table = vec!["introduction".to_string(), "conclusion".to_string()];

        // Create postings with section_ids
        let postings = vec![vec![
            PostingEntry {
                doc_id: 0,
                section_idx: 0,
            }, // No section (title)
            PostingEntry {
                doc_id: 0,
                section_idx: 1,
            }, // "introduction"
            PostingEntry {
                doc_id: 0,
                section_idx: 2,
            }, // "conclusion"
        ]];

        let lev_dfa_bytes = build_lev_dfa_bytes();
        let docs_bytes = encode_docs_binary(&[]);

        let layer = BinaryLayer::build_v6(
            &vocabulary,
            &suffix_array,
            &postings,
            &section_table,
            1,
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

        // Verify postings with section_ids
        let entries = loaded.get_postings("test").unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].section_idx, 0);
        assert_eq!(entries[1].section_idx, 1);
        assert_eq!(entries[2].section_idx, 2);

        // Verify section resolution
        assert_eq!(loaded.resolve_section_id(0), None);
        assert_eq!(loaded.resolve_section_id(1), Some("introduction"));
        assert_eq!(loaded.resolve_section_id(2), Some("conclusion"));
    }

    #[test]
    fn test_header_roundtrip() {
        let header = SieveHeader {
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
        assert_eq!(buf.len(), SieveHeader::SIZE);

        let decoded = SieveHeader::read(&mut io::Cursor::new(&buf)).unwrap();
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
        let corruption_idx = SieveHeader::SIZE + 5;
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
        let mut bytes = vec![0u8; SieveHeader::SIZE + SieveFooter::SIZE];
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
        let result = SieveFooter::read(&small);
        assert!(result.is_err());
    }

    #[test]
    fn test_footer_roundtrip() {
        let data = b"Hello, world!";
        let crc32 = SieveFooter::compute_crc32(data);

        let footer = SieveFooter { crc32 };
        let mut buf = Vec::new();
        footer.write(&mut buf).unwrap();
        assert_eq!(buf.len(), SieveFooter::SIZE);

        // Append footer to data and verify
        let mut full = data.to_vec();
        full.extend_from_slice(&buf);

        let parsed = SieveFooter::read(&full).unwrap();
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
        }]];
        let lev_dfa_bytes = build_lev_dfa_bytes();
        let docs_bytes = encode_docs_binary(&[]);

        // Simulate WASM bytes (just some recognizable pattern)
        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]; // WASM magic

        let layer = BinaryLayer::build_v7(
            &vocabulary,
            &suffix_array,
            &postings,
            &vec![],
            1,
            lev_dfa_bytes,
            docs_bytes,
            wasm_bytes.clone(),
        )
        .unwrap();

        let bytes = layer.to_bytes().unwrap();

        // Verify header wasm_len is correct
        let header = SieveHeader::read(&mut &bytes[..]).unwrap();
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
                PostingEntry { doc_id: 0, section_idx: 0 },
                PostingEntry { doc_id: 1, section_idx: 0 },
            ],
            vec![
                PostingEntry { doc_id: 1, section_idx: 0 },
                PostingEntry { doc_id: 2, section_idx: 0 },
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
            &vec![],
            3,
            lev_dfa_bytes,
            docs_bytes,
            wasm_bytes.clone(),
        )
        .unwrap();

        let bytes = layer.to_bytes().unwrap();
        let header = SieveHeader::read(&mut &bytes[..]).unwrap();

        // Calculate expected WASM offset
        let wasm_offset = SieveHeader::SIZE
            + header.vocab_len as usize
            + header.sa_len as usize
            + header.postings_len as usize
            + header.skip_len as usize
            + header.section_table_len as usize
            + header.lev_dfa_len as usize
            + header.docs_len as usize;

        // Verify WASM bytes are at the correct offset
        let extracted_wasm = &bytes[wasm_offset..wasm_offset + wasm_bytes.len()];
        assert_eq!(extracted_wasm, &wasm_bytes[..]);
    }

    #[test]
    fn test_v7_dict_tables_roundtrip() {
        use crate::dict_table::DictTables;

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
            &vec![],
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
            &vec![],
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
