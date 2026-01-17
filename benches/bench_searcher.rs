//! Criterion benchmarks for pure Rust TierSearcher
//!
//! Measures Rust-native performance without WASM overhead.
//! Useful for comparing against browser benchmarks to quantify WASM bridge overhead.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use sorex::binary::LoadedLayer;
use sorex::tiered_search::TierSearcher;
use std::fs;
use std::collections::HashSet;

fn load_cutlass_searcher() -> TierSearcher {
    let path = "target/datasets/cutlass/index.sorex";
    let bytes = fs::read(path).expect("Failed to read Cutlass index file");
    let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to load index");
    TierSearcher::from_layer(layer).expect("Failed to create searcher")
}

// ============================================================================
// TIER 1 EXACT MATCH BENCHMARKS
// ============================================================================

fn bench_t1_exact_matches(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();

    c.bench_function("t1_exact_kernel", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("kernel"), black_box(10)))
    });

    c.bench_function("t1_exact_gemm", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("gemm"), black_box(10)))
    });

    c.bench_function("t1_exact_tensor", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("tensor"), black_box(10)))
    });

    c.bench_function("t1_exact_warp", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("warp"), black_box(10)))
    });

    c.bench_function("t1_exact_mma", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("mma"), black_box(10)))
    });

    c.bench_function("t1_exact_blockwise", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("blockwise"), black_box(10)))
    });

    c.bench_function("t1_exact_threadblock", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("threadblock"), black_box(10)))
    });
}

fn bench_t1_limit_variations(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();
    let mut group = c.benchmark_group("t1_limit_variations");

    for limit in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.iter(|| searcher.search_tier1_exact(black_box("kernel"), black_box(limit)))
        });
    }

    group.finish();
}

// ============================================================================
// TIER 2 PREFIX MATCH BENCHMARKS
// ============================================================================

fn bench_t2_prefix_matches(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();

    c.bench_function("t2_prefix_kern", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("kern"), black_box(&exclude), black_box(10))
        })
    });

    c.bench_function("t2_prefix_gem", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("gem"), black_box(&exclude), black_box(10))
        })
    });

    c.bench_function("t2_prefix_c", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("c"), black_box(&exclude), black_box(10))
        })
    });

    c.bench_function("t2_prefix_t", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("t"), black_box(&exclude), black_box(10))
        })
    });
}

fn bench_t2_with_exclusions(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();
    let mut group = c.benchmark_group("t2_with_exclusions");

    // Small exclude set
    let small_exclude: HashSet<usize> = vec![0, 1, 2].into_iter().collect();
    group.bench_function("small_exclude", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("kern"), black_box(&small_exclude), black_box(10))
        })
    });

    // Large exclude set
    let large_exclude: HashSet<usize> = (0..30).collect();
    group.bench_function("large_exclude", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("kern"), black_box(&large_exclude), black_box(10))
        })
    });

    group.finish();
}

// ============================================================================
// TIER 3 FUZZY MATCH BENCHMARKS
// ============================================================================

fn bench_t3_fuzzy_matches(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();

    c.bench_function("t3_fuzzy_kernl", |b| {
        b.iter(|| {
            searcher.search_tier3_fuzzy(black_box("kernl"), black_box(&exclude), black_box(10))
        })
    });

    c.bench_function("t3_fuzzy_gemma", |b| {
        b.iter(|| {
            searcher.search_tier3_fuzzy(black_box("gemma"), black_box(&exclude), black_box(10))
        })
    });

    c.bench_function("t3_fuzzy_tensr", |b| {
        b.iter(|| {
            searcher.search_tier3_fuzzy(black_box("tensr"), black_box(&exclude), black_box(10))
        })
    });

    c.bench_function("t3_fuzzy_wrop", |b| {
        b.iter(|| {
            searcher.search_tier3_fuzzy(black_box("wrop"), black_box(&exclude), black_box(10))
        })
    });
}

// ============================================================================
// FULL THREE-TIER SEARCH BENCHMARKS
// ============================================================================

fn bench_full_search(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();

    // Exact match queries (T1 dominant)
    c.bench_function("full_search_kernel_exact", |b| {
        b.iter(|| searcher.search(black_box("kernel"), black_box(10)))
    });

    c.bench_function("full_search_gemm_exact", |b| {
        b.iter(|| searcher.search(black_box("gemm"), black_box(10)))
    });

    // Fuzzy queries (T1 miss, may hit T2 or T3)
    c.bench_function("full_search_kernl_fuzzy", |b| {
        b.iter(|| searcher.search(black_box("kernl"), black_box(10)))
    });

    c.bench_function("full_search_gemma_fuzzy", |b| {
        b.iter(|| searcher.search(black_box("gemma"), black_box(10)))
    });

    // Prefix queries
    c.bench_function("full_search_kern_prefix", |b| {
        b.iter(|| searcher.search(black_box("kern"), black_box(10)))
    });

    c.bench_function("full_search_gem_prefix", |b| {
        b.iter(|| searcher.search(black_box("gem"), black_box(10)))
    });
}

fn bench_full_search_limit_variations(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();
    let mut group = c.benchmark_group("full_search_limit_variations");

    for limit in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.iter(|| searcher.search(black_box("kernel"), black_box(limit)))
        });
    }

    group.finish();
}

// ============================================================================
// COMPARATIVE TIER BENCHMARKS
// ============================================================================

fn bench_tier_comparison(c: &mut Criterion) {
    let searcher = load_cutlass_searcher();
    let exclude = HashSet::new();
    let mut group = c.benchmark_group("tier_comparison");

    // Compare all three tiers on same query
    group.bench_function("tier1_kernel", |b| {
        b.iter(|| searcher.search_tier1_exact(black_box("kernel"), black_box(10)))
    });

    group.bench_function("tier2_kernel", |b| {
        b.iter(|| {
            searcher.search_tier2_prefix(black_box("kernel"), black_box(&exclude), black_box(10))
        })
    });

    group.bench_function("tier3_kernl", |b| {
        b.iter(|| {
            searcher.search_tier3_fuzzy(black_box("kernl"), black_box(&exclude), black_box(10))
        })
    });

    group.finish();
}

// ============================================================================
// INITIALIZATION BENCHMARK
// ============================================================================

fn bench_searcher_initialization(c: &mut Criterion) {
    c.bench_function("load_searcher_from_bytes", |b| {
        b.iter(|| {
            let bytes = fs::read("target/datasets/cutlass/index.sorex")
                .expect("Failed to read index");
            let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to parse");
            let _ = TierSearcher::from_layer(layer).expect("Failed to create");
        })
    });
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

criterion_group!(
    name = benches;
    config = Criterion::default()
        .measurement_time(std::time::Duration::from_secs(5))
        .sample_size(100);
    targets =
        bench_t1_exact_matches,
        bench_t1_limit_variations,
        bench_t2_prefix_matches,
        bench_t2_with_exclusions,
        bench_t3_fuzzy_matches,
        bench_full_search,
        bench_full_search_limit_variations,
        bench_tier_comparison,
        bench_searcher_initialization
);

criterion_main!(benches);
