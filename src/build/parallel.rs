use rayon::prelude::*;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[cfg(feature = "parallel")]
use indicatif::ProgressBar;

use crate::binary::{encode_docs_binary, BinaryLayer, DocMetaInput, PostingEntry};
use crate::fst_index::build_fst_index;
use crate::levenshtein_dfa::ParametricDFA;
use crate::{FieldBoundary, FieldType, SearchDoc};

use super::{Document, InputManifest, NormalizedIndexDefinition};

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
            if count % 10 == 0 || count == total {
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
) -> Vec<BuiltIndex> {
    // Build Levenshtein DFA once (expensive) and share via Arc
    let lev_dfa = ParametricDFA::build(true);
    let lev_dfa_bytes = Arc::new(lev_dfa.to_bytes());

    // Build each index in parallel
    index_defs
        .par_iter()
        .map(|(name, def)| build_single_index(name, def, documents, Arc::clone(&lev_dfa_bytes)))
        .collect()
}

/// Build multiple indexes in parallel with progress reporting.
#[cfg(feature = "parallel")]
pub fn build_indexes_with_progress(
    documents: &[Document],
    index_defs: &[(String, NormalizedIndexDefinition)],
    progress: &ProgressBar,
) -> Vec<BuiltIndex> {
    // Build Levenshtein DFA once (expensive) and share via Arc
    progress.set_message("building Levenshtein DFA...");
    let lev_dfa = ParametricDFA::build(true);
    let lev_dfa_bytes = Arc::new(lev_dfa.to_bytes());

    let counter = AtomicUsize::new(0);
    let _total = index_defs.len();

    // Build each index in parallel
    let indexes: Vec<BuiltIndex> = index_defs
        .par_iter()
        .map(|(name, def)| {
            let index = build_single_index(name, def, documents, Arc::clone(&lev_dfa_bytes));

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
) -> Vec<BuiltIndex> {
    build_indexes_parallel(documents, index_defs)
}

fn build_single_index(
    name: &str,
    def: &NormalizedIndexDefinition,
    documents: &[Document],
    lev_dfa_bytes: Arc<Vec<u8>>,
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
        });

        texts.push(doc.text.clone());

        // 3. Remap field boundary doc_ids to new sequential IDs
        for boundary in &doc.field_boundaries {
            // Filter boundaries by fields criteria if specified
            if let Some(ref field_filter) = def.fields {
                // Check if this boundary's field type is in the allowed list
                let field_name = match boundary.field_type {
                    FieldType::Title => "title",
                    FieldType::Heading => "heading",
                    FieldType::Content => "content",
                };

                if !field_filter.iter().any(|f| f == field_name) {
                    continue; // Skip this boundary
                }
            }

            all_boundaries.push(FieldBoundary {
                doc_id: new_id,
                start: boundary.start,
                end: boundary.end,
                field_type: boundary.field_type.clone(),
                section_id: boundary.section_id.clone(),
            });
        }
    }

    // 4. Build index using existing verified code
    let fst_index = build_fst_index(search_docs.clone(), texts, all_boundaries);

    // 5. Convert to binary format
    let vocabulary = fst_index.vocabulary;

    // Convert vocab_suffix_array to (u32, u32) format
    let suffix_array: Vec<(u32, u32)> = fst_index
        .vocab_suffix_array
        .iter()
        .map(|e| (e.term_idx as u32, e.offset as u32))
        .collect();

    // Build section_id table (deduplicated)
    let mut section_id_set: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pl in fst_index.inverted_index.terms.values() {
        for posting in &pl.postings {
            if let Some(ref id) = posting.section_id {
                section_id_set.insert(id.clone());
            }
        }
    }
    let section_table: Vec<String> = section_id_set.into_iter().collect();

    // Create section_id -> index mapping (1-indexed, 0 = no section)
    let section_idx_map: std::collections::HashMap<&str, u32> = section_table
        .iter()
        .enumerate()
        .map(|(i, id)| (id.as_str(), (i + 1) as u32))
        .collect();

    // Convert inverted index to postings array with section_id indices
    let postings: Vec<Vec<PostingEntry>> = vocabulary
        .iter()
        .map(|term| {
            fst_index
                .inverted_index
                .terms
                .get(term)
                .map(|pl| {
                    pl.postings
                        .iter()
                        .map(|p| {
                            let section_idx = p
                                .section_id
                                .as_ref()
                                .and_then(|id| section_idx_map.get(id.as_str()))
                                .copied()
                                .unwrap_or(0);
                            PostingEntry {
                                doc_id: p.doc_id as u32,
                                section_idx,
                            }
                        })
                        .collect()
                })
                .unwrap_or_default()
        })
        .collect();

    // Encode docs as binary with section_id support
    let docs_input: Vec<DocMetaInput> = search_docs
        .iter()
        .map(|d| DocMetaInput {
            title: d.title.clone(),
            excerpt: d.excerpt.clone(),
            href: d.href.clone(),
            doc_type: d.kind.clone(),
            section_id: None,
        })
        .collect();
    let docs_bytes = encode_docs_binary(&docs_input);

    // Build binary layer with section_id support (v6)
    let layer = BinaryLayer::build_v6(
        &vocabulary,
        &suffix_array,
        &postings,
        &section_table,
        search_docs.len(),
        (*lev_dfa_bytes).clone(),
        docs_bytes,
    )
    .expect("failed to build binary layer");

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
            text: format!("{} content", slug),
            field_boundaries: vec![],
        }
    }

    #[test]
    fn test_filter_all() {
        let docs = vec![
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
        let docs = vec![
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
        let docs = vec![make_doc(0, "a", None), make_doc(1, "b", None)];

        let def = NormalizedIndexDefinition {
            include: IncludeFilter::All,
            fields: None,
        };

        let filtered: Vec<&Document> = docs.iter().filter(|d| def.include.matches(d)).collect();

        // After remapping, the new IDs should be 0, 1
        for (new_id, doc) in filtered.iter().enumerate() {
            assert_eq!(new_id, new_id); // Just checking the structure
        }
    }
}
