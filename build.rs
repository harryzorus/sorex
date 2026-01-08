//! Build script for sorex crate.
//!
//! When the `embed-wasm` feature is enabled, this script automatically:
//! 1. Builds the WASM module using wasm-pack
//! 2. Builds the JavaScript loader using bun
//! 3. Copies artifacts to OUT_DIR for include_bytes!/include_str!

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap_or_default();

    // Skip WASM build when we ARE the WASM target (prevents infinite recursion)
    let is_wasm_target = target.contains("wasm32");

    // Only run WASM build when embed-wasm feature is enabled AND not building for wasm
    if env::var("CARGO_FEATURE_EMBED_WASM").is_ok() && !is_wasm_target {
        build_wasm(&out_dir);
        build_loader(&out_dir);
    }

    // Tell rustc where to find the artifacts
    println!("cargo:rustc-env=SOREX_OUT_DIR={out_dir}");
}

fn build_wasm(out_dir: &str) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let pkg_dir = Path::new(&manifest_dir).join("pkg");
    let wasm_dest = Path::new(out_dir).join("sorex_bg.wasm");

    // Rerun if source files change
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/search.rs");
    println!("cargo:rerun-if-changed=src/types.rs");
    println!("cargo:rerun-if-changed=src/scoring.rs");
    println!("cargo:rerun-if-changed=src/levenshtein.rs");
    println!("cargo:rerun-if-changed=src/levenshtein_dfa.rs");

    // Build WASM if pkg doesn't exist
    let wasm_src = pkg_dir.join("sorex_bg.wasm");
    if !wasm_src.exists() {
        println!("cargo:warning=Building WASM module with wasm-pack...");

        // Use separate target dir to avoid deadlock with outer cargo
        let wasm_target_dir = Path::new(out_dir).join("wasm-target");

        // Clear ALL cargo env vars to prevent deadlock/interference
        let mut cmd = Command::new("wasm-pack");
        cmd.args([
            "build",
            "--target",
            "web",
            "--no-default-features",
            "--features",
            "wasm",
        ])
        .current_dir(&manifest_dir)
        .env("CARGO_TARGET_DIR", &wasm_target_dir);

        // Remove all CARGO_* env vars that outer cargo sets
        for (key, _) in env::vars() {
            if key.starts_with("CARGO") && key != "CARGO_TARGET_DIR" {
                cmd.env_remove(&key);
            }
        }

        let status = cmd
            .status()
            .expect("Failed to run wasm-pack. Install it with: cargo install wasm-pack");

        if !status.success() {
            panic!("wasm-pack build failed");
        }
    }

    // Copy WASM to OUT_DIR
    fs::copy(&wasm_src, &wasm_dest).expect("Failed to copy WASM to OUT_DIR");
}

fn build_loader(out_dir: &str) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let loader_src_dir = Path::new(&manifest_dir).join("src/build/loader");
    let loader_src = Path::new(&manifest_dir).join("target/loader/sorex-loader.js");
    let loader_map_src = Path::new(&manifest_dir).join("target/loader/sorex-loader.js.map");
    let loader_dest = Path::new(out_dir).join("sorex-loader.js");
    let loader_map_dest = Path::new(out_dir).join("sorex-loader.js.map");

    // Rerun if loader source changes
    println!("cargo:rerun-if-changed=src/build/loader/index.ts");
    println!("cargo:rerun-if-changed=src/build/loader/parser.ts");
    println!("cargo:rerun-if-changed=src/build/loader/searcher.ts");
    println!("cargo:rerun-if-changed=src/build/loader/wasm-state.ts");
    println!("cargo:rerun-if-changed=src/build/loader/imports.ts");
    println!("cargo:rerun-if-changed=src/build/loader/build.ts");

    // Build loader if source doesn't exist
    if !loader_src.exists() {
        println!("cargo:warning=Building JavaScript loader with bun...");

        let status = Command::new("bun")
            .args(["run", "build.ts"])
            .current_dir(&loader_src_dir)
            .status()
            .expect("Failed to run bun. Install it from: https://bun.sh");

        if !status.success() {
            panic!("bun build failed");
        }
    }

    // Copy to OUT_DIR
    fs::copy(&loader_src, &loader_dest).expect("Failed to copy loader JS to OUT_DIR");
    fs::copy(&loader_map_src, &loader_map_dest).expect("Failed to copy loader map to OUT_DIR");
}
