//! Union index: combines separate indexes for titles, headings, and content.
//!
//! # Architecture
//!
//! Instead of one large index with field boundaries, we maintain three separate
//! HybridIndex instances:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                      UnionIndex                              │
//! ├─────────────────┬─────────────────┬─────────────────────────┤
//! │  TitlesIndex    │  HeadingsIndex  │     ContentIndex        │
//! │  (tiny, fast)   │  (small, fast)  │     (large)             │
//! │                 │                 │                         │
//! │  "Rust Guide"   │  "Getting"      │  "Rust is a systems..." │
//! │  "Python Intro" │  "Started"      │  "programming language" │
//! │                 │  "Advanced"     │  "with memory safety"   │
//! └─────────────────┴─────────────────┴─────────────────────────┘
//! ```
//!
//! # Parallel Construction
//!
//! Building uses map-reduce parallelism:
//!
//! ```text
//!                    ┌─────────────────────────────────────────────────────────┐
//!                    │                  Per-Index Pipeline                      │
//! Documents ──┬──►   │  Map (tokenize) ──► Reduce (merge) ──► Vocab ──► SA-IS  │
//!             │      └─────────────────────────────────────────────────────────┘
//!             │
//!             ├──► Titles Pipeline    ──┐
//!             ├──► Headings Pipeline  ──┼──► (all 3 run in parallel via rayon::join)
//!             └──► Content Pipeline   ──┘
//! ```
//!
//! Within each pipeline, SA-IS runs *after* reduce (needs vocabulary).
//!
//! # Benefits
//!
//! 1. **Smaller indexes**: Title index has ~100 terms vs ~10K in content
//! 2. **Source attribution**: Results include where the match was found
//! 3. **Early termination**: Can stop after finding title matches
//! 4. **Separate optimization**: Each index tuned for its content type
//! 5. **Parallel build**: All three indexes built concurrently

use crate::hybrid::{build_hybrid_index, build_hybrid_index_parallel, search_hybrid};
use crate::types::{HybridIndex, SearchDoc, SearchResult, SearchSource, UnionIndex};
use std::collections::HashMap;

/// Input data for building a union index.
///
/// Each document provides separate text for titles, headings, and content.
#[derive(Debug, Clone)]
pub struct UnionIndexInput {
    /// Document metadata
    pub doc: SearchDoc,
    /// Post/page title text
    pub title: String,
    /// Section headings (h2, h3, etc.) concatenated
    pub headings: String,
    /// Body content text
    pub content: String,
}

/// Build a union index from document inputs.
///
/// Creates three separate HybridIndex instances for titles, headings, and content.
/// Uses parallel construction via rayon:
/// 1. Extract texts for each index type (parallel over documents)
/// 2. Build all three indexes in parallel
pub fn build_union_index(inputs: Vec<UnionIndexInput>) -> UnionIndex {
    let docs: Vec<SearchDoc> = inputs.iter().map(|i| i.doc.clone()).collect();

    // Extract texts for each index type
    let title_texts: Vec<String> = inputs.iter().map(|i| i.title.clone()).collect();
    let heading_texts: Vec<String> = inputs.iter().map(|i| i.headings.clone()).collect();
    let content_texts: Vec<String> = inputs.iter().map(|i| i.content.clone()).collect();

    // Check which indexes have content
    let has_titles = title_texts.iter().any(|t| !t.is_empty());
    let has_headings = heading_texts.iter().any(|t| !t.is_empty());
    let has_content = content_texts.iter().any(|t| !t.is_empty());

    // Build all three indexes
    #[cfg(feature = "parallel")]
    let (titles, headings, content) = {
        let (titles, (headings, content)) = rayon::join(
            || {
                if has_titles {
                    Some(build_hybrid_index(docs.clone(), title_texts, vec![]))
                } else {
                    None
                }
            },
            || {
                rayon::join(
                    || {
                        if has_headings {
                            Some(build_hybrid_index(docs.clone(), heading_texts, vec![]))
                        } else {
                            None
                        }
                    },
                    || {
                        if has_content {
                            Some(build_hybrid_index(docs.clone(), content_texts, vec![]))
                        } else {
                            None
                        }
                    },
                )
            },
        );
        (titles, headings, content)
    };

    #[cfg(not(feature = "parallel"))]
    let (titles, headings, content) = {
        let titles = if has_titles {
            Some(build_hybrid_index(docs.clone(), title_texts, vec![]))
        } else {
            None
        };
        let headings = if has_headings {
            Some(build_hybrid_index(docs.clone(), heading_texts, vec![]))
        } else {
            None
        };
        let content = if has_content {
            Some(build_hybrid_index(docs.clone(), content_texts, vec![]))
        } else {
            None
        };
        (titles, headings, content)
    };

    UnionIndex {
        docs,
        titles,
        headings,
        content,
    }
}

/// Build a union index using parallel construction.
///
/// Uses parallel hybrid index construction for each sub-index,
/// and builds all three indexes concurrently.
/// Best for large corpora (100+ documents).
pub fn build_union_index_parallel(inputs: Vec<UnionIndexInput>) -> UnionIndex {
    let docs: Vec<SearchDoc> = inputs.iter().map(|i| i.doc.clone()).collect();

    // Extract texts for each index type
    let title_texts: Vec<String> = inputs.iter().map(|i| i.title.clone()).collect();
    let heading_texts: Vec<String> = inputs.iter().map(|i| i.headings.clone()).collect();
    let content_texts: Vec<String> = inputs.iter().map(|i| i.content.clone()).collect();

    // Check which indexes have content
    let has_titles = title_texts.iter().any(|t| !t.is_empty());
    let has_headings = heading_texts.iter().any(|t| !t.is_empty());
    let has_content = content_texts.iter().any(|t| !t.is_empty());

    // Build all three indexes using parallel hybrid construction
    #[cfg(feature = "parallel")]
    let (titles, headings, content) = {
        let (titles, (headings, content)) = rayon::join(
            || {
                if has_titles {
                    Some(build_hybrid_index_parallel(
                        docs.clone(),
                        title_texts,
                        vec![],
                    ))
                } else {
                    None
                }
            },
            || {
                rayon::join(
                    || {
                        if has_headings {
                            Some(build_hybrid_index_parallel(
                                docs.clone(),
                                heading_texts,
                                vec![],
                            ))
                        } else {
                            None
                        }
                    },
                    || {
                        if has_content {
                            Some(build_hybrid_index_parallel(
                                docs.clone(),
                                content_texts,
                                vec![],
                            ))
                        } else {
                            None
                        }
                    },
                )
            },
        );
        (titles, headings, content)
    };

    #[cfg(not(feature = "parallel"))]
    let (titles, headings, content) = {
        let titles = if has_titles {
            Some(build_hybrid_index_parallel(
                docs.clone(),
                title_texts,
                vec![],
            ))
        } else {
            None
        };
        let headings = if has_headings {
            Some(build_hybrid_index_parallel(
                docs.clone(),
                heading_texts,
                vec![],
            ))
        } else {
            None
        };
        let content = if has_content {
            Some(build_hybrid_index_parallel(
                docs.clone(),
                content_texts,
                vec![],
            ))
        } else {
            None
        };
        (titles, headings, content)
    };

    UnionIndex {
        docs,
        titles,
        headings,
        content,
    }
}

/// Score multipliers for each source type.
/// These ensure title matches always rank above heading matches,
/// which always rank above content matches.
const TITLE_MULTIPLIER: f64 = 100.0;
const HEADING_MULTIPLIER: f64 = 10.0;
const CONTENT_MULTIPLIER: f64 = 1.0;

/// Search the union index and return results with source attribution.
///
/// Searches all three indexes (titles, headings, content) and merges results.
/// For documents matching in multiple indexes, keeps the highest-scoring source.
pub fn search_union(index: &UnionIndex, query: &str) -> Vec<SearchResult> {
    let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

    // Search titles index (highest priority)
    if let Some(titles) = &index.titles {
        for doc in search_hybrid(titles, query) {
            let score = base_score_for_doc(titles, &doc) * TITLE_MULTIPLIER;
            update_best_result(&mut results_by_doc, doc, SearchSource::Title, score);
        }
    }

    // Search headings index (medium priority)
    if let Some(headings) = &index.headings {
        for doc in search_hybrid(headings, query) {
            let score = base_score_for_doc(headings, &doc) * HEADING_MULTIPLIER;
            update_best_result(&mut results_by_doc, doc, SearchSource::Heading, score);
        }
    }

    // Search content index (base priority)
    if let Some(content) = &index.content {
        for doc in search_hybrid(content, query) {
            let score = base_score_for_doc(content, &doc) * CONTENT_MULTIPLIER;
            update_best_result(&mut results_by_doc, doc, SearchSource::Content, score);
        }
    }

    // Sort by score descending
    let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results
}

/// Search the union index and return results grouped by source.
///
/// Returns three separate vectors for title, heading, and content matches.
/// Useful for UI that displays "Found in title" sections.
pub fn search_union_grouped(
    index: &UnionIndex,
    query: &str,
) -> (Vec<SearchDoc>, Vec<SearchDoc>, Vec<SearchDoc>) {
    let title_results = index
        .titles
        .as_ref()
        .map(|idx| search_hybrid(idx, query))
        .unwrap_or_default();

    let heading_results = index
        .headings
        .as_ref()
        .map(|idx| search_hybrid(idx, query))
        .unwrap_or_default();

    let content_results = index
        .content
        .as_ref()
        .map(|idx| search_hybrid(idx, query))
        .unwrap_or_default();

    (title_results, heading_results, content_results)
}

/// Get base score for a document (position in result list as proxy).
fn base_score_for_doc(_index: &HybridIndex, _doc: &SearchDoc) -> f64 {
    // For now, use a fixed base score. In a more sophisticated implementation,
    // we could extract the actual score from the search.
    1.0
}

/// Update the best result for a document if this score is higher.
fn update_best_result(
    results: &mut HashMap<usize, SearchResult>,
    doc: SearchDoc,
    source: SearchSource,
    score: f64,
) {
    let doc_id = doc.id;
    // Section ID is None for now - will be populated from field boundaries
    // when the TypeScript build pipeline sets them
    let section_id = match source {
        SearchSource::Title => None, // Title matches link to top of page
        _ => None,                   // Heading/Content section_id to be added via field boundaries
    };
    results
        .entry(doc_id)
        .and_modify(|existing| {
            if score > existing.score {
                existing.source = source;
                existing.score = score;
                existing.section_id = section_id.clone();
            }
        })
        .or_insert(SearchResult {
            doc,
            source,
            score,
            section_id,
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_input(id: usize, title: &str, headings: &str, content: &str) -> UnionIndexInput {
        UnionIndexInput {
            doc: SearchDoc {
                id,
                title: title.to_string(),
                excerpt: String::new(),
                href: format!("/doc/{}", id),
                kind: "post".to_string(),
            },
            title: title.to_string(),
            headings: headings.to_string(),
            content: content.to_string(),
        }
    }

    #[test]
    fn test_build_union_index() {
        let inputs = vec![
            make_input(
                0,
                "Rust Guide",
                "Getting Started",
                "Rust is a systems language",
            ),
            make_input(1, "Python Intro", "Basics", "Python is interpreted"),
        ];
        let index = build_union_index(inputs);

        assert_eq!(index.docs.len(), 2);
        assert!(index.titles.is_some());
        assert!(index.headings.is_some());
        assert!(index.content.is_some());
    }

    #[test]
    fn test_title_match_ranks_highest() {
        let inputs = vec![
            make_input(0, "Rust Programming", "Overview", "Learn about languages"),
            make_input(1, "Python Guide", "Rust Section", "Python basics"),
            make_input(2, "Java Tutorial", "Getting Started", "Rust mentioned here"),
        ];
        let index = build_union_index(inputs);

        let results = search_union(&index, "rust");

        // Should find all three docs
        assert_eq!(results.len(), 3);

        // Title match should be first
        assert_eq!(results[0].doc.id, 0);
        assert_eq!(results[0].source, SearchSource::Title);

        // Heading match should be second
        assert_eq!(results[1].doc.id, 1);
        assert_eq!(results[1].source, SearchSource::Heading);

        // Content match should be third
        assert_eq!(results[2].doc.id, 2);
        assert_eq!(results[2].source, SearchSource::Content);
    }

    #[test]
    fn test_search_grouped() {
        let inputs = vec![
            make_input(0, "Rust Guide", "Overview", "Content here"),
            make_input(1, "Other", "Rust Section", "More content"),
            make_input(2, "Another", "Intro", "Rust mentioned"),
        ];
        let index = build_union_index(inputs);

        let (titles, headings, content) = search_union_grouped(&index, "rust");

        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].id, 0);

        assert_eq!(headings.len(), 1);
        assert_eq!(headings[0].id, 1);

        assert_eq!(content.len(), 1);
        assert_eq!(content[0].id, 2);
    }

    #[test]
    fn test_substring_search_in_union() {
        let inputs = vec![
            make_input(0, "TypeScript Guide", "Overview", "Content"),
            make_input(1, "JavaScript Basics", "Intro", "More content"),
        ];
        let index = build_union_index(inputs);

        // "script" should match both TypeScript and JavaScript
        let results = search_union(&index, "script");
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_empty_index_sections() {
        let inputs = vec![
            make_input(0, "Title Only", "", ""), // No headings or content
        ];
        let index = build_union_index(inputs);

        assert!(index.titles.is_some());
        // Headings and content should still be Some but with empty vocabularies

        let results = search_union(&index, "title");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, SearchSource::Title);
    }

    #[test]
    fn test_multi_word_query() {
        let inputs = vec![
            make_input(
                0,
                "Rust Programming",
                "Getting Started",
                "Learn Rust basics",
            ),
            make_input(1, "Python Guide", "Programming Basics", "Python intro"),
        ];
        let index = build_union_index(inputs);

        // "rust programming" should match doc 0 in title
        let results = search_union(&index, "rust programming");
        assert!(!results.is_empty());
        assert_eq!(results[0].doc.id, 0);
    }
}
