//! Utility functions for string processing.

#[cfg(feature = "unicode-normalization")]
use unicode_normalization::UnicodeNormalization;

/// Normalize a string for search: lowercase, strip diacritics, and collapse whitespace.
///
/// This enables fuzzy matching between ASCII and accented versions:
/// - "café" → "cafe"
/// - "tummalachērla" → "tummalacherla"
/// - "harīṣh" → "harish"
/// - "naïve" → "naive"
///
/// # Algorithm (with unicode-normalization feature)
///
/// 1. NFD normalize (decompose characters into base + combining marks)
/// 2. Filter out combining marks (category Mn = Mark, Nonspacing)
/// 3. Lowercase
/// 4. Collapse whitespace
///
/// # Algorithm (without unicode-normalization, e.g. WASM)
///
/// 1. Lowercase only (assumes input is pre-normalized or ASCII)
/// 2. Collapse whitespace
#[cfg(feature = "unicode-normalization")]
pub fn normalize(value: &str) -> String {
    value
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Lightweight normalization for WASM (no unicode-normalization dependency).
/// Just lowercases and collapses whitespace. Assumes input is ASCII or pre-normalized.
#[cfg(not(feature = "unicode-normalization"))]
pub fn normalize(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Check if a character is a combining mark (diacritic).
///
/// Combining marks have Unicode category "Mn" (Mark, Nonspacing).
/// Examples: ́ (acute), ̄ (macron), ̣ (dot below)
#[cfg(feature = "unicode-normalization")]
fn is_combining_mark(c: char) -> bool {
    // Unicode category Mn (Mark, Nonspacing) range
    // This covers the most common combining diacritical marks
    matches!(c,
        '\u{0300}'..='\u{036F}' |  // Combining Diacritical Marks
        '\u{0C00}'..='\u{0C7F}' |  // Telugu (some combining marks)
        '\u{0900}'..='\u{097F}' |  // Devanagari (some combining marks)
        '\u{1DC0}'..='\u{1DFF}' |  // Combining Diacritical Marks Supplement
        '\u{20D0}'..='\u{20FF}' |  // Combining Diacritical Marks for Symbols
        '\u{FE20}'..='\u{FE2F}'    // Combining Half Marks
    )
}

/// Calculate the common prefix length of two strings (in bytes).
///
/// This is the original byte-based version for backwards compatibility.
#[allow(dead_code)]
pub fn common_prefix_len(a: &str, b: &str) -> usize {
    let min_len = std::cmp::min(a.len(), b.len());
    let a_bytes = a.as_bytes();
    let b_bytes = b.as_bytes();
    let mut k = 0usize;
    while k < min_len && a_bytes[k] == b_bytes[k] {
        k += 1;
    }
    k
}

/// Calculate the common prefix length of two strings (in characters).
///
/// This version counts Unicode characters, not bytes, making it compatible
/// with JavaScript's string indexing. Used for LCP array computation.
pub fn common_prefix_len_chars(a: &str, b: &str) -> usize {
    a.chars()
        .zip(b.chars())
        .take_while(|(ca, cb)| ca == cb)
        .count()
}
