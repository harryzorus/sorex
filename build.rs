//! Build script for sorex crate.
//!
//! When the `embed-wasm` feature is enabled, this script automatically:
//! 1. Builds the WASM module using wasm-pack
//! 2. Optionally optimizes with wasm-opt if installed (not required)
//! 3. Builds the JavaScript loader using deno
//! 4. Copies artifacts to OUT_DIR for include_bytes!/include_str!

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let target = env::var("TARGET").unwrap_or_default();

    // Skip WASM build when we ARE the WASM target (prevents infinite recursion)
    let is_wasm_target = target.contains("wasm32");

    // Only run WASM build when embed-wasm feature is enabled AND not building for wasm
    if env::var("CARGO_FEATURE_EMBED_WASM").is_ok() && !is_wasm_target {
        // Find system wasm-opt (optional optimization)
        let wasm_opt_path = find_wasm_opt();

        // Build WASM and optionally optimize
        build_wasm(&out_dir, wasm_opt_path.as_ref());
        build_loader(&out_dir);
    }

    // Tell rustc where to find the artifacts
    println!("cargo:rustc-env=SOREX_OUT_DIR={out_dir}");

    // Tell src/build/mod.rs whether we should try to include the loader
    if is_wasm_target {
        println!("cargo:rustc-env=SOREX_SKIP_LOADER=1");
    }
}

/// Find wasm-opt in system PATH or common locations.
/// Returns None if not found (optimization is optional).
fn find_wasm_opt() -> Option<PathBuf> {
    // Check PATH first
    if let Ok(output) = Command::new("which").arg("wasm-opt").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Some(PathBuf::from(path));
            }
        }
    }

    // Check common Homebrew locations
    let homebrew_paths = ["/opt/homebrew/bin/wasm-opt", "/usr/local/bin/wasm-opt"];

    for path in homebrew_paths {
        if Path::new(path).exists() {
            return Some(PathBuf::from(path));
        }
    }

    // wasm-opt not found - this is OK, optimization is optional
    println!("cargo:warning=wasm-opt not found, skipping WASM optimization");
    None
}

/// Run wasm-opt on the WASM file for size optimization.
fn optimize_wasm(wasm_path: &Path, wasm_opt_path: &Path) {
    let size_before = fs::metadata(wasm_path).map(|m| m.len()).unwrap_or(0);

    let optimized_path = wasm_path.with_extension("wasm.optimized");

    let status = Command::new(wasm_opt_path)
        .args([
            "-O4",
            "--enable-bulk-memory",
            "--enable-nontrapping-float-to-int",
        ])
        .arg(wasm_path)
        .arg("-o")
        .arg(&optimized_path)
        .status();

    match status {
        Ok(s) if s.success() => {
            if let Err(e) = fs::rename(&optimized_path, wasm_path) {
                println!("cargo:warning=Failed to replace WASM: {e}");
            } else {
                let size_after = fs::metadata(wasm_path).map(|m| m.len()).unwrap_or(0);
                println!(
                    "cargo:warning=WASM optimized: {} -> {} bytes",
                    size_before, size_after
                );
            }
        }
        Ok(_) => {
            println!("cargo:warning=wasm-opt failed");
            let _ = fs::remove_file(&optimized_path);
        }
        Err(e) => {
            println!("cargo:warning=wasm-opt error: {e}");
            let _ = fs::remove_file(&optimized_path);
        }
    }
}

fn build_wasm(out_dir: &str, wasm_opt_path: Option<&PathBuf>) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let pkg_dir = Path::new(&manifest_dir).join("target/pkg");
    let wasm_dest = Path::new(out_dir).join("sorex_bg.wasm");

    // Source files that affect WASM output
    let wasm_sources = [
        "src/lib.rs",
        "src/types.rs",
        "src/runtime/wasm.rs",
        "src/scoring/core.rs",
        "src/scoring/ranking.rs",
        "src/search/tiered.rs",
        "src/search/dedup.rs",
        "src/fuzzy/levenshtein.rs",
        "src/fuzzy/dfa.rs",
    ];

    // Rerun if source files change
    for src in &wasm_sources {
        println!("cargo:rerun-if-changed={}", src);
    }

    // Check if WASM needs rebuilding
    let wasm_src = pkg_dir.join("sorex_bg.wasm");
    let needs_rebuild = if !wasm_src.exists() {
        println!("cargo:warning=WASM not found, building...");
        true
    } else {
        // Check if any source file is newer than the WASM
        let wasm_mtime = fs::metadata(&wasm_src).and_then(|m| m.modified()).ok();

        let any_source_newer = wasm_sources.iter().any(|src| {
            let src_path = Path::new(&manifest_dir).join(src);
            if let (Some(wasm_time), Ok(src_meta)) = (wasm_mtime, fs::metadata(&src_path)) {
                if let Ok(src_time) = src_meta.modified() {
                    if src_time > wasm_time {
                        println!(
                            "cargo:warning=Source {} is newer than WASM, rebuilding...",
                            src
                        );
                        return true;
                    }
                }
            }
            false
        });

        any_source_newer
    };

    if needs_rebuild {
        println!("cargo:warning=Building WASM module with wasm-pack...");

        // Use separate target dir to avoid deadlock with outer cargo
        let wasm_target_dir = Path::new(out_dir).join("wasm-target");

        // Create temporary build directory with config (crates.io cache is read-only)
        let temp_build_dir = Path::new(out_dir).join("wasm-build");
        // Clean up any previous build attempt
        let _ = fs::remove_dir_all(&temp_build_dir);
        fs::create_dir_all(&temp_build_dir).ok();

        // Create .cargo/config.toml in the temp directory
        let cargo_config_dir = temp_build_dir.join(".cargo");
        fs::create_dir_all(&cargo_config_dir).ok();
        fs::write(
            cargo_config_dir.join("config.toml"),
            r#"[target.wasm32-unknown-unknown]
rustflags = [
  "-Ctarget-feature=+simd128,+atomics,+bulk-memory",
  "-Clink-arg=--shared-memory",
  "-Clink-arg=--max-memory=1073741824",
  "-Clink-arg=--import-memory",
  "-Clink-arg=--export=__wasm_init_tls",
  "-Clink-arg=--export=__tls_size",
  "-Clink-arg=--export=__tls_align",
  "-Clink-arg=--export=__tls_base",
]

[unstable]
build-std = ["panic_abort", "std"]
"#,
        )
        .expect("Failed to create .cargo/config.toml");

        // Create standalone Cargo.toml with cdylib in temp directory
        // Remove workspace references since we're building in isolation
        let cargo_toml_path = Path::new(&manifest_dir).join("Cargo.toml");
        let original_cargo_toml =
            fs::read_to_string(&cargo_toml_path).expect("Failed to read Cargo.toml");

        // Remove workspace section and inline workspace dependencies
        let mut temp_cargo_toml = original_cargo_toml.clone();

        // Remove [workspace] section entirely
        if let Some(ws_start) = temp_cargo_toml.find("[workspace]") {
            if let Some(next_section) = temp_cargo_toml[ws_start + 11..].find("\n[") {
                temp_cargo_toml = format!(
                    "{}{}",
                    &temp_cargo_toml[..ws_start],
                    &temp_cargo_toml[ws_start + 11 + next_section + 1..]
                );
            }
        }

        // Replace workspace dependency references with inline versions
        temp_cargo_toml = temp_cargo_toml.replace(
            "serde = { workspace = true }",
            r#"serde = { version = "1.0", features = ["derive"] }"#,
        );

        // Remove path from sorex-lean-macros (use crates.io version for isolated build)
        temp_cargo_toml = temp_cargo_toml.replace(
            r#"sorex-lean-macros = { version = "1.0.0", path = "macros", optional = true }"#,
            r#"sorex-lean-macros = { version = "1.0.0", optional = true }"#,
        );

        // Remove [[bench]] sections (benches aren't in the temp dir)
        while let Some(bench_start) = temp_cargo_toml.find("[[bench]]") {
            // Find the end of this bench section (next section or end of file)
            let section_end = temp_cargo_toml[bench_start + 9..]
                .find("\n[")
                .map(|i| bench_start + 9 + i)
                .unwrap_or(temp_cargo_toml.len());
            temp_cargo_toml = format!(
                "{}{}",
                &temp_cargo_toml[..bench_start],
                &temp_cargo_toml[section_end..]
            );
        }

        // Add cdylib if needed (wasm-pack requires it)
        if temp_cargo_toml.contains(r#"crate-type = ["rlib"]"#) {
            temp_cargo_toml = temp_cargo_toml.replace(
                r#"crate-type = ["rlib"]"#,
                r#"crate-type = ["cdylib", "rlib"]"#,
            );
        }

        // Add empty [workspace] to make it independent (not part of parent workspace)
        temp_cargo_toml.push_str("\n[workspace]\n");

        fs::write(temp_build_dir.join("Cargo.toml"), &temp_cargo_toml)
            .expect("Failed to write temp Cargo.toml");

        // Symlink src directory to temp build dir
        let src_link = temp_build_dir.join("src");
        if !src_link.exists() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(Path::new(&manifest_dir).join("src"), &src_link).ok();
            #[cfg(windows)]
            std::os::windows::fs::symlink_dir(Path::new(&manifest_dir).join("src"), &src_link).ok();
        }

        // Create empty build.rs (wasm build doesn't need it but cargo expects it)
        fs::write(temp_build_dir.join("build.rs"), "fn main() {}").ok();

        // Clear ALL cargo env vars to prevent deadlock/interference
        let mut cmd = Command::new("wasm-pack");
        // Use multi-threaded WASM with atomics for parallel search
        cmd.args(["build", "--target", "web", "--release", "--out-dir"])
            .arg(pkg_dir.to_str().unwrap()) // Output to original pkg location
            .args(["--no-default-features", "--features", "wasm,serde_json"])
            .current_dir(&temp_build_dir)
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

        // Run wasm-opt -O4 if available
        if let Some(wasm_opt) = wasm_opt_path {
            optimize_wasm(&wasm_src, wasm_opt);
        }
    }

    // Copy WASM to OUT_DIR
    fs::copy(&wasm_src, &wasm_dest).expect("Failed to copy WASM to OUT_DIR");
}

fn build_loader(out_dir: &str) {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let tools_dir = Path::new(&manifest_dir).join("tools");
    let build_script = tools_dir.join("build.ts");
    let loader_ts = tools_dir.join("loader.ts");
    let pkg_js = Path::new(&manifest_dir).join("target/pkg/sorex.js");
    let loader_output = Path::new(&manifest_dir).join("target/loader/sorex.js");
    let loader_dest = Path::new(out_dir).join("sorex.js");

    // Rerun if build script, loader source, or wasm-pack output changes
    println!("cargo:rerun-if-changed=tools/build.ts");
    println!("cargo:rerun-if-changed=tools/loader.ts");
    println!("cargo:rerun-if-changed=target/pkg/sorex.js");

    // Check if loader needs rebuilding
    let needs_rebuild = if !loader_output.exists() {
        println!("cargo:warning=Loader not found, building...");
        true
    } else {
        // Check if any source is newer than loader output
        let loader_mtime = fs::metadata(&loader_output).and_then(|m| m.modified()).ok();
        let pkg_mtime = fs::metadata(&pkg_js).and_then(|m| m.modified()).ok();
        let build_mtime = fs::metadata(&build_script).and_then(|m| m.modified()).ok();
        let loader_ts_mtime = fs::metadata(&loader_ts).and_then(|m| m.modified()).ok();

        match (loader_mtime, pkg_mtime, build_mtime, loader_ts_mtime) {
            (Some(loader), Some(pkg), Some(build), Some(ts)) => {
                if pkg > loader {
                    println!(
                        "cargo:warning=target/pkg/sorex.js is newer than loader, rebuilding..."
                    );
                    true
                } else if build > loader {
                    println!("cargo:warning=tools/build.ts is newer than loader, rebuilding...");
                    true
                } else if ts > loader {
                    println!("cargo:warning=tools/loader.ts is newer than loader, rebuilding...");
                    true
                } else {
                    false
                }
            }
            _ => true,
        }
    };

    if needs_rebuild {
        println!("cargo:warning=Building JavaScript loader with deno...");

        let status = Command::new("deno")
            .args(["task", "build"])
            .current_dir(&tools_dir)
            .status()
            .expect("Failed to run deno. Install it from: https://deno.land");

        if !status.success() {
            panic!("deno task build failed");
        }
    }

    // Copy to OUT_DIR
    fs::copy(&loader_output, &loader_dest).expect("Failed to copy loader JS to OUT_DIR");
}
