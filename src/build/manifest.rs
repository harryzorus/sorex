use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize, Clone, Debug)]
pub struct InputManifest {
    pub version: u32,
    pub documents: Vec<String>,
    #[serde(default)]
    pub indexes: HashMap<String, IndexDefinition>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct IndexDefinition {
    pub include: IncludeFilterValue,
    #[serde(default)]
    pub fields: Option<Vec<String>>,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum IncludeFilterValue {
    /// Match all documents (represented as "*")
    All(String),
    /// Match documents by field=value criteria
    Filter(HashMap<String, String>),
}

/// Normalized include filter for matching
#[derive(Clone, Debug)]
pub enum IncludeFilter {
    All,
    Filter(HashMap<String, String>),
}

impl From<IncludeFilterValue> for IncludeFilter {
    fn from(val: IncludeFilterValue) -> Self {
        match val {
            IncludeFilterValue::All(_) => IncludeFilter::All,
            IncludeFilterValue::Filter(f) => IncludeFilter::Filter(f),
        }
    }
}

impl IncludeFilter {
    /// Check if a document matches this filter
    pub fn matches(&self, doc: &super::Document) -> bool {
        match self {
            IncludeFilter::All => true,
            IncludeFilter::Filter(filters) => {
                filters.iter().all(|(key, value)| {
                    match key.as_str() {
                        "category" => doc.category.as_ref() == Some(value),
                        "type" => &doc.doc_type == value,
                        _ => {
                            // Unknown filter key - warn but continue
                            eprintln!("Warning: Unknown filter key '{}'", key);
                            true
                        }
                    }
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_all_filter() {
        let json = r#"{"include": "*"}"#;
        let def: IndexDefinition = serde_json::from_str(json).unwrap();
        match def.include {
            IncludeFilterValue::All(_) => (),
            _ => panic!("Expected All"),
        }
    }

    #[test]
    fn test_parse_filter_map() {
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
    fn test_parse_manifest() {
        let json = r#"{
            "version": 1,
            "documents": ["a.json", "b.json"],
            "indexes": {
                "all": {"include": "*"},
                "eng": {"include": {"category": "engineering"}}
            }
        }"#;
        let manifest: InputManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.documents.len(), 2);
        assert_eq!(manifest.indexes.len(), 2);
    }
}
