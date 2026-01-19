// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! WebAssembly bindings for Sorex search.
//!
//! This is the browser-facing API. Two modes: progressive callbacks for responsive
//! UI, or synchronous for when you just need answers. The progressive API calls
//! your `onUpdate` callback after each search tier completes, so users see exact
//! matches instantly while fuzzy search is still running.
//!
//! # Callback API
//!
//! ```js
//! searcher.search(query, 10, {
//!     onUpdate: (results) => setResults(results),
//!     onFinish: (results) => setLoading(false)
//! });
//! ```
//!
//! # Threading Support
//!
//! With the `wasm-threads` feature, T3 fuzzy search runs in parallel via Web Workers.
//! Call `initThreadPool(navigator.hardwareConcurrency)` after loading the module.

#[cfg(feature = "rayon")]
use crate::binary::IncrementalLoader;
use crate::binary::LoadedLayer;
use crate::scoring::ranking::compare_results;
use crate::search::dedup::ResultMerger;
#[cfg(feature = "rayon")]
use crate::search::tiered::UIMessage;
use crate::search::tiered::{SearchOptions, SearchResult, TierSearcher};
use crate::types::SearchDoc;
use js_sys::Function;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

// Re-export thread pool initialization when wasm+rayon is enabled.
// Usage: await initThreadPool(navigator.hardwareConcurrency);
#[cfg(all(feature = "wasm", feature = "rayon"))]
#[allow(unused_imports)]
pub use wasm_bindgen_rayon::init_thread_pool;

/// Search result for JavaScript consumption.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsSearchResult {
    href: String,
    title: String,
    excerpt: String,
    section_id: Option<String>,
    tier: u8,
    match_type: u8,
    score: f64,
    matched_term: Option<String>,
}

impl JsSearchResult {
    fn from_result(
        r: &SearchResult,
        doc: &SearchDoc,
        section_table: &[String],
        vocabulary: &[String],
    ) -> Self {
        // Resolve section_idx to section_id string (lazy resolution at WASM boundary)
        let section_id = if r.section_idx == 0 {
            None
        } else {
            section_table.get((r.section_idx - 1) as usize).cloned()
        };

        // Resolve matched_term index to actual term string
        let matched_term = r
            .matched_term
            .and_then(|idx| vocabulary.get(idx as usize).cloned());

        Self {
            href: doc.href.clone(),
            title: doc.title.clone(),
            excerpt: doc.excerpt.clone(),
            section_id,
            tier: r.tier,
            match_type: r.match_type.to_u8(),
            score: r.score,
            matched_term,
        }
    }
}

/// Search result with per-tier timing breakdown.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TierTimingResult {
    results: Vec<JsSearchResult>,
    t1_count: usize,
    t2_count: usize,
    t3_count: usize,
    t1_time_us: f64,
    t2_time_us: f64,
    t3_time_us: f64,
}

/// Search options for JavaScript consumption.
///
/// Passed to search methods to configure behavior.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct JsSearchOptions {
    /// Whether to deduplicate sections within a document (default: true).
    /// When false, returns multiple results per document if different sections match.
    #[serde(default = "default_dedup_sections")]
    dedup_sections: bool,
}

fn default_dedup_sections() -> bool {
    true
}

impl From<JsSearchOptions> for SearchOptions {
    fn from(js: JsSearchOptions) -> Self {
        SearchOptions {
            dedup_sections: js.dedup_sections,
        }
    }
}

/// WASM searcher - thin wrapper around TierSearcher.
#[wasm_bindgen]
pub struct SorexSearcher {
    searcher: Rc<TierSearcher>,
}

#[wasm_bindgen]
impl SorexSearcher {
    /// Create searcher from .sorex binary format.
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<SorexSearcher, JsValue> {
        let layer = LoadedLayer::from_bytes(bytes)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse: {}", e)))?;
        let searcher = TierSearcher::from_layer(layer).map_err(|e| JsValue::from_str(&e))?;
        Ok(SorexSearcher {
            searcher: Rc::new(searcher),
        })
    }

    /// Number of documents.
    #[wasm_bindgen]
    pub fn doc_count(&self) -> usize {
        self.searcher.docs().len()
    }

    /// Number of vocabulary terms.
    #[wasm_bindgen]
    pub fn vocab_size(&self) -> usize {
        self.searcher.vocabulary().len()
    }

    /// Progressive search with callbacks after each tier.
    ///
    /// - `on_update`: Called after each tier (1-3 times) with current results
    /// - `on_finish`: Called once when search is complete with final results
    ///
    /// Each callback receives the full deduplicated result set (not deltas).
    ///
    /// ```js
    /// searcher.search(query, 10, onUpdate, onFinish);
    /// ```
    ///
    /// Works without threading - tiers run sequentially.
    #[wasm_bindgen]
    pub fn search(
        &self,
        query: &str,
        limit: usize,
        on_update: &Function,
        on_finish: &Function,
    ) -> Result<(), JsValue> {
        if query.is_empty() {
            // Empty query: call finish with empty results
            let empty: Vec<JsSearchResult> = vec![];
            let js_empty = to_value(&empty).map_err(|e| JsValue::from_str(&e.to_string()))?;
            on_finish.call1(&JsValue::NULL, &js_empty)?;
            return Ok(());
        }

        let query_lower = query.to_lowercase();
        let docs = self.searcher.docs();

        // Use ResultMerger for type-safe doc_id-only deduplication
        let mut merger = ResultMerger::new(docs);

        // Fetch extra candidates to ensure enough after deduplication
        // Use at least 100 or 3x the requested limit
        let fetch_limit = limit.clamp(100, 1000);

        // Tier 1: Exact matches
        let t1_results = self.searcher.search_tier1_exact(&query_lower, fetch_limit);
        merger.merge_all(t1_results);
        self.invoke_callback_from_merger(on_update, &merger, limit)?;

        // Tier 2: Prefix matches
        let t2_results = self
            .searcher
            .search_tier2_prefix_no_exclude(&query_lower, fetch_limit);
        merger.merge_all(t2_results);
        self.invoke_callback_from_merger(on_update, &merger, limit)?;

        // Tier 3: Fuzzy matches (threaded with wasm-threads feature)
        let t3_results = self
            .searcher
            .search_tier3_fuzzy_no_exclude(&query_lower, fetch_limit);
        merger.merge_all(t3_results);
        self.invoke_callback_from_merger(on_update, &merger, limit)?;

        // Final callback
        self.invoke_callback_from_merger(on_finish, &merger, limit)?;

        Ok(())
    }

    /// Three-tier search: exact → prefix → fuzzy (blocking).
    /// For progressive results, use `search()` instead.
    #[wasm_bindgen(js_name = "searchSync")]
    pub fn search_sync(&self, query: &str, limit: Option<usize>) -> Result<JsValue, JsValue> {
        self.search_sync_with_options(query, limit, JsValue::UNDEFINED)
    }

    /// Three-tier search with options (blocking).
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `limit` - Maximum results (default: 10)
    /// * `options` - Search options object: `{ dedupSections: boolean }`
    ///   - `dedupSections`: Whether to deduplicate sections within a document (default: true)
    ///
    /// ```js
    /// // Default behavior (section dedup enabled)
    /// searcher.searchSyncWithOptions("kernel", 10);
    ///
    /// // Return all matching sections per document
    /// searcher.searchSyncWithOptions("kernel", 10, { dedupSections: false });
    /// ```
    #[wasm_bindgen(js_name = "searchSyncWithOptions")]
    pub fn search_sync_with_options(
        &self,
        query: &str,
        limit: Option<usize>,
        options: JsValue,
    ) -> Result<JsValue, JsValue> {
        let limit = limit.unwrap_or(10).min(10000);
        if query.is_empty() {
            return to_value(&Vec::<JsSearchResult>::new()).map_err(|e| e.to_string().into());
        }

        // Parse options, using defaults if undefined/null
        let opts: JsSearchOptions = if options.is_undefined() || options.is_null() {
            JsSearchOptions::default()
        } else {
            serde_wasm_bindgen::from_value(options)
                .map_err(|e| JsValue::from_str(&format!("Invalid options: {}", e)))?
        };

        let results = self.searcher.search_with_options(query, limit, opts.into());
        let output = self.to_js_results(results);
        to_value(&output).map_err(|e| e.to_string().into())
    }

    /// Three-tier search with per-tier timing breakdown.
    ///
    /// Returns an object with:
    /// - `results`: Array of search results
    /// - `t1Count`: Number of T1 exact matches
    /// - `t2Count`: Number of T2 prefix matches
    /// - `t3Count`: Number of T3 fuzzy matches
    /// - `t1TimeUs`: T1 search time in microseconds
    /// - `t2TimeUs`: T2 search time in microseconds
    /// - `t3TimeUs`: T3 search time in microseconds
    #[wasm_bindgen(js_name = "searchWithTierTiming")]
    pub fn search_with_tier_timing(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<JsValue, JsValue> {
        use js_sys::Date;
        use std::collections::HashSet;

        let limit = limit.unwrap_or(10).min(10000);
        if query.is_empty() {
            let empty = TierTimingResult {
                results: vec![],
                t1_count: 0,
                t2_count: 0,
                t3_count: 0,
                t1_time_us: 0.0,
                t2_time_us: 0.0,
                t3_time_us: 0.0,
            };
            return to_value(&empty).map_err(|e| e.to_string().into());
        }

        // T1: Exact match
        let t1_start = Date::now();
        let t1_results = self.searcher.search_tier1_exact(query, limit);
        let t1_time = Date::now() - t1_start;
        let t1_count = t1_results.len();
        let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();

        // T2: Prefix match (exclude T1)
        let t2_start = Date::now();
        let t2_results = self.searcher.search_tier2_prefix(query, &t1_ids, limit);
        let t2_time = Date::now() - t2_start;
        let t2_count = t2_results.len();
        let t2_ids: HashSet<usize> = t2_results.iter().map(|r| r.doc_id).collect();

        // T3: Fuzzy match (exclude T1 and T2)
        let mut exclude_ids = t1_ids;
        exclude_ids.extend(t2_ids);
        let t3_start = Date::now();
        let t3_results = self.searcher.search_tier3_fuzzy(query, &exclude_ids, limit);
        let t3_time = Date::now() - t3_start;
        let t3_count = t3_results.len();

        // Merge and sort results
        let mut all_results: Vec<_> = t1_results
            .into_iter()
            .chain(t2_results)
            .chain(t3_results)
            .collect();
        all_results.sort_by(|a, b| compare_results(a, b, self.searcher.docs()));
        all_results.truncate(limit);

        let output = TierTimingResult {
            results: self.to_js_results(all_results),
            t1_count,
            t2_count,
            t3_count,
            t1_time_us: t1_time * 1000.0, // ms to µs
            t2_time_us: t2_time * 1000.0,
            t3_time_us: t3_time * 1000.0,
        };

        to_value(&output).map_err(|e| e.to_string().into())
    }

    /// Streaming parallel search with dedicated dedup worker.
    ///
    /// Architecture:
    /// - T1/T2/T3 workers run in parallel on Web Workers
    /// - Dedup worker maintains heap on separate thread
    /// - Main thread ONLY receives ready results and calls JS callback
    ///
    /// Results are emitted in ranked order (T1 first, then T2, then T3).
    ///
    /// ```js
    /// searcher.searchStreaming("kernel", 10, {
    ///   onResult: (result) => {
    ///     results.push(result);
    ///     renderResults(results);  // Immediate UI update
    ///   },
    ///   onFinish: (finalResults) => {
    ///     setResults(finalResults);  // Final sorted order
    ///   }
    /// });
    /// ```
    ///
    /// Requires `wasm-threads` feature (Web Workers + SharedArrayBuffer).
    #[wasm_bindgen(js_name = "searchStreaming")]
    #[cfg(all(feature = "wasm", feature = "rayon"))]
    pub fn search_streaming(
        &self,
        query: &str,
        limit: usize,
        on_result: &Function,
        on_finish: &Function,
    ) -> Result<(), JsValue> {
        if query.is_empty() {
            let empty: Vec<JsSearchResult> = vec![];
            let js_empty = to_value(&empty).map_err(|e| JsValue::from_str(&e.to_string()))?;
            on_finish.call1(&JsValue::NULL, &js_empty)?;
            return Ok(());
        }

        // Get UI channel (dedup already handled by worker)
        let rx = self.searcher.search_streaming(query, limit);

        // Main thread: just receive and call JS callbacks
        for msg in rx {
            match msg {
                UIMessage::Result(result) => {
                    if let Some(doc) = self.searcher.docs().get(result.doc_id) {
                        let js_result = JsSearchResult::from_result(
                            &result,
                            doc,
                            self.searcher.section_table(),
                            self.searcher.vocabulary(),
                        );
                        let js_value =
                            to_value(&js_result).map_err(|e| JsValue::from_str(&e.to_string()))?;
                        on_result.call1(&JsValue::NULL, &js_value)?;
                    }
                }
                UIMessage::Finished(results) => {
                    let js_results: Vec<JsSearchResult> = results
                        .iter()
                        .filter_map(|r| {
                            self.searcher.docs().get(r.doc_id).map(|doc| {
                                JsSearchResult::from_result(
                                    r,
                                    doc,
                                    self.searcher.section_table(),
                                    self.searcher.vocabulary(),
                                )
                            })
                        })
                        .collect();
                    let js_array =
                        to_value(&js_results).map_err(|e| JsValue::from_str(&e.to_string()))?;
                    on_finish.call1(&JsValue::NULL, &js_array)?;
                    break;
                }
            }
        }

        Ok(())
    }
}

impl SorexSearcher {
    /// Invoke a JS callback with sorted, limited results from the merger.
    ///
    /// Uses `ResultMerger::get_sorted()` to get a snapshot of current results
    /// without consuming the merger (allowing multiple callbacks).
    fn invoke_callback_from_merger(
        &self,
        callback: &Function,
        merger: &ResultMerger<'_>,
        limit: usize,
    ) -> Result<(), JsValue> {
        let sorted = merger.get_sorted(limit);
        let js_results = self.to_js_results(sorted);
        let js_array = to_value(&js_results).map_err(|e| JsValue::from_str(&e.to_string()))?;
        callback.call1(&JsValue::NULL, &js_array)?;
        Ok(())
    }

    /// Convert internal results to JS-serializable format.
    fn to_js_results(&self, results: Vec<SearchResult>) -> Vec<JsSearchResult> {
        results
            .iter()
            .filter_map(|r| {
                self.searcher.docs().get(r.doc_id).map(|doc| {
                    JsSearchResult::from_result(
                        r,
                        doc,
                        self.searcher.section_table(),
                        self.searcher.vocabulary(),
                    )
                })
            })
            .collect()
    }
}

// ============================================================================
// INCREMENTAL LOADER (streaming section decode)
// ============================================================================

/// Section byte offsets for JavaScript to track download progress.
#[cfg(feature = "rayon")]
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JsSectionOffsets {
    /// WASM section (start, end)
    pub wasm_start: usize,
    pub wasm_end: usize,
    /// Vocabulary section (start, end)
    pub vocabulary_start: usize,
    pub vocabulary_end: usize,
    /// Dictionary tables section (start, end)
    pub dict_tables_start: usize,
    pub dict_tables_end: usize,
    /// Postings section (start, end)
    pub postings_start: usize,
    pub postings_end: usize,
    /// Suffix array section (start, end)
    pub suffix_array_start: usize,
    pub suffix_array_end: usize,
    /// Docs section (start, end)
    pub docs_start: usize,
    pub docs_end: usize,
    /// Section table section (start, end)
    pub section_table_start: usize,
    pub section_table_end: usize,
    /// Skip lists section (start, end)
    pub skip_lists_start: usize,
    pub skip_lists_end: usize,
    /// Levenshtein DFA section (start, end)
    pub lev_dfa_start: usize,
    pub lev_dfa_end: usize,
    /// Total content size (before footer)
    pub content_size: usize,
    /// Header info
    pub term_count: u32,
    pub doc_count: u32,
    /// Flags for skip list decoding
    pub has_skip_lists: bool,
}

/// Incremental loader for streaming section decode.
///
/// Each section is decoded in a background thread as bytes arrive.
/// Call `finalize()` to build the final `SorexSearcher`.
///
/// # Example
///
/// ```js
/// const loader = new SorexIncrementalLoader();
/// const offsets = loader.loadHeader(headerBytes);
///
/// // As bytes arrive, dispatch for background decode:
/// loader.loadVocabulary(vocabBytes);
/// loader.loadPostings(postingsBytes);
/// // ... etc
///
/// // Wait for all sections and build searcher:
/// const searcher = loader.finalize();
/// ```
#[cfg(feature = "rayon")]
#[wasm_bindgen]
pub struct SorexIncrementalLoader {
    loader: Option<IncrementalLoader>,
    term_count: u32,
    has_skip_lists: bool,
}

#[cfg(feature = "rayon")]
#[wasm_bindgen]
impl SorexIncrementalLoader {
    /// Create a new incremental loader.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            loader: Some(IncrementalLoader::new()),
            term_count: 0,
            has_skip_lists: false,
        }
    }

    /// Parse header bytes. Returns section offsets as JSON for JavaScript to track.
    ///
    /// This must be called first before loading any sections.
    #[wasm_bindgen(js_name = "loadHeader")]
    pub fn load_header(&mut self, bytes: &[u8]) -> Result<JsValue, JsValue> {
        let loader = self
            .loader
            .as_mut()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;

        let offsets = loader
            .load_header(bytes)
            .map_err(|e| JsValue::from_str(&format!("Failed to parse header: {}", e)))?;

        // Store header info for later use
        if let Some(header) = loader.header() {
            self.term_count = header.term_count;
            self.has_skip_lists = header.flags.has_skip_lists();
        }

        let js_offsets = JsSectionOffsets {
            wasm_start: offsets.wasm.0,
            wasm_end: offsets.wasm.1,
            vocabulary_start: offsets.vocabulary.0,
            vocabulary_end: offsets.vocabulary.1,
            dict_tables_start: offsets.dict_tables.0,
            dict_tables_end: offsets.dict_tables.1,
            postings_start: offsets.postings.0,
            postings_end: offsets.postings.1,
            suffix_array_start: offsets.suffix_array.0,
            suffix_array_end: offsets.suffix_array.1,
            docs_start: offsets.docs.0,
            docs_end: offsets.docs.1,
            section_table_start: offsets.section_table.0,
            section_table_end: offsets.section_table.1,
            skip_lists_start: offsets.skip_lists.0,
            skip_lists_end: offsets.skip_lists.1,
            lev_dfa_start: offsets.lev_dfa.0,
            lev_dfa_end: offsets.lev_dfa.1,
            content_size: offsets.content_size(),
            term_count: self.term_count,
            doc_count: loader.header().map(|h| h.doc_count).unwrap_or(0),
            has_skip_lists: self.has_skip_lists,
        };

        to_value(&js_offsets).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Decode vocabulary in background thread. Non-blocking.
    #[wasm_bindgen(js_name = "loadVocabulary")]
    pub fn load_vocabulary(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_vocabulary(bytes.to_vec());
        Ok(())
    }

    /// Decode dict tables in background thread. Non-blocking.
    #[wasm_bindgen(js_name = "loadDictTables")]
    pub fn load_dict_tables(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_dict_tables(bytes.to_vec());
        Ok(())
    }

    /// Decode postings in background thread. Non-blocking.
    ///
    /// This is typically the largest section (~30-50% of file size).
    #[wasm_bindgen(js_name = "loadPostings")]
    pub fn load_postings(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_postings(bytes.to_vec(), self.term_count);
        Ok(())
    }

    /// Decode suffix array in background thread. Non-blocking.
    #[wasm_bindgen(js_name = "loadSuffixArray")]
    pub fn load_suffix_array(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_suffix_array(bytes.to_vec());
        Ok(())
    }

    /// Decode docs in background thread. Non-blocking.
    #[wasm_bindgen(js_name = "loadDocs")]
    pub fn load_docs(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_docs(bytes.to_vec());
        Ok(())
    }

    /// Decode section table in background thread. Non-blocking.
    #[wasm_bindgen(js_name = "loadSectionTable")]
    pub fn load_section_table(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_section_table(bytes.to_vec());
        Ok(())
    }

    /// Decode skip lists in background thread. Non-blocking.
    #[wasm_bindgen(js_name = "loadSkipLists")]
    pub fn load_skip_lists(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;

        use crate::binary::FormatFlags;
        let flags = if self.has_skip_lists {
            FormatFlags::new().with_skip_lists()
        } else {
            FormatFlags::new()
        };
        loader.load_skip_lists(bytes.to_vec(), flags);
        Ok(())
    }

    /// Store Levenshtein DFA bytes. Non-blocking.
    #[wasm_bindgen(js_name = "loadLevDfa")]
    pub fn load_lev_dfa(&self, bytes: &[u8]) -> Result<(), JsValue> {
        let loader = self
            .loader
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;
        loader.load_lev_dfa(bytes.to_vec());
        Ok(())
    }

    /// Check if all sections are loaded (non-blocking).
    #[wasm_bindgen(js_name = "isComplete")]
    pub fn is_complete(&self) -> bool {
        self.loader
            .as_ref()
            .map(|l| l.is_complete())
            .unwrap_or(true)
    }

    /// Get number of sections still pending.
    #[wasm_bindgen(js_name = "pendingCount")]
    pub fn pending_count(&self) -> u8 {
        self.loader.as_ref().map(|l| l.pending_count()).unwrap_or(0)
    }

    /// Wait for all sections and build the final SorexSearcher.
    ///
    /// This blocks until all background decode tasks complete.
    #[wasm_bindgen]
    pub fn finalize(mut self) -> Result<SorexSearcher, JsValue> {
        let loader = self
            .loader
            .take()
            .ok_or_else(|| JsValue::from_str("Loader already finalized"))?;

        let layer = loader
            .finalize()
            .map_err(|e| JsValue::from_str(&format!("Failed to finalize: {}", e)))?;

        let searcher = TierSearcher::from_layer(layer).map_err(|e| JsValue::from_str(&e))?;

        Ok(SorexSearcher {
            searcher: Rc::new(searcher),
        })
    }
}

#[cfg(feature = "rayon")]
impl Default for SorexIncrementalLoader {
    fn default() -> Self {
        Self::new()
    }
}
