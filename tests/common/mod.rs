//! Shared test utilities and fixtures.

#![allow(dead_code)]

use sieve::{build_index, build_hybrid_index, FieldBoundary, FieldType, HybridIndex, SearchDoc, SearchIndex};

/// Create a simple test document.
pub fn make_doc(id: usize, title: &str) -> SearchDoc {
    SearchDoc {
        id,
        title: title.to_string(),
        excerpt: format!("Excerpt for {}", title),
        href: format!("/doc/{}", id),
        kind: "post".to_string(),
        category: None,
        author: None,
        tags: vec![],
    }
}

/// Build a test index from text strings.
pub fn build_test_index(texts: &[&str]) -> SearchIndex {
    let docs: Vec<SearchDoc> = texts
        .iter()
        .enumerate()
        .map(|(i, _)| make_doc(i, &format!("Doc {}", i)))
        .collect();

    let texts: Vec<String> = texts.iter().map(|s| s.to_string()).collect();

    build_index(docs, texts, vec![])
}

/// Build a test index with field boundaries.
pub fn build_test_index_with_fields(
    docs_data: &[(String, Vec<(String, FieldType)>)],
) -> SearchIndex {
    let docs: Vec<SearchDoc> = docs_data
        .iter()
        .enumerate()
        .map(|(i, (title, _))| make_doc(i, title))
        .collect();

    let mut texts: Vec<String> = Vec::new();
    let mut field_boundaries: Vec<FieldBoundary> = Vec::new();

    for (doc_id, (_title, fields)) in docs_data.iter().enumerate() {
        let mut text = String::new();
        let mut offset = 0;

        for (field_text, field_type) in fields {
            if !text.is_empty() {
                text.push(' ');
                offset += 1;
            }

            let start = offset;
            text.push_str(field_text);
            offset += field_text.len();

            field_boundaries.push(FieldBoundary {
                doc_id,
                start,
                end: offset,
                field_type: field_type.clone(),
                section_id: None,
            });
        }

        texts.push(text);
    }

    build_index(docs, texts, field_boundaries)
}

/// Assert that an index satisfies all well-formedness invariants.
pub fn assert_index_well_formed(index: &SearchIndex) {
    // Invariant: docs.len() == texts.len()
    assert_eq!(
        index.docs.len(),
        index.texts.len(),
        "INVARIANT VIOLATED: docs.len() != texts.len()"
    );

    // Invariant: lcp.len() == suffix_array.len()
    assert_eq!(
        index.lcp.len(),
        index.suffix_array.len(),
        "INVARIANT VIOLATED: lcp.len() != suffix_array.len()"
    );

    // Invariant: all suffix entries are valid
    // Note: Uses strict inequality (offset < char_count) because suffix arrays index non-empty suffixes
    // offset is a CHARACTER offset (not byte offset)
    for (i, entry) in index.suffix_array.iter().enumerate() {
        assert!(
            entry.doc_id < index.texts.len(),
            "INVARIANT VIOLATED: suffix_array[{}].doc_id {} >= texts.len() {}",
            i,
            entry.doc_id,
            index.texts.len()
        );
        let char_count = index.texts[entry.doc_id].chars().count();
        assert!(
            entry.offset < char_count,
            "INVARIANT VIOLATED: suffix_array[{}].offset {} >= texts[{}].char_count() {}",
            i,
            entry.offset,
            entry.doc_id,
            char_count
        );
    }

    // Invariant: suffix array is sorted
    // Note: suffix array uses CHARACTER offsets (not byte offsets)
    // This matches JavaScript's UTF-16 string semantics
    for i in 1..index.suffix_array.len() {
        let prev = &index.suffix_array[i - 1];
        let curr = &index.suffix_array[i];

        // Use character-based slicing (skip N characters, not N bytes)
        let prev_suffix: String = index.texts[prev.doc_id]
            .chars()
            .skip(prev.offset)
            .collect();
        let curr_suffix: String = index.texts[curr.doc_id]
            .chars()
            .skip(curr.offset)
            .collect();

        assert!(
            prev_suffix <= curr_suffix,
            "INVARIANT VIOLATED: suffix_array not sorted at {}: '{}' > '{}'",
            i,
            prev_suffix.chars().take(20).collect::<String>(),
            curr_suffix.chars().take(20).collect::<String>()
        );
    }

    // Invariant: LCP[0] == 0
    if !index.lcp.is_empty() {
        assert_eq!(
            index.lcp[0], 0,
            "INVARIANT VIOLATED: lcp[0] = {} (expected 0)",
            index.lcp[0]
        );
    }
}

/// Assert that suffix array is complete (all suffixes present).
/// Note: Uses CHARACTER offsets (not byte offsets).
pub fn assert_suffix_array_complete(index: &SearchIndex) {
    for (doc_id, text) in index.texts.iter().enumerate() {
        let char_count = text.chars().count();
        for offset in 0..char_count {
            let found = index
                .suffix_array
                .iter()
                .any(|e| e.doc_id == doc_id && e.offset == offset);

            assert!(
                found,
                "INVARIANT VIOLATED: missing suffix entry for doc_id={}, offset={}",
                doc_id, offset
            );
        }
    }
}

/// Build a hybrid test index from text strings.
///
/// Creates a HybridIndex with both inverted index and suffix array
/// for testing streaming search functionality.
pub fn build_hybrid_test_index(texts: &[String]) -> HybridIndex {
    let docs: Vec<SearchDoc> = texts
        .iter()
        .enumerate()
        .map(|(i, _)| make_doc(i, &format!("Doc {}", i)))
        .collect();

    let texts: Vec<String> = texts.to_vec();

    build_hybrid_index(docs, texts, vec![])
}
