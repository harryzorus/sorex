// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Dictionary table for string deduplication (Parquet-style compression).
//!
//! Maps repeated strings to compact u16 indices. Category "engineering"
//! appearing 50 times? Store it once, reference by index everywhere else.
//! This is the same trick Parquet and Dremel use for columnar compression.
//!
//! Four dictionaries: category, author, tags, and href prefixes. The href
//! prefix extraction is particularly useful. "/posts/2024/01/" appears on
//! dozens of posts, so storing it once saves real bytes.
//!
//! # References
//!
//! - **Dictionary Encoding**: Columnar database compression technique where
//!   repeated values are replaced by integer indices into a dictionary.
//!   See Melnik et al. (2010): "Dremel: Interactive Analysis of Web-Scale
//!   Datasets", VLDB 2010. Also Apache Parquet format specification:
//!   <https://parquet.apache.org/docs/file-format/data-pages/encodings/>
//!
//! # Wire Format
//!
//! ```text
//! count: varint (number of entries)
//! for each entry:
//!   len: varint (string length in bytes)
//!   bytes: [u8; len] (UTF-8 string data)
//! ```

use std::collections::HashMap;
use std::io;

use crate::binary::{decode_varint, encode_varint};

/// A dictionary table mapping strings to u16 indices.
///
/// Supports up to 65535 unique values per dictionary.
#[derive(Debug, Clone, Default)]
pub struct DictTable {
    /// Ordered list of unique strings
    strings: Vec<String>,
    /// Reverse lookup: string → index
    lookup: HashMap<String, u16>,
}

impl DictTable {
    /// Create an empty dictionary table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a string and return its index.
    ///
    /// If the string already exists, returns the existing index.
    /// Panics if the dictionary exceeds 65535 entries.
    pub fn insert(&mut self, s: &str) -> u16 {
        if let Some(&idx) = self.lookup.get(s) {
            return idx;
        }

        let idx = self.strings.len() as u16;
        assert!(
            idx < u16::MAX,
            "Dictionary overflow: cannot store more than 65535 unique values"
        );

        self.strings.push(s.to_string());
        self.lookup.insert(s.to_string(), idx);
        idx
    }

    /// Get a string by its index.
    pub fn get(&self, idx: u16) -> Option<&str> {
        self.strings.get(idx as usize).map(|s| s.as_str())
    }

    /// Number of entries in the dictionary.
    pub fn len(&self) -> usize {
        self.strings.len()
    }

    /// Check if the dictionary is empty.
    pub fn is_empty(&self) -> bool {
        self.strings.is_empty()
    }

    /// Get all strings in order.
    pub fn strings(&self) -> &[String] {
        &self.strings
    }

    /// Encode the dictionary table to a byte buffer.
    ///
    /// Format: varint(count) + for each: varint(len) + utf8_bytes
    pub fn encode(&self, buf: &mut Vec<u8>) {
        encode_varint(self.strings.len() as u64, buf);
        for s in &self.strings {
            let bytes = s.as_bytes();
            encode_varint(bytes.len() as u64, buf);
            buf.extend_from_slice(bytes);
        }
    }

    /// Decode a dictionary table from bytes.
    ///
    /// Returns the decoded table and number of bytes consumed.
    pub fn decode(data: &[u8]) -> io::Result<(Self, usize)> {
        let (count, mut pos) = decode_varint(data)?;
        let count = count as usize;

        let mut strings = Vec::with_capacity(count);
        let mut lookup = HashMap::with_capacity(count);

        for idx in 0..count {
            let (len, consumed) = decode_varint(&data[pos..])?;
            pos += consumed;
            let len = len as usize;

            if pos + len > data.len() {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "Dictionary string extends past end of data",
                ));
            }

            let s = String::from_utf8(data[pos..pos + len].to_vec())
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            pos += len;

            lookup.insert(s.clone(), idx as u16);
            strings.push(s);
        }

        Ok((Self { strings, lookup }, pos))
    }
}

/// Collection of all dictionary tables used for document compression.
#[derive(Debug, Clone, Default)]
pub struct DictTables {
    /// Dictionary for category field
    pub category: DictTable,
    /// Dictionary for author field
    pub author: DictTable,
    /// Dictionary for tags field
    pub tags: DictTable,
    /// Dictionary for href prefixes (e.g., "/posts/2024/01/")
    pub href_prefix: DictTable,
}

impl DictTables {
    /// Create empty dictionary tables.
    pub fn new() -> Self {
        Self::default()
    }

    /// Encode all dictionary tables to a byte buffer.
    ///
    /// Format:
    /// ```text
    /// num_tables: u8 (4)
    /// category_table: encoded DictTable
    /// author_table: encoded DictTable
    /// tags_table: encoded DictTable
    /// href_prefix_table: encoded DictTable
    /// ```
    pub fn encode(&self, buf: &mut Vec<u8>) {
        buf.push(4); // num_tables
        self.category.encode(buf);
        self.author.encode(buf);
        self.tags.encode(buf);
        self.href_prefix.encode(buf);
    }

    /// Decode all dictionary tables from bytes.
    ///
    /// Returns the decoded tables and number of bytes consumed.
    pub fn decode(data: &[u8]) -> io::Result<(Self, usize)> {
        if data.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "Empty dict tables data",
            ));
        }

        let num_tables = data[0];
        if num_tables != 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Expected 4 dict tables, got {}", num_tables),
            ));
        }

        let mut offset = 1;

        let (category, consumed) = DictTable::decode(&data[offset..])?;
        offset += consumed;

        let (author, consumed) = DictTable::decode(&data[offset..])?;
        offset += consumed;

        let (tags, consumed) = DictTable::decode(&data[offset..])?;
        offset += consumed;

        let (href_prefix, consumed) = DictTable::decode(&data[offset..])?;
        offset += consumed;

        Ok((
            Self {
                category,
                author,
                tags,
                href_prefix,
            },
            offset,
        ))
    }

    /// Check if all tables are empty.
    pub fn is_empty(&self) -> bool {
        self.category.is_empty()
            && self.author.is_empty()
            && self.tags.is_empty()
            && self.href_prefix.is_empty()
    }

    /// Total number of entries across all tables.
    pub fn total_entries(&self) -> usize {
        self.category.len() + self.author.len() + self.tags.len() + self.href_prefix.len()
    }
}

/// Extract a common href prefix for compression.
///
/// Looks for patterns like "/posts/2024/01/" or "/blog/category/".
/// Returns None if the href is too short or doesn't have a useful prefix.
pub fn extract_href_prefix(href: &str) -> Option<String> {
    // Skip hrefs that are too short to benefit from prefix compression
    if href.len() < 8 {
        return None;
    }

    // Find the last slash (the one before the filename/slug)
    // e.g., "/posts/2024/01/my-post" → "/posts/2024/01/"
    if let Some(last_slash_pos) = href.rfind('/') {
        // The prefix includes everything up to and including the last slash
        let prefix = &href[..=last_slash_pos];

        // Only extract if prefix is meaningful (at least 4 chars like "/a/")
        if prefix.len() >= 4 {
            return Some(prefix.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dict_table_insert_and_get() {
        let mut dict = DictTable::new();

        let idx0 = dict.insert("apple");
        let idx1 = dict.insert("banana");
        let idx2 = dict.insert("apple"); // duplicate

        assert_eq!(idx0, 0);
        assert_eq!(idx1, 1);
        assert_eq!(idx2, 0); // same as first "apple"

        assert_eq!(dict.get(0), Some("apple"));
        assert_eq!(dict.get(1), Some("banana"));
        assert_eq!(dict.get(2), None);
        assert_eq!(dict.len(), 2);
    }

    #[test]
    fn test_dict_table_roundtrip() {
        let mut dict = DictTable::new();
        dict.insert("engineering");
        dict.insert("adventures");
        dict.insert("personal");

        let mut buf = Vec::new();
        dict.encode(&mut buf);

        let (decoded, consumed) = DictTable::decode(&buf).unwrap();
        assert_eq!(consumed, buf.len());
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded.get(0), Some("engineering"));
        assert_eq!(decoded.get(1), Some("adventures"));
        assert_eq!(decoded.get(2), Some("personal"));
    }

    #[test]
    fn test_dict_table_empty() {
        let dict = DictTable::new();
        assert!(dict.is_empty());
        assert_eq!(dict.len(), 0);

        let mut buf = Vec::new();
        dict.encode(&mut buf);

        let (decoded, _) = DictTable::decode(&buf).unwrap();
        assert!(decoded.is_empty());
    }

    #[test]
    fn test_dict_tables_roundtrip() {
        let mut tables = DictTables::new();

        tables.category.insert("engineering");
        tables.category.insert("adventures");
        tables.author.insert("Harry");
        tables.author.insert("Guest");
        tables.tags.insert("rust");
        tables.tags.insert("wasm");
        tables.tags.insert("search");
        tables.href_prefix.insert("/posts/2024/");
        tables.href_prefix.insert("/posts/2025/");

        let mut buf = Vec::new();
        tables.encode(&mut buf);

        let (decoded, consumed) = DictTables::decode(&buf).unwrap();
        assert_eq!(consumed, buf.len());

        assert_eq!(decoded.category.len(), 2);
        assert_eq!(decoded.author.len(), 2);
        assert_eq!(decoded.tags.len(), 3);
        assert_eq!(decoded.href_prefix.len(), 2);

        assert_eq!(decoded.category.get(0), Some("engineering"));
        assert_eq!(decoded.author.get(0), Some("Harry"));
        assert_eq!(decoded.tags.get(2), Some("search"));
    }

    #[test]
    fn test_extract_href_prefix() {
        assert_eq!(
            extract_href_prefix("/posts/2024/01/my-post"),
            Some("/posts/2024/01/".to_string())
        );
        assert_eq!(
            extract_href_prefix("/blog/engineering/article"),
            Some("/blog/engineering/".to_string())
        );
        assert_eq!(extract_href_prefix("/about"), None); // too short
        assert_eq!(extract_href_prefix("/a/b"), None); // too short
    }

    #[test]
    fn test_dict_table_unicode() {
        let mut dict = DictTable::new();
        dict.insert("日本語");
        dict.insert("한국어");
        dict.insert("Émile");

        let mut buf = Vec::new();
        dict.encode(&mut buf);

        let (decoded, _) = DictTable::decode(&buf).unwrap();
        assert_eq!(decoded.get(0), Some("日本語"));
        assert_eq!(decoded.get(1), Some("한국어"));
        assert_eq!(decoded.get(2), Some("Émile"));
    }
}
