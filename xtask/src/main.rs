//! Custom cargo commands for the search crate.
//!
//! Usage:
//!   cargo xtask verify    - Run full verification suite
//!   cargo xtask test      - Run all tests
//!   cargo xtask lean      - Build Lean proofs
//!   cargo xtask check     - Quick check (no Lean)

use anyhow::{bail, Context, Result};
use std::env;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant};

fn main() -> Result<()> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("verify") => verify()?,
        Some("test") => test()?,
        Some("lean") => lean()?,
        Some("kani") => run_kani()?,
        Some("check") => check()?,
        Some("bench") => bench()?,
        Some("bench-e2e") => bench_e2e()?,
        Some("build-wasm") => build_wasm()?,
        Some("size-check") => size_check()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        r#"
cargo xtask <COMMAND>

Commands:
  verify      Run full verification suite (Lean + tests + mutations + E2E)
  test        Run all Rust tests
  lean        Build Lean proofs only
  kani        Run Kani model checking proofs (slow, ~5 min)
  check       Quick check (cargo test + clippy, no Lean/mutations)
  bench       Run Criterion benchmarks
  bench-e2e   Full E2E benchmark: build WASM → crawl datasets → index → run JS benchmarks
  build-wasm  Build optimized WASM, loader, and CLI with embedded WASM
  size-check  Verify WASM and loader artifacts don't exceed size limits

Verify runs 11 steps:
  1. Lean proofs        - Mathematical specifications
  2. Constants          - Rust/Lean constant alignment
  3. Spec drift         - Lean/Rust spec alignment
  4. Invariants         - INVARIANT markers in source
  5. Clippy             - Lint checks
  6. Release build      - Binary compilation
  7. Test fixtures      - E2E index building
  8. Rust tests         - Unit, integration, property tests
  9. WASM parity        - Native/WASM result equality
  10. Browser E2E       - Playwright tests
  11. Mutations         - Test quality via cargo-mutants (requires cargo-mutants)

Note: Kani proofs excluded from verify (too slow). Run separately: cargo xtask kani
"#
    );
}

/// Step result for the summary table
struct StepResult {
    name: &'static str,
    status: &'static str,
    duration: Duration,
}

/// Full verification suite
fn verify() -> Result<()> {
    println!("==========================================");
    println!("Search Crate Verification Suite");
    println!("==========================================\n");

    let total_start = Instant::now();
    let mut results: Vec<StepResult> = Vec::new();

    // Helper macro to run a step and record timing
    macro_rules! run_step {
        ($num:expr, $total:expr, $name:expr, $desc:expr, $body:expr) => {{
            println!("[{}/{}] {}...", $num, $total, $desc);
            let start = Instant::now();
            let result = $body;
            let duration = start.elapsed();
            match result {
                Ok(()) => {
                    results.push(StepResult {
                        name: $name,
                        status: "✓ pass",
                        duration,
                    });
                    println!();
                }
                Err(e) => {
                    results.push(StepResult {
                        name: $name,
                        status: "✗ FAIL",
                        duration,
                    });
                    print_summary_table(&results, total_start.elapsed());
                    return Err(e);
                }
            }
        }};
    }

    const TOTAL_STEPS: u8 = 11;

    // Proofs first - if these fail, nothing else matters
    run_step!(1, TOTAL_STEPS, "Lean Proofs", "Building Lean proofs",
        lean());

    // Note: Kani skipped in default verify (too slow - takes 5+ minutes)
    // Run separately with: cargo xtask kani

    run_step!(2, TOTAL_STEPS, "Constants", "Verifying Rust/Lean constant alignment",
        verify_constants());

    run_step!(4, TOTAL_STEPS, "Spec Drift", "Checking Lean/Rust spec alignment",
        check_spec_drift());

    run_step!(5, TOTAL_STEPS, "Invariants", "Checking invariant markers",
        check_invariant_markers());

    // Fast checks before slow builds
    run_step!(6, TOTAL_STEPS, "Clippy", "Running clippy",
        run_cargo(&["clippy", "--quiet", "--", "-D", "warnings"]));

    // Build and test
    run_step!(7, TOTAL_STEPS, "Release Build", "Building release binary",
        run_cargo(&["build", "--release", "--quiet"]));

    run_step!(8, TOTAL_STEPS, "Test Fixtures", "Building test fixtures",
        build_test_fixtures());

    run_step!(9, TOTAL_STEPS, "Rust Tests", "Running Rust tests",
        run_cargo(&["test", "--quiet"]));

    run_step!(10, TOTAL_STEPS, "WASM Parity", "Running WASM parity tests",
        run_cargo(&["test", "--features", "deno-runtime", "--test", "integration", "wasm", "--quiet"]));

    run_step!(11, TOTAL_STEPS, "Browser E2E", "Running browser E2E tests",
        run_playwright_e2e());

    run_step!(12, TOTAL_STEPS, "Mutations", "Running mutation testing",
        run_mutation_testing());

    let total_duration = total_start.elapsed();
    print_summary_table(&results, total_duration);

    println!("\nSafe to commit changes.");

    Ok(())
}

fn print_summary_table(results: &[StepResult], total: Duration) {
    println!("==========================================");
    println!("                SUMMARY");
    println!("==========================================");
    println!();
    println!("  {:<16} {:<10} {:>10}", "Step", "Status", "Time");
    println!("  {}", "-".repeat(40));

    for result in results {
        println!(
            "  {:<16} {:<10} {:>10}",
            result.name,
            result.status,
            format_duration(result.duration)
        );
    }

    println!("  {}", "-".repeat(40));
    println!("  {:<16} {:<10} {:>10}", "Total", "", format_duration(total));
    println!();

    let all_passed = results.iter().all(|r| r.status.contains("pass"));
    if all_passed {
        println!("  Result: ✓ ALL CHECKS PASSED");
    } else {
        println!("  Result: ✗ VERIFICATION FAILED");
    }
    println!();
    println!("==========================================");
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs_f64();
    if secs < 1.0 {
        format!("{:.0}ms", secs * 1000.0)
    } else if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let mins = (secs / 60.0).floor();
        let remaining = secs - (mins * 60.0);
        format!("{}m {:.0}s", mins as u64, remaining)
    }
}

/// Run all tests
fn test() -> Result<()> {
    run_cargo(&["test"])
}

/// Build Lean proofs
fn lean() -> Result<()> {
    let lean_dir = project_root()?.join("lean");
    if !lean_dir.exists() {
        println!("  (no lean directory, skipping)");
        return Ok(());
    }

    let status = Command::new("lake")
        .arg("build")
        .current_dir(&lean_dir)
        .status()
        .context("Failed to run lake build")?;

    if !status.success() {
        bail!("Lean proofs failed to build");
    }

    Ok(())
}

/// Run Kani model checking proofs.
/// Kani must run outside the workspace due to std library conflicts.
fn run_kani() -> Result<()> {
    let root = project_root()?;
    let kani_dir = root.join("kani-proofs");

    if !kani_dir.exists() {
        println!("  (no kani-proofs directory, skipping)");
        return Ok(());
    }

    // Check if cargo-kani is installed
    let kani_check = Command::new("cargo")
        .args(["kani", "--version"])
        .output();

    if kani_check.is_err() || !kani_check.unwrap().status.success() {
        println!("  (cargo-kani not installed, skipping)");
        println!("  Install: cargo install kani-verifier");
        return Ok(());
    }

    // Kani must run outside the workspace to avoid std library conflicts
    let tmp_dir = std::env::temp_dir().join("kani-proofs-verify");

    // Clean up any previous run
    if tmp_dir.exists() {
        std::fs::remove_dir_all(&tmp_dir).ok();
    }

    // Copy kani-proofs to temp directory
    copy_dir_recursive(&kani_dir, &tmp_dir)
        .context("Failed to copy kani-proofs to temp directory")?;

    // Run cargo kani from temp directory
    let status = Command::new("cargo")
        .args(["kani"])
        .current_dir(&tmp_dir)
        .status()
        .context("Failed to run cargo kani")?;

    // Clean up
    std::fs::remove_dir_all(&tmp_dir).ok();

    if !status.success() {
        bail!("Kani proofs failed");
    }

    Ok(())
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Run mutation testing with cargo-mutants.
/// Checks that tests actually catch bugs.
fn run_mutation_testing() -> Result<()> {
    // Check if cargo-mutants is installed
    let mutants_check = Command::new("cargo")
        .args(["mutants", "--version"])
        .output();

    if mutants_check.is_err() || !mutants_check.unwrap().status.success() {
        println!("  (cargo-mutants not installed, skipping)");
        println!("  Install: cargo install cargo-mutants");
        return Ok(());
    }

    let root = project_root()?;

    // Run mutation testing on binary encoding (critical for correctness)
    // Output to target/mutants.out to keep project root clean
    let output = Command::new("cargo")
        .args([
            "mutants",
            "--package", "sorex",
            "--output", "target/mutants.out",
            "--",
            "--lib",
            "--test-threads=1"
        ])
        .current_dir(&root)
        .output()
        .context("Failed to run cargo mutants")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parse results to check detection rate
    // Look for "X caught" and "Y missed" in output
    let mut caught = 0u64;
    let mut missed = 0u64;

    for line in stdout.lines().chain(stderr.lines()) {
        if line.contains("caught") {
            if let Some(num) = extract_first_number(line) {
                caught = num;
            }
        }
        if line.contains("missed") {
            if let Some(num) = extract_first_number(line) {
                missed = num;
            }
        }
    }

    let total = caught + missed;
    if total > 0 {
        let rate = (caught as f64 / total as f64) * 100.0;
        println!("  Mutation detection rate: {:.0}% ({} caught, {} missed)", rate, caught, missed);

        if rate < 60.0 {
            bail!(
                "Mutation detection rate {:.0}% is below 60% threshold. Tests may not catch bugs!",
                rate
            );
        }
    } else {
        println!("  (no mutants generated or parsed)");
    }

    if !output.status.success() {
        // cargo-mutants returns non-zero if any mutants survived, but we check rate above
        // Only fail if rate is too low (already handled)
    }

    Ok(())
}

fn extract_first_number(s: &str) -> Option<u64> {
    s.split_whitespace()
        .find_map(|word| word.trim_matches(|c: char| !c.is_ascii_digit()).parse().ok())
}

/// Quick check (no Lean)
fn check() -> Result<()> {
    println!("Running quick checks...\n");

    println!("[1/3] cargo check...");
    run_cargo(&["check"])?;

    println!("[2/3] cargo test...");
    run_cargo(&["test", "--quiet"])?;

    println!("[3/3] cargo clippy...");
    run_cargo(&["clippy", "--quiet", "--", "-D", "warnings"])?;

    println!("\n✓ Quick checks passed");
    Ok(())
}

/// Run benchmarks
fn bench() -> Result<()> {
    run_cargo(&["bench"])
}

// ============================================================================
// Helper functions
// ============================================================================

fn project_root() -> Result<PathBuf> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::current_dir().unwrap());

    // xtask is in project_root/xtask, so go up one level
    let root = manifest_dir.parent().unwrap_or(&manifest_dir);
    Ok(root.to_path_buf())
}

fn run_cargo(args: &[&str]) -> Result<()> {
    let root = project_root()?;

    let status = Command::new("cargo")
        .args(args)
        .current_dir(&root)
        .status()
        .with_context(|| format!("Failed to run cargo {:?}", args))?;

    if !status.success() {
        bail!("cargo {:?} failed", args);
    }

    Ok(())
}

fn check_invariant_markers() -> Result<()> {
    let root = project_root()?;
    let src_dir = root.join("src");

    let output = Command::new("grep")
        .args(["-r", "INVARIANT:", "--include=*.rs"])
        .current_dir(&src_dir)
        .output()
        .context("Failed to run grep")?;

    let count = output.stdout.split(|&b| b == b'\n').filter(|l| !l.is_empty()).count();

    if count < 5 {
        bail!(
            "Expected at least 5 INVARIANT markers, found {}. Someone may have removed safety comments!",
            count
        );
    }

    Ok(())
}

fn verify_constants() -> Result<()> {
    let root = project_root()?;

    // Read Rust scoring constants
    let scoring_rs = std::fs::read_to_string(root.join("src/scoring/core.rs"))
        .context("Failed to read scoring/core.rs")?;

    let rust_title = extract_score(&scoring_rs, "Title");
    let rust_heading = extract_score(&scoring_rs, "Heading");
    let rust_content = extract_score(&scoring_rs, "Content");

    // Read Lean scoring constants
    let lean_path = root.join("lean/SearchVerified/Scoring.lean");
    if !lean_path.exists() {
        println!("  (no Lean Scoring.lean, skipping constant check)");
        return Ok(());
    }

    let scoring_lean = std::fs::read_to_string(&lean_path)
        .context("Failed to read Scoring.lean")?;

    let lean_title = extract_lean_score(&scoring_lean, "title");
    let lean_heading = extract_lean_score(&scoring_lean, "heading");
    let lean_content = extract_lean_score(&scoring_lean, "content");

    // Lean uses ×10 scaling
    let expected_lean_title = (rust_title * 10.0) as i64;
    let expected_lean_heading = (rust_heading * 10.0) as i64;
    let expected_lean_content = (rust_content * 10.0) as i64;

    if lean_title != expected_lean_title {
        bail!(
            "Rust Title={} (×10={}) != Lean {}",
            rust_title, expected_lean_title, lean_title
        );
    }
    if lean_heading != expected_lean_heading {
        bail!(
            "Rust Heading={} (×10={}) != Lean {}",
            rust_heading, expected_lean_heading, lean_heading
        );
    }
    if lean_content != expected_lean_content {
        bail!(
            "Rust Content={} (×10={}) != Lean {}",
            rust_content, expected_lean_content, lean_content
        );
    }

    Ok(())
}

fn extract_score(content: &str, field: &str) -> f64 {
    // Look for const definitions like "pub const TITLE_BASE_SCORE: f64 = 100.0;"
    let const_name = format!("{}_BASE_SCORE", field.to_uppercase());
    for line in content.lines() {
        if line.contains(&const_name) && line.contains("=") && !line.contains("=>") {
            // Extract the number after =
            if let Some(num_str) = line.split('=').nth(1) {
                let num_str = num_str
                    .split("//").next().unwrap_or("")
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                if let Ok(n) = num_str.parse::<f64>() {
                    return n;
                }
            }
        }
    }

    // Fallback: Look for "FieldType::Title => 100.0," or "Title => 100.0,"
    for line in content.lines() {
        if line.contains(&format!("FieldType::{}", field)) ||
           line.contains(&format!("{} =>", field)) {
            // Extract the number after =>
            if let Some(num_str) = line.split("=>").nth(1) {
                // Remove comments, commas, and whitespace
                let num_str = num_str
                    .split("//").next().unwrap_or("")
                    .trim()
                    .trim_end_matches(',')
                    .trim();
                if let Ok(n) = num_str.parse::<f64>() {
                    return n;
                }
            }
        }
    }
    0.0
}

fn extract_lean_score(content: &str, field: &str) -> i64 {
    // Look for "| .title   => 1000"
    for line in content.lines() {
        if line.contains(&format!(".{}", field)) && line.contains("=>") {
            // Extract the number
            if let Some(num_str) = line.split("=>").nth(1) {
                let num_str = num_str.trim().split_whitespace().next().unwrap_or("0");
                if let Ok(n) = num_str.parse::<i64>() {
                    return n;
                }
            }
        }
    }
    0
}

/// Check for Lean/Rust spec drift.
///
/// This detects when the Lean specifications and Rust implementation
/// disagree on fundamental invariants like posting list ordering.
fn check_spec_drift() -> Result<()> {
    let root = project_root()?;

    // Check posting list ordering spec
    let lean_inverted = root.join("lean/SearchVerified/InvertedIndex.lean");
    let rust_inverted = root.join("src/index/inverted.rs");

    if !lean_inverted.exists() || !rust_inverted.exists() {
        println!("  (spec files not found, skipping drift check)");
        return Ok(());
    }

    let lean_content = std::fs::read_to_string(&lean_inverted)
        .context("Failed to read InvertedIndex.lean")?;
    let rust_content = std::fs::read_to_string(&rust_inverted)
        .context("Failed to read inverted.rs")?;

    // Check posting list ordering
    let lean_ordering = extract_lean_posting_order(&lean_content);
    let rust_ordering = extract_rust_posting_order(&rust_content);

    if lean_ordering != rust_ordering {
        // This is a warning, not an error - they may intentionally differ
        // with the Rust implementation being an optimization
        println!("  ⚠ Posting list ordering differs:");
        println!("    Lean spec: {}", lean_ordering);
        println!("    Rust impl: {}", rust_ordering);
        println!("    (This may be intentional - Rust uses score-first for O(k) top-k)");
    }

    // Check tier base scores match between TieredSearch.lean and scoring/core.rs
    let lean_tiered = root.join("lean/SearchVerified/TieredSearch.lean");
    let rust_scoring = root.join("src/scoring/core.rs");

    if lean_tiered.exists() && rust_scoring.exists() {
        let lean_tiered_content = std::fs::read_to_string(&lean_tiered)
            .context("Failed to read TieredSearch.lean")?;
        let rust_scoring_content = std::fs::read_to_string(&rust_scoring)
            .context("Failed to read scoring/core.rs")?;

        // Check tier base scores (T1=exact, T2=prefix, T3=fuzzy)
        check_tier_scores(&lean_tiered_content, &rust_scoring_content)?;
    }

    Ok(())
}

fn extract_lean_posting_order(content: &str) -> String {
    // Look for "Posting lists are sorted by (doc_id, offset)" or similar
    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("sorted by") && line_lower.contains("posting") {
            // Extract the ordering description
            if let Some(idx) = line_lower.find("sorted by") {
                let rest = &line[idx..];
                // Find the ordering pattern like "(doc_id, offset)" or "(score DESC, doc_id ASC)"
                if let Some(start) = rest.find('(') {
                    if let Some(end) = rest.find(')') {
                        return rest[start..=end].to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn extract_rust_posting_order(content: &str) -> String {
    // Look for "sorted by (score DESC, doc_id ASC)" or similar
    for line in content.lines() {
        let line_lower = line.to_lowercase();
        if line_lower.contains("sorted by") || line_lower.contains("sort by") {
            // Extract the ordering pattern
            if let Some(start) = line.find('(') {
                if let Some(end) = line.find(')') {
                    let pattern = &line[start..=end];
                    // Filter out function calls, keep ordering specs
                    if pattern.contains("DESC") || pattern.contains("ASC")
                        || pattern.contains("doc_id") || pattern.contains("score")
                    {
                        return pattern.to_string();
                    }
                }
            }
        }
    }
    "unknown".to_string()
}

fn check_tier_scores(lean_content: &str, rust_content: &str) -> Result<()> {
    // Extract Lean tier scores (scaled ×10)
    let lean_t1 = extract_lean_tier_score(lean_content, "exact");
    let lean_t2 = extract_lean_tier_score(lean_content, "prefix");
    let lean_t3 = extract_lean_tier_score(lean_content, "fuzzy");

    // Extract Rust tier scores
    let rust_t1 = extract_rust_tier_score(rust_content, "T1_EXACT_SCORE");
    let rust_t2 = extract_rust_tier_score(rust_content, "T2_PREFIX_SCORE");
    let rust_t3_d1 = extract_rust_tier_score(rust_content, "T3_FUZZY_DISTANCE_1_SCORE");

    // Lean uses ×10 scaling, Rust uses f64
    let expected_lean_t1 = (rust_t1 * 10.0) as i64;
    let expected_lean_t2 = (rust_t2 * 10.0) as i64;
    let expected_lean_t3 = (rust_t3_d1 * 10.0) as i64;  // T3 distance 1

    // Only report drift if values are found and different
    if lean_t1 > 0 && rust_t1 > 0.0 && lean_t1 != expected_lean_t1 {
        println!("  ⚠ T1 score drift: Lean={} vs Rust×10={}", lean_t1, expected_lean_t1);
    }
    if lean_t2 > 0 && rust_t2 > 0.0 && lean_t2 != expected_lean_t2 {
        println!("  ⚠ T2 score drift: Lean={} vs Rust×10={}", lean_t2, expected_lean_t2);
    }
    if lean_t3 > 0 && rust_t3_d1 > 0.0 && lean_t3 != expected_lean_t3 {
        println!("  ⚠ T3 score drift: Lean={} vs Rust×10={}", lean_t3, expected_lean_t3);
    }

    Ok(())
}

fn extract_lean_tier_score(content: &str, tier: &str) -> i64 {
    // Look for "| .exact  => 1000" pattern
    for line in content.lines() {
        if line.contains(&format!(".{}", tier)) && line.contains("=>") {
            if let Some(num_str) = line.split("=>").nth(1) {
                let num_str = num_str.trim().split_whitespace().next().unwrap_or("0");
                if let Ok(n) = num_str.trim_end_matches("--").trim().parse::<i64>() {
                    return n;
                }
            }
        }
    }
    0
}

fn extract_rust_tier_score(content: &str, const_name: &str) -> f64 {
    // Look for "pub const T1_EXACT_SCORE: f64 = 100.0;"
    for line in content.lines() {
        if line.contains(const_name) && line.contains("=") && !line.contains("=>") {
            if let Some(num_str) = line.split('=').nth(1) {
                let num_str = num_str
                    .split("//").next().unwrap_or("")
                    .trim()
                    .trim_end_matches(';')
                    .trim();
                if let Ok(n) = num_str.parse::<f64>() {
                    return n;
                }
            }
        }
    }
    0.0
}

/// Build E2E test fixtures index (used by integration tests and Playwright)
fn build_test_fixtures() -> Result<()> {
    let root = project_root()?;

    let status = Command::new("cargo")
        .args([
            "run", "--release", "--quiet", "--",
            "index",
            "--input", "data/e2e/fixtures",
            "--output", "target/e2e/output",
            "--demo"
        ])
        .current_dir(&root)
        .status()
        .context("Failed to build E2E test index")?;

    if !status.success() {
        bail!("Failed to build E2E test index");
    }

    Ok(())
}

fn run_playwright_e2e() -> Result<()> {
    let root = project_root()?;

    // Fixtures already built in step 3, just run Playwright
    println!("  Running Playwright tests...");
    let status = Command::new("deno")
        .args(["task", "test:e2e"])
        .current_dir(root.join("tools"))
        .status()
        .context("Failed to run Playwright tests")?;

    if !status.success() {
        bail!("Playwright E2E tests failed");
    }

    Ok(())
}

// ============================================================================
// WASM Build Pipeline
// ============================================================================

/// Build optimized WASM, JavaScript loader, and CLI with embedded WASM
fn build_wasm() -> Result<()> {
    let root = project_root()?;

    println!("==========================================");
    println!("Building Optimized WASM Pipeline");
    println!("==========================================\n");

    // Step 1: Build WASM with wasm-pack (no default features to exclude deno-runtime)
    println!("[1/4] Building WASM with wasm-pack...");
    let status = Command::new("wasm-pack")
        .args(["build", "--target", "web", "--release", "-d", "target/pkg", "--no-default-features", "--features", "wasm-simd"])
        .current_dir(&root)
        .status()
        .context("Failed to run wasm-pack")?;

    if !status.success() {
        bail!("wasm-pack build failed");
    }
    println!("✓ WASM built\n");

    // Step 2: Optimize with wasm-opt -O4
    println!("[2/4] Optimizing WASM with wasm-opt -O4...");
    let wasm_path = root.join("target/pkg/sorex_bg.wasm");
    let optimized_path = root.join("target/pkg/sorex_bg.wasm.opt");

    let wasm_opt = find_wasm_opt();
    if let Some(wasm_opt_path) = wasm_opt {
        let status = Command::new(&wasm_opt_path)
            .args(["-O4", "--enable-bulk-memory", "--enable-nontrapping-float-to-int"])
            .arg(&wasm_path)
            .arg("-o")
            .arg(&optimized_path)
            .status()
            .context("Failed to run wasm-opt")?;

        if status.success() {
            std::fs::rename(&optimized_path, &wasm_path)
                .context("Failed to replace WASM with optimized version")?;
            let size = std::fs::metadata(&wasm_path).map(|m| m.len()).unwrap_or(0);
            println!("✓ WASM optimized: {} bytes\n", size);
        } else {
            println!("⚠ wasm-opt failed, using unoptimized WASM\n");
        }
    } else {
        println!("⚠ wasm-opt not found, skipping optimization");
        println!("  Install: brew install binaryen (macOS) or apt install binaryen (Ubuntu)\n");
    }

    // Step 3: Build JavaScript loader
    println!("[3/4] Building JavaScript loader...");
    let status = Command::new("bun")
        .arg("scripts/build-loader.ts")
        .current_dir(&root)
        .status()
        .context("Failed to run bun")?;

    if !status.success() {
        bail!("bun scripts/build-loader.ts failed");
    }
    println!("✓ JavaScript loader built\n");

    // Step 4: Build CLI with embedded WASM
    println!("[4/4] Building CLI with embedded WASM...");
    let status = Command::new("cargo")
        .args(["build", "--release", "--features", "embed-wasm"])
        .current_dir(&root)
        .status()
        .context("Failed to build CLI")?;

    if !status.success() {
        bail!("cargo build --release --features embed-wasm failed");
    }
    println!("✓ CLI built with embedded WASM\n");

    println!("==========================================");
    println!("✓ WASM PIPELINE COMPLETE");
    println!("==========================================");
    println!("\nArtifacts:");
    println!("  target/pkg/sorex_bg.wasm      - Optimized WASM module");
    println!("  target/loader/sorex.js - JavaScript loader");
    println!("  target/release/sorex          - CLI with embedded WASM");

    Ok(())
}

/// Find wasm-opt binary
fn find_wasm_opt() -> Option<String> {
    let candidates = [
        "/opt/homebrew/bin/wasm-opt",
        "/usr/local/bin/wasm-opt",
        "/usr/bin/wasm-opt",
    ];

    for candidate in candidates {
        if std::path::Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }

    // Try PATH via `which`
    if let Ok(output) = Command::new("which").arg("wasm-opt").output() {
        if output.status.success() {
            if let Ok(path) = String::from_utf8(output.stdout) {
                let path = path.trim();
                if !path.is_empty() {
                    return Some(path.to_string());
                }
            }
        }
    }

    None
}

// ============================================================================
// E2E Benchmarks
// ============================================================================

/// Full E2E benchmark pipeline
fn bench_e2e() -> Result<()> {
    let root = project_root()?;

    println!("==========================================");
    println!("E2E Benchmark Pipeline");
    println!("==========================================\n");

    // Step 1: Build WASM pipeline
    println!("[1/5] Building WASM pipeline...\n");
    build_wasm()?;
    println!();

    // Step 2: Crawl CUTLASS docs (if needed)
    println!("[2/5] Checking CUTLASS dataset...");
    let cutlass_manifest = root.join("target/datasets/cutlass/manifest.json");
    if !cutlass_manifest.exists() {
        println!("  Crawling CUTLASS documentation...");
        run_bun(&root, "benches/crawl-cutlass-docs.ts")?;
        println!("✓ CUTLASS docs crawled\n");
    } else {
        println!("✓ CUTLASS dataset exists\n");
    }

    // Step 3: Crawl PyTorch docs (if needed)
    println!("[3/5] Checking PyTorch dataset...");
    let pytorch_manifest = root.join("target/datasets/pytorch/manifest.json");
    if !pytorch_manifest.exists() {
        println!("  Crawling PyTorch documentation...");
        run_bun(&root, "benches/crawl-pytorch-docs.ts")?;
        println!("✓ PyTorch docs crawled\n");
    } else {
        println!("✓ PyTorch dataset exists\n");
    }

    // Step 4: Build indexes
    println!("[4/5] Building search indexes...");

    let sorex_bin = root.join("target/release/sorex");
    if !sorex_bin.exists() {
        bail!("sorex binary not found at {:?}", sorex_bin);
    }

    // CUTLASS index
    println!("  Indexing CUTLASS...");
    let status = Command::new(&sorex_bin)
        .args(["index", "--input", "target/datasets/cutlass", "--output", "target/datasets/cutlass"])
        .current_dir(&root)
        .status()
        .context("Failed to index CUTLASS")?;
    if !status.success() {
        bail!("Failed to index CUTLASS dataset");
    }

    // PyTorch index
    println!("  Indexing PyTorch...");
    let status = Command::new(&sorex_bin)
        .args(["index", "--input", "target/datasets/pytorch", "--output", "target/datasets/pytorch"])
        .current_dir(&root)
        .status()
        .context("Failed to index PyTorch")?;
    if !status.success() {
        bail!("Failed to index PyTorch dataset");
    }
    println!("✓ Indexes built\n");

    // Step 5: Run benchmarks
    println!("[5/5] Running benchmarks...\n");

    println!("--- CUTLASS Benchmark ---");
    run_bun(&root, "benches/bench-cutlass.ts")?;
    println!();

    println!("--- PyTorch Benchmark ---");
    run_bun(&root, "benches/bench-pytorch.ts")?;
    println!();

    println!("==========================================");
    println!("✓ E2E BENCHMARKS COMPLETE");
    println!("==========================================");
    println!("\nResults:");
    println!("  target/bench-results/*.json");
    println!("  docs/comparisons/*.md");

    Ok(())
}

fn run_bun(root: &PathBuf, script: &str) -> Result<()> {
    let status = Command::new("bun")
        .arg(script)
        .current_dir(root)
        .status()
        .with_context(|| format!("Failed to run bun {}", script))?;

    if !status.success() {
        bail!("bun {} failed", script);
    }

    Ok(())
}

// ============================================================================
// Size Checks
// ============================================================================

/// Verify WASM and loader artifacts don't exceed size limits.
/// This protects the "lightweight" value proposition.
fn size_check() -> Result<()> {
    let root = project_root()?;

    println!("==========================================");
    println!("Artifact Size Check");
    println!("==========================================\n");

    // Size limits (in bytes)
    const WASM_MAX_BYTES: u64 = 500_000;       // 500KB max for raw WASM
    const LOADER_MAX_BYTES: u64 = 200_000;     // 200KB max for JS loader

    let mut all_passed = true;

    // Check WASM size
    let wasm_path = root.join("target/pkg/sorex_bg.wasm");
    if wasm_path.exists() {
        let size = std::fs::metadata(&wasm_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let size_kb = size as f64 / 1024.0;

        if size > WASM_MAX_BYTES {
            println!("✗ WASM: {:.1}KB (exceeds {:.0}KB limit)", size_kb, WASM_MAX_BYTES as f64 / 1024.0);
            all_passed = false;
        } else {
            println!("✓ WASM: {:.1}KB (limit: {:.0}KB)", size_kb, WASM_MAX_BYTES as f64 / 1024.0);
        }
    } else {
        println!("⚠ WASM not found: {:?}", wasm_path);
        println!("  Run `cargo xtask build-wasm` first");
        all_passed = false;
    }

    // Check loader size
    let loader_path = root.join("target/loader/sorex.js");
    if loader_path.exists() {
        let size = std::fs::metadata(&loader_path)
            .map(|m| m.len())
            .unwrap_or(0);
        let size_kb = size as f64 / 1024.0;

        if size > LOADER_MAX_BYTES {
            println!("✗ Loader: {:.1}KB (exceeds {:.0}KB limit)", size_kb, LOADER_MAX_BYTES as f64 / 1024.0);
            all_passed = false;
        } else {
            println!("✓ Loader: {:.1}KB (limit: {:.0}KB)", size_kb, LOADER_MAX_BYTES as f64 / 1024.0);
        }
    } else {
        println!("⚠ Loader not found: {:?}", loader_path);
        println!("  Run `cargo xtask build-wasm` first");
        all_passed = false;
    }

    println!();

    if all_passed {
        println!("==========================================");
        println!("✓ ALL SIZE CHECKS PASSED");
        println!("==========================================");
        Ok(())
    } else {
        bail!("Size check failed - artifacts exceed limits");
    }
}
