//! Tests for document loading.

use sorex::build::{load_documents, InputManifest};
use std::path::Path;

const BUILD_FIXTURES_DIR: &str = "data/build-fixtures";

fn parse_manifest(fixture: &str) -> InputManifest {
    let path = format!("{}/{}/manifest.json", BUILD_FIXTURES_DIR, fixture);
    let content = std::fs::read_to_string(&path).expect("Failed to read manifest");
    serde_json::from_str(&content).expect("Failed to parse manifest")
}

#[test]
fn test_load_valid_documents() {
    let manifest = parse_manifest("valid");
    let input_path = Path::new(BUILD_FIXTURES_DIR).join("valid");
    let result = load_documents(&input_path, &manifest);

    assert!(result.is_ok(), "Loading valid documents should succeed");
    let docs = result.unwrap();
    assert_eq!(docs.len(), 3, "Should load 3 documents");
}

#[test]
fn test_load_documents_sorted_by_id() {
    let manifest = parse_manifest("valid");
    let input_path = Path::new(BUILD_FIXTURES_DIR).join("valid");
    let docs = load_documents(&input_path, &manifest).unwrap();

    // Documents should be sorted by id
    for i in 1..docs.len() {
        assert!(
            docs[i - 1].id < docs[i].id,
            "Documents should be sorted by id: {} >= {}",
            docs[i - 1].id,
            docs[i].id
        );
    }
}

#[test]
fn test_load_documents_with_missing_file() {
    let manifest = parse_manifest("missing-doc");
    let input_path = Path::new(BUILD_FIXTURES_DIR).join("missing-doc");
    let result = load_documents(&input_path, &manifest);

    assert!(
        result.is_err(),
        "Loading with missing file should fail"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("missing") || err.contains("Failed to read"),
        "Error should mention missing file: {}",
        err
    );
}

#[test]
fn test_load_documents_with_invalid_json() {
    let manifest = parse_manifest("invalid-doc");
    let input_path = Path::new(BUILD_FIXTURES_DIR).join("invalid-doc");
    let result = load_documents(&input_path, &manifest);

    assert!(
        result.is_err(),
        "Loading invalid JSON document should fail"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("Invalid JSON") || err.contains("missing field"),
        "Error should mention invalid JSON: {}",
        err
    );
}

#[test]
fn test_load_documents_fields_parsed_correctly() {
    let manifest = parse_manifest("valid");
    let input_path = Path::new(BUILD_FIXTURES_DIR).join("valid");
    let docs = load_documents(&input_path, &manifest).unwrap();

    // Check first document fields
    let doc0 = &docs[0];
    assert_eq!(doc0.slug, "rust-intro");
    assert_eq!(doc0.title, "Introduction to Rust");
    assert_eq!(doc0.doc_type, "doc");
    assert_eq!(doc0.category, Some("engineering".to_string()));
    assert!(!doc0.field_boundaries.is_empty());
}

#[test]
fn test_load_documents_parallel_consistency() {
    // Loading the same documents multiple times should give the same order
    let manifest = parse_manifest("valid");
    let input_path = Path::new(BUILD_FIXTURES_DIR).join("valid");

    let docs1 = load_documents(&input_path, &manifest).unwrap();
    let docs2 = load_documents(&input_path, &manifest).unwrap();

    assert_eq!(docs1.len(), docs2.len());
    for (d1, d2) in docs1.iter().zip(docs2.iter()) {
        assert_eq!(d1.id, d2.id, "Document order should be consistent");
        assert_eq!(d1.slug, d2.slug);
    }
}
