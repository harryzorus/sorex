//! WebAssembly bindings for the Sorex search index.
//!
//! Provides two WASM-accessible index types:
//! - `SorexProgressiveIndex`: Progressive layer loading for fast initial results
//! - `SorexSearcher`: Direct binary search (loads .sorex file)

use crate::binary::{LoadedLayer, PostingEntry};
use crate::hybrid::search_hybrid;
use crate::types::{
    HybridIndex, InvertedIndex, Posting, PostingList, SearchDoc, SearchResult, SearchSource,
    VocabSuffixEntry,
};
use serde::Serialize;
use serde_wasm_bindgen::{from_value, to_value};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

/// Search result output for TypeScript consumption.
/// Matches the SearchResult interface in SearchState.svelte.ts
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchResultOutput {
    href: String,
    title: String,
    excerpt: String,
    /// Section ID for deep linking (null for title matches)
    section_id: Option<String>,
}

/// Score multipliers for each source type (must match union.rs).
const TITLE_MULTIPLIER: f64 = 100.0;
const HEADING_MULTIPLIER: f64 = 10.0;
const CONTENT_MULTIPLIER: f64 = 1.0;

/// Search options passed from JavaScript.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default)]
pub struct SearchOptions {
    /// Maximum number of results to return (default: 10)
    pub limit: usize,
    /// Enable fuzzy matching (default: true)
    pub fuzzy: bool,
    /// Enable prefix matching (default: true)
    pub prefix: bool,
    /// Custom boost multipliers for each field
    pub boost: Option<BoostOptions>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            fuzzy: true,
            prefix: true,
            boost: None,
        }
    }
}

/// Custom boost multipliers for search fields.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BoostOptions {
    /// Title field boost (default: 100.0)
    #[serde(default = "default_title_boost")]
    pub title: f64,
    /// Heading field boost (default: 10.0)
    #[serde(default = "default_heading_boost")]
    pub heading: f64,
    /// Content field boost (default: 1.0)
    #[serde(default = "default_content_boost")]
    pub content: f64,
}

fn default_title_boost() -> f64 {
    TITLE_MULTIPLIER
}
fn default_heading_boost() -> f64 {
    HEADING_MULTIPLIER
}
fn default_content_boost() -> f64 {
    CONTENT_MULTIPLIER
}

// CompactLayer JSON format removed - binary-only now (no base64 dependency in WASM)

/// WASM-accessible progressive search index.
///
/// Supports loading index layers incrementally for fast initial results:
/// 1. Load manifest (docs only) - instant
/// 2. Load titles layer (~5KB) - fast first results
/// 3. Load headings layer (~20KB) - expanded coverage
/// 4. Load content layer (~200KB) - full search
///
/// Each layer is a separate HybridIndex that can be loaded independently.
#[wasm_bindgen]
pub struct SorexProgressiveIndex {
    /// Document metadata (always present after init)
    docs: Vec<SearchDoc>,
    /// Titles layer (loaded first, smallest)
    titles: Option<HybridIndex>,
    /// Headings layer (loaded second)
    headings: Option<HybridIndex>,
    /// Content layer (loaded last, largest)
    content: Option<HybridIndex>,
}

#[wasm_bindgen]
impl SorexProgressiveIndex {
    /// Create a new progressive index from a manifest.
    ///
    /// The manifest contains only document metadata, no search data.
    /// Layers must be loaded separately via `load_layer()`.
    #[wasm_bindgen(constructor)]
    pub fn new(manifest: JsValue) -> Result<SorexProgressiveIndex, JsValue> {
        let docs: Vec<SearchDoc> = from_value(manifest).map_err(|e| e.to_string())?;
        Ok(SorexProgressiveIndex {
            docs,
            titles: None,
            headings: None,
            content: None,
        })
    }

    /// Load a specific layer from binary format.
    ///
    /// Valid layer names: "titles", "headings", "content"
    ///
    /// Binary format (.sorex files) uses:
    /// - FST vocabulary (5-10x smaller than JSON)
    /// - Block PFOR postings (Lucene-style 128-doc blocks)
    /// - Skip lists for large posting lists
    ///
    /// This method is ~3-5x faster to decode than JSON format.
    #[wasm_bindgen]
    pub fn load_layer_binary(
        &mut self,
        layer_name: &str,
        layer_bytes: &[u8],
    ) -> Result<(), JsValue> {
        use crate::types::FieldType;

        let field_type = match layer_name {
            "titles" => FieldType::Title,
            "headings" => FieldType::Heading,
            "content" => FieldType::Content,
            _ => return Err(format!("Unknown layer: {}", layer_name).into()),
        };

        let loaded = LoadedLayer::from_bytes(layer_bytes)
            .map_err(|e| format!("Failed to parse {} binary layer: {}", layer_name, e))?;

        let index = loaded_layer_to_hybrid_index(loaded, &self.docs, field_type);

        match layer_name {
            "titles" => self.titles = Some(index),
            "headings" => self.headings = Some(index),
            "content" => self.content = Some(index),
            _ => unreachable!(),
        }

        Ok(())
    }

    /// Check if a specific layer is loaded.
    #[wasm_bindgen]
    pub fn has_layer(&self, layer_name: &str) -> bool {
        match layer_name {
            "titles" => self.titles.is_some(),
            "headings" => self.headings.is_some(),
            "content" => self.content.is_some(),
            _ => false,
        }
    }

    /// Get list of loaded layer names.
    #[wasm_bindgen]
    pub fn loaded_layers(&self) -> Vec<String> {
        let mut layers = Vec::new();
        if self.titles.is_some() {
            layers.push("titles".to_string());
        }
        if self.headings.is_some() {
            layers.push("headings".to_string());
        }
        if self.content.is_some() {
            layers.push("content".to_string());
        }
        layers
    }

    /// Check if all layers are loaded.
    #[wasm_bindgen]
    pub fn is_fully_loaded(&self) -> bool {
        self.titles.is_some() && self.headings.is_some() && self.content.is_some()
    }

    /// Get the total number of documents.
    #[wasm_bindgen]
    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    /// Search across all loaded layers and return results.
    ///
    /// Results include source attribution (title, heading, or content).
    /// For documents matching in multiple layers, the highest-scoring source is used.
    ///
    /// Options (all optional):
    /// - `limit`: Maximum results (default: 10)
    /// - `fuzzy`: Enable fuzzy matching (default: true)
    /// - `prefix`: Enable prefix matching (default: true)
    /// - `boost`: Custom field boosts `{title: 100, heading: 10, content: 1}`
    #[wasm_bindgen]
    pub fn search(&self, query: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
        // Parse options or use defaults
        let options: SearchOptions = match options {
            Some(opts) => from_value(opts).unwrap_or_default(),
            None => SearchOptions::default(),
        };

        // Get boost multipliers
        let (title_boost, heading_boost, content_boost) = match &options.boost {
            Some(b) => (b.title, b.heading, b.content),
            None => (TITLE_MULTIPLIER, HEADING_MULTIPLIER, CONTENT_MULTIPLIER),
        };

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Search titles layer (highest priority)
        if let Some(titles) = &self.titles {
            for doc in search_hybrid(titles, query) {
                let score = 1.0 * title_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Title, score);
            }
        }

        // Search headings layer (medium priority)
        if let Some(headings) = &self.headings {
            for doc in search_hybrid(headings, query) {
                let score = 1.0 * heading_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Heading, score);
            }
        }

        // Search content layer (base priority)
        if let Some(content) = &self.content {
            for doc in search_hybrid(content, query) {
                let score = 1.0 * content_boost;
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

        // Apply limit
        results.truncate(options.limit);

        to_value(&results).map_err(|e| e.to_string().into())
    }

    // =========================================================================
    // STREAMING SEARCH API
    // =========================================================================
    // Two-phase search for progressive results:
    // 1. search_exact() - O(1) inverted index lookup (returns first results fast)
    // 2. search_expanded() - O(log k) suffix array search (additional matches)
    //
    // Lean Specification: StreamingSearch.lean

    /// Search using only inverted index (O(1) exact word matches).
    ///
    /// Returns results from exact word matches only. This is the fast path
    /// that provides first results immediately.
    ///
    /// Use this for the first phase of streaming search.
    #[wasm_bindgen]
    pub fn search_exact(&self, query: &str, options: Option<JsValue>) -> Result<JsValue, JsValue> {
        let options: SearchOptions = match options {
            Some(opts) => from_value(opts).unwrap_or_default(),
            None => SearchOptions::default(),
        };

        let (title_boost, heading_boost, content_boost) = match &options.boost {
            Some(b) => (b.title, b.heading, b.content),
            None => (TITLE_MULTIPLIER, HEADING_MULTIPLIER, CONTENT_MULTIPLIER),
        };

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Search each layer using exact-only search
        if let Some(titles) = &self.titles {
            for doc in crate::hybrid::search_exact(titles, query) {
                let score = 1.0 * title_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Title, score);
            }
        }
        if let Some(headings) = &self.headings {
            for doc in crate::hybrid::search_exact(headings, query) {
                let score = 1.0 * heading_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Heading, score);
            }
        }
        if let Some(content) = &self.content {
            for doc in crate::hybrid::search_exact(content, query) {
                let score = 1.0 * content_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Content, score);
            }
        }

        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(options.limit);

        to_value(&results).map_err(|e| e.to_string().into())
    }

    /// Search using suffix array, excluding already-found IDs (O(log k)).
    ///
    /// Returns additional results not found by exact search.
    /// Pass the doc IDs from search_exact() as exclude_ids.
    ///
    /// Use this for the second phase of streaming search.
    #[wasm_bindgen]
    pub fn search_expanded(
        &self,
        query: &str,
        exclude_ids: JsValue,
        options: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let options: SearchOptions = match options {
            Some(opts) => from_value(opts).unwrap_or_default(),
            None => SearchOptions::default(),
        };

        let exclude_ids: Vec<usize> = from_value(exclude_ids).unwrap_or_default();

        let (title_boost, heading_boost, content_boost) = match &options.boost {
            Some(b) => (b.title, b.heading, b.content),
            None => (TITLE_MULTIPLIER, HEADING_MULTIPLIER, CONTENT_MULTIPLIER),
        };

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Search each layer using expanded-only search
        if let Some(titles) = &self.titles {
            for doc in crate::hybrid::search_expanded(titles, query, &exclude_ids) {
                let score = 1.0 * title_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Title, score);
            }
        }
        if let Some(headings) = &self.headings {
            for doc in crate::hybrid::search_expanded(headings, query, &exclude_ids) {
                let score = 1.0 * heading_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Heading, score);
            }
        }
        if let Some(content) = &self.content {
            for doc in crate::hybrid::search_expanded(content, query, &exclude_ids) {
                let score = 1.0 * content_boost;
                update_best_result(&mut results_by_doc, doc, SearchSource::Content, score);
            }
        }

        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(options.limit);

        to_value(&results).map_err(|e| e.to_string().into())
    }

    /// Get suggestions for a partial query (prefix search on vocabulary).
    ///
    /// Returns terms from the index that start with the given prefix,
    /// sorted by document frequency (most common first).
    #[wasm_bindgen]
    pub fn suggest(&self, partial: &str, limit: Option<usize>) -> Result<JsValue, JsValue> {
        let limit = limit.unwrap_or(5);
        let partial_lower = partial.to_lowercase();

        // Collect matching terms with their frequencies from all layers
        let mut term_freqs: HashMap<String, usize> = HashMap::new();

        // Helper to collect terms from a layer's vocabulary
        let collect_from_layer = |index: &HybridIndex, freqs: &mut HashMap<String, usize>| {
            for term in &index.vocabulary {
                if term.starts_with(&partial_lower) {
                    let freq = index
                        .inverted_index
                        .terms
                        .get(term)
                        .map(|pl| pl.doc_freq)
                        .unwrap_or(0);
                    *freqs.entry(term.clone()).or_insert(0) += freq;
                }
            }
        };

        if let Some(titles) = &self.titles {
            collect_from_layer(titles, &mut term_freqs);
        }
        if let Some(headings) = &self.headings {
            collect_from_layer(headings, &mut term_freqs);
        }
        if let Some(content) = &self.content {
            collect_from_layer(content, &mut term_freqs);
        }

        // Sort by frequency descending, then alphabetically
        let mut suggestions: Vec<(String, usize)> = term_freqs.into_iter().collect();
        suggestions.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

        // Take top N and extract just the terms
        let result: Vec<String> = suggestions
            .into_iter()
            .take(limit)
            .map(|(term, _)| term)
            .collect();

        to_value(&result).map_err(|e| e.to_string().into())
    }
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

/// Convert a LoadedLayer from binary format to HybridIndex.
fn loaded_layer_to_hybrid_index(
    layer: LoadedLayer,
    docs: &[SearchDoc],
    field_type: crate::types::FieldType,
) -> HybridIndex {
    // Use vocabulary directly from layer (no FST needed)
    let vocabulary = layer.vocabulary.clone();

    // Build vocab suffix array from binary format
    let vocab_suffix_array: Vec<VocabSuffixEntry> = layer
        .suffix_array
        .iter()
        .map(|&(term_idx, offset)| VocabSuffixEntry {
            term_idx: term_idx as usize,
            offset: offset as usize,
        })
        .collect();

    // Build inverted index from postings (v6 format with section_ids)
    let mut terms = HashMap::new();
    for (term_idx, posting_list) in layer.postings.iter().enumerate() {
        if let Some(term) = vocabulary.get(term_idx) {
            let postings: Vec<Posting> = posting_list
                .iter()
                .map(|entry| {
                    // Resolve section_id from section_table (0 = None, 1+ = table index)
                    let section_id = if entry.section_idx == 0 {
                        None
                    } else {
                        layer
                            .section_table
                            .get((entry.section_idx - 1) as usize)
                            .cloned()
                    };
                    Posting {
                        doc_id: entry.doc_id as usize,
                        offset: 0,
                        field_type: field_type.clone(),
                        section_id,
                    }
                })
                .collect();

            terms.insert(
                term.clone(),
                PostingList {
                    doc_freq: postings.len(),
                    postings,
                },
            );
        }
    }

    let inverted_index = InvertedIndex {
        terms,
        total_docs: docs.len(),
    };

    HybridIndex {
        docs: docs.to_vec(),
        texts: vec![String::new(); docs.len()],
        field_boundaries: Vec::new(),
        inverted_index,
        vocabulary: layer.vocabulary,
        vocab_suffix_array,
    }
}

// =============================================================================
// SOREX SEARCHER (Zero-CPU Fuzzy Search with Precomputed Levenshtein Automata)
// =============================================================================
//
// Implementation of Schulz-Mihov (2002) Universal Levenshtein Automata.
//
// Key insight: DFA transition tables are query-independent. We precompute them
// at build time and embed them in the .sorex binary file. At search time,
// building query-specific matchers is pure table lookups (~1μs).
//
// Performance:
// - DFA load: one-time deserialization from embedded bytes
// - Query matcher build: ~1μs (table lookups only)
// - Fuzzy match per term: ~8ns
// - Total fuzzy query: ~0.1ms (was ~10ms with naive Levenshtein)
//
// References:
// - Paper: https://dmice.ohsu.edu/bedricks/courses/cs655/pdf/readings/2002_Schulz.pdf
// - Blog: https://fulmicoton.com/posts/levenshtein/

use crate::levenshtein_dfa::{ParametricDFA, QueryMatcher};

/// Result from FST fuzzy search with edit distance.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub distance: u8,
    pub term_idx: usize,
}

/// WASM-accessible searcher with precomputed Levenshtein automata.
///
/// This is the fastest search implementation:
/// - O(1) exact lookup via inverted index
/// - O(log k) prefix search via suffix array
/// - O(vocabulary) fuzzy search via Levenshtein DFA (~8ns per term)
///
/// Load from binary .sorex format for best performance.
#[wasm_bindgen]
pub struct SorexSearcher {
    /// Document metadata
    docs: Vec<SearchDoc>,
    /// Section ID string table (for deep linking, v6+)
    section_table: Vec<String>,
    /// Sorted vocabulary (for term lookup and fuzzy search)
    vocabulary: Vec<String>,
    /// Vocabulary suffix array for prefix search
    suffix_array: Vec<(u32, u32)>,
    /// Postings: term_idx → posting entries (doc_id + section_idx)
    postings: Vec<Vec<PostingEntry>>,
    /// Precomputed Levenshtein DFA (loaded from .sorex file)
    lev_dfa: Option<ParametricDFA>,
}

#[wasm_bindgen]
impl SorexSearcher {
    /// Create a new searcher from binary .sorex format.
    ///
    /// The binary format is 5-7x smaller than JSON and loads ~3-5x faster.
    /// Since v5, document metadata is embedded in the binary (no separate load_docs call needed).
    /// Since v6, section_ids are stored per-posting for deep linking to specific sections.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<SorexSearcher, JsValue> {
        let layer =
            LoadedLayer::from_bytes(bytes).map_err(|e| format!("Failed to parse binary: {}", e))?;

        // Load precomputed Levenshtein DFA from embedded bytes
        let lev_dfa = if !layer.lev_dfa_bytes.is_empty() {
            ParametricDFA::from_bytes(&layer.lev_dfa_bytes).ok()
        } else {
            None
        };

        // Convert embedded DocMeta to SearchDoc
        let docs: Vec<SearchDoc> = layer
            .docs
            .iter()
            .enumerate()
            .map(|(id, doc)| SearchDoc {
                id,
                title: doc.title.clone(),
                excerpt: doc.excerpt.clone(),
                href: doc.href.clone(),
                kind: doc.doc_type.clone(),
                category: doc.category.clone(),
                author: doc.author.clone(),
                tags: doc.tags.clone(),
            })
            .collect();

        Ok(SorexSearcher {
            docs,
            section_table: layer.section_table,
            vocabulary: layer.vocabulary,
            suffix_array: layer.suffix_array,
            postings: layer.postings,
            lev_dfa,
        })
    }

    /// Load document metadata (for backward compatibility with v4 files).
    ///
    /// Not needed for v5+ files where docs are embedded in the binary.
    #[wasm_bindgen]
    pub fn load_docs(&mut self, docs: JsValue) -> Result<(), JsValue> {
        self.docs = from_value(docs).map_err(|e| e.to_string())?;
        // Clear section_table for backward compatibility (no section navigation)
        self.section_table = Vec::new();
        Ok(())
    }

    /// Check if document metadata is loaded (either embedded or via load_docs).
    #[wasm_bindgen]
    pub fn has_docs(&self) -> bool {
        !self.docs.is_empty()
    }

    /// Get the number of terms in the vocabulary.
    #[wasm_bindgen]
    pub fn vocab_size(&self) -> usize {
        self.vocabulary.len()
    }

    /// Get the number of documents.
    #[wasm_bindgen]
    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    /// Check if vocabulary is available for fuzzy search.
    #[wasm_bindgen]
    pub fn has_vocabulary(&self) -> bool {
        !self.vocabulary.is_empty()
    }

    /// Resolve section_idx to section_id string
    fn resolve_section_id(&self, section_idx: u32) -> Option<String> {
        if section_idx == 0 {
            None // 0 means no section_id (title match)
        } else {
            // 1-indexed into section_table
            self.section_table.get((section_idx - 1) as usize).cloned()
        }
    }

    /// Search with three-tier strategy: exact → prefix → fuzzy.
    ///
    /// Returns JSON array of SearchResult objects with section_ids for deep linking.
    ///
    /// Tier 1 (O(1)): Exact word match via inverted index
    /// Tier 2 (O(log k)): Prefix match via vocabulary suffix array
    /// Tier 3 (O(FST)): Fuzzy match via FST + Levenshtein DFA
    #[wasm_bindgen]
    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<JsValue, JsValue> {
        let limit = limit.unwrap_or(10);
        let query_lower = query.to_lowercase();

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Tier 1: Exact match (O(1) inverted index lookup)
        if let Some(term_idx) = self.vocabulary.iter().position(|t| t == &query_lower) {
            if let Some(postings) = self.postings.get(term_idx) {
                for entry in postings {
                    if let Some(doc) = self.docs.get(entry.doc_id as usize) {
                        let score = 100.0; // Exact match = highest score
                        let section_id = self.resolve_section_id(entry.section_idx);
                        results_by_doc
                            .entry(entry.doc_id as usize)
                            .or_insert(SearchResult {
                                doc: doc.clone(),
                                source: SearchSource::Title, // Placeholder
                                score,
                                section_id,
                            });
                    }
                }
            }
        }

        // Tier 2: Prefix match (O(log k) binary search on suffix array)
        let prefix_matches = self.prefix_search(&query_lower);
        for term_idx in prefix_matches {
            if let Some(postings) = self.postings.get(term_idx) {
                for entry in postings {
                    if let Some(doc) = self.docs.get(entry.doc_id as usize) {
                        let score = 50.0; // Prefix match
                        let section_id = self.resolve_section_id(entry.section_idx);
                        results_by_doc
                            .entry(entry.doc_id as usize)
                            .or_insert(SearchResult {
                                doc: doc.clone(),
                                source: SearchSource::Title,
                                score,
                                section_id,
                            });
                    }
                }
            }
        }

        // Tier 3: Fuzzy match (FST + Levenshtein DFA, zero Levenshtein computation)
        if results_by_doc.len() < limit {
            let fuzzy_matches = self.fuzzy_search(&query_lower, 2);
            for FuzzyMatch {
                term_idx, distance, ..
            } in fuzzy_matches
            {
                if let Some(postings) = self.postings.get(term_idx) {
                    for entry in postings {
                        if let Some(doc) = self.docs.get(entry.doc_id as usize) {
                            // Score inversely proportional to edit distance
                            let score = match distance {
                                0 => 100.0, // Exact (shouldn't happen in fuzzy tier)
                                1 => 30.0,  // One edit
                                2 => 15.0,  // Two edits
                                _ => 5.0,
                            };
                            let section_id = self.resolve_section_id(entry.section_idx);
                            results_by_doc
                                .entry(entry.doc_id as usize)
                                .or_insert(SearchResult {
                                    doc: doc.clone(),
                                    source: SearchSource::Title,
                                    score,
                                    section_id,
                                });
                        }
                    }
                }
            }
        }

        // Sort by score descending
        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);

        // Convert to SearchResultOutput with section_id for deep linking
        let output: Vec<SearchResultOutput> = results
            .into_iter()
            .map(|r| SearchResultOutput {
                href: r.doc.href,
                title: r.doc.title,
                excerpt: r.doc.excerpt,
                section_id: r.section_id,
            })
            .collect();

        to_value(&output).map_err(|e| e.to_string().into())
    }

    /// Prefix search using vocabulary suffix array (O(log k)).
    fn prefix_search(&self, prefix: &str) -> Vec<usize> {
        if self.suffix_array.is_empty() || prefix.is_empty() {
            return Vec::new();
        }

        let mut matches = std::collections::HashSet::new();

        // Binary search for first suffix starting with prefix
        let start = self.suffix_array.partition_point(|(term_idx, offset)| {
            let term = &self.vocabulary[*term_idx as usize];
            let suffix = &term[*offset as usize..];
            suffix < prefix
        });

        // Collect all matching suffixes
        for i in start..self.suffix_array.len() {
            let (term_idx, offset) = self.suffix_array[i];
            let term = &self.vocabulary[term_idx as usize];
            let suffix = &term[offset as usize..];

            if suffix.starts_with(prefix) {
                // Only count if prefix matches at word start (offset == 0)
                if offset == 0 {
                    matches.insert(term_idx as usize);
                }
            } else {
                break; // Past all prefix matches
            }
        }

        matches.into_iter().collect()
    }

    /// Fuzzy search using Levenshtein DFA (O(vocabulary), ~8ns per term).
    ///
    /// Uses precomputed Levenshtein automaton tables (Schulz-Mihov 2002).
    /// DFA loaded from embedded bytes, query matcher build is pure table lookups (~1μs).
    fn fuzzy_search(&self, query: &str, max_distance: u8) -> Vec<FuzzyMatch> {
        // Need precomputed DFA for fuzzy search
        let lev_dfa = match &self.lev_dfa {
            Some(dfa) => dfa,
            None => return Vec::new(),
        };

        if self.vocabulary.is_empty() {
            return Vec::new();
        }

        // Build query-specific matcher from precomputed DFA tables (~1μs)
        let matcher = QueryMatcher::new(lev_dfa, query);

        let mut matches = Vec::new();

        // Iterate vocabulary and check each term against the matcher
        // For ~150 terms, this is faster than FST setup overhead
        for (term_idx, term) in self.vocabulary.iter().enumerate() {
            // Check if term matches within max_distance (~8ns per term)
            if let Some(distance) = matcher.matches(term) {
                if distance <= max_distance {
                    matches.push(FuzzyMatch { distance, term_idx });
                }
            }
        }

        // Sort by distance ascending
        matches.sort_by_key(|m| m.distance);
        matches
    }

    // =========================================================================
    // STREAMING SEARCH API
    // For progressive UX: show exact matches immediately, then prefix, then fuzzy
    // =========================================================================

    /// Tier 1: Exact word match only (O(1) inverted index lookup).
    /// Returns results immediately for fast first-result display.
    #[wasm_bindgen]
    pub fn search_tier1_exact(&self, query: &str, limit: Option<usize>) -> Result<JsValue, JsValue> {
        let limit = limit.unwrap_or(10);
        let query_lower = query.to_lowercase();

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Tier 1: Exact match only
        if let Some(term_idx) = self.vocabulary.iter().position(|t| t == &query_lower) {
            if let Some(postings) = self.postings.get(term_idx) {
                for entry in postings {
                    if let Some(doc) = self.docs.get(entry.doc_id as usize) {
                        let score = 100.0;
                        let section_id = self.resolve_section_id(entry.section_idx);
                        results_by_doc
                            .entry(entry.doc_id as usize)
                            .or_insert(SearchResult {
                                doc: doc.clone(),
                                source: SearchSource::Title,
                                score,
                                section_id,
                            });
                    }
                }
            }
        }

        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        let output: Vec<SearchResultOutput> = results
            .into_iter()
            .map(|r| SearchResultOutput {
                href: r.doc.href,
                title: r.doc.title,
                excerpt: r.doc.excerpt,
                section_id: r.section_id,
            })
            .collect();

        to_value(&output).map_err(|e| e.to_string().into())
    }

    /// Tier 2: Prefix match only (O(log k) binary search).
    /// Pass doc IDs from tier1 as exclude_ids to avoid duplicates.
    #[wasm_bindgen]
    pub fn search_tier2_prefix(
        &self,
        query: &str,
        exclude_ids: JsValue,
        limit: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        let limit = limit.unwrap_or(10);
        let query_lower = query.to_lowercase();
        let exclude: std::collections::HashSet<usize> =
            from_value::<Vec<usize>>(exclude_ids).unwrap_or_default().into_iter().collect();

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Tier 2: Prefix match
        let prefix_matches = self.prefix_search(&query_lower);
        for term_idx in prefix_matches {
            if let Some(postings) = self.postings.get(term_idx) {
                for entry in postings {
                    let doc_id = entry.doc_id as usize;
                    if exclude.contains(&doc_id) {
                        continue;
                    }
                    if let Some(doc) = self.docs.get(doc_id) {
                        let score = 50.0;
                        let section_id = self.resolve_section_id(entry.section_idx);
                        results_by_doc
                            .entry(doc_id)
                            .or_insert(SearchResult {
                                doc: doc.clone(),
                                source: SearchSource::Title,
                                score,
                                section_id,
                            });
                    }
                }
            }
        }

        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        let output: Vec<SearchResultOutput> = results
            .into_iter()
            .map(|r| SearchResultOutput {
                href: r.doc.href,
                title: r.doc.title,
                excerpt: r.doc.excerpt,
                section_id: r.section_id,
            })
            .collect();

        to_value(&output).map_err(|e| e.to_string().into())
    }

    /// Tier 3: Fuzzy match only (O(vocabulary) via Levenshtein DFA).
    /// Pass doc IDs from tier1+tier2 as exclude_ids to avoid duplicates.
    #[wasm_bindgen]
    pub fn search_tier3_fuzzy(
        &self,
        query: &str,
        exclude_ids: JsValue,
        limit: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        let limit = limit.unwrap_or(10);
        let query_lower = query.to_lowercase();
        let exclude: std::collections::HashSet<usize> =
            from_value::<Vec<usize>>(exclude_ids).unwrap_or_default().into_iter().collect();

        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        // Tier 3: Fuzzy match
        let fuzzy_matches = self.fuzzy_search(&query_lower, 2);
        for FuzzyMatch { term_idx, distance, .. } in fuzzy_matches {
            if let Some(postings) = self.postings.get(term_idx) {
                for entry in postings {
                    let doc_id = entry.doc_id as usize;
                    if exclude.contains(&doc_id) {
                        continue;
                    }
                    if let Some(doc) = self.docs.get(doc_id) {
                        let score = match distance {
                            0 => 100.0,
                            1 => 30.0,
                            2 => 15.0,
                            _ => 5.0,
                        };
                        let section_id = self.resolve_section_id(entry.section_idx);
                        results_by_doc
                            .entry(doc_id)
                            .or_insert(SearchResult {
                                doc: doc.clone(),
                                source: SearchSource::Title,
                                score,
                                section_id,
                            });
                    }
                }
            }
        }

        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(limit);

        let output: Vec<SearchResultOutput> = results
            .into_iter()
            .map(|r| SearchResultOutput {
                href: r.doc.href,
                title: r.doc.title,
                excerpt: r.doc.excerpt,
                section_id: r.section_id,
            })
            .collect();

        to_value(&output).map_err(|e| e.to_string().into())
    }
}
