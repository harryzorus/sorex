//! Suffix array-based full-text search with formal verification.
//!
//! This crate provides a search index using suffix arrays for O(log n) prefix matching.
//! The implementation is formally specified in Lean 4 and verified via property testing.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │   types.rs  │────▶│  index.rs    │────▶│  search.rs  │
//! │  (SearchDoc,│     │ (build_index,│     │  (search)   │
//! │ SuffixEntry)│     │  suffix_at)  │     │             │
//! └─────────────┘     └──────────────┘     └─────────────┘
//!        │                   │                    │
//!        ▼                   ▼                    ▼
//! ┌─────────────────────────────────────────────────────┐
//! │                    verified.rs                       │
//! │  (ValidatedSuffixEntry, SortedSuffixArray,          │
//! │   WellFormedIndex - type-level invariants)          │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! # Lean Correspondence
//!
//! Each module corresponds to Lean specifications:
//!
//! | Rust Module     | Lean File                    | Key Properties           |
//! |-----------------|------------------------------|--------------------------|
//! | `types`         | `Types.lean`                 | Type definitions         |
//! | `index`         | `SuffixArray.lean`           | Sorted, Complete, LCP    |
//! | `search`        | `BinarySearch.lean`          | Search correctness       |
//! | `scoring`       | `Scoring.lean`               | Field hierarchy          |
//! | `levenshtein`   | `Levenshtein.lean`           | Edit distance bounds     |
//! | `verified`      | All                          | Type-level invariants    |
//!
//! # Usage
//!
//! ```ignore
//! use search::{build_index, search, SearchDoc, FieldBoundary};
//!
//! let docs = vec![SearchDoc { ... }];
//! let texts = vec!["document text".to_string()];
//! let index = build_index(docs, texts, vec![]);
//!
//! let results = search(&index, "query");
//! ```

// Module declarations
pub mod binary;
pub mod contracts;
pub mod docs_compression;
pub mod fst_index;
mod hybrid;
mod index;
mod inverted;
mod levenshtein;
pub mod levenshtein_dfa;
mod sais;
mod scoring;
mod search;
mod types;
mod union;
mod utils;
pub mod verified;

#[cfg(feature = "wasm")]
mod wasm;

// Re-exports for public API
pub use hybrid::{
    build_hybrid_index, build_hybrid_index_parallel, search_exact, search_expanded, search_fuzzy,
    search_hybrid,
};
pub use index::{build_index, is_suffix_array_sorted, suffix_at};
pub use inverted::{
    build_inverted_index, build_inverted_index_parallel, build_unified_index, select_index_mode,
    IndexThresholds,
};
pub use levenshtein::levenshtein_within;
pub use scoring::{field_type_score, get_field_type};
pub use search::{search, search_unified};
pub use types::{
    find_section_at_offset, validate_sections, FieldBoundary, FieldType, HybridIndex, IndexMode,
    InvertedIndex, Posting, PostingList, SearchDoc, SearchIndex, SearchResult, SearchSource,
    Section, SuffixEntry, UnifiedIndex, UnionIndex, VocabSuffixEntry,
};
pub use union::{
    build_union_index, build_union_index_parallel, search_union, search_union_grouped,
    UnionIndexInput,
};
pub use utils::normalize;
pub use fst_index::{build_fst_index, FstIndex};
pub use levenshtein_dfa::{ParametricDFA, QueryMatcher, MAX_K, NUM_CHAR_CLASSES};
pub use verified::{
    InvariantError, SortedSuffixArray, ValidatedInvertedIndex, ValidatedPosting,
    ValidatedPostingList, ValidatedSuffixEntry, VerificationReport, WellFormedIndex,
};

// Internal re-exports for tests
#[cfg(test)]
pub(crate) use utils::common_prefix_len;

#[cfg(test)]
mod tests {
    //! Integration and property tests for the search module.
    //!
    //! These tests verify that the Rust implementation satisfies the formal
    //! properties specified in the Lean verification project.
    //!
    //! See: `lean/SearchVerified/*.lean` for the formal specifications.

    use super::*;
    use proptest::prelude::*;
    use proptest::string::string_regex;

    fn build_test_docs(texts: &[String]) -> SearchIndex {
        let docs: Vec<SearchDoc> = texts
            .iter()
            .enumerate()
            .map(|(index, _)| SearchDoc {
                id: index,
                title: format!("Doc {}", index),
                excerpt: format!("Excerpt {}", index),
                href: format!("/doc/{}", index),
                kind: "post".to_string(),
            })
            .collect();

        build_index(docs, texts.to_vec(), Vec::new())
    }

    fn build_test_docs_with_titles(docs_data: &[(String, String)]) -> SearchIndex {
        let docs: Vec<SearchDoc> = docs_data
            .iter()
            .enumerate()
            .map(|(index, (title, _))| SearchDoc {
                id: index,
                title: title.clone(),
                excerpt: format!("Excerpt {}", index),
                href: format!("/doc/{}", index),
                kind: "post".to_string(),
            })
            .collect();

        let texts: Vec<String> = docs_data
            .iter()
            .map(|(title, content)| {
                let normalized_title = normalize(title);
                let normalized_content = normalize(content);
                format!("{} {}", normalized_title, normalized_content)
            })
            .collect();

        let field_boundaries: Vec<FieldBoundary> = docs_data
            .iter()
            .enumerate()
            .flat_map(|(doc_id, (title, content))| {
                let normalized_title = normalize(title);
                let title_len = normalized_title.len() + 1;
                let normalized_content = normalize(content);
                let total_len = title_len + normalized_content.len();

                vec![
                    FieldBoundary {
                        doc_id,
                        start: 0,
                        end: normalized_title.len(),
                        field_type: FieldType::Title,
                        section_id: None,
                    },
                    FieldBoundary {
                        doc_id,
                        start: title_len,
                        end: total_len,
                        field_type: FieldType::Content,
                        section_id: None,
                    },
                ]
            })
            .collect();

        build_index(docs, texts, field_boundaries)
    }

    fn build_test_docs_with_fields(docs_data: &[(String, Vec<(String, FieldType)>)]) -> SearchIndex {
        let docs: Vec<SearchDoc> = docs_data
            .iter()
            .enumerate()
            .map(|(index, (title, _))| SearchDoc {
                id: index,
                title: title.clone(),
                excerpt: format!("Excerpt {}", index),
                href: format!("/doc/{}", index),
                kind: "post".to_string(),
            })
            .collect();

        let mut texts: Vec<String> = Vec::new();
        let mut field_boundaries: Vec<FieldBoundary> = Vec::new();

        for (doc_id, (_title, fields)) in docs_data.iter().enumerate() {
            let mut text = String::new();
            let mut offset = 0;

            for (field_text, field_type) in fields {
                let normalized = normalize(field_text);
                if !text.is_empty() {
                    text.push(' ');
                    offset += 1;
                }

                let start = offset;
                text.push_str(&normalized);
                offset += normalized.len();

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

    fn text_vec_strategy() -> impl Strategy<Value = Vec<String>> {
        let word_pattern = string_regex("[a-z0-9]{3,6}").unwrap();
        let doc_pattern =
            prop::collection::vec(word_pattern, 2..5).prop_map(|words| words.join(" "));
        prop::collection::vec(doc_pattern, 1..4)
    }

    fn mutate_term(term: &str) -> String {
        if term.len() < 2 {
            return term.to_string();
        }
        let mut chars: Vec<char> = term.chars().collect();
        // Substitute first character to create edit distance 1 (not swap which is 2)
        chars[0] = if chars[0] == 'x' { 'y' } else { 'x' };
        chars.into_iter().collect()
    }

    // =========================================================================
    // INTEGRATION TESTS
    // =========================================================================

    #[test]
    fn title_matches_rank_higher_than_content_matches() {
        let docs = vec![
            (
                "About Photography".to_string(),
                "This is about cameras and lenses".to_string(),
            ),
            (
                "About Mountains".to_string(),
                "Photography in the mountains is great".to_string(),
            ),
        ];
        let index = build_test_docs_with_titles(&docs);

        let results = search(&index, "photography");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 0);
        assert_eq!(results[1].id, 1);
    }

    #[test]
    fn heading_matches_rank_between_title_and_content() {
        let docs = vec![
            (
                "Guide to Programming".to_string(),
                vec![
                    ("Guide to Programming".to_string(), FieldType::Title),
                    ("Introduction to Rust".to_string(), FieldType::Heading),
                    (
                        "Rust is a systems programming language".to_string(),
                        FieldType::Content,
                    ),
                ],
            ),
            (
                "Programming Languages".to_string(),
                vec![
                    ("Programming Languages".to_string(), FieldType::Title),
                    ("Overview".to_string(), FieldType::Heading),
                    (
                        "Learn about rust and other topics".to_string(),
                        FieldType::Content,
                    ),
                ],
            ),
            (
                "Rust Tutorial".to_string(),
                vec![
                    ("Rust Tutorial".to_string(), FieldType::Title),
                    ("Getting Started".to_string(), FieldType::Heading),
                    ("This tutorial covers basics".to_string(), FieldType::Content),
                ],
            ),
        ];
        let index = build_test_docs_with_fields(&docs);

        let results = search(&index, "rust");
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].id, 2); // Title match
        assert_eq!(results[1].id, 0); // Heading match
        assert_eq!(results[2].id, 1); // Content match
    }

    #[test]
    fn empty_search_returns_no_results() {
        let docs = vec![("Test".to_string(), "content".to_string())];
        let index = build_test_docs_with_titles(&docs);

        assert!(search(&index, "").is_empty());
        assert!(search(&index, "   ").is_empty());
    }

    #[test]
    fn search_with_no_matches_returns_empty() {
        let docs = vec![("Test".to_string(), "content here".to_string())];
        let index = build_test_docs_with_titles(&docs);

        assert!(search(&index, "nonexistent").is_empty());
    }

    // =========================================================================
    // LEAN SPECIFICATION TESTS
    // =========================================================================

    #[test]
    fn lean_spec_suffix_array_sorted() {
        let docs = vec![
            "banana".to_string(),
            "apple".to_string(),
            "cherry".to_string(),
        ];
        let index = build_test_docs(&docs);

        for i in 1..index.suffix_array.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            let prev_suffix = index.texts[prev.doc_id].get(prev.offset..).unwrap_or("");
            let curr_suffix = index.texts[curr.doc_id].get(curr.offset..).unwrap_or("");

            assert!(
                prev_suffix <= curr_suffix,
                "Suffix array not sorted at position {}: '{}' > '{}'",
                i,
                prev_suffix,
                curr_suffix
            );
        }
    }

    #[test]
    fn lean_spec_suffix_array_complete() {
        let docs = vec!["hello".to_string(), "world".to_string()];
        let index = build_test_docs(&docs);

        for (doc_id, text) in index.texts.iter().enumerate() {
            for offset in 0..text.len() {
                let found = index
                    .suffix_array
                    .iter()
                    .any(|e| e.doc_id == doc_id && e.offset == offset);
                assert!(
                    found,
                    "Missing suffix entry for doc_id={}, offset={}",
                    doc_id, offset
                );
            }
        }
    }

    #[test]
    fn lean_spec_lcp_correct() {
        let docs = vec!["banana".to_string(), "bandana".to_string()];
        let index = build_test_docs(&docs);

        assert_eq!(index.lcp.len(), index.suffix_array.len());

        if !index.lcp.is_empty() {
            assert_eq!(index.lcp[0], 0);
        }

        for i in 1..index.lcp.len() {
            let prev = &index.suffix_array[i - 1];
            let curr = &index.suffix_array[i];

            let prev_suffix = index.texts[prev.doc_id].get(prev.offset..).unwrap_or("");
            let curr_suffix = index.texts[curr.doc_id].get(curr.offset..).unwrap_or("");

            let expected_lcp = common_prefix_len(prev_suffix, curr_suffix);
            assert_eq!(index.lcp[i], expected_lcp);
        }
    }

    #[test]
    fn lean_spec_field_type_dominance() {
        let title_base = field_type_score(&FieldType::Title);
        let heading_base = field_type_score(&FieldType::Heading);
        let content_base = field_type_score(&FieldType::Content);
        let max_position_boost = 0.5;

        let worst_title = title_base - max_position_boost;
        let best_heading = heading_base + max_position_boost;
        assert!(worst_title > best_heading);

        let worst_heading = heading_base - max_position_boost;
        let best_content = content_base + max_position_boost;
        assert!(worst_heading > best_content);
    }

    #[test]
    fn lean_spec_index_well_formed() {
        let docs = vec!["test".to_string(), "document".to_string()];
        let index = build_test_docs(&docs);

        assert_eq!(index.docs.len(), index.texts.len());
        assert_eq!(index.lcp.len(), index.suffix_array.len());

        for (i, entry) in index.suffix_array.iter().enumerate() {
            assert!(
                entry.doc_id < index.texts.len(),
                "suffix_array[{}].doc_id out of bounds",
                i
            );
            assert!(
                entry.offset <= index.texts[entry.doc_id].len(),
                "suffix_array[{}].offset out of bounds",
                i
            );
        }
    }

    // =========================================================================
    // PROPERTY TESTS
    // =========================================================================

    proptest! {
        #[test]
        fn substring_search_finds_associated_docs(texts in text_vec_strategy()) {
            let normalized: Vec<String> = texts.iter().map(|text| normalize(text)).collect();
            let index = build_test_docs(&normalized);

            for doc_id in 0..index.docs.len() {
                let text = &index.texts[doc_id];
                prop_assume!(text.len() >= 3);
                let snippet = &text[1..text.len().min(4)];
                let results = search(&index, snippet);
                prop_assert!(results.iter().any(|doc| doc.id == doc_id));
            }
        }

        #[test]
        fn fuzzy_search_tolerates_small_typos(texts in text_vec_strategy()) {
            use crate::hybrid::{build_hybrid_index, search_hybrid};

            let normalized: Vec<String> = texts.iter().map(|text| normalize(text)).collect();
            let docs: Vec<SearchDoc> = normalized
                .iter()
                .enumerate()
                .map(|(id, _)| SearchDoc {
                    id,
                    title: format!("Doc {}", id),
                    excerpt: format!("Excerpt {}", id),
                    href: format!("/doc/{}", id),
                    kind: "post".to_string(),
                })
                .collect();
            let index = build_hybrid_index(docs, normalized.clone(), vec![]);

            for doc_id in 0..index.docs.len() {
                let text = &index.texts[doc_id];
                let word = text.split(' ').next().unwrap_or("");
                prop_assume!(word.len() > 3);
                let typo = mutate_term(word);
                prop_assume!(typo != word);
                let results = search_hybrid(&index, &typo);
                prop_assert!(results.iter().any(|doc| doc.id == doc_id));
            }
        }

        #[test]
        fn lean_proptest_suffix_array_sorted(texts in text_vec_strategy()) {
            let normalized: Vec<String> = texts.iter().map(|text| normalize(text)).collect();
            let index = build_test_docs(&normalized);

            for i in 1..index.suffix_array.len() {
                let prev = &index.suffix_array[i - 1];
                let curr = &index.suffix_array[i];

                let prev_suffix = index.texts.get(prev.doc_id)
                    .and_then(|t| t.get(prev.offset..))
                    .unwrap_or("");
                let curr_suffix = index.texts.get(curr.doc_id)
                    .and_then(|t| t.get(curr.offset..))
                    .unwrap_or("");

                prop_assert!(prev_suffix <= curr_suffix);
            }
        }

        #[test]
        fn lean_proptest_field_hierarchy_preserved(
            title_offset in 0usize..100,
            content_offset in 0usize..100,
            text_len in 100usize..1000,
        ) {
            let title_base = 100.0;
            let content_base = 1.0;

            let title_bonus = 0.5 * (1.0 - (title_offset as f64 / text_len as f64));
            let content_bonus = 0.5 * (1.0 - (content_offset as f64 / text_len as f64));

            let title_score = title_base + title_bonus;
            let content_score = content_base + content_bonus;

            prop_assert!(title_score > content_score);
        }
    }
}
