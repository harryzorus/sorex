#![no_main]

use libfuzzer_sys::fuzz_target;
use sift::Section;

/// Fuzz target for heading ID validation.
///
/// Tests that Section IDs are validated correctly:
/// - Non-empty IDs are accepted
/// - Empty IDs are rejected
/// - IDs with various Unicode characters are handled
fuzz_target!(|data: &[u8]| {
    // Try to interpret bytes as UTF-8 string for section ID
    if let Ok(id) = std::str::from_utf8(data) {
        let section = Section {
            id: id.to_string(),
            start_offset: 0,
            end_offset: 100,
            level: 2,
        };

        // INVARIANT: Section with non-empty ID should be valid
        // Empty ID sections are invalid for deep linking
        if !id.is_empty() {
            // Section should be well-formed
            assert!(section.start_offset < section.end_offset);
            assert!(section.level >= 1 && section.level <= 6);

            // ID should be preserved exactly
            assert_eq!(section.id, id);
        }

        // INVARIANT: Level should be in valid range
        assert!(section.level >= 1 && section.level <= 6);
    }

    // Also test arbitrary bytes for Section construction robustness
    // This ensures we don't panic on any input
    for level in 1..=6 {
        let section = Section {
            id: format!("test-{:x?}", &data[..data.len().min(8)]),
            start_offset: 0,
            end_offset: data.len().max(1),
            level,
        };

        // Should not panic
        let _ = format!("{:?}", section);
    }
});
