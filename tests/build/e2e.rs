//! End-to-end tests for the build workflow.

use sorex::binary::LoadedLayer;
use sorex::build::run_build;
use sorex::tiered_search::TierSearcher;
use std::fs;
use tempfile::TempDir;

const BUILD_FIXTURES_DIR: &str = "data/build-fixtures";

#[test]
fn test_run_build_e2e_basic() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output");
    let input_path = format!("{}/valid", BUILD_FIXTURES_DIR);

    let result = run_build(&input_path, output_path.to_str().unwrap(), false);

    assert!(result.is_ok(), "Build should succeed: {:?}", result.err());

    // Verify output files exist
    assert!(
        output_path.join("index.sorex").exists(),
        "index.sorex should be created"
    );
}

#[test]
fn test_run_build_e2e_with_demo() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output");
    let input_path = format!("{}/valid", BUILD_FIXTURES_DIR);

    let result = run_build(&input_path, output_path.to_str().unwrap(), true);

    assert!(result.is_ok(), "Build with demo should succeed");

    // Verify demo.html exists
    assert!(
        output_path.join("demo.html").exists(),
        "demo.html should be created"
    );

    // Verify demo.html has content
    let demo_content = fs::read_to_string(output_path.join("demo.html")).unwrap();
    assert!(
        demo_content.contains("index.sorex"),
        "demo.html should reference index.sorex"
    );
}

#[test]
fn test_run_build_e2e_missing_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output");

    // Point to a directory without manifest.json
    let result = run_build(temp_dir.path().to_str().unwrap(), output_path.to_str().unwrap(), false);

    assert!(result.is_err(), "Build should fail without manifest");
    let err = result.unwrap_err();
    assert!(
        err.contains("manifest") || err.contains("Failed to read"),
        "Error should mention manifest: {}",
        err
    );
}

#[test]
fn test_run_build_e2e_invalid_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output");
    let input_path = format!("{}/invalid-manifest", BUILD_FIXTURES_DIR);

    let result = run_build(&input_path, output_path.to_str().unwrap(), false);

    assert!(result.is_err(), "Build should fail with invalid manifest");
    let err = result.unwrap_err();
    assert!(
        err.contains("Invalid") || err.contains("JSON"),
        "Error should mention invalid JSON: {}",
        err
    );
}

#[test]
fn test_built_index_roundtrip_search() {
    let temp_dir = TempDir::new().unwrap();
    let output_path = temp_dir.path().join("output");
    let input_path = format!("{}/valid", BUILD_FIXTURES_DIR);

    run_build(&input_path, output_path.to_str().unwrap(), false).unwrap();

    // Load the built index
    let bytes = fs::read(output_path.join("index.sorex")).unwrap();
    let layer = LoadedLayer::from_bytes(&bytes).unwrap();
    let searcher = TierSearcher::from_layer(layer).unwrap();

    // Test various searches
    let rust_results = searcher.search("rust", 10);
    assert!(!rust_results.is_empty(), "Should find 'rust'");

    let intro_results = searcher.search("introduction", 10);
    assert!(!intro_results.is_empty(), "Should find 'introduction'");

    let europe_results = searcher.search("europe", 10);
    assert!(!europe_results.is_empty(), "Should find 'europe'");

    // Non-existent term should return empty
    let xyz_results = searcher.search("xyznonexistent", 10);
    assert!(xyz_results.is_empty(), "Should not find 'xyznonexistent'");
}
