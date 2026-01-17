//! Criterion benchmarks for WASM search via Deno runtime.
//!
//! Measures WASM performance to compare against native Rust (bench_searcher.rs).
//! Uses the Deno JavaScript runtime to execute the same WASM that runs in browsers.
//!
//! Two benchmark modes:
//! - "Cold" benchmarks: Include WASM initialization overhead (like first page load)
//! - "Warm" benchmarks: Reuse initialized WASM (like subsequent searches)
//!
//! Run with: cargo bench --bench bench_wasm --features deno-runtime

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::cell::RefCell;
use std::fs;

#[cfg(feature = "deno-runtime")]
use sorex::deno_runtime::{DenoRuntime, DenoSearchContext};

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

#[cfg(feature = "deno-runtime")]
fn load_loader_js() -> String {
    let paths = [
        "target/loader/sorex.js",
        "target/datasets/cutlass/sorex.js",
    ];

    for path in paths {
        if let Ok(js) = fs::read_to_string(path) {
            return js;
        }
    }

    panic!(
        "Could not find sorex.js. Run: bun scripts/build-loader.ts\nTried: {:?}",
        paths
    );
}

#[cfg(feature = "deno-runtime")]
fn load_sorex_bytes() -> Vec<u8> {
    let path = "target/datasets/cutlass/index.sorex";
    fs::read(path).expect("Failed to read Cutlass index file")
}

// ============================================================================
// WARM WASM BENCHMARKS (WASM initialized once, reused for all searches)
// ============================================================================

#[cfg(feature = "deno-runtime")]
fn bench_wasm_warm_search(c: &mut Criterion) {
    let sorex_bytes = load_sorex_bytes();
    let loader_js = load_loader_js();

    // Create persistent context - WASM initialized once
    let ctx = RefCell::new(
        DenoSearchContext::new(&sorex_bytes, &loader_js).expect("Failed to create DenoSearchContext"),
    );

    // Warm up TurboFan before benchmarking
    // V8 uses tiered compilation: Liftoff -> TurboFan
    // We need ~100 iterations per query to trigger TurboFan optimization
    {
        let mut ctx = ctx.borrow_mut();
        ctx.warmup_turbofan("kernel", 10);
        ctx.warmup_turbofan("gemm", 10);
        ctx.warmup_turbofan("tensor", 10);
        ctx.warmup_turbofan("kernl", 10);  // fuzzy
        ctx.warmup_turbofan("gem", 10);    // prefix
    }

    let mut group = c.benchmark_group("wasm_warm");

    // Exact match queries
    group.bench_function("kernel_exact", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("kernel"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("gemm_exact", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("gemm"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("tensor_exact", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("tensor"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("warp_exact", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("warp"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("mma_exact", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("mma"), black_box(10))
                .unwrap()
                .len()
        })
    });

    // Fuzzy queries
    group.bench_function("kernl_fuzzy", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("kernl"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("gemma_fuzzy", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("gemma"), black_box(10))
                .unwrap()
                .len()
        })
    });

    // Typo queries
    group.bench_function("epilouge_typo", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("epilouge"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("syncronize_typo", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("syncronize"), black_box(10))
                .unwrap()
                .len()
        })
    });

    // Prefix queries
    group.bench_function("kern_prefix", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("kern"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.bench_function("gem_prefix", |b| {
        b.iter(|| {
            ctx.borrow_mut()
                .search(black_box("gem"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.finish();
}

#[cfg(feature = "deno-runtime")]
fn bench_wasm_warm_limit_variations(c: &mut Criterion) {
    let sorex_bytes = load_sorex_bytes();
    let loader_js = load_loader_js();
    let ctx = RefCell::new(
        DenoSearchContext::new(&sorex_bytes, &loader_js).expect("Failed to create DenoSearchContext"),
    );

    let mut group = c.benchmark_group("wasm_warm_limit");

    for limit in [1, 5, 10, 20].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(limit), limit, |b, &limit| {
            b.iter(|| {
                ctx.borrow_mut()
                    .search(black_box("kernel"), black_box(limit))
                    .unwrap()
                    .len()
            })
        });
    }

    group.finish();
}

// ============================================================================
// COLD WASM BENCHMARKS (Full initialization on each search)
// ============================================================================

#[cfg(feature = "deno-runtime")]
fn bench_wasm_cold_initialization(c: &mut Criterion) {
    let sorex_bytes = load_sorex_bytes();
    let loader_js = load_loader_js();

    c.bench_function("wasm_cold_full_init", |b| {
        b.iter(|| {
            let runtime = DenoRuntime::new().expect("Failed to create runtime");
            runtime
                .search(&sorex_bytes, &loader_js, "kernel", 1)
                .unwrap()
                .len()
        })
    });
}

#[cfg(feature = "deno-runtime")]
fn bench_wasm_context_creation(c: &mut Criterion) {
    let sorex_bytes = load_sorex_bytes();
    let loader_js = load_loader_js();

    c.bench_function("wasm_context_creation", |b| {
        b.iter(|| {
            let _ctx = DenoSearchContext::new(&sorex_bytes, &loader_js)
                .expect("Failed to create DenoSearchContext");
        })
    });
}

// ============================================================================
// COMPARATIVE: NATIVE vs WASM (WARM)
// ============================================================================

#[cfg(feature = "deno-runtime")]
fn bench_native_vs_wasm_warm(c: &mut Criterion) {
    use sorex::binary::LoadedLayer;
    use sorex::tiered_search::TierSearcher;

    let mut group = c.benchmark_group("native_vs_wasm_warm");

    // Load native searcher
    let bytes = load_sorex_bytes();
    let layer = LoadedLayer::from_bytes(&bytes).expect("Failed to load index");
    let native_searcher = TierSearcher::from_layer(layer).expect("Failed to create searcher");

    // Load WASM context (initialized once)
    let loader_js = load_loader_js();
    let wasm_ctx = RefCell::new(
        DenoSearchContext::new(&bytes, &loader_js).expect("Failed to create DenoSearchContext"),
    );

    // Compare on exact match
    group.bench_function("native_kernel", |b| {
        b.iter(|| native_searcher.search(black_box("kernel"), black_box(10)))
    });

    group.bench_function("wasm_kernel", |b| {
        b.iter(|| {
            wasm_ctx
                .borrow_mut()
                .search(black_box("kernel"), black_box(10))
                .unwrap()
                .len()
        })
    });

    // Compare on fuzzy
    group.bench_function("native_kernl_fuzzy", |b| {
        b.iter(|| native_searcher.search(black_box("kernl"), black_box(10)))
    });

    group.bench_function("wasm_kernl_fuzzy", |b| {
        b.iter(|| {
            wasm_ctx
                .borrow_mut()
                .search(black_box("kernl"), black_box(10))
                .unwrap()
                .len()
        })
    });

    // Compare on prefix
    group.bench_function("native_gem_prefix", |b| {
        b.iter(|| native_searcher.search(black_box("gem"), black_box(10)))
    });

    group.bench_function("wasm_gem_prefix", |b| {
        b.iter(|| {
            wasm_ctx
                .borrow_mut()
                .search(black_box("gem"), black_box(10))
                .unwrap()
                .len()
        })
    });

    group.finish();
}

// ============================================================================
// CRITERION CONFIGURATION
// ============================================================================

#[cfg(feature = "deno-runtime")]
criterion_group!(
    name = wasm_benches;
    config = Criterion::default()
        .measurement_time(std::time::Duration::from_secs(5))
        .sample_size(100);
    targets =
        bench_wasm_warm_search,
        bench_wasm_warm_limit_variations,
        bench_wasm_cold_initialization,
        bench_wasm_context_creation,
        bench_native_vs_wasm_warm
);

#[cfg(feature = "deno-runtime")]
criterion_main!(wasm_benches);

// Stub for non-deno builds
#[cfg(not(feature = "deno-runtime"))]
fn main() {
    eprintln!("WASM benchmarks require deno-runtime feature.");
    eprintln!("Run with: cargo bench --bench bench_wasm --features deno-runtime");
}
