//! Document compression benchmarking
//!
//! Compares different approaches for embedding docs in .sieve binary:
//! 1. JSON (current baseline)
//! 2. Custom binary format (length-prefixed strings, 1-bit type)

use serde::{Deserialize, Serialize};

/// Document metadata for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Doc {
    pub id: u32,
    pub title: String,
    pub excerpt: String,
    pub href: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    /// Category for client-side filtering (optional)
    #[serde(default)]
    pub category: Option<String>,
}

/// Type flag as single byte
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DocType {
    Page = 0,
    Post = 1,
}

impl DocType {
    fn from_str(s: &str) -> Self {
        match s {
            "page" => DocType::Page,
            _ => DocType::Post,
        }
    }
}

/// Write a length-prefixed string using varint encoding
fn write_varint_string(buf: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    write_varint(buf, bytes.len() as u32);
    buf.extend_from_slice(bytes);
}

/// Write a varint (LEB128 encoding)
fn write_varint(buf: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7F) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        buf.push(byte);
        if value == 0 {
            break;
        }
    }
}

/// Read a varint from a slice, returning (value, bytes_read)
fn read_varint(data: &[u8]) -> (u32, usize) {
    let mut result = 0u32;
    let mut shift = 0;
    let mut i = 0;
    loop {
        let byte = data[i];
        result |= ((byte & 0x7F) as u32) << shift;
        i += 1;
        if byte & 0x80 == 0 {
            break;
        }
        shift += 7;
    }
    (result, i)
}

/// Read a length-prefixed string from a slice
fn read_varint_string(data: &[u8]) -> (String, usize) {
    let (len, varint_size) = read_varint(data);
    let s = String::from_utf8_lossy(&data[varint_size..varint_size + len as usize]).to_string();
    (s, varint_size + len as usize)
}

/// Encode docs to custom binary format
///
/// Format per doc:
/// - type: 1 byte (0=page, 1=post)
/// - title: varint_len + utf8
/// - excerpt: varint_len + utf8
/// - href: varint_len + utf8
/// - category: varint_len + utf8 (empty string if None)
///
/// Note: id is omitted since it's always sequential (0, 1, 2...)
pub fn encode_docs_binary(docs: &[Doc]) -> Vec<u8> {
    let mut buf = Vec::new();

    // Header: doc count
    write_varint(&mut buf, docs.len() as u32);

    for doc in docs {
        // Type flag (1 byte)
        buf.push(DocType::from_str(&doc.doc_type) as u8);

        // Length-prefixed strings
        write_varint_string(&mut buf, &doc.title);
        write_varint_string(&mut buf, &doc.excerpt);
        write_varint_string(&mut buf, &doc.href);
        // Category (empty string if None)
        write_varint_string(&mut buf, doc.category.as_deref().unwrap_or(""));
    }

    buf
}

/// Decode docs from custom binary format
pub fn decode_docs_binary(data: &[u8]) -> Vec<Doc> {
    let mut offset = 0;

    // Header: doc count
    let (doc_count, size) = read_varint(&data[offset..]);
    offset += size;

    let mut docs = Vec::with_capacity(doc_count as usize);

    for id in 0..doc_count {
        // Type flag
        let doc_type = if data[offset] == 0 { "page" } else { "post" };
        offset += 1;

        // Strings
        let (title, size) = read_varint_string(&data[offset..]);
        offset += size;

        let (excerpt, size) = read_varint_string(&data[offset..]);
        offset += size;

        let (href, size) = read_varint_string(&data[offset..]);
        offset += size;

        let (category_str, size) = read_varint_string(&data[offset..]);
        offset += size;
        let category = if category_str.is_empty() {
            None
        } else {
            Some(category_str)
        };

        docs.push(Doc {
            id,
            title,
            excerpt,
            href,
            doc_type: doc_type.to_string(),
            category,
        });
    }

    docs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_docs() -> Vec<Doc> {
        vec![
            Doc {
                id: 0,
                title: "About Harry".to_string(),
                excerpt: "I'm a performance engineer...".to_string(),
                href: "/about".to_string(),
                doc_type: "page".to_string(),
                category: None,
            },
            Doc {
                id: 1,
                title: "American Adventures Worth Planning For".to_string(),
                excerpt: "Organizing a 2026 adventure calendar...".to_string(),
                href: "/posts/2026/01/american-adventures-worth-planning-for".to_string(),
                doc_type: "post".to_string(),
                category: Some("adventures".to_string()),
            },
        ]
    }

    #[test]
    fn test_roundtrip() {
        let docs = sample_docs();
        let encoded = encode_docs_binary(&docs);
        let decoded = decode_docs_binary(&encoded);

        assert_eq!(docs.len(), decoded.len());
        for (orig, dec) in docs.iter().zip(decoded.iter()) {
            assert_eq!(orig.title, dec.title);
            assert_eq!(orig.excerpt, dec.excerpt);
            assert_eq!(orig.href, dec.href);
            assert_eq!(orig.doc_type, dec.doc_type);
            assert_eq!(orig.category, dec.category);
        }
    }

    fn compress_brotli(data: &[u8]) -> Vec<u8> {
        use std::io::Write;
        let mut compressed = Vec::new();
        let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
        encoder.write_all(data).unwrap();
        drop(encoder);
        compressed
    }

    #[test]
    fn test_size_comparison() {
        // Create test docs with realistic content
        let categories = ["engineering", "adventures", "learning", "training"];
        let docs: Vec<Doc> = (0..20)
            .map(|i| Doc {
                id: i,
                title: format!("Article Title Number {}: A Longer Title for Testing", i),
                excerpt: format!(
                    "This is the excerpt for article {}. It contains a summary of the content \
                     that would typically appear in a search result. The excerpt provides context \
                     for the reader to understand what the article is about.",
                    i
                ),
                href: format!(
                    "/posts/2024/0{}/{}",
                    (i % 12) + 1,
                    format!("article-slug-{}", i)
                ),
                doc_type: if i % 3 == 0 { "page" } else { "post" }.to_string(),
                category: if i % 3 == 0 {
                    None
                } else {
                    Some(categories[i as usize % categories.len()].to_string())
                },
            })
            .collect();

        // JSON size
        let json_bytes = serde_json::to_string(&docs).unwrap();
        let json_size = json_bytes.len();
        let json_brotli = compress_brotli(json_bytes.as_bytes());

        // Custom binary size
        let binary = encode_docs_binary(&docs);
        let binary_size = binary.len();
        let binary_brotli = compress_brotli(&binary);

        // Verify roundtrip
        let decoded = decode_docs_binary(&binary);
        assert_eq!(docs.len(), decoded.len());

        // Binary should be more compact than JSON
        assert!(
            binary_size < json_size,
            "Binary should be smaller than JSON"
        );

        println!("\n=== Compression Comparison ===");
        println!("Documents: {}", docs.len());
        println!();
        println!("              Raw       Brotli");
        println!(
            "JSON:     {:>6} bytes  {:>5} bytes",
            json_size,
            json_brotli.len()
        );
        println!(
            "Binary:   {:>6} bytes  {:>5} bytes",
            binary_size,
            binary_brotli.len()
        );
        println!();
        println!(
            "Raw savings:    {} bytes ({:.1}%)",
            json_size as i64 - binary_size as i64,
            (1.0 - binary_size as f64 / json_size as f64) * 100.0
        );
        println!(
            "Brotli savings: {} bytes ({:.1}%)",
            json_brotli.len() as i64 - binary_brotli.len() as i64,
            (1.0 - binary_brotli.len() as f64 / json_brotli.len() as f64) * 100.0
        );
    }
}
