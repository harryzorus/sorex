//! Tests for index construction.

use sorex::binary::LoadedLayer;
use sorex::build::run_build;
use sorex::tiered_search::TierSearcher;
use std::fs;
use tempfile::TempDir;

const BUILD_FIXTURES_DIR: &str = "data/build-fixtures";

fn build_fixture(fixture: &str) -> (TempDir, std::path::PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let input_path = format!("{}/{}", BUILD_FIXTURES_DIR, fixture);
    let output_path = temp_dir.path().join("output");

    run_build(&input_path, output_path.to_str().unwrap(), false)
        .expect("Build should succeed");

    (temp_dir, output_path)
}

#[test]
fn test_build_single_index_basic() {
    let (_temp_dir, output_path) = build_fixture("valid");

    // Verify index.sorex exists
    let index_path = output_path.join("index.sorex");
    assert!(index_path.exists(), "index.sorex should be created");

    // Verify file has content
    let bytes = fs::read(&index_path).unwrap();
    assert!(!bytes.is_empty(), "Index file should not be empty");
}

#[test]
fn test_build_index_produces_valid_sorex() {
    let (_temp_dir, output_path) = build_fixture("valid");

    let index_path = output_path.join("index.sorex");
    let bytes = fs::read(&index_path).unwrap();

    // Should be parseable by LoadedLayer
    let layer = LoadedLayer::from_bytes(&bytes);
    assert!(layer.is_ok(), "Built index should be parseable: {:?}", layer.err());
}

#[test]
fn test_build_index_doc_count() {
    let (_temp_dir, output_path) = build_fixture("valid");

    let index_path = output_path.join("index.sorex");
    let bytes = fs::read(&index_path).unwrap();
    let layer = LoadedLayer::from_bytes(&bytes).unwrap();

    // Should have 3 documents
    assert_eq!(layer.doc_count, 3, "Should have 3 documents");
}

#[test]
fn test_build_index_has_vocabulary() {
    let (_temp_dir, output_path) = build_fixture("valid");

    let index_path = output_path.join("index.sorex");
    let bytes = fs::read(&index_path).unwrap();
    let layer = LoadedLayer::from_bytes(&bytes).unwrap();

    // Should have vocabulary (searchable terms)
    assert!(!layer.vocabulary.is_empty(), "Should have vocabulary");
}

#[test]
fn test_build_index_searchable() {
    let (_temp_dir, output_path) = build_fixture("valid");

    let index_path = output_path.join("index.sorex");
    let bytes = fs::read(&index_path).unwrap();
    let layer = LoadedLayer::from_bytes(&bytes).unwrap();
    let searcher = TierSearcher::from_layer(layer).expect("Should create searcher");

    // Search for "rust" which appears in fixtures
    let results = searcher.search("rust", 10);
    assert!(
        !results.is_empty(),
        "Search for 'rust' should find results"
    );
}

#[test]
fn test_build_index_suffix_array_sorted() {
    let (_temp_dir, output_path) = build_fixture("valid");

    let index_path = output_path.join("index.sorex");
    let bytes = fs::read(&index_path).unwrap();
    let layer = LoadedLayer::from_bytes(&bytes).unwrap();

    // Verify suffix array is sorted
    for i in 1..layer.suffix_array.len() {
        let (prev_idx, prev_off) = layer.suffix_array[i - 1];
        let (curr_idx, curr_off) = layer.suffix_array[i];

        let prev_suffix = &layer.vocabulary[prev_idx as usize][prev_off as usize..];
        let curr_suffix = &layer.vocabulary[curr_idx as usize][curr_off as usize..];

        assert!(
            prev_suffix <= curr_suffix,
            "INVARIANT VIOLATED: Suffix array not sorted at position {}",
            i
        );
    }
}
