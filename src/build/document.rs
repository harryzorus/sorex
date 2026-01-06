use crate::FieldBoundary;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: usize,
    pub slug: String,
    pub title: String,
    pub excerpt: String,
    pub href: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub category: Option<String>,
    /// Author name (for multi-author blogs)
    #[serde(default)]
    pub author: Option<String>,
    /// Tags/labels for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    pub text: String,
    pub field_boundaries: Vec<FieldBoundary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc() -> Document {
        Document {
            id: 0,
            slug: "test".to_string(),
            title: "Test".to_string(),
            excerpt: "Test excerpt".to_string(),
            href: "/test".to_string(),
            doc_type: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
            text: "test content".to_string(),
            field_boundaries: vec![],
        }
    }

    #[test]
    fn test_parse_minimal_document() {
        let json = r#"{
            "id": 0,
            "slug": "about",
            "title": "About Me",
            "excerpt": "Test",
            "href": "/about",
            "type": "page",
            "category": null,
            "text": "about me",
            "fieldBoundaries": []
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.slug, "about");
        assert_eq!(doc.category, None);
    }

    #[test]
    fn test_parse_document_with_category() {
        let json = r#"{
            "id": 0,
            "slug": "test",
            "title": "Test",
            "excerpt": "Test",
            "href": "/test",
            "type": "post",
            "category": "engineering",
            "text": "test",
            "fieldBoundaries": []
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.category, Some("engineering".to_string()));
    }

    #[test]
    fn test_parse_document_with_boundaries() {
        let json = r#"{
            "id": 0,
            "slug": "test",
            "title": "Test",
            "excerpt": "Test",
            "href": "/test",
            "type": "post",
            "category": null,
            "text": "test content",
            "fieldBoundaries": [
                {"docId": 0, "start": 0, "end": 4, "fieldType": "title", "sectionId": null}
            ]
        }"#;
        let doc: Document = serde_json::from_str(json).unwrap();
        assert_eq!(doc.field_boundaries.len(), 1);
    }
}
