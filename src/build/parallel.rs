// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Parallel document loading and index construction.
//!
//! The expensive parts of building a search index are (1) loading JSON files
//! from disk and (2) building the inverted index. Both are embarrassingly parallel.
//! Rayon makes this trivial: `par_iter()` over documents, `par_iter()` over indexes.
//!
//! The one tricky bit is the Levenshtein DFA. It's expensive to build (~50ms) but
//! identical for all indexes. We build it once, wrap it in an `Arc`, and share it
//! across parallel index builds. Same for the embedded WASM bytes when that feature
//! is enabled.

use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "parallel")]
use indicatif::ProgressBar;

use crate::binary::{encode_docs_binary, BinaryLayer, DocMetaInput, PostingEntry};
use crate::fuzzy::dfa::ParametricDFA;
use crate::index::fst::build_fst_index;
use crate::runtime::deno::{
    ScoringContext, ScoringDocContext, ScoringEvaluator, ScoringMatchContext,
};
use crate::util::dict_table::{extract_href_prefix, DictTables};
use crate::{FieldBoundary, FieldType, SearchDoc};

use super::{Document, InputManifest, NormalizedIndexDefinition};

/// Build dictionary tables from documents for Parquet-style compression.
///
/// Collects unique values for category, author, tags, and href_prefix fields.
fn build_dict_tables(docs: &[SearchDoc]) -> DictTables {
    let mut tables = DictTables::new();

    for doc in docs {
        // Category dictionary
        if let Some(ref cat) = doc.category {
            tables.category.insert(cat);
        }

        // Author dictionary
        if let Some(ref author) = doc.author {
            tables.author.insert(author);
        }

        // Tags dictionary
        for tag in &doc.tags {
            tables.tags.insert(tag);
        }

        // Href prefix dictionary
        if let Some(prefix) = extract_href_prefix(&doc.href) {
            tables.href_prefix.insert(&prefix);
        }
    }

    tables
}

/// Loaded documents ready for indexing
pub struct LoadedDocuments {
    pub docs: Vec<Document>,
}

/// A built search index ready to serialize
pub struct BuiltIndex {
    pub name: String,
    pub bytes: Vec<u8>,
    pub doc_count: usize,
    pub term_count: usize,
}

/// Load all documents from input directory in parallel.
///
/// Reads and parses JSON files listed in manifest. Warns and continues on parse errors.
pub fn load_documents(input_dir: &Path, manifest: &InputManifest) -> Result<Vec<Document>, String> {
    manifest
        .documents
        .par_iter()
        .map(|filename| {
            let path = input_dir.join(filename);
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            serde_json::from_str::<Document>(&content).map_err(|e| {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                format!("Invalid JSON in {}: {}", filename, e)
            })
        })
        .collect::<Result<Vec<Document>, _>>()
        .map(|mut docs: Vec<Document>| {
            // Sort by ID to maintain consistent ordering
            docs.sort_by_key(|d| d.id);
            docs
        })
}

/// Load all documents from input directory in parallel with progress reporting.
#[cfg(feature = "parallel")]
pub fn load_documents_with_progress(
    input_dir: &Path,
    manifest: &InputManifest,
    progress: &ProgressBar,
) -> Result<Vec<Document>, String> {
    let counter = AtomicUsize::new(0);
    let total = manifest.documents.len();

    let result = manifest
        .documents
        .par_iter()
        .map(|filename| {
            let path = input_dir.join(filename);
            let content = fs::read_to_string(&path)
                .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;
            let doc = serde_json::from_str::<Document>(&content).map_err(|e| {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
                format!("Invalid JSON in {}: {}", filename, e)
            })?;

            // Update progress
            let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
            progress.set_position(count as u64);
            if count.is_multiple_of(10) || count == total {
                progress.set_message(format!("{}/{}", count, total));
            }

            Ok(doc)
        })
        .collect::<Result<Vec<Document>, String>>()?;

    let mut docs = result;
    // Sort by ID to maintain consistent ordering
    docs.sort_by_key(|d| d.id);
    Ok(docs)
}

/// Load all documents from input directory in parallel with progress reporting.
/// Non-parallel fallback (no-op progress).
#[cfg(not(feature = "parallel"))]
pub fn load_documents_with_progress(
    input_dir: &Path,
    manifest: &InputManifest,
) -> Result<Vec<Document>, String> {
    load_documents(input_dir, manifest)
}

/// Build multiple indexes in parallel with shared Levenshtein DFA.
///
/// Uses Rayon to construct each index in parallel, then writes binary format.
pub fn build_indexes_parallel(
    documents: &[Document],
    index_defs: &[(String, NormalizedIndexDefinition)],
    ranking_path: Option<&str>,
    ranking_batch_size: Option<usize>,
) -> Vec<BuiltIndex> {
    // Build Levenshtein DFA once (expensive) and share via Arc
    let lev_dfa = ParametricDFA::build(true);
    let lev_dfa_bytes = Arc::new(lev_dfa.to_bytes());

    // Always embed WASM when feature is enabled
    #[cfg(feature = "embed-wasm")]
    let wasm_bytes: Arc<Vec<u8>> =
        Arc::new(include_bytes!(concat!(env!("SOREX_OUT_DIR"), "/sorex_bg.wasm")).to_vec());

    // Load ranking function if specified
    let ranking_path = ranking_path.map(|s| s.to_string());

    // Build each index in parallel
    index_defs
        .par_iter()
        .map(|(name, def)| {
            build_single_index(
                name,
                def,
                documents,
                Arc::clone(&lev_dfa_bytes),
                #[cfg(feature = "embed-wasm")]
                Arc::clone(&wasm_bytes),
                ranking_path.as_deref(),
                ranking_batch_size,
            )
        })
        .collect()
}

/// Build multiple indexes in parallel with progress reporting.
#[cfg(feature = "parallel")]
pub fn build_indexes_with_progress(
    documents: &[Document],
    index_defs: &[(String, NormalizedIndexDefinition)],
    ranking_path: Option<&str>,
    ranking_batch_size: Option<usize>,
    progress: &ProgressBar,
) -> Vec<BuiltIndex> {
    // Build Levenshtein DFA once (expensive) and share via Arc
    progress.set_message("building Levenshtein DFA...");
    let lev_dfa = ParametricDFA::build(true);
    let lev_dfa_bytes = Arc::new(lev_dfa.to_bytes());

    // Always embed WASM when feature is enabled
    #[cfg(feature = "embed-wasm")]
    let wasm_bytes: Arc<Vec<u8>> =
        Arc::new(include_bytes!(concat!(env!("SOREX_OUT_DIR"), "/sorex_bg.wasm")).to_vec());

    // Load ranking function if specified
    let ranking_path = ranking_path.map(|s| s.to_string());

    let counter = AtomicUsize::new(0);
    let _total = index_defs.len();

    // Build each index in parallel
    let indexes: Vec<BuiltIndex> = index_defs
        .par_iter()
        .map(|(name, def)| {
            let index = build_single_index(
                name,
                def,
                documents,
                Arc::clone(&lev_dfa_bytes),
                #[cfg(feature = "embed-wasm")]
                Arc::clone(&wasm_bytes),
                ranking_path.as_deref(),
                ranking_batch_size,
            );

            // Update progress
            let count = counter.fetch_add(1, Ordering::Relaxed) + 1;
            progress.set_position(count as u64);
            progress.set_message(format!("{} ({} docs)", name, index.doc_count));

            index
        })
        .collect();

    indexes
}

/// Build multiple indexes in parallel with progress reporting.
/// Non-parallel fallback (no-op progress).
#[cfg(not(feature = "parallel"))]
pub fn build_indexes_with_progress(
    documents: &[Document],
    index_defs: &[(String, NormalizedIndexDefinition)],
    ranking_path: Option<&str>,
    ranking_batch_size: Option<usize>,
) -> Vec<BuiltIndex> {
    build_indexes_parallel(documents, index_defs, ranking_path, ranking_batch_size)
}

fn build_single_index(
    name: &str,
    def: &NormalizedIndexDefinition,
    documents: &[Document],
    lev_dfa_bytes: Arc<Vec<u8>>,
    #[cfg(feature = "embed-wasm")] wasm_bytes: Arc<Vec<u8>>,
    ranking_path: Option<&str>,
    ranking_batch_size: Option<usize>,
) -> BuiltIndex {
    // 1. Filter documents by include criteria
    let filtered_docs: Vec<&Document> = documents
        .iter()
        .filter(|d| def.include.matches(d))
        .collect();

    // 2. Convert to SearchDoc and texts with remapped doc IDs
    let mut search_docs: Vec<SearchDoc> = Vec::new();
    let mut texts: Vec<String> = Vec::new();
    let mut all_boundaries: Vec<FieldBoundary> = Vec::new();

    for (new_id, doc) in filtered_docs.iter().enumerate() {
        search_docs.push(SearchDoc {
            id: new_id,
            title: doc.title.clone(),
            excerpt: doc.excerpt.clone(),
            href: doc.href.clone(),
            kind: doc.doc_type.clone(),
            category: doc.category.clone(),
            author: doc.author.clone(),
            tags: doc.tags.clone(),
        });

        texts.push(doc.text.clone());

        // 3. Remap field boundary doc_ids to new sequential IDs
        for boundary in &doc.field_boundaries {
            // Filter boundaries by fields criteria if specified
            if let Some(ref field_filter) = def.fields {
                // Check if this boundary's field type is in the allowed list
                let field_name = boundary.field_type.as_str();

                if !field_filter.iter().any(|f| f == field_name) {
                    continue; // Skip this boundary
                }
            }

            all_boundaries.push(FieldBoundary {
                doc_id: new_id,
                start: boundary.start,
                end: boundary.end,
                field_type: boundary.field_type,
                section_id: boundary.section_id.clone(),
                heading_level: boundary.heading_level,
            });
        }
    }

    // 4. Build index using existing verified code
    let fst_index = build_fst_index(search_docs.clone(), texts, all_boundaries.clone());

    // 5. Convert to binary format
    let vocabulary = fst_index.vocabulary.clone();

    // Convert vocab_suffix_array to (u32, u32) format
    let suffix_array: Vec<(u32, u32)> = fst_index
        .vocab_suffix_array
        .iter()
        .map(|e| (e.term_idx as u32, e.offset as u32))
        .collect();

    // Build section_id table (deduplicated) and extract heading levels
    let mut section_id_map_with_levels: std::collections::HashMap<String, u8> =
        std::collections::HashMap::new();

    // Extract heading levels from field boundaries for each section_id
    for boundary in &all_boundaries {
        if let Some(ref section_id) = boundary.section_id {
            // Use the heading_level from the boundary (always set by build pipeline)
            let heading_level = boundary.heading_level;

            // Store the minimum heading level for each section_id
            // (section heading level determines bucket rank for all content under it)
            section_id_map_with_levels
                .entry(section_id.clone())
                .and_modify(|h| {
                    // Keep the lowest value (Title=0 < h1=1 < h2=2 < ... for better rank)
                    if heading_level < *h {
                        *h = heading_level;
                    }
                })
                .or_insert(heading_level);
        }
    }

    let mut section_table: Vec<String> = section_id_map_with_levels.keys().cloned().collect();
    section_table.sort(); // Sort for deterministic ordering

    // Create section_id -> index mapping (1-indexed, 0 = no section)
    let section_idx_map: std::collections::HashMap<&str, u32> = section_table
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), (i + 1) as u32))
        .collect();

    // Convert inverted index to postings array with section_id indices and heading levels
    // Always use the Deno evaluator for scoring (default or custom)
    let postings: Vec<Vec<PostingEntry>> = {
        // Load scoring evaluator (custom file or embedded default)
        let mut evaluator = if let Some(ranking_file) = ranking_path {
            ScoringEvaluator::from_file(std::path::Path::new(ranking_file))
                .expect("Failed to load custom scoring function")
        } else {
            ScoringEvaluator::from_default().expect("Failed to load default scoring function")
        };

        vocabulary
            .iter()
            .map(|term| {
                fst_index
                    .inverted_index
                    .terms
                    .get(term)
                    .map(|pl| {
                        // Build scoring contexts for batch evaluation
                        let contexts: Vec<ScoringContext> = pl
                            .postings
                            .iter()
                            .map(|p| {
                                let doc = &search_docs[p.doc_id];
                                // Get text length from the filtered docs' text
                                let text_length = filtered_docs
                                    .get(p.doc_id)
                                    .map(|d| d.text.len())
                                    .unwrap_or(0);
                                ScoringContext {
                                    term: term.clone(),
                                    doc: ScoringDocContext {
                                        id: doc.id,
                                        title: doc.title.clone(),
                                        excerpt: doc.excerpt.clone(),
                                        href: doc.href.clone(),
                                        doc_type: doc.kind.clone(),
                                        category: doc.category.clone(),
                                        author: doc.author.clone(),
                                        tags: doc.tags.clone(),
                                    },
                                    match_info: ScoringMatchContext {
                                        field_type: match p.field_type {
                                            FieldType::Title => "title".to_string(),
                                            FieldType::Heading => "heading".to_string(),
                                            FieldType::Content => "content".to_string(),
                                        },
                                        heading_level: p.heading_level,
                                        section_id: p.section_id.clone(),
                                        offset: p.offset,
                                        text_length,
                                    },
                                }
                            })
                            .collect();

                        // Evaluate scores in batch (with configurable chunk size)
                        let scores = evaluator
                            .evaluate_batch_chunked(&contexts, ranking_batch_size)
                            .expect("Scoring evaluation failed");

                        // Build posting entries with scores from evaluator
                        pl.postings
                            .iter()
                            .zip(scores.iter())
                            .map(|(p, &score)| {
                                let section_idx = if let Some(ref section_id) = p.section_id {
                                    section_idx_map
                                        .get(section_id.as_str())
                                        .copied()
                                        .unwrap_or(0)
                                } else {
                                    0
                                };

                                PostingEntry {
                                    doc_id: p.doc_id as u32,
                                    section_idx,
                                    heading_level: p.heading_level,
                                    score,
                                }
                            })
                            .collect()
                    })
                    .unwrap_or_default()
            })
            .collect()
    };

    // Encode docs as binary with section_id support
    let docs_input: Vec<DocMetaInput> = search_docs
        .iter()
        .map(|d| DocMetaInput {
            title: d.title.clone(),
            excerpt: d.excerpt.clone(),
            href: d.href.clone(),
            doc_type: d.kind.clone(),
            section_id: None,
            category: d.category.clone(),
            author: d.author.clone(),
            tags: d.tags.clone(),
        })
        .collect();
    let docs_bytes = encode_docs_binary(&docs_input);

    // Build dictionary tables for Parquet-style compression (v7)
    let dict_tables = build_dict_tables(&search_docs);
    let mut dict_table_bytes = Vec::new();
    dict_tables.encode(&mut dict_table_bytes);

    // Get WASM bytes (embedded when feature enabled, empty otherwise)
    #[cfg(feature = "embed-wasm")]
    let wasm_bytes_vec = (*wasm_bytes).clone();
    #[cfg(not(feature = "embed-wasm"))]
    let wasm_bytes_vec: Vec<u8> = Vec::new();

    // Build binary layer with v7 features: section_id support, heading_level, dict tables, WASM
    let mut layer = BinaryLayer::build_v7(
        &vocabulary,
        &suffix_array,
        &postings,
        &section_table,
        search_docs.len(),
        (*lev_dfa_bytes).clone(),
        docs_bytes,
        wasm_bytes_vec,
    )
    .expect("failed to build binary layer");

    // Add dictionary tables to the layer (v7 compression)
    layer.header.dict_table_len = dict_table_bytes.len() as u32;
    layer.dict_table_bytes = dict_table_bytes;

    let bytes = layer.to_bytes().expect("failed to serialize binary layer");

    BuiltIndex {
        name: name.to_string(),
        bytes,
        doc_count: search_docs.len(),
        term_count: vocabulary.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::{IncludeFilter, NormalizedIndexDefinition};

    fn make_doc(id: usize, slug: &str, category: Option<&str>) -> Document {
        Document {
            id,
            slug: slug.to_string(),
            title: format!("Title {}", slug),
            excerpt: format!("Excerpt {}", slug),
            href: format!("/{}", slug),
            doc_type: "post".to_string(),
            category: category.map(|s| s.to_string()),
            author: None,
            tags: vec![],
            text: format!("{} content", slug),
            field_boundaries: vec![],
        }
    }

    #[test]
    fn test_filter_all() {
        let docs = [
            make_doc(0, "a", Some("eng")),
            make_doc(1, "b", Some("adventures")),
        ];

        let def = NormalizedIndexDefinition {
            include: IncludeFilter::All,
            fields: None,
        };

        let filtered: Vec<&Document> = docs.iter().filter(|d| def.include.matches(d)).collect();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_category() {
        let docs = [
            make_doc(0, "a", Some("eng")),
            make_doc(1, "b", Some("adventures")),
            make_doc(2, "c", Some("eng")),
        ];

        let mut filters = std::collections::HashMap::new();
        filters.insert("category".to_string(), "eng".to_string());
        let def = NormalizedIndexDefinition {
            include: IncludeFilter::Filter(filters),
            fields: None,
        };

        let filtered: Vec<&Document> = docs.iter().filter(|d| def.include.matches(d)).collect();
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].slug, "a");
        assert_eq!(filtered[1].slug, "c");
    }

    #[test]
    fn test_doc_id_remapping() {
        let docs = [make_doc(0, "a", None), make_doc(1, "b", None)];

        let def = NormalizedIndexDefinition {
            include: IncludeFilter::All,
            fields: None,
        };

        let filtered: Vec<&Document> = docs.iter().filter(|d| def.include.matches(d)).collect();

        // After remapping, the new IDs should be 0, 1
        for (new_id, _doc) in filtered.iter().enumerate() {
            assert_eq!(new_id, new_id); // Just checking the structure
        }
    }
}
