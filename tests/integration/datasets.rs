//! Integration tests for the search crate.
//!
//! These tests verify end-to-end behavior using realistic inputs.

use super::common::assert_index_well_formed;
use sorex::{build_index, search, FieldBoundary, FieldType, SearchDoc, SearchIndex};
use std::fs;

// ============================================================================
// FIXTURE-BASED TESTS
// ============================================================================

fn load_fixture() -> (Vec<SearchDoc>, Vec<String>, Vec<FieldBoundary>) {
    let content = fs::read_to_string("data/fixtures/test_docs.json").expect("Failed to read fixture");
    let json: serde_json::Value = serde_json::from_str(&content).expect("Invalid JSON");

    let docs: Vec<SearchDoc> = json["docs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| SearchDoc {
            id: d["id"].as_u64().unwrap() as usize,
            title: d["title"].as_str().unwrap().to_string(),
            excerpt: d["excerpt"].as_str().unwrap().to_string(),
            href: d["href"].as_str().unwrap().to_string(),
            kind: d["type"].as_str().unwrap().to_string(),
            category: d.get("category").and_then(|c| c.as_str()).map(|s| s.to_string()),
            author: None,
            tags: vec![],
        })
        .collect();

    let texts: Vec<String> = json["texts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t.as_str().unwrap().to_string())
        .collect();

    let boundaries: Vec<FieldBoundary> = json["fieldBoundaries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| FieldBoundary {
            doc_id: b["docId"].as_u64().unwrap() as usize,
            start: b["start"].as_u64().unwrap() as usize,
            end: b["end"].as_u64().unwrap() as usize,
            field_type: match b["fieldType"].as_str().unwrap() {
                "title" => FieldType::Title,
                "heading" => FieldType::Heading,
                _ => FieldType::Content,
            },
            section_id: b["sectionId"].as_str().map(|s| s.to_string()),
            heading_level: b["headingLevel"].as_u64().unwrap_or(0) as u8,
        })
        .collect();

    (docs, texts, boundaries)
}

#[test]
fn test_fixture_index_well_formed() {
    let (docs, texts, boundaries) = load_fixture();
    let index = build_index(docs, texts, boundaries);
    assert_index_well_formed(&index);
}

#[test]
fn test_fixture_search_rust() {
    let (docs, texts, boundaries) = load_fixture();
    let index = build_index(docs, texts, boundaries);

    let results = search(&index, "rust");
    assert!(!results.is_empty(), "Should find 'rust' in corpus");

    // Both Rust-related docs should be found
    let ids: Vec<usize> = results.iter().map(|d| d.id).collect();
    assert!(ids.contains(&0), "Should find 'Introduction to Rust'");
    assert!(ids.contains(&1), "Should find 'Advanced Rust Patterns'");
}

#[test]
fn test_fixture_search_ranking() {
    let (docs, texts, boundaries) = load_fixture();
    let index = build_index(docs, texts, boundaries);

    // Search for "search" - should find doc 2 with title match
    let results = search(&index, "search");
    assert!(!results.is_empty(), "Should find 'search' in corpus");

    // The doc with "Search" in the title should rank first
    assert_eq!(
        results[0].id, 2,
        "Doc with title match should rank first"
    );
}

// ============================================================================
// END-TO-END WORKFLOW TESTS
// ============================================================================

#[test]
fn test_indexer_roundtrip() {
    // Simulate the indexer binary workflow
    let input = r#"{
        "docs": [
            {"id": 0, "title": "Test", "excerpt": "Test excerpt", "href": "/test", "type": "post"}
        ],
        "texts": ["test document content"],
        "fieldBoundaries": []
    }"#;

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Payload {
        docs: Vec<SearchDoc>,
        texts: Vec<String>,
        field_boundaries: Vec<FieldBoundary>,
    }

    let payload: Payload = serde_json::from_str(input).unwrap();
    let index = build_index(payload.docs, payload.texts, payload.field_boundaries);

    // Serialize and deserialize
    let serialized = serde_json::to_string(&index).unwrap();
    let deserialized: SearchIndex = serde_json::from_str(&serialized).unwrap();

    // Verify roundtrip preserves structure
    assert_eq!(index.docs.len(), deserialized.docs.len());
    assert_eq!(index.texts.len(), deserialized.texts.len());
    assert_eq!(index.suffix_array.len(), deserialized.suffix_array.len());
    assert_eq!(index.lcp.len(), deserialized.lcp.len());
}

#[test]
fn test_wasm_compatible_api() {
    // Verify the API works as WASM would use it
    let docs = vec![SearchDoc {
        id: 0,
        title: "WASM Test".to_string(),
        excerpt: "Testing WASM compatibility".to_string(),
        href: "/wasm".to_string(),
        kind: "post".to_string(),
        category: None,
        author: None,
        tags: vec![],
    }];
    let texts = vec!["webassembly rust wasm bindgen".to_string()];
    let index = build_index(docs, texts, vec![]);

    // Serialize to JSON (what WASM would receive)
    let json = serde_json::to_string(&index).unwrap();

    // Deserialize (what WASM would do)
    let loaded: SearchIndex = serde_json::from_str(&json).unwrap();

    // Search should work
    let results = search(&loaded, "wasm");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "WASM Test");
}

// ============================================================================
// REAL-WORLD SCENARIO TESTS
// ============================================================================

#[test]
fn test_multi_term_search() {
    let docs = vec![
        SearchDoc {
            id: 0,
            title: "Rust Programming".to_string(),
            excerpt: "Learn Rust".to_string(),
            href: "/rust".to_string(),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        },
        SearchDoc {
            id: 1,
            title: "Go Programming".to_string(),
            excerpt: "Learn Go".to_string(),
            href: "/go".to_string(),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        },
        SearchDoc {
            id: 2,
            title: "Rust and Go Comparison".to_string(),
            excerpt: "Compare languages".to_string(),
            href: "/compare".to_string(),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        },
    ];

    let texts = vec![
        "rust programming systems language".to_string(),
        "go programming concurrent language".to_string(),
        "rust and go comparison both are systems programming languages".to_string(),
    ];

    let index = build_index(docs, texts, vec![]);

    // Single term search
    let rust_results = search(&index, "rust");
    assert_eq!(rust_results.len(), 2); // Docs 0 and 2

    // Multi-term search (AND behavior)
    let both_results = search(&index, "rust go");
    // Only doc 2 contains both "rust" AND "go"
    assert_eq!(both_results.len(), 1);
    assert_eq!(both_results[0].id, 2);
}

#[test]
fn test_fuzzy_matching() {
    use sorex::{build_hybrid_index, search_hybrid};

    let docs = vec![SearchDoc {
        id: 0,
        title: "Programming Languages".to_string(),
        excerpt: "Various languages".to_string(),
        href: "/langs".to_string(),
        kind: "post".to_string(),
        category: None,
        author: None,
        tags: vec![],
    }];

    let texts = vec!["programming languages rust python javascript".to_string()];

    // Use HybridIndex for fuzzy matching support
    let index = build_hybrid_index(docs, texts, vec![]);

    // Typo in search term (should match via fuzzy)
    let results = search_hybrid(&index, "programing"); // Missing 'm'
    assert!(
        !results.is_empty(),
        "Fuzzy search should find 'programming' when searching 'programing'"
    );
}

#[test]
fn test_case_insensitive_search() {
    let docs = vec![SearchDoc {
        id: 0,
        title: "Case Test".to_string(),
        excerpt: "Testing case".to_string(),
        href: "/case".to_string(),
        kind: "post".to_string(),
        category: None,
        author: None,
        tags: vec![],
    }];

    // Note: Index stores normalized (lowercase) text
    // The search function normalizes queries before matching
    let texts = vec!["uppercase lowercase mixedcase".to_string()];

    let index = build_index(docs, texts, vec![]);

    // All should match regardless of query case
    assert!(!search(&index, "uppercase").is_empty());
    assert!(!search(&index, "UPPERCASE").is_empty());
    assert!(!search(&index, "UpperCase").is_empty());
    assert!(!search(&index, "LOWERCASE").is_empty());
    assert!(!search(&index, "MixedCase").is_empty());
}

// ============================================================================
// SOREX INSPECT CLI TESTS
// ============================================================================

use std::process::Command;

/// Run the sorex binary with the given arguments.
/// Tries to find the binary in target/release or target/debug.
fn run_sorex(args: &[&str]) -> std::process::Output {
    let release_path = "target/release/sorex";
    let debug_path = "target/debug/sorex";

    let binary = if std::path::Path::new(release_path).exists() {
        release_path
    } else if std::path::Path::new(debug_path).exists() {
        debug_path
    } else {
        // Fallback: try to build first
        Command::new("cargo")
            .args(["build", "--release", "--bin", "sorex"])
            .status()
            .expect("Failed to build sorex");
        release_path
    };

    Command::new(binary)
        .args(args)
        .output()
        .expect("failed to run sorex")
}

#[test]
fn test_inspect_shows_encoding_techniques() {
    // Find the sorex index file
    let index_path = "target/e2e/output/index.sorex";
    if !std::path::Path::new(index_path).exists() {
        // Skip test if index doesn't exist (CI environment)
        return;
    }

    let output = run_sorex(&["inspect", index_path]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify format labels appear in binary structure (v10 format)
    assert!(
        stdout.contains("[DELTA"),
        "Should show [DELTA] label for postings: {}",
        stdout
    );
    assert!(
        stdout.contains("[FC"),
        "Should show [FC] label for vocabulary: {}",
        stdout
    );
    assert!(
        stdout.contains("[STRM"),
        "Should show [STRM] label for suffix array: {}",
        stdout
    );
    assert!(
        stdout.contains("[DEDUP"),
        "Should show [DEDUP] label for section table: {}",
        stdout
    );
    assert!(
        stdout.contains("[RAW"),
        "Should show [RAW] label for uncompressed sections: {}",
        stdout
    );
}

#[test]
fn test_inspect_shows_encoding_legend() {
    let index_path = "target/e2e/output/index.sorex";
    if !std::path::Path::new(index_path).exists() {
        return;
    }

    let output = run_sorex(&["inspect", index_path]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify formats section appears
    assert!(
        stdout.contains("FORMATS"),
        "Should have FORMATS section: {}",
        stdout
    );

    // Verify legend explanations (v10 format)
    assert!(
        stdout.contains("Delta+Varint"),
        "Should explain Delta+Varint encoding: {}",
        stdout
    );
    assert!(
        stdout.contains("Front compression"),
        "Should explain front compression format: {}",
        stdout
    );
}

#[test]
fn test_inspect_has_dynamic_stats() {
    let index_path = "target/e2e/output/index.sorex";
    if !std::path::Path::new(index_path).exists() {
        return;
    }

    let output = run_sorex(&["inspect", index_path]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify dynamic stats are present (not hardcoded)
    assert!(
        stdout.contains("unique"),
        "Should show unique section count: {}",
        stdout
    );
}

#[test]
fn test_inspect_output_alignment() {
    let index_path = "target/e2e/output/index.sorex";
    if !std::path::Path::new(index_path).exists() {
        return;
    }

    let output = run_sorex(&["inspect", index_path]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Strip ANSI escape codes for alignment checking
    fn strip_ansi(s: &str) -> String {
        let mut result = String::new();
        let mut in_escape = false;
        for c in s.chars() {
            if c == '\x1b' {
                in_escape = true;
            } else if in_escape && c == 'm' {
                in_escape = false;
            } else if !in_escape {
                result.push(c);
            }
        }
        result
    }

    // Find all lines that are box content lines (contain │ after stripping ANSI)
    let box_lines: Vec<String> = stdout
        .lines()
        .map(strip_ansi)
        .filter(|line| line.starts_with('│') || line.starts_with('║'))
        .collect();

    assert!(
        !box_lines.is_empty(),
        "Should have box content lines: {}",
        stdout
    );

    // All single-line box content lines should end with │ or ║
    for (i, line) in box_lines.iter().enumerate() {
        let last_char = line.chars().last().unwrap_or(' ');
        assert!(
            last_char == '│' || last_char == '║',
            "Line {} does not end with box character: '{}'",
            i, line
        );
    }
}

#[test]
fn test_inspect_shows_file_size() {
    let index_path = "target/e2e/output/index.sorex";
    if !std::path::Path::new(index_path).exists() {
        return;
    }

    // Get actual file size
    let file_size = std::fs::metadata(index_path).unwrap().len();

    let output = run_sorex(&["inspect", index_path]);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify size format (should be MB or KB)
    assert!(
        stdout.contains("MB") || stdout.contains("KB") || stdout.contains(" B"),
        "Should show size with unit (MB, KB, or B): {}",
        stdout
    );

    // Verify the actual size value is reasonable
    let expected_str = if file_size >= 1024 * 1024 {
        format!("{:.1} MB", file_size as f64 / 1024.0 / 1024.0)
    } else {
        format!("{:.1} KB", file_size as f64 / 1024.0)
    };
    assert!(
        stdout.contains(&expected_str),
        "Should show file size as '{}', got: {}",
        expected_str,
        stdout
    );

    // Verify compression ratio is shown (now uses arrow format)
    assert!(
        stdout.contains("→") || stdout.contains("smaller"),
        "Should show compression indicator: {}",
        stdout
    );

    // Verify raw vs file size comparison is shown in breakdown
    assert!(
        stdout.contains("RAW") && stdout.contains("FILE") && stdout.contains("SAVED"),
        "Should show RAW, FILE, SAVED columns: {}",
        stdout
    );

    // Verify sections show their encoding savings
    assert!(
        stdout.contains("[FC") || stdout.contains("[DELTA") || stdout.contains("[STRM"),
        "Should show encoding technique labels: {}",
        stdout
    );
}
