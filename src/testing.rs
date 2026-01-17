//! Test utilities shared across unit and integration tests.
//!
//! This module is always compiled but hidden from documentation.
//! It provides canonical implementations of test helpers to avoid duplication.

#![doc(hidden)]

use crate::types::{FieldBoundary, FieldType, SearchDoc};

/// Create a simple test document with default fields.
///
/// This is the canonical implementation used across all tests.
pub fn make_doc(id: usize, title: &str) -> SearchDoc {
    SearchDoc {
        id,
        title: title.to_string(),
        excerpt: format!("Excerpt for {}", title),
        href: format!("/doc/{}", id),
        kind: "post".to_string(),
        category: None,
        author: None,
        tags: vec![],
    }
}

/// Create a minimal test document with just an id.
pub fn make_doc_simple(id: usize) -> SearchDoc {
    SearchDoc {
        id,
        title: format!("Doc {}", id),
        excerpt: String::new(),
        href: format!("/doc/{}", id),
        kind: "post".to_string(),
        category: None,
        author: None,
        tags: vec![],
    }
}

/// Create a test document with category.
pub fn make_doc_with_category(id: usize, title: &str, category: &str) -> SearchDoc {
    SearchDoc {
        id,
        title: title.to_string(),
        excerpt: format!("Excerpt for {}", title),
        href: format!("/doc/{}", id),
        kind: "post".to_string(),
        category: Some(category.to_string()),
        author: None,
        tags: vec![],
    }
}

/// Create a field boundary for a title field.
pub fn make_title_boundary(doc_id: usize, start: usize, end: usize) -> FieldBoundary {
    FieldBoundary {
        doc_id,
        start,
        end,
        field_type: FieldType::Title,
        section_id: None,
        heading_level: 0,
    }
}

/// Create a field boundary for a heading field.
pub fn make_heading_boundary(
    doc_id: usize,
    start: usize,
    end: usize,
    section_id: Option<String>,
    heading_level: u8,
) -> FieldBoundary {
    FieldBoundary {
        doc_id,
        start,
        end,
        field_type: FieldType::Heading,
        section_id,
        heading_level,
    }
}

/// Create a field boundary for content.
pub fn make_content_boundary(
    doc_id: usize,
    start: usize,
    end: usize,
    section_id: Option<String>,
) -> FieldBoundary {
    FieldBoundary {
        doc_id,
        start,
        end,
        field_type: FieldType::Content,
        section_id,
        heading_level: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_doc() {
        let doc = make_doc(42, "Test Title");
        assert_eq!(doc.id, 42);
        assert_eq!(doc.title, "Test Title");
        assert_eq!(doc.href, "/doc/42");
    }

    #[test]
    fn test_make_doc_simple() {
        let doc = make_doc_simple(7);
        assert_eq!(doc.id, 7);
        assert_eq!(doc.title, "Doc 7");
    }

    #[test]
    fn test_make_title_boundary() {
        let boundary = make_title_boundary(0, 0, 10);
        assert_eq!(boundary.doc_id, 0);
        assert_eq!(boundary.field_type, FieldType::Title);
        assert_eq!(boundary.start, 0);
        assert_eq!(boundary.end, 10);
    }
}
