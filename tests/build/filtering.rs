//! Tests for document filtering.

use sorex::build::{load_documents, Document, IncludeFilter, InputManifest};
use std::collections::HashMap;
use std::path::Path;

const BUILD_FIXTURES_DIR: &str = "data/build-fixtures";

fn parse_manifest(fixture: &str) -> InputManifest {
    let path = format!("{}/{}/manifest.json", BUILD_FIXTURES_DIR, fixture);
    let content = std::fs::read_to_string(&path).expect("Failed to read manifest");
    serde_json::from_str(&content).expect("Failed to parse manifest")
}

fn load_filtered_docs(fixture: &str) -> Vec<Document> {
    let manifest = parse_manifest(fixture);
    let input_path = Path::new(BUILD_FIXTURES_DIR).join(fixture);
    load_documents(&input_path, &manifest).unwrap()
}

#[test]
fn test_filter_all_includes_everything() {
    let docs = load_filtered_docs("filtered");
    let filter = IncludeFilter::All;

    let matched: Vec<_> = docs.iter().filter(|d| filter.matches(d)).collect();
    assert_eq!(
        matched.len(),
        docs.len(),
        "All filter should include all documents"
    );
}

#[test]
fn test_filter_by_category() {
    let docs = load_filtered_docs("filtered");

    let mut filter_map = HashMap::new();
    filter_map.insert("category".to_string(), "engineering".to_string());
    let filter = IncludeFilter::Filter(filter_map);

    let matched: Vec<_> = docs.iter().filter(|d| filter.matches(d)).collect();
    assert_eq!(matched.len(), 2, "Should match 2 engineering docs");

    for doc in matched {
        assert_eq!(
            doc.category,
            Some("engineering".to_string()),
            "Matched doc should have engineering category"
        );
    }
}

#[test]
fn test_filter_by_type() {
    let docs = load_filtered_docs("filtered");

    let mut filter_map = HashMap::new();
    filter_map.insert("type".to_string(), "post".to_string());
    let filter = IncludeFilter::Filter(filter_map);

    let matched: Vec<_> = docs.iter().filter(|d| filter.matches(d)).collect();
    assert_eq!(matched.len(), 1, "Should match 1 post");
    assert_eq!(matched[0].doc_type, "post");
}

#[test]
fn test_filter_empty_result() {
    let docs = load_filtered_docs("filtered");

    let mut filter_map = HashMap::new();
    filter_map.insert("category".to_string(), "nonexistent".to_string());
    let filter = IncludeFilter::Filter(filter_map);

    let matched: Vec<_> = docs.iter().filter(|d| filter.matches(d)).collect();
    assert!(matched.is_empty(), "Should match no documents");
}

#[test]
fn test_filter_multiple_criteria() {
    let docs = load_filtered_docs("filtered");

    let mut filter_map = HashMap::new();
    filter_map.insert("category".to_string(), "engineering".to_string());
    filter_map.insert("type".to_string(), "doc".to_string());
    let filter = IncludeFilter::Filter(filter_map);

    let matched: Vec<_> = docs.iter().filter(|d| filter.matches(d)).collect();
    assert_eq!(matched.len(), 2, "Should match 2 engineering docs of type doc");

    for doc in matched {
        assert_eq!(doc.category, Some("engineering".to_string()));
        assert_eq!(doc.doc_type, "doc");
    }
}
