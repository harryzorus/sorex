//! Inspect suffix array ordering from actual production index
//! This test loads the Cutlass .sorex file and examines the raw suffix array
//! to determine if it's properly sorted lexicographically.
//!
//! Requires Cutlass dataset: `cargo xtask bench-e2e` to build.

use super::common::{cutlass_available, load_cutlass_layer, CUTLASS_INDEX};
use std::fs;

#[test]
fn inspect_cutlass_suffix_array_ordering() {
    if !cutlass_available() {
        println!("Skipping: Cutlass index not available");
        return;
    }

    // Read version from raw header (byte 4) for diagnostic output
    let bytes = fs::read(CUTLASS_INDEX).expect("Failed to read index file");
    let version = bytes[4];

    let layer = load_cutlass_layer();

    println!("Index version: {}", version);
    println!("Header info:");
    println!("  docs: {}", layer.doc_count);
    println!("  terms: {}", layer.vocabulary.len());
    println!("  suffix_array entries: {}", layer.suffix_array.len());

    let vocab = &layer.vocabulary;
    let suffix_array = &layer.suffix_array;

    println!("Decoded {} vocabulary terms", vocab.len());
    println!("\nFirst 20 vocabulary terms:");
    for (i, term) in vocab.iter().take(20).enumerate() {
        println!("  [{}] \"{}\"", i, term);
    }

    println!("\nSuffix array count: {}", suffix_array.len());
    println!("Expected entries: ~{} (7.7x multiplier)", vocab.len());

    // Print first 100 entries with the actual suffix string
    for (i, &(term_idx, char_offset)) in suffix_array.iter().take(100).enumerate() {
        let term_idx = term_idx as usize;
        let char_offset = char_offset as usize;

        if term_idx < vocab.len() {
            let term = &vocab[term_idx];
            let suffix = if char_offset < term.len() {
                &term[char_offset..]
            } else {
                "[OUT_OF_BOUNDS]"
            };
            println!(
                "[{:4}] term_idx={:4}, offset={:3}, term=\"{}\", suffix=\"{}\"",
                i, term_idx, char_offset, term, suffix
            );
        }
    }

    println!("\nDecoded {} suffix array entries", suffix_array.len());

    // Check if suffix array is sorted
    println!("\n=== CHECKING SUFFIX ARRAY ORDERING ===");
    let mut sorted = true;
    let mut first_unsorted = None;

    for i in 1..suffix_array.len().min(1000) {
        let (prev_idx, prev_off) = suffix_array[i - 1];
        let (curr_idx, curr_off) = suffix_array[i];

        let prev_idx = prev_idx as usize;
        let curr_idx = curr_idx as usize;
        let prev_off = prev_off as usize;
        let curr_off = curr_off as usize;

        let prev_term = &vocab[prev_idx];
        let curr_term = &vocab[curr_idx];

        let prev_suffix = if prev_off < prev_term.len() {
            &prev_term[prev_off..]
        } else {
            ""
        };

        let curr_suffix = if curr_off < curr_term.len() {
            &curr_term[curr_off..]
        } else {
            ""
        };

        if prev_suffix > curr_suffix {
            if first_unsorted.is_none() {
                first_unsorted = Some((i, prev_suffix.to_string(), curr_suffix.to_string()));
            }
            sorted = false;
            if i < 20 {
                println!("UNSORTED at [{}]: \"{}\" > \"{}\"", i, prev_suffix, curr_suffix);
            }
        }
    }

    if sorted {
        println!("✓ Suffix array appears SORTED (checked first 1000 entries)");
    } else {
        if let Some((idx, prev, curr)) = first_unsorted {
            println!("✗ Suffix array NOT SORTED");
            println!(
                "  First problem at index {}: \"{}\" > \"{}\"",
                idx, prev, curr
            );
        }
    }

    // Now test partition_point on actual suffix array
    println!("\n=== TESTING partition_point ===");
    test_partition_point(suffix_array, vocab);
}

fn test_partition_point(suffix_array: &[(u32, u32)], vocab: &[String]) {
    let test_prefixes = vec!["a", "ap", "apple", "aug", "aug1", "device", "kernel"];

    for prefix in test_prefixes {
        let pos = suffix_array.partition_point(|(term_idx, offset)| {
            let term = &vocab[*term_idx as usize];
            let suffix = if (*offset as usize) < term.len() {
                &term[*offset as usize..]
            } else {
                ""
            };
            suffix < prefix
        });

        let (suffix_at_pos, matches) = if pos < suffix_array.len() {
            let (term_idx, offset) = suffix_array[pos];
            let term = &vocab[term_idx as usize];
            let suffix = if (offset as usize) < term.len() {
                &term[offset as usize..]
            } else {
                "[OUT_OF_BOUNDS]"
            };
            (suffix.to_string(), suffix.starts_with(prefix))
        } else {
            ("[END_OF_ARRAY]".to_string(), false)
        };

        let status = if matches { "✓" } else { "✗" };
        println!(
            "{} prefix=\"{}\": partition_point={}, suffix_at_pos=\"{}\"",
            status, prefix, pos, suffix_at_pos
        );

        // For prefixes that should match, print next 5 entries
        if matches && pos + 5 < suffix_array.len() {
            println!("  Next entries:");
            for j in 0..5 {
                let (term_idx, offset) = suffix_array[pos + j];
                let term = &vocab[term_idx as usize];
                let suffix = if (offset as usize) < term.len() {
                    &term[offset as usize..]
                } else {
                    "[OUT_OF_BOUNDS]"
                };
                let starts = suffix.starts_with(prefix);
                println!(
                    "    [{}] \"{}\" {}",
                    pos + j,
                    suffix,
                    if starts { "✓" } else { "✗" }
                );
            }
        }
    }
}
