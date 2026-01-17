//! Tests to verify constants are synchronized across Rust, Lean, and TypeScript.
//!
//! These tests ensure the "single source of truth" invariant is maintained.

use std::fs;

/// The Rust source file containing scoring constants
const SCORING_RUST: &str = "src/scoring/core.rs";

/// The Lean specification file
const SCORING_LEAN: &str = "lean/SearchVerified/Scoring.lean";

/// The Rust source file containing binary format version
const HEADER_RUST: &str = "src/binary/header.rs";

/// The TypeScript loader build script
const LOADER_TS: &str = "tools/build.ts";

// ============================================================================
// SCORING CONSTANTS SYNC
// ============================================================================

/// Extract TITLE_BASE_SCORE from Rust source
fn extract_rust_title_score() -> f64 {
    let content = fs::read_to_string(SCORING_RUST)
        .expect("Failed to read scoring/core.rs");

    for line in content.lines() {
        if line.contains("pub const TITLE_BASE_SCORE") && line.contains("f64") {
            // Parse: pub const TITLE_BASE_SCORE: f64 = 100.0;
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse().expect("Failed to parse TITLE_BASE_SCORE");
            }
        }
    }
    panic!("TITLE_BASE_SCORE not found in {}", SCORING_RUST);
}

/// Extract HEADING_BASE_SCORE from Rust source
fn extract_rust_heading_score() -> f64 {
    let content = fs::read_to_string(SCORING_RUST)
        .expect("Failed to read scoring/core.rs");

    for line in content.lines() {
        if line.contains("pub const HEADING_BASE_SCORE") && line.contains("f64") {
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse().expect("Failed to parse HEADING_BASE_SCORE");
            }
        }
    }
    panic!("HEADING_BASE_SCORE not found in {}", SCORING_RUST);
}

/// Extract CONTENT_BASE_SCORE from Rust source
fn extract_rust_content_score() -> f64 {
    let content = fs::read_to_string(SCORING_RUST)
        .expect("Failed to read scoring/core.rs");

    for line in content.lines() {
        if line.contains("pub const CONTENT_BASE_SCORE") && line.contains("f64") {
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse().expect("Failed to parse CONTENT_BASE_SCORE");
            }
        }
    }
    panic!("CONTENT_BASE_SCORE not found in {}", SCORING_RUST);
}

/// Extract MAX_POSITION_BONUS from Rust source
fn extract_rust_max_bonus() -> f64 {
    let content = fs::read_to_string(SCORING_RUST)
        .expect("Failed to read scoring/core.rs");

    for line in content.lines() {
        if line.contains("pub const MAX_POSITION_BONUS") && line.contains("f64") {
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse().expect("Failed to parse MAX_POSITION_BONUS");
            }
        }
    }
    panic!("MAX_POSITION_BONUS not found in {}", SCORING_RUST);
}

/// Extract baseScore .title from Lean specification (scaled by 10)
fn extract_lean_title_score() -> u64 {
    let content = fs::read_to_string(SCORING_LEAN)
        .expect("Failed to read Scoring.lean");

    for line in content.lines() {
        // Look for: | .title   => 1000
        if line.contains(".title") && line.contains("=>") {
            if let Some(val) = line.split("=>").nth(1) {
                let val = val.split_whitespace().next().unwrap_or("");
                let val = val.trim_end_matches(|c: char| !c.is_ascii_digit());
                return val.parse().expect("Failed to parse Lean title score");
            }
        }
    }
    panic!("baseScore .title not found in {}", SCORING_LEAN);
}

/// Extract baseScore .heading from Lean specification
fn extract_lean_heading_score() -> u64 {
    let content = fs::read_to_string(SCORING_LEAN)
        .expect("Failed to read Scoring.lean");

    for line in content.lines() {
        if line.contains(".heading") && line.contains("=>") && !line.contains(".subheading") {
            if let Some(val) = line.split("=>").nth(1) {
                let val = val.split_whitespace().next().unwrap_or("");
                let val = val.trim_end_matches(|c: char| !c.is_ascii_digit());
                return val.parse().expect("Failed to parse Lean heading score");
            }
        }
    }
    panic!("baseScore .heading not found in {}", SCORING_LEAN);
}

/// Extract baseScore .content from Lean specification
fn extract_lean_content_score() -> u64 {
    let content = fs::read_to_string(SCORING_LEAN)
        .expect("Failed to read Scoring.lean");

    for line in content.lines() {
        if line.contains(".content") && line.contains("=>") {
            if let Some(val) = line.split("=>").nth(1) {
                let val = val.split_whitespace().next().unwrap_or("");
                let val = val.trim_end_matches(|c: char| !c.is_ascii_digit());
                return val.parse().expect("Failed to parse Lean content score");
            }
        }
    }
    panic!("baseScore .content not found in {}", SCORING_LEAN);
}

/// Extract maxPositionBoost from Lean specification
fn extract_lean_max_bonus() -> u64 {
    let content = fs::read_to_string(SCORING_LEAN)
        .expect("Failed to read Scoring.lean");

    for line in content.lines() {
        if line.contains("def maxPositionBoost") {
            if let Some(val) = line.split(":=").nth(1) {
                let val = val.trim();
                return val.parse().expect("Failed to parse Lean maxPositionBoost");
            }
        }
    }
    panic!("maxPositionBoost not found in {}", SCORING_LEAN);
}

#[test]
fn test_scoring_constants_rust_lean_aligned() {
    // Lean uses Nat scaled by 10 for decidable proofs
    // Rust: 100.0 → Lean: 1000 (×10)

    let rust_title = extract_rust_title_score();
    let lean_title = extract_lean_title_score();
    assert_eq!(
        (rust_title * 10.0) as u64, lean_title,
        "TITLE_BASE_SCORE drift: Rust {} × 10 = {} ≠ Lean {}",
        rust_title, (rust_title * 10.0) as u64, lean_title
    );

    let rust_heading = extract_rust_heading_score();
    let lean_heading = extract_lean_heading_score();
    assert_eq!(
        (rust_heading * 10.0) as u64, lean_heading,
        "HEADING_BASE_SCORE drift: Rust {} × 10 = {} ≠ Lean {}",
        rust_heading, (rust_heading * 10.0) as u64, lean_heading
    );

    let rust_content = extract_rust_content_score();
    let lean_content = extract_lean_content_score();
    assert_eq!(
        (rust_content * 10.0) as u64, lean_content,
        "CONTENT_BASE_SCORE drift: Rust {} × 10 = {} ≠ Lean {}",
        rust_content, (rust_content * 10.0) as u64, lean_content
    );

    let rust_bonus = extract_rust_max_bonus();
    let lean_bonus = extract_lean_max_bonus();
    assert_eq!(
        (rust_bonus * 10.0) as u64, lean_bonus,
        "MAX_POSITION_BONUS drift: Rust {} × 10 = {} ≠ Lean {}",
        rust_bonus, (rust_bonus * 10.0) as u64, lean_bonus
    );
}

// ============================================================================
// BINARY VERSION SYNC
// ============================================================================

/// Extract VERSION from Rust binary/header.rs
fn extract_rust_version() -> u8 {
    let content = fs::read_to_string(HEADER_RUST)
        .expect("Failed to read binary/header.rs");

    for line in content.lines() {
        if line.contains("pub const VERSION") && line.contains("u8") {
            if let Some(val) = line.split('=').nth(1) {
                let val = val.trim().trim_end_matches(';').trim();
                return val.parse().expect("Failed to parse VERSION");
            }
        }
    }
    panic!("VERSION not found in {}", HEADER_RUST);
}

/// Verify TypeScript reads version from Rust source (not hardcoded)
fn verify_ts_reads_from_rust() -> bool {
    let content = fs::read_to_string(LOADER_TS)
        .expect("Failed to read tools/build.ts");

    // Check that TypeScript reads from Rust source file
    content.contains("readRustVersion()")
        && content.contains("src/binary/header.rs")
        && content.contains("pub const VERSION")
}

#[test]
fn test_binary_version_rust_ts_aligned() {
    // Verify Rust version is readable
    let rust_version = extract_rust_version();
    assert!(rust_version > 0, "Rust VERSION should be > 0");

    // Verify TypeScript reads from Rust source (single source of truth)
    assert!(
        verify_ts_reads_from_rust(),
        "TypeScript build-loader.ts should read VERSION from Rust source, not hardcode it"
    );
}

// ============================================================================
// FIELD TYPE DOMINANCE INVARIANT
// ============================================================================

#[test]
fn test_field_type_dominance_invariant() {
    // This is the critical invariant from Lean: title_beats_heading
    // Even with worst position bonus, title must beat best heading

    let title = extract_rust_title_score();
    let heading = extract_rust_heading_score();
    let content = extract_rust_content_score();
    let max_bonus = extract_rust_max_bonus();

    // Worst title - max_bonus > Best heading + max_bonus
    let worst_title = title - max_bonus;
    let best_heading = heading + max_bonus;
    assert!(
        worst_title > best_heading,
        "FIELD_TYPE_DOMINANCE VIOLATED: worst_title ({} - {} = {}) <= best_heading ({} + {} = {})",
        title, max_bonus, worst_title, heading, max_bonus, best_heading
    );

    // Worst heading - max_bonus > Best content + max_bonus
    let worst_heading = heading - max_bonus;
    let best_content = content + max_bonus;
    assert!(
        worst_heading > best_content,
        "FIELD_TYPE_DOMINANCE VIOLATED: worst_heading ({} - {} = {}) <= best_content ({} + {} = {})",
        heading, max_bonus, worst_heading, content, max_bonus, best_content
    );
}
