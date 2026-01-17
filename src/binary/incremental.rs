// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Incremental index loader for streaming decode.
//!
//! The idea is simple: don't wait for the whole file to download before you
//! start parsing. As each section's bytes arrive over the wire, dispatch them
//! to a rayon thread for decoding. By the time the last bytes arrive, most
//! sections are already decoded and waiting.
//!
//! This matters more than you'd think. Postings are typically 30-50% of the
//! file. If you wait for the full download, then decode sequentially, you're
//! leaving half your CPUs idle while the user stares at a spinner.
//!
//! # Usage
//!
//! ```ignore
//! let mut loader = IncrementalLoader::new();
//!
//! // Parse header first (gives section offsets)
//! let offsets = loader.load_header(&header_bytes)?;
//!
//! // As each section's bytes arrive, dispatch for background decode
//! loader.load_vocabulary(vocab_bytes);
//! loader.load_dict_tables(dict_bytes);
//! loader.load_postings(postings_bytes, term_count);
//! // ... etc
//!
//! // Finalize waits for all sections and builds LoadedLayer
//! let layer = loader.finalize()?;
//! ```

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;

use super::header::{FormatFlags, SectionOffsets, SorexHeader, VERSION};
use super::postings::{decode_postings, PostingEntry, SkipList};
use super::{decode_section_table, decode_suffix_array, decode_varint, decode_vocabulary};
use super::{decode_docs_binary, DocMeta, LoadedLayer};
use crate::util::dict_table::DictTables;

/// Number of sections that need to be loaded (excluding WASM which is handled separately)
const SECTION_COUNT: u8 = 8;

/// Incremental loader that accepts sections as they arrive.
///
/// Each section is decoded in a background thread using rayon.
/// Call `finalize()` to wait for all sections and build the final `LoadedLayer`.
#[allow(clippy::type_complexity)]
pub struct IncrementalLoader {
    // Header (parsed synchronously, required first)
    header: Option<SorexHeader>,

    // Decoded sections (populated by background threads)
    vocabulary: Arc<RwLock<Option<Vec<String>>>>,
    dict_tables: Arc<RwLock<Option<DictTables>>>,
    postings: Arc<RwLock<Option<Vec<Vec<PostingEntry>>>>>,
    suffix_array: Arc<RwLock<Option<Vec<(u32, u32)>>>>,
    docs: Arc<RwLock<Option<Vec<DocMeta>>>>,
    section_table: Arc<RwLock<Option<Vec<String>>>>,
    skip_lists: Arc<RwLock<Option<HashMap<usize, SkipList>>>>,
    lev_dfa_bytes: Arc<RwLock<Option<Vec<u8>>>>,

    // Completion tracking (counts down from SECTION_COUNT)
    sections_pending: Arc<AtomicU8>,
}

impl Default for IncrementalLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl IncrementalLoader {
    /// Create a new incremental loader.
    pub fn new() -> Self {
        Self {
            header: None,
            vocabulary: Arc::new(RwLock::new(None)),
            dict_tables: Arc::new(RwLock::new(None)),
            postings: Arc::new(RwLock::new(None)),
            suffix_array: Arc::new(RwLock::new(None)),
            docs: Arc::new(RwLock::new(None)),
            section_table: Arc::new(RwLock::new(None)),
            skip_lists: Arc::new(RwLock::new(None)),
            lev_dfa_bytes: Arc::new(RwLock::new(None)),
            sections_pending: Arc::new(AtomicU8::new(SECTION_COUNT)),
        }
    }

    /// Parse header bytes. Returns section offsets for tracking download progress.
    ///
    /// This must be called first before loading any sections.
    pub fn load_header(&mut self, bytes: &[u8]) -> io::Result<SectionOffsets> {
        if bytes.len() < SorexHeader::SIZE {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                format!(
                    "Header too short: {} bytes (need {})",
                    bytes.len(),
                    SorexHeader::SIZE
                ),
            ));
        }

        let header = SorexHeader::read(&mut io::Cursor::new(bytes))?;

        if header.version != VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Unsupported version: {} (expected {})",
                    header.version, VERSION
                ),
            ));
        }

        let offsets = header.section_offsets();
        self.header = Some(header);
        Ok(offsets)
    }

    /// Get the parsed header (if load_header was called).
    pub fn header(&self) -> Option<&SorexHeader> {
        self.header.as_ref()
    }

    /// Decode vocabulary in background thread. Non-blocking.
    ///
    /// Vocabulary is needed by suffix array decoding.
    #[cfg(feature = "rayon")]
    pub fn load_vocabulary(&self, bytes: Vec<u8>) {
        let term_count = self.header.as_ref().map(|h| h.term_count).unwrap_or(0) as usize;
        let vocab_lock = self.vocabulary.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            match decode_vocabulary(&bytes, term_count) {
                Ok(vocab) => {
                    *vocab_lock.write() = Some(vocab);
                }
                Err(e) => {
                    eprintln!("Error decoding vocabulary: {}", e);
                    // Store empty vocab on error so finalize can detect failure
                    *vocab_lock.write() = Some(Vec::new());
                }
            }
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Decode dict tables in background thread. Non-blocking.
    ///
    /// Dict tables are needed by docs decoding.
    #[cfg(feature = "rayon")]
    pub fn load_dict_tables(&self, bytes: Vec<u8>) {
        let dict_lock = self.dict_tables.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            if bytes.is_empty() {
                *dict_lock.write() = Some(DictTables::default());
            } else {
                match DictTables::decode(&bytes) {
                    Ok((tables, _)) => {
                        *dict_lock.write() = Some(tables);
                    }
                    Err(e) => {
                        eprintln!("Error decoding dict tables: {}", e);
                        *dict_lock.write() = Some(DictTables::default());
                    }
                }
            }
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Decode postings in background thread. Non-blocking.
    ///
    /// This is typically the largest section (~30-50% of file size).
    #[cfg(feature = "rayon")]
    pub fn load_postings(&self, bytes: Vec<u8>, term_count: u32) {
        let postings_lock = self.postings.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            let mut postings = Vec::with_capacity(term_count as usize);
            let mut pos = 0;

            while pos < bytes.len() {
                match decode_postings(&bytes[pos..]) {
                    Ok((posting_list, consumed)) => {
                        postings.push(posting_list);
                        pos += consumed;
                    }
                    Err(e) => {
                        eprintln!("Error decoding postings at offset {}: {}", pos, e);
                        break;
                    }
                }
            }

            *postings_lock.write() = Some(postings);
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Decode suffix array in background thread. Non-blocking.
    ///
    /// Note: Suffix array decode is independent (doesn't actually need vocab at decode time).
    #[cfg(feature = "rayon")]
    pub fn load_suffix_array(&self, bytes: Vec<u8>) {
        let sa_lock = self.suffix_array.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            match decode_suffix_array(&bytes) {
                Ok((sa, _)) => {
                    *sa_lock.write() = Some(sa);
                }
                Err(e) => {
                    eprintln!("Error decoding suffix array: {}", e);
                    *sa_lock.write() = Some(Vec::new());
                }
            }
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Decode docs in background thread. Non-blocking.
    ///
    /// Note: Docs decode is independent of dict_tables at decode time
    /// (dict_tables are used at query time for field resolution).
    #[cfg(feature = "rayon")]
    pub fn load_docs(&self, bytes: Vec<u8>) {
        let docs_lock = self.docs.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            match decode_docs_binary(&bytes) {
                Ok(docs) => {
                    *docs_lock.write() = Some(docs);
                }
                Err(e) => {
                    eprintln!("Error decoding docs: {}", e);
                    *docs_lock.write() = Some(Vec::new());
                }
            }
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Decode section table in background thread. Non-blocking.
    #[cfg(feature = "rayon")]
    pub fn load_section_table(&self, bytes: Vec<u8>) {
        let table_lock = self.section_table.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            if bytes.is_empty() {
                *table_lock.write() = Some(Vec::new());
            } else {
                match decode_section_table(&bytes) {
                    Ok((table, _)) => {
                        *table_lock.write() = Some(table);
                    }
                    Err(e) => {
                        eprintln!("Error decoding section table: {}", e);
                        *table_lock.write() = Some(Vec::new());
                    }
                }
            }
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Decode skip lists in background thread. Non-blocking.
    #[cfg(feature = "rayon")]
    pub fn load_skip_lists(&self, bytes: Vec<u8>, flags: FormatFlags) {
        let skip_lock = self.skip_lists.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            let mut skip_lists = HashMap::new();

            if flags.has_skip_lists() && !bytes.is_empty() {
                let mut pos = 0;
                while pos < bytes.len() {
                    match decode_varint(&bytes[pos..]) {
                        Ok((term_ord, consumed)) => {
                            pos += consumed;
                            if pos >= bytes.len() {
                                break;
                            }
                            match SkipList::decode(&bytes[pos..]) {
                                Ok((skip_list, consumed)) => {
                                    pos += consumed;
                                    skip_lists.insert(term_ord as usize, skip_list);
                                }
                                Err(e) => {
                                    eprintln!("Error decoding skip list: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Error decoding skip list term ordinal: {}", e);
                            break;
                        }
                    }
                }
            }

            *skip_lock.write() = Some(skip_lists);
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Store Levenshtein DFA bytes. Non-blocking.
    ///
    /// Note: DFA bytes are not decoded, just stored for lazy initialization.
    #[cfg(feature = "rayon")]
    pub fn load_lev_dfa(&self, bytes: Vec<u8>) {
        let dfa_lock = self.lev_dfa_bytes.clone();
        let pending = self.sections_pending.clone();

        rayon::spawn(move || {
            *dfa_lock.write() = Some(bytes);
            pending.fetch_sub(1, Ordering::SeqCst);
        });
    }

    /// Check if all sections are loaded (non-blocking).
    pub fn is_complete(&self) -> bool {
        self.sections_pending.load(Ordering::SeqCst) == 0
    }

    /// Get number of sections still pending.
    pub fn pending_count(&self) -> u8 {
        self.sections_pending.load(Ordering::SeqCst)
    }

    /// Wait for all sections and build the final LoadedLayer.
    ///
    /// This blocks until all background decode tasks complete.
    pub fn finalize(self) -> io::Result<LoadedLayer> {
        // Spin-wait for all sections to complete
        while !self.is_complete() {
            std::hint::spin_loop();
        }

        let header = self.header.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "Header not loaded")
        })?;

        // Extract all sections (now guaranteed to be Some)
        let vocabulary = self
            .vocabulary
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Vocabulary not loaded"))?;

        let suffix_array = self
            .suffix_array
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Suffix array not loaded"))?;

        let postings = self
            .postings
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Postings not loaded"))?;

        let section_table = self
            .section_table
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Section table not loaded"))?;

        let skip_lists = self
            .skip_lists
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Skip lists not loaded"))?;

        let lev_dfa_bytes = self
            .lev_dfa_bytes
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Levenshtein DFA not loaded"))?;

        let docs = self
            .docs
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Docs not loaded"))?;

        let dict_tables = self
            .dict_tables
            .write()
            .take()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Dict tables not loaded"))?;

        // Validate term count
        if postings.len() != header.term_count as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Term count mismatch: header says {}, got {} posting lists",
                    header.term_count,
                    postings.len()
                ),
            ));
        }

        Ok(LoadedLayer {
            doc_count: header.doc_count as usize,
            vocabulary,
            suffix_array,
            postings,
            section_table,
            skip_lists,
            lev_dfa_bytes,
            docs,
            dict_tables,
            wasm_bytes: Vec::new(), // WASM is handled separately by JS
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{BinaryLayer, DocMetaInput, encode_docs_binary};
    use crate::fuzzy::dfa::ParametricDFA;

    fn build_test_index() -> Vec<u8> {
        let vocabulary = vec!["apple".to_string(), "banana".to_string()];
        let suffix_array = vec![(0, 0), (1, 0)];
        let postings = vec![
            vec![PostingEntry { doc_id: 0, section_idx: 0, heading_level: 0 }],
            vec![PostingEntry { doc_id: 1, section_idx: 0, heading_level: 0 }],
        ];
        let section_table = vec!["intro".to_string()];
        let lev_dfa_bytes = ParametricDFA::build(true).to_bytes();
        let docs = vec![
            DocMetaInput {
                title: "Test".to_string(),
                excerpt: "Test excerpt".to_string(),
                href: "/test".to_string(),
                doc_type: "page".to_string(),
                section_id: None,
                category: None,
                author: None,
                tags: vec![],
            },
        ];
        let docs_bytes = encode_docs_binary(&docs);

        let layer = BinaryLayer::build_v6(
            &vocabulary,
            &suffix_array,
            &postings,
            &section_table,
            2,
            lev_dfa_bytes,
            docs_bytes,
        )
        .unwrap();

        layer.to_bytes().unwrap()
    }

    #[test]
    #[cfg(feature = "rayon")]
    fn test_incremental_loader() {
        let bytes = build_test_index();

        let mut loader = IncrementalLoader::new();

        // Load header
        let offsets = loader.load_header(&bytes).unwrap();

        // Get header info for postings
        let term_count = loader.header().unwrap().term_count;
        let flags = loader.header().unwrap().flags;

        // Load all sections in parallel
        loader.load_vocabulary(bytes[offsets.vocabulary.0..offsets.vocabulary.1].to_vec());
        loader.load_dict_tables(bytes[offsets.dict_tables.0..offsets.dict_tables.1].to_vec());
        loader.load_postings(
            bytes[offsets.postings.0..offsets.postings.1].to_vec(),
            term_count,
        );
        loader.load_suffix_array(bytes[offsets.suffix_array.0..offsets.suffix_array.1].to_vec());
        loader.load_docs(bytes[offsets.docs.0..offsets.docs.1].to_vec());
        loader.load_section_table(bytes[offsets.section_table.0..offsets.section_table.1].to_vec());
        loader.load_skip_lists(bytes[offsets.skip_lists.0..offsets.skip_lists.1].to_vec(), flags);
        loader.load_lev_dfa(bytes[offsets.lev_dfa.0..offsets.lev_dfa.1].to_vec());

        // Finalize
        let layer = loader.finalize().unwrap();

        assert_eq!(layer.doc_count, 2);
        assert_eq!(layer.vocabulary.len(), 2);
        assert_eq!(layer.vocabulary[0], "apple");
        assert_eq!(layer.vocabulary[1], "banana");
        assert_eq!(layer.section_table.len(), 1);
        assert_eq!(layer.section_table[0], "intro");
    }
}
