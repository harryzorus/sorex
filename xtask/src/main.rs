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

fn main() -> Result<()> {
    let task = env::args().nth(1);
    match task.as_deref() {
        Some("verify") => verify()?,
        Some("test") => test()?,
        Some("lean") => lean()?,
        Some("check") => check()?,
        Some("bench") => bench()?,
        _ => print_help(),
    }
    Ok(())
}

fn print_help() {
    eprintln!(
        r#"
cargo xtask <COMMAND>

Commands:
  verify    Run full verification suite (tests + Lean + constant alignment)
  test      Run all Rust tests
  lean      Build Lean proofs only
  check     Quick check (cargo test + clippy, no Lean)
  bench     Run benchmarks
"#
    );
}

/// Full verification suite
fn verify() -> Result<()> {
    println!("==========================================");
    println!("Search Crate Verification Suite");
    println!("==========================================\n");

    // Step 1: Check invariant markers
    println!("[1/5] Checking invariant markers...");
    check_invariant_markers()?;
    println!("✓ Invariant markers present\n");

    // Step 2: Run tests
    println!("[2/5] Running Rust tests...");
    run_cargo(&["test", "--quiet"])?;
    println!("✓ All Rust tests passed\n");

    // Step 3: Clippy
    println!("[3/5] Running clippy...");
    run_cargo(&["clippy", "--quiet", "--", "-D", "warnings"])?;
    println!("✓ Clippy passed\n");

    // Step 4: Build Lean
    println!("[4/5] Building Lean proofs...");
    lean()?;
    println!("✓ Lean proofs build\n");

    // Step 5: Verify constant alignment
    println!("[5/5] Verifying Rust/Lean constant alignment...");
    verify_constants()?;
    println!("✓ Constants aligned\n");

    println!("==========================================");
    println!("✓ ALL VERIFICATION CHECKS PASSED");
    println!("==========================================");
    println!("\nSafe to commit changes.");

    Ok(())
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
    let scoring_rs = std::fs::read_to_string(root.join("src/scoring.rs"))
        .context("Failed to read scoring.rs")?;

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
    // Look for "FieldType::Title => 100.0," or "Title => 100.0,"
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
