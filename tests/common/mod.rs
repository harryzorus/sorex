//! Shared test utilities and fixtures.

#![allow(dead_code)]

use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;
use sorex::{
    build_hybrid_index, build_index, FieldBoundary, FieldType, HybridIndex, SearchDoc, SearchIndex,
};
use std::fs;
use std::sync::LazyLock;

// Re-export canonical test utilities from sorex::testing
pub use sorex::testing::make_doc;

// ============================================================================
// DATASET DIRECTORIES
// ============================================================================

/// Directory containing the Cutlass dataset.
pub const CUTLASS_DIR: &str = "target/datasets/cutlass";

/// Directory containing the PyTorch dataset.
pub const PYTORCH_DIR: &str = "target/datasets/pytorch";

/// Directory containing the E2E test fixtures.
pub const FIXTURES_DIR: &str = "data/e2e/fixtures";

// ============================================================================
// INDEX PATHS
// ============================================================================

/// Path to the Cutlass dataset index.
pub const CUTLASS_INDEX: &str = "target/datasets/cutlass/index.sorex";

/// Path to the PyTorch dataset index.
pub const PYTORCH_INDEX: &str = "target/datasets/pytorch/index.sorex";

/// Path to the E2E test fixtures index (built by xtask verify).
pub const FIXTURES_INDEX: &str = "target/e2e/output/index.sorex";

// ============================================================================
// CACHED LAYER/SEARCHER BYTES
// ============================================================================

/// Lazy-loaded bytes for Cutlass index (avoids repeated disk reads in tests).
/// Returns empty vec if file doesn't exist (optional fixture).
static CUTLASS_BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| fs::read(CUTLASS_INDEX).unwrap_or_default());

/// Lazy-loaded bytes for PyTorch index.
/// Returns empty vec if file doesn't exist (optional fixture).
static PYTORCH_BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| fs::read(PYTORCH_INDEX).unwrap_or_default());

/// Lazy-loaded bytes for E2E fixtures index.
static FIXTURES_BYTES: LazyLock<Vec<u8>> =
    LazyLock::new(|| fs::read(FIXTURES_INDEX).expect("Failed to read fixtures index"));

// ============================================================================
// LAYER LOADERS
// ============================================================================

/// Check if Cutlass index is available.
pub fn cutlass_available() -> bool {
    !CUTLASS_BYTES.is_empty()
}

/// Check if PyTorch index is available.
pub fn pytorch_available() -> bool {
    !PYTORCH_BYTES.is_empty()
}

/// Load the Cutlass dataset as a LoadedLayer.
/// Panics with clear message if Cutlass index doesn't exist.
pub fn load_cutlass_layer() -> LoadedLayer {
    if CUTLASS_BYTES.is_empty() {
        panic!(
            "Cutlass index not found at {}. Run `cargo xtask bench-e2e` to build it.",
            CUTLASS_INDEX
        );
    }
    LoadedLayer::from_bytes(&CUTLASS_BYTES).expect("Failed to parse Cutlass index")
}

/// Load the PyTorch dataset as a LoadedLayer.
/// Panics with clear message if PyTorch index doesn't exist.
pub fn load_pytorch_layer() -> LoadedLayer {
    if PYTORCH_BYTES.is_empty() {
        panic!(
            "PyTorch index not found at {}. Run `cargo xtask bench-e2e` to build it.",
            PYTORCH_INDEX
        );
    }
    LoadedLayer::from_bytes(&PYTORCH_BYTES).expect("Failed to parse PyTorch index")
}

/// Load the E2E fixtures dataset as a LoadedLayer.
pub fn load_fixtures_layer() -> LoadedLayer {
    LoadedLayer::from_bytes(&FIXTURES_BYTES).expect("Failed to parse fixtures index")
}

// ============================================================================
// SEARCHER LOADERS
// ============================================================================

/// Load the Cutlass dataset as a TierSearcher.
pub fn load_cutlass_searcher() -> TierSearcher {
    let layer = load_cutlass_layer();
    TierSearcher::from_layer(layer).expect("Failed to create Cutlass searcher")
}

/// Load the PyTorch dataset as a TierSearcher.
pub fn load_pytorch_searcher() -> TierSearcher {
    let layer = load_pytorch_layer();
    TierSearcher::from_layer(layer).expect("Failed to create PyTorch searcher")
}

/// Load the E2E fixtures dataset as a TierSearcher.
pub fn load_fixtures_searcher() -> TierSearcher {
    let layer = load_fixtures_layer();
    TierSearcher::from_layer(layer).expect("Failed to create fixtures searcher")
}

// Note: make_doc is now imported from sorex::testing

/// Build a test index from text strings.
pub fn build_test_index(texts: &[&str]) -> SearchIndex {
    let docs: Vec<SearchDoc> = texts
        .iter()
        .enumerate()
        .map(|(i, _)| make_doc(i, &format!("Doc {}", i)))
        .collect();

    let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

    build_index(docs, texts, vec![])
}

/// Build a test index with field boundaries.
pub fn build_test_index_with_fields(
    docs_data: &[(String, Vec<(String, FieldType)>)],
) -> SearchIndex {
    let docs: Vec<SearchDoc> = docs_data
        .iter()
        .enumerate()
        .map(|(i, (title, _))| make_doc(i, title))
        .collect();

    let mut texts: Vec<String> = Vec::new();
    let mut field_boundaries: Vec<FieldBoundary> = Vec::new();

    for (doc_id, (_title, fields)) in docs_data.iter().enumerate() {
        let mut text = String::new();
        let mut offset = 0;

        for (field_text, field_type) in fields {
            if !text.is_empty() {
                text.push(' ');
                offset += 1;
            }

            let start = offset;
            text.push_str(field_text);
            offset += field_text.len();

            field_boundaries.push(FieldBoundary {
                doc_id,
                start,
                end: offset,
                field_type: *field_type,
                section_id: None,
                heading_level: 0,
            });
        }

        texts.push(text);
    }

    build_index(docs, texts, field_boundaries)
}

/// Assert that an index satisfies all well-formedness invariants.
pub fn assert_index_well_formed(index: &SearchIndex) {
    // Invariant: docs.len() == texts.len()
    assert_eq!(
        index.docs.len(),
        index.texts.len(),
        "INVARIANT VIOLATED: docs.len() != texts.len()"
    );

    // Invariant: lcp.len() == suffix_array.len()
    assert_eq!(
        index.lcp.len(),
        index.suffix_array.len(),
        "INVARIANT VIOLATED: lcp.len() != suffix_array.len()"
    );

    // Invariant: all suffix entries are valid
    // Note: Uses strict inequality (offset < char_count) because suffix arrays index non-empty suffixes
    // offset is a CHARACTER offset (not byte offset)
    for (i, entry) in index.suffix_array.iter().enumerate() {
        assert!(
            entry.doc_id < index.texts.len(),
            "INVARIANT VIOLATED: suffix_array[{}].doc_id {} >= texts.len() {}",
            i,
            entry.doc_id,
            index.texts.len()
        );
        let char_count = index.texts[entry.doc_id].chars().count();
        assert!(
            entry.offset < char_count,
            "INVARIANT VIOLATED: suffix_array[{}].offset {} >= texts[{}].char_count() {}",
            i,
            entry.offset,
            entry.doc_id,
            char_count
        );
    }

    // Invariant: suffix array is sorted
    // Note: suffix array uses CHARACTER offsets (not byte offsets)
    // This matches JavaScript's UTF-16 string semantics
    for i in 1..index.suffix_array.len() {
        let prev = &index.suffix_array[i - 1];
        let curr = &index.suffix_array[i];

        // Use character-based slicing (skip N characters, not N bytes)
        let prev_suffix: String = index.texts[prev.doc_id].chars().skip(prev.offset).collect();
        let curr_suffix: String = index.texts[curr.doc_id].chars().skip(curr.offset).collect();

        assert!(
            prev_suffix <= curr_suffix,
            "INVARIANT VIOLATED: suffix_array not sorted at {}: '{}' > '{}'",
            i,
            prev_suffix.chars().take(20).collect::<String>(),
            curr_suffix.chars().take(20).collect::<String>()
        );
    }

    // Invariant: LCP[0] == 0
    if !index.lcp.is_empty() {
        assert_eq!(
            index.lcp[0], 0,
            "INVARIANT VIOLATED: lcp[0] = {} (expected 0)",
            index.lcp[0]
        );
    }
}

/// Assert that suffix array is complete (all suffixes present).
/// Note: Uses CHARACTER offsets (not byte offsets).
pub fn assert_suffix_array_complete(index: &SearchIndex) {
    for (doc_id, text) in index.texts.iter().enumerate() {
        let char_count = text.chars().count();
        for offset in 0..char_count {
            let found = index
                .suffix_array
                .iter()
                .any(|e| e.doc_id == doc_id && e.offset == offset);

            assert!(
                found,
                "INVARIANT VIOLATED: missing suffix entry for doc_id={}, offset={}",
                doc_id, offset
            );
        }
    }
}

/// Build a hybrid test index from text strings.
///
/// Creates a HybridIndex with both inverted index and suffix array
/// for testing streaming search functionality.
pub fn build_hybrid_test_index(texts: &[String]) -> HybridIndex {
    let docs: Vec<SearchDoc> = texts
        .iter()
        .enumerate()
        .map(|(i, _)| make_doc(i, &format!("Doc {}", i)))
        .collect();

    let texts: Vec<String> = texts.to_vec();

    build_hybrid_index(docs, texts, vec![])
}

// ============================================================================
// BUILD SYSTEM TEST HELPERS
// ============================================================================

/// Directory containing build test fixtures.
pub const BUILD_FIXTURES_DIR: &str = "data/build-fixtures";

/// Create a temporary directory for build output.
pub fn create_temp_output_dir() -> tempfile::TempDir {
    tempfile::TempDir::new().expect("Failed to create temp directory")
}

/// Build an index from fixtures and return the output path.
///
/// Returns the TempDir (to keep it alive) and the output path.
pub fn build_fixture_index(fixture_name: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    use sorex::build::run_build;

    let temp_dir = create_temp_output_dir();
    let input_path = format!("{}/{}", BUILD_FIXTURES_DIR, fixture_name);
    let output_path = temp_dir.path().join("output");

    run_build(
        &input_path,
        output_path.to_str().unwrap(),
        false,
        None,
        None,
    )
    .expect("Build should succeed");

    (temp_dir, output_path)
}

/// Load a searcher from a fixture build.
///
/// Returns the TempDir (to keep it alive) and the searcher.
pub fn load_fixture_build_searcher(fixture_name: &str) -> (tempfile::TempDir, TierSearcher) {
    let (temp_dir, output_path) = build_fixture_index(fixture_name);
    let index_path = output_path.join("index.sorex");

    let bytes = fs::read(&index_path).expect("Failed to read built index");
    let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to parse built index");
    let searcher = TierSearcher::from_layer(layer).expect("Failed to create searcher");

    (temp_dir, searcher)
}

/// Assert that a built index is well-formed.
///
/// Verifies that the built bytes can be parsed and the suffix array is sorted.
pub fn assert_built_index_valid(index_bytes: &[u8]) {
    let layer = LoadedLayer::from_bytes(index_bytes)
        .expect("INVARIANT VIOLATED: Built index bytes are not valid");

    // Verify suffix array is sorted
    for i in 1..layer.suffix_array.len() {
        let (prev_idx, prev_off) = layer.suffix_array[i - 1];
        let (curr_idx, curr_off) = layer.suffix_array[i];

        let prev_suffix = &layer.vocabulary[prev_idx as usize][prev_off as usize..];
        let curr_suffix = &layer.vocabulary[curr_idx as usize][curr_off as usize..];

        assert!(
            prev_suffix <= curr_suffix,
            "INVARIANT VIOLATED: Built index suffix array not sorted at {}",
            i
        );
    }
}
