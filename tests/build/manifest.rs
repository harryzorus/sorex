//! Tests for manifest parsing.

use sorex::build::{IncludeFilter, IncludeFilterValue, IndexDefinition, InputManifest};

#[test]
fn test_parse_manifest_valid() {
    let json = r#"{
        "version": 1,
        "documents": ["0.json", "1.json"]
    }"#;
    let manifest: InputManifest = serde_json::from_str(json).unwrap();
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.documents.len(), 2);
    assert!(manifest.indexes.is_empty());
}

#[test]
fn test_parse_manifest_empty_documents() {
    let json = r#"{
        "version": 1,
        "documents": []
    }"#;
    let manifest: InputManifest = serde_json::from_str(json).unwrap();
    assert_eq!(manifest.version, 1);
    assert!(manifest.documents.is_empty());
}

#[test]
fn test_parse_manifest_with_indexes() {
    let json = r#"{
        "version": 1,
        "documents": ["0.json"],
        "indexes": {
            "all": {"include": "*"},
            "eng": {"include": {"category": "engineering"}}
        }
    }"#;
    let manifest: InputManifest = serde_json::from_str(json).unwrap();
    assert_eq!(manifest.version, 1);
    assert_eq!(manifest.indexes.len(), 2);
    assert!(manifest.indexes.contains_key("all"));
    assert!(manifest.indexes.contains_key("eng"));
}

#[test]
fn test_include_filter_all_star() {
    let json = r#"{"include": "*"}"#;
    let def: IndexDefinition = serde_json::from_str(json).unwrap();

    let filter: IncludeFilter = def.include.into();
    assert!(matches!(filter, IncludeFilter::All));
}

#[test]
fn test_include_filter_category_match() {
    let json = r#"{"include": {"category": "engineering"}}"#;
    let def: IndexDefinition = serde_json::from_str(json).unwrap();

    match def.include {
        IncludeFilterValue::Filter(m) => {
            assert_eq!(m.get("category"), Some(&"engineering".to_string()));
        }
        _ => panic!("Expected Filter variant"),
    }
}

#[test]
fn test_include_filter_type_match() {
    let json = r#"{"include": {"type": "post"}}"#;
    let def: IndexDefinition = serde_json::from_str(json).unwrap();

    match def.include {
        IncludeFilterValue::Filter(m) => {
            assert_eq!(m.get("type"), Some(&"post".to_string()));
        }
        _ => panic!("Expected Filter variant"),
    }
}

#[test]
fn test_parse_manifest_invalid_json() {
    let json = r#"{
        "version": 1,
        "documents": ["0.json"
    }"#;
    let result: Result<InputManifest, _> = serde_json::from_str(json);
    assert!(result.is_err());
}
