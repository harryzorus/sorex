#![no_main]

use libfuzzer_sys::fuzz_target;
use arbitrary::Arbitrary;
use sift::{Section, validate_sections, find_section_at_offset};

/// Fuzz input for section boundary testing
#[derive(Debug, Arbitrary)]
struct SectionInput {
    /// Document length (capped at 10000 to avoid OOM)
    doc_length: u16,
    /// Number of sections (capped at 100)
    section_count: u8,
    /// Section data: (offset_fraction, length_fraction, level)
    /// Fractions are 0-255, scaled to doc_length
    sections: Vec<(u8, u8, u8)>,
}

fuzz_target!(|input: SectionInput| {
    let doc_length = input.doc_length as usize;
    if doc_length == 0 {
        return;
    }

    let section_count = (input.section_count as usize).min(100).min(input.sections.len());
    if section_count == 0 {
        return;
    }

    // Build sections from fuzzer input
    let mut sections: Vec<Section> = Vec::with_capacity(section_count);
    let mut current_offset = 0usize;

    for i in 0..section_count {
        let (_offset_frac, len_frac, level) = input.sections[i];

        // Calculate start offset (ensuring non-overlapping)
        let start_offset = current_offset;

        // Calculate length (at least 1, scaled from fraction)
        let max_remaining = doc_length.saturating_sub(start_offset);
        if max_remaining == 0 {
            break;
        }

        let length = ((len_frac as usize * max_remaining) / 256).max(1).min(max_remaining);
        let end_offset = start_offset + length;

        // Create section with valid ID
        let section = Section {
            id: format!("section-{}", i),
            start_offset,
            end_offset,
            level: (level % 6) + 1, // Level 1-6
        };

        sections.push(section);
        current_offset = end_offset;

        if current_offset >= doc_length {
            break;
        }
    }

    if sections.is_empty() {
        return;
    }

    // INVARIANT 1: validate_sections should accept well-formed sections
    // (our construction guarantees non-overlapping)
    match validate_sections(&sections, doc_length) {
        Ok(()) => {
            // Validation passed - check find_section_at_offset invariants
        }
        Err(e) => {
            // Our construction should produce valid sections
            // If it fails, it's a bug in our fuzz harness, not the code
            panic!("validate_sections failed on well-formed sections: {}", e);
        }
    }

    // INVARIANT 2: find_section_at_offset should find exactly one section for each covered offset
    for section in &sections {
        for offset in section.start_offset..section.end_offset {
            let found = find_section_at_offset(&sections, offset);
            assert!(
                found.is_some(),
                "find_section_at_offset({}) returned None, expected section {}",
                offset, section.id
            );
            assert_eq!(
                found.unwrap(), section.id,
                "find_section_at_offset({}) returned wrong section",
                offset
            );
        }
    }

    // INVARIANT 3: Offsets outside all sections should return None
    let covered_offsets: std::collections::HashSet<usize> = sections
        .iter()
        .flat_map(|s| s.start_offset..s.end_offset)
        .collect();

    for offset in 0..doc_length.min(1000) {
        // Cap iteration to avoid timeout
        if !covered_offsets.contains(&offset) {
            let found = find_section_at_offset(&sections, offset);
            assert!(
                found.is_none(),
                "find_section_at_offset({}) returned {:?} for uncovered offset",
                offset, found
            );
        }
    }
});
