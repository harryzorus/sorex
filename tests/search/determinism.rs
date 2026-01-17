//! Direct test for determinism regression case.
//!
//! This tests the specific case that was failing in prop_search_deterministic.
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.
//! These tests are skipped unless the bench-datasets feature is enabled.

#![cfg(feature = "bench-datasets")]

use std::fs;
use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;

#[test]
fn test_search_ass_32_deterministic() {
    let bytes = fs::read("target/datasets/cutlass/index.sorex")
        .expect("Failed to read index");
    let layer = LoadedLayer::from_bytes(&bytes)
        .expect("Failed to parse index");
    let searcher = TierSearcher::from_layer(layer)
        .expect("Failed to create searcher");

    // Run the same search twice
    let results1 = searcher.search("ass", 32);

    // Print first results
    eprintln!("=== First search ===");
    for (i, r) in results1.iter().enumerate() {
        eprintln!("  {}: doc_id={}, tier={}, score={}, match_type={:?}",
            i, r.doc_id, r.tier, r.score, r.match_type);
    }

    let results2 = searcher.search("ass", 32);

    // Print second results
    eprintln!("=== Second search ===");
    for (i, r) in results2.iter().enumerate() {
        eprintln!("  {}: doc_id={}, tier={}, score={}, match_type={:?}",
            i, r.doc_id, r.tier, r.score, r.match_type);
    }

    // Should have same length
    assert_eq!(
        results1.len(), results2.len(),
        "Search 'ass' with limit 32: length differs {} vs {}",
        results1.len(), results2.len()
    );

    // Should have same doc_ids in same order
    for (i, (r1, r2)) in results1.iter().zip(results2.iter()).enumerate() {
        assert_eq!(
            r1.doc_id, r2.doc_id,
            "Position {} doc_id differs: {} vs {} (scores: {} vs {})",
            i, r1.doc_id, r2.doc_id, r1.score, r2.score
        );
        assert_eq!(
            r1.tier, r2.tier,
            "Position {} tier differs: {} vs {}",
            i, r1.tier, r2.tier
        );
        assert!(
            (r1.score - r2.score).abs() < 0.001,
            "Position {} score differs: {} vs {}",
            i, r1.score, r2.score
        );
    }

    // Run 10 more times to be sure
    for run in 0..10 {
        let results = searcher.search("ass", 32);
        assert_eq!(
            results.len(), results1.len(),
            "Run {}: length differs {} vs {}",
            run, results.len(), results1.len()
        );
        for (i, (r, r1)) in results.iter().zip(results1.iter()).enumerate() {
            assert_eq!(
                r.doc_id, r1.doc_id,
                "Run {}, position {} doc_id differs: {} vs {}",
                run, i, r.doc_id, r1.doc_id
            );
        }
    }
}

#[test]
fn test_search_determinism_across_fresh_searchers() {
    // This tests if creating multiple searchers from the same data gives the same results
    let bytes = fs::read("target/datasets/cutlass/index.sorex")
        .expect("Failed to read index");

    let layer1 = LoadedLayer::from_bytes(&bytes).expect("Failed to parse index");
    let searcher1 = TierSearcher::from_layer(layer1).expect("Failed to create searcher");
    let results1 = searcher1.search("ass", 32);

    let layer2 = LoadedLayer::from_bytes(&bytes).expect("Failed to parse index");
    let searcher2 = TierSearcher::from_layer(layer2).expect("Failed to create searcher");
    let results2 = searcher2.search("ass", 32);

    assert_eq!(
        results1.len(), results2.len(),
        "Different searchers: length differs {} vs {}",
        results1.len(), results2.len()
    );

    for (i, (r1, r2)) in results1.iter().zip(results2.iter()).enumerate() {
        assert_eq!(
            r1.doc_id, r2.doc_id,
            "Different searchers, position {} doc_id differs: {} vs {}",
            i, r1.doc_id, r2.doc_id
        );
    }
}
