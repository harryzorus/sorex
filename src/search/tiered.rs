// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! The three-tier search core: exact → prefix → fuzzy.
//!
//! This is where all the index structures pay off. Tier 1 (exact match) uses
//! the inverted index for O(1) lookup. If you search "rust" and a document
//! contains "rust", you get it immediately. Tier 2 (prefix) uses the suffix
//! array for O(log k) binary search. "rust" matches "rustic", "rusted". Tier 3
//! (fuzzy) runs the Levenshtein DFA over the vocabulary. "ryst" finds "rust".
//!
//! The tiers run in order: exact results come back in <1ms, prefix in ~5ms,
//! fuzzy in ~50ms. For search-as-you-type UX, this progressive disclosure
//! matters more than total latency.
//!
//! Platform-specific bindings delegate to these functions:
//! - `wasm.rs` - WebAssembly bindings with JS callbacks
//! - Native Rust code can use `TierSearcher` directly
//!
//! ## Streaming Parallel Search
//!
//! With the `rayon` feature, `search_streaming()` runs T1/T2/T3 in parallel:
//! - Each tier sends results to a shared channel as they're found
//! - A dedup worker maintains an ordered heap and forwards unique results
//! - Results are emitted in ranked order (T1 > T2 > T3 by score)
//! - Caller receives `Receiver<UIMessage>` for platform-specific handling

use crate::binary::{LoadedLayer, PostingEntry};
use crate::fuzzy::dfa::{ParametricDFA, QueryMatcher};
use crate::scoring::ranking::compare_results;
use crate::scoring::{
    T1_EXACT_SCORE, T2_PREFIX_SCORE, T2_TITLE_BOOST,
    T3_FUZZY_DISTANCE_1_SCORE, T3_FUZZY_DISTANCE_2_SCORE, T3_FUZZY_DISTANCE_3_SCORE,
    T3_EDIT_DISTANCE_PENALTY, T3_LENGTH_BONUS_COEFFICIENT, T3_TITLE_BOOST,
};
use crate::util::simd::{to_lowercase_ascii_simd, starts_with_simd};
use crate::types::{SearchDoc, MatchType};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

#[cfg(feature = "rayon")]
use std::sync::mpsc::{channel, Sender, Receiver};
#[cfg(feature = "rayon")]
use std::collections::BTreeMap;
#[cfg(feature = "rayon")]
use std::cmp::Reverse;
#[cfg(feature = "rayon")]
use rayon::prelude::*;

/// Wrapper for f64 that implements Ord for use in BTreeMap keys.
#[cfg(feature = "rayon")]
#[derive(Debug, Clone, Copy)]
struct OrderedFloat(f64);

#[cfg(feature = "rayon")]
impl PartialEq for OrderedFloat {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

#[cfg(feature = "rayon")]
impl Eq for OrderedFloat {}

#[cfg(feature = "rayon")]
impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "rayon")]
impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(std::cmp::Ordering::Equal)
    }
}

/// Raw result from tier workers (may have duplicates).
#[cfg(feature = "rayon")]
#[derive(Debug, Clone)]
pub struct RawResult {
    pub result: SearchResult,
    pub tier_done: Option<u8>,  // Some(tier_num) if this tier finished
}

/// Deduped result ready for UI.
#[cfg(feature = "rayon")]
#[derive(Debug, Clone)]
pub enum UIMessage {
    /// New or updated result (already deduped)
    Result(SearchResult),
    /// Final sorted results (when all tiers complete)
    Finished(Vec<SearchResult>),
}

/// Search result from the three-tier search algorithm.
///
/// Uses section_idx (raw index) instead of section_id (String) to avoid
/// heap allocation in the hot path. Resolve to String at WASM boundary.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub doc_id: usize,
    pub score: f64,
    pub section_idx: u32,  // 0 = no section, >0 = section_table[idx-1]
    pub tier: u8,           // 1=exact, 2=prefix, 3=fuzzy
    pub match_type: MatchType, // Primary sort key: Title > Section > ... > Content
}

/// Match found by fuzzy search with edit distance.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub distance: u8,
    pub term_idx: usize,
}

/// Accumulator for multi-term search score summing.
///
/// Tracks cumulative scores per (doc_id, section_idx) pair, best match_type,
/// and which query terms hit each document for AND semantics filtering.
struct MultiTermAccumulator {
    /// Cumulative score per (doc_id, section_idx) pair
    doc_scores: HashMap<(usize, u32), f64>,
    /// Best match_type per (doc_id, section_idx) pair
    doc_match_types: HashMap<(usize, u32), MatchType>,
    /// Which query term indices hit each doc_id (for AND semantics)
    doc_term_hits: HashMap<usize, HashSet<usize>>,
    /// Total number of query terms (for AND filtering)
    num_terms: usize,
}

impl MultiTermAccumulator {
    /// Create a new accumulator for a query with the given number of terms.
    fn new(num_terms: usize) -> Self {
        Self {
            doc_scores: HashMap::new(),
            doc_match_types: HashMap::new(),
            doc_term_hits: HashMap::new(),
            num_terms,
        }
    }

    /// Add a posting entry match for a specific query term.
    ///
    /// # Arguments
    /// * `term_idx` - Index of the query term (0-based)
    /// * `doc_id` - Document ID from the posting
    /// * `section_idx` - Section index from the posting
    /// * `match_type` - Match type (Title, Heading, Content)
    /// * `score` - Score to add for this match
    #[inline]
    fn add_match(
        &mut self,
        term_idx: usize,
        doc_id: usize,
        section_idx: u32,
        match_type: MatchType,
        score: f64,
    ) {
        let key = (doc_id, section_idx);

        // Sum scores across terms
        *self.doc_scores.entry(key).or_insert(0.0) += score;

        // Track best match_type (lowest enum value = higher priority)
        self.doc_match_types
            .entry(key)
            .and_modify(|mt| {
                if match_type < *mt {
                    *mt = match_type;
                }
            })
            .or_insert(match_type);

        // Track which terms hit this doc
        self.doc_term_hits.entry(doc_id).or_default().insert(term_idx);
    }

    /// Build search results from accumulated scores.
    ///
    /// Filters to documents matching ALL query terms (AND semantics),
    /// then deduplicates by doc_id (keeping best match_type/score per doc),
    /// sorts by score descending and truncates to limit.
    fn into_results(self, tier: u8, limit: usize, docs: &[SearchDoc]) -> Vec<SearchResult> {
        // First pass: collect all (doc_id, section_idx) matches
        let section_results: Vec<SearchResult> = self
            .doc_scores
            .into_iter()
            .filter(|((doc_id, _), _)| {
                self.doc_term_hits
                    .get(doc_id)
                    .is_some_and(|hits| hits.len() == self.num_terms)
            })
            .map(|((doc_id, section_idx), score)| SearchResult {
                doc_id,
                score,
                section_idx,
                tier,
                match_type: self
                    .doc_match_types
                    .get(&(doc_id, section_idx))
                    .copied()
                    .unwrap_or(MatchType::Content),
            })
            .collect();

        // Second pass: deduplicate by doc_id, keeping best (match_type, score)
        let mut best_per_doc: HashMap<usize, SearchResult> = HashMap::new();
        for result in section_results {
            best_per_doc
                .entry(result.doc_id)
                .and_modify(|existing| {
                    // Keep best match_type (lowest enum value = higher priority)
                    if result.match_type < existing.match_type {
                        *existing = result.clone();
                    } else if result.match_type == existing.match_type && result.score > existing.score {
                        // Same match_type: keep higher score
                        existing.score = result.score;
                        existing.section_idx = result.section_idx;
                    }
                })
                .or_insert(result);
        }

        let mut results: Vec<SearchResult> = best_per_doc.into_values().collect();
        results.sort_by(|a, b| compare_results(a, b, docs));
        results.truncate(limit);
        results
    }
}

/// Compute fuzzy match score with edit distance penalty.
///
/// # Arguments
/// * `distance` - Edit distance (1, 2, or more)
/// * `query_len` - Length of the query term
/// * `matched_len` - Length of the matched vocabulary term
/// * `is_title` - Whether the match is in the document title
///
/// # Returns
/// Final score with all penalties and bonuses applied.
///
/// # Score Constants (defined in scoring/core.rs)
/// - Distance 1: T3_FUZZY_DISTANCE_1_SCORE (30.0)
/// - Distance 2: T3_FUZZY_DISTANCE_2_SCORE (15.0)
/// - Distance 3+: T3_FUZZY_DISTANCE_3_SCORE (5.0)
/// - Edit penalty: T3_EDIT_DISTANCE_PENALTY (20% per edit)
/// - Length bonus: T3_LENGTH_BONUS_COEFFICIENT (30% of score)
/// - Title boost: T3_TITLE_BOOST (50% boost)
#[inline]
fn compute_fuzzy_score(distance: u8, query_len: usize, matched_len: usize, is_title: bool) -> f64 {
    // Base score by distance (constants from scoring/core.rs)
    let base_score = match distance {
        1 => T3_FUZZY_DISTANCE_1_SCORE,
        2 => T3_FUZZY_DISTANCE_2_SCORE,
        _ => T3_FUZZY_DISTANCE_3_SCORE,
    };

    // Apply edit distance penalty (20% per edit)
    let penalty = 1.0 - (distance as f64 * T3_EDIT_DISTANCE_PENALTY);
    let penalized_score = base_score * penalty;

    // Length similarity bonus: prefer terms with similar length to query term
    let length_diff = (query_len as i32 - matched_len as i32).abs();
    let length_bonus = 1.0 / (1.0 + length_diff as f64);
    let score_with_length = penalized_score * (1.0 + length_bonus * T3_LENGTH_BONUS_COEFFICIENT);

    // Boost score if match is in document title
    if is_title {
        score_with_length * T3_TITLE_BOOST
    } else {
        score_with_length
    }
}

/// Thread-safe inner data for the three-tier searcher.
///
/// Wrapped in Arc to allow sharing across threads during parallel search.
#[derive(Debug)]
pub struct TierSearcherInner {
    pub docs: Vec<SearchDoc>,
    pub section_table: Vec<String>,
    pub vocabulary: Vec<String>,
    pub suffix_array: Vec<(u32, u32)>,
    pub postings: Vec<Vec<PostingEntry>>,
    pub inverted_index: HashMap<String, Vec<PostingEntry>>,
    pub lev_dfa: Option<ParametricDFA>,
}

/// Pure Rust three-tier searcher (exact → prefix → fuzzy).
///
/// This is the core search logic extracted from WASM. It's fully testable
/// in pure Rust without WASM overhead. The WASM layer delegates all search
/// logic to this struct.
///
/// Thread-safe via Arc: can be cloned and shared across threads.
#[derive(Debug, Clone)]
pub struct TierSearcher {
    inner: Arc<TierSearcherInner>,
}

impl TierSearcher {
    /// Access docs slice.
    #[inline]
    pub fn docs(&self) -> &[SearchDoc] {
        &self.inner.docs
    }

    /// Access section table slice.
    #[inline]
    pub fn section_table(&self) -> &[String] {
        &self.inner.section_table
    }

    /// Access vocabulary slice.
    #[inline]
    pub fn vocabulary(&self) -> &[String] {
        &self.inner.vocabulary
    }

    /// Access suffix array slice.
    #[inline]
    pub fn suffix_array(&self) -> &[(u32, u32)] {
        &self.inner.suffix_array
    }

    /// Access postings lists.
    #[inline]
    pub fn postings(&self) -> &[Vec<PostingEntry>] {
        &self.inner.postings
    }

    /// Access inverted index.
    #[inline]
    pub fn inverted_index(&self) -> &HashMap<String, Vec<PostingEntry>> {
        &self.inner.inverted_index
    }

    /// Access Levenshtein DFA.
    #[inline]
    pub fn lev_dfa(&self) -> Option<&ParametricDFA> {
        self.inner.lev_dfa.as_ref()
    }
}

impl TierSearcher {
    /// Build searcher from loaded binary layer.
    ///
    /// The LoadedLayer contains all the binary data that needs to be deserialized:
    /// - Document metadata
    /// - Vocabulary terms
    /// - Suffix array for prefix searching
    /// - Postings lists for exact and fuzzy matching
    /// - Precomputed Levenshtein DFA for fuzzy search
    /// - Section IDs for deep linking
    pub fn from_layer(layer: LoadedLayer) -> Result<Self, String> {
        // Load DFA if present
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

        // Build fast inverted index: term → postings for O(1) exact lookup
        let mut inverted_index: HashMap<String, Vec<PostingEntry>> = HashMap::new();
        for (term_idx, postings_vec) in layer.postings.iter().enumerate() {
            if let Some(term) = layer.vocabulary.get(term_idx) {
                inverted_index.insert(term.clone(), postings_vec.clone());
            }
        }

        let inner = TierSearcherInner {
            docs,
            section_table: layer.section_table,
            vocabulary: layer.vocabulary,
            suffix_array: layer.suffix_array,
            postings: layer.postings,
            inverted_index,
            lev_dfa,
        };

        let searcher = TierSearcher {
            inner: Arc::new(inner),
        };

        // Validate once at construction time, not per-search
        if let Some(error) = searcher.validate() {
            return Err(error);
        }

        Ok(searcher)
    }

    /// Validate that the searcher state is consistent.
    /// Returns error string if validation fails, None if valid.
    pub fn validate(&self) -> Option<String> {
        // Check basic invariants
        if self.inner.docs.is_empty() {
            return Some("No documents loaded".to_string());
        }

        if self.inner.vocabulary.is_empty() {
            return Some("No vocabulary loaded".to_string());
        }

        // Validate suffix array entries
        for (term_idx, offset) in &self.inner.suffix_array {
            let term_idx = *term_idx as usize;
            let offset = *offset as usize;

            if term_idx >= self.inner.vocabulary.len() {
                return Some(format!(
                    "Suffix array term_idx {} >= vocabulary len {}",
                    term_idx,
                    self.inner.vocabulary.len()
                ));
            }

            let term_len = self.inner.vocabulary[term_idx].len();
            if offset > term_len {
                // Note: offset == term_len is allowed (empty suffix at end)
                return Some(format!(
                    "Suffix array offset {} > term[{}] len {}",
                    offset, term_idx, term_len
                ));
            }
        }

        // Validate postings
        for (term_idx, postings) in self.inner.postings.iter().enumerate() {
            if term_idx >= self.inner.vocabulary.len() {
                return Some(format!("Postings term_idx {} >= vocabulary len {}", term_idx, self.inner.vocabulary.len()));
            }

            for posting in postings {
                if posting.doc_id as usize >= self.inner.docs.len() {
                    return Some(format!(
                        "Posting doc_id {} >= docs len {}",
                        posting.doc_id,
                        self.inner.docs.len()
                    ));
                }
            }
        }

        None
    }

    /// Full three-tier search: exact → prefix → fuzzy.
    ///
    /// Returns up to `limit` results, ordered by score descending.
    /// Each result includes tier classification (1, 2, or 3).
    ///
    /// This method delegates to the individual tier methods which properly handle
    /// multi-term queries (splitting on whitespace, AND semantics).
    ///
    /// Note: Searcher is validated at construction time in `from_layer()`.
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        // Return empty results for empty queries to prevent edge cases
        if query.is_empty() || limit == 0 {
            return Vec::new();
        }

        // Tier 1: Exact match (handles multi-term with AND semantics)
        let t1_results = self.search_tier1_exact(query, limit);
        let t1_ids: HashSet<usize> = t1_results.iter().map(|r| r.doc_id).collect();

        // Tier 2: Prefix match (exclude T1 results)
        let t2_results = self.search_tier2_prefix(query, &t1_ids, limit);
        let t2_ids: HashSet<usize> = t2_results.iter().map(|r| r.doc_id).collect();

        // Tier 3: Fuzzy match (exclude T1 and T2 results)
        let mut exclude_ids = t1_ids;
        exclude_ids.extend(t2_ids);
        let t3_results = self.search_tier3_fuzzy(query, &exclude_ids, limit);

        // Merge and sort results
        let mut results: Vec<_> = t1_results
            .into_iter()
            .chain(t2_results)
            .chain(t3_results)
            .collect();
        results.sort_by(|a, b| compare_results(a, b, &self.inner.docs));
        results.truncate(limit);
        results
    }

    /// Tier 1: Exact word match only (O(1) inverted index lookup).
    ///
    /// Returns doc IDs for exact matches only. Fast path for progressive search.
    /// Results are bucketed by match type (Title > Section > Subsection > etc.)
    /// to ensure structural field hierarchy is respected in ranking.
    ///
    /// For multi-term queries, scores are SUMMED across matching terms.
    /// E.g., "rust optimization" → doc matching both gets 200 (100+100).
    ///
    /// OPTIMIZATION: Posting lists are presorted by (score DESC, doc_id ASC).
    /// For single-term queries, the first posting for each unique doc_id is the
    /// highest-scoring one, so we can early-exit after finding `limit` unique docs.
    pub fn search_tier1_exact(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_lower = to_lowercase_ascii_simd(query);

        // Fast path: single word (no whitespace) - skip split/collect
        if !query_lower.contains(' ') && !query_lower.is_empty() {
            return self.search_tier1_single_term(&query_lower, limit);
        }

        let parts: Vec<&str> = query_lower.split_whitespace().filter(|p| !p.is_empty()).collect();

        // Single-term optimization: leverage presorted posting list
        if parts.len() == 1 {
            return self.search_tier1_single_term(parts[0], limit);
        }

        // Multi-term: sum scores across matching terms (AND semantics)
        let mut acc = MultiTermAccumulator::new(parts.len());

        for (term_idx, part) in parts.iter().enumerate() {
            if let Some(postings) = self.inner.inverted_index.get(*part) {
                for entry in postings {
                    let doc_id = entry.doc_id as usize;
                    if self.inner.docs.get(doc_id).is_none() {
                        continue;
                    }

                    let match_type = MatchType::from_heading_level(entry.heading_level);
                    acc.add_match(term_idx, doc_id, entry.section_idx, match_type, T1_EXACT_SCORE);
                }
            }
        }

        acc.into_results(1, limit, &self.inner.docs)
    }

    /// Single-term T1 search with early-exit optimization.
    ///
    /// Since posting lists are sorted by (score DESC, doc_id ASC), we iterate
    /// through and take the first `limit` unique doc_ids. The first posting for
    /// each doc is guaranteed to be the highest-scoring one.
    ///
    /// No sorting needed: posting list order = correct ranking order because
    /// score dominates match_type (Title=100 > Heading=10 > Content=1).
    #[inline]
    fn search_tier1_single_term(&self, term: &str, limit: usize) -> Vec<SearchResult> {
        let mut results = Vec::with_capacity(limit);
        let mut seen_docs = HashSet::with_capacity(limit);

        if let Some(postings) = self.inner.inverted_index.get(term) {
            for entry in postings {
                let doc_id = entry.doc_id as usize;
                if self.inner.docs.get(doc_id).is_none() {
                    continue;
                }

                // First occurrence is best (presorted by score DESC)
                if seen_docs.insert(doc_id) {
                    let match_type = MatchType::from_heading_level(entry.heading_level);
                    results.push(SearchResult {
                        doc_id,
                        score: T1_EXACT_SCORE,
                        section_idx: entry.section_idx,
                        tier: 1,
                        match_type,
                    });

                    // Early exit: we have enough unique docs
                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        // No sort needed - posting list is presorted by score which aligns with match_type
        results
    }

    /// Tier 2: Prefix match only (O(log k) binary search).
    ///
    /// Pass doc IDs from tier1 as exclude_ids to avoid duplicates.
    pub fn search_tier2_prefix(
        &self,
        query: &str,
        exclude_ids: &HashSet<usize>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let query_lower = to_lowercase_ascii_simd(query);

        // Split query into parts for multi-term handling
        let parts: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|p| !p.is_empty())
            .collect();

        if parts.is_empty() {
            return vec![];
        }

        // Single-term fast path
        if parts.len() == 1 {
            return self.search_tier2_single_term(parts[0], exclude_ids, limit);
        }

        // Multi-term: sum scores across matching prefix terms (AND semantics)
        let mut acc = MultiTermAccumulator::new(parts.len());

        for (term_idx, part) in parts.iter().enumerate() {
            let prefix_matches =
                prefix_search_vocabulary(&self.inner.suffix_array, &self.inner.vocabulary, part);

            for vocab_idx in prefix_matches {
                if let Some(postings) = self.inner.postings.get(vocab_idx) {
                    for entry in postings {
                        let doc_id = entry.doc_id as usize;
                        if exclude_ids.contains(&doc_id) || self.inner.docs.get(doc_id).is_none() {
                            continue;
                        }

                        let match_type = MatchType::from_heading_level(entry.heading_level);

                        // Boost score if match is in document title (section_idx == 0)
                        let base_score = if entry.section_idx == 0 {
                            T2_PREFIX_SCORE * T2_TITLE_BOOST
                        } else {
                            T2_PREFIX_SCORE
                        };

                        acc.add_match(term_idx, doc_id, entry.section_idx, match_type, base_score);
                    }
                }
            }
        }

        acc.into_results(2, limit, &self.inner.docs)
    }

    /// Single-term T2 search optimized for single prefix query.
    #[inline]
    fn search_tier2_single_term(
        &self,
        prefix: &str,
        exclude_ids: &HashSet<usize>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let mut results_by_doc: HashMap<usize, SearchResult> = HashMap::new();

        let prefix_matches =
            prefix_search_vocabulary(&self.inner.suffix_array, &self.inner.vocabulary, prefix);

        for vocab_idx in prefix_matches {
            if let Some(postings) = self.inner.postings.get(vocab_idx) {
                for entry in postings {
                    let doc_id = entry.doc_id as usize;
                    if exclude_ids.contains(&doc_id) || self.inner.docs.get(doc_id).is_none() {
                        continue;
                    }

                    // Boost score if match is in document title (section_idx == 0)
                    let score = if entry.section_idx == 0 {
                        T2_PREFIX_SCORE * T2_TITLE_BOOST
                    } else {
                        T2_PREFIX_SCORE
                    };
                    let match_type = MatchType::from_heading_level(entry.heading_level);

                    // Keep best match_type and score for each document
                    results_by_doc
                        .entry(doc_id)
                        .and_modify(|r| {
                            // Keep best match_type (lowest enum value = higher priority)
                            if match_type < r.match_type {
                                r.match_type = match_type;
                                r.section_idx = entry.section_idx;
                                r.score = score;
                            } else if score > r.score {
                                r.score = score;
                            }
                        })
                        .or_insert_with(|| SearchResult {
                            doc_id,
                            score,
                            section_idx: entry.section_idx,
                            tier: 2,
                            match_type,
                        });
                }
            }
        }

        let mut results: Vec<SearchResult> = results_by_doc.into_values().collect();
        results.sort_by(|a, b| compare_results(a, b, &self.inner.docs));
        results.truncate(limit);
        results
    }

    /// Tier 3: Fuzzy match only (O(vocabulary) via Levenshtein DFA).
    ///
    /// Pass doc IDs from tier1+tier2 as exclude_ids to avoid duplicates.
    /// NOTE: Only returns matches at distance > 0 (exact matches are T1's job)
    ///
    /// Applies edit distance penalty: 20% per edit distance.
    /// Multi-term queries sum scores across matching fuzzy terms.
    pub fn search_tier3_fuzzy(
        &self,
        query: &str,
        exclude_ids: &HashSet<usize>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let query_lower = to_lowercase_ascii_simd(query);

        // Split query into parts for multi-term handling
        let parts: Vec<&str> = query_lower
            .split_whitespace()
            .filter(|p| !p.is_empty())
            .collect();

        if parts.is_empty() {
            return vec![];
        }

        // Single-term fast path
        if parts.len() == 1 {
            return self.search_tier3_single_term(parts[0], exclude_ids, limit);
        }

        // Multi-term: sum scores across matching fuzzy terms (AND semantics)
        let mut acc = MultiTermAccumulator::new(parts.len());

        for (term_idx, part) in parts.iter().enumerate() {
            let fuzzy_matches =
                fuzzy_search_vocabulary(&self.inner.vocabulary, self.inner.lev_dfa.as_ref(), part, 2);

            for FuzzyMatch { term_idx: vocab_idx, distance } in fuzzy_matches {
                // Skip exact matches (distance 0) - those are T1's responsibility
                if distance == 0 {
                    continue;
                }

                // Get the matched term to compute length similarity bonus
                let matched_term = self.inner.vocabulary
                    .get(vocab_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");

                if let Some(postings) = self.inner.postings.get(vocab_idx) {
                    for entry in postings {
                        let doc_id = entry.doc_id as usize;
                        if exclude_ids.contains(&doc_id) || self.inner.docs.get(doc_id).is_none() {
                            continue;
                        }

                        let match_type = MatchType::from_heading_level(entry.heading_level);

                        // Compute fuzzy score with edit distance penalty
                        let final_score = compute_fuzzy_score(distance, part.len(), matched_term.len(), entry.section_idx == 0);

                        acc.add_match(term_idx, doc_id, entry.section_idx, match_type, final_score);
                    }
                }
            }
        }

        acc.into_results(3, limit, &self.inner.docs)
    }

    /// Single-term T3 search optimized for single fuzzy query.
    #[inline]
    fn search_tier3_single_term(
        &self,
        term: &str,
        exclude_ids: &HashSet<usize>,
        limit: usize,
    ) -> Vec<SearchResult> {
        let mut doc_scores: HashMap<usize, f64> = HashMap::new();
        let mut doc_section_idxs: HashMap<usize, u32> = HashMap::new();
        let mut doc_match_types: HashMap<usize, MatchType> = HashMap::new();

        let fuzzy_matches =
            fuzzy_search_vocabulary(&self.inner.vocabulary, self.inner.lev_dfa.as_ref(), term, 2);

        for FuzzyMatch { term_idx: vocab_idx, distance } in fuzzy_matches {
            // Skip exact matches (distance 0) - those are T1's responsibility
            if distance == 0 {
                continue;
            }

            // Get the matched term to compute length similarity bonus
            let matched_term = self.inner.vocabulary
                .get(vocab_idx)
                .map(|s| s.as_str())
                .unwrap_or("");

            if let Some(postings) = self.inner.postings.get(vocab_idx) {
                for entry in postings {
                    let doc_id = entry.doc_id as usize;
                    if exclude_ids.contains(&doc_id) || self.inner.docs.get(doc_id).is_none() {
                        continue;
                    }

                    let match_type = MatchType::from_heading_level(entry.heading_level);

                    // Compute fuzzy score with edit distance penalty
                    let final_score = compute_fuzzy_score(distance, term.len(), matched_term.len(), entry.section_idx == 0);

                    // Keep best score and match_type per doc
                    match (doc_scores.get(&doc_id), doc_match_types.get(&doc_id)) {
                        (None, None) => {
                            doc_scores.insert(doc_id, final_score);
                            doc_match_types.insert(doc_id, match_type);
                            doc_section_idxs.insert(doc_id, entry.section_idx);
                        }
                        (_, Some(&prev_match_type)) if match_type < prev_match_type => {
                            doc_scores.insert(doc_id, final_score);
                            doc_match_types.insert(doc_id, match_type);
                            doc_section_idxs.insert(doc_id, entry.section_idx);
                        }
                        (Some(&prev_score), Some(&prev_match_type))
                            if match_type == prev_match_type && final_score > prev_score =>
                        {
                            doc_scores.insert(doc_id, final_score);
                            doc_section_idxs.insert(doc_id, entry.section_idx);
                        }
                        _ => {}
                    }
                }
            }
        }

        let mut results: Vec<SearchResult> = doc_scores
            .into_iter()
            .map(|(doc_id, score)| SearchResult {
                doc_id,
                score,
                section_idx: doc_section_idxs[&doc_id],
                tier: 3,
                match_type: doc_match_types[&doc_id],
            })
            .collect();

        results.sort_by(|a, b| compare_results(a, b, &self.inner.docs));
        results.truncate(limit);
        results
    }

    /// Tier 2: Prefix match without exclusion (for streaming search).
    ///
    /// Same as `search_tier2_prefix` but doesn't exclude any docs.
    /// Used by streaming search to allow cross-tier duplicates (deduplication
    /// happens at the cursor level with replace-if-better semantics).
    pub fn search_tier2_prefix_no_exclude(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.search_tier2_prefix(query, &HashSet::new(), limit)
    }

    /// Tier 3: Fuzzy match without exclusion (for streaming search).
    ///
    /// Same as `search_tier3_fuzzy` but doesn't exclude any docs.
    /// Used by streaming search to allow cross-tier duplicates (deduplication
    /// happens at the cursor level with replace-if-better semantics).
    pub fn search_tier3_fuzzy_no_exclude(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        self.search_tier3_fuzzy(query, &HashSet::new(), limit)
    }

    // ========================================================================
    // STREAMING PARALLEL SEARCH (wasm feature only)
    // ========================================================================

    /// Start streaming parallel search.
    ///
    /// Returns a receiver for UI-ready messages (already deduped).
    /// Main thread should just iterate and call JS callback for each.
    ///
    /// Architecture:
    /// - T1/T2/T3 run in parallel on worker threads
    /// - Dedup worker maintains ordered heap on separate thread
    /// - Results are emitted in ranked order (T1 first, then T2, then T3)
    /// - Main thread only receives ready-to-display results
    #[cfg(feature = "rayon")]
    pub fn search_streaming(&self, query: &str, limit: usize) -> Receiver<UIMessage> {
        let (ui_tx, ui_rx) = channel();

        if query.is_empty() || limit == 0 {
            let _ = ui_tx.send(UIMessage::Finished(vec![]));
            return ui_rx;
        }

        // Channel from tier workers → dedup worker
        let (raw_tx, raw_rx) = channel::<RawResult>();

        let query_lower = to_lowercase_ascii_simd(query);

        // Spawn T1 worker
        let tx1 = raw_tx.clone();
        let q1 = query_lower.clone();
        let s1 = self.clone();
        rayon::spawn(move || {
            s1.stream_tier1(&q1, limit, tx1);
        });

        // Spawn T2 worker
        let tx2 = raw_tx.clone();
        let q2 = query_lower.clone();
        let s2 = self.clone();
        rayon::spawn(move || {
            s2.stream_tier2(&q2, limit, tx2);
        });

        // Spawn T3 worker
        let tx3 = raw_tx.clone();
        let q3 = query_lower.clone();
        let s3 = self.clone();
        rayon::spawn(move || {
            s3.stream_tier3(&q3, limit, tx3);
        });

        // Close sender so dedup worker knows when all tiers are done
        drop(raw_tx);

        // Spawn DEDUP WORKER on separate thread
        let docs = self.inner.docs.clone();
        rayon::spawn(move || {
            Self::dedup_worker(raw_rx, ui_tx, limit, docs);
        });

        ui_rx
    }

    /// Stream T1 exact matches to channel.
    #[cfg(feature = "rayon")]
    fn stream_tier1(&self, query: &str, limit: usize, tx: Sender<RawResult>) {
        if let Some(postings) = self.inner.inverted_index.get(query) {
            let mut count = 0;
            for entry in postings {
                if count >= limit {
                    break;
                }
                if self.inner.docs.get(entry.doc_id as usize).is_none() {
                    continue;
                }
                let result = SearchResult {
                    doc_id: entry.doc_id as usize,
                    score: T1_EXACT_SCORE,
                    section_idx: entry.section_idx,
                    tier: 1,
                    match_type: MatchType::from_heading_level(entry.heading_level),
                };
                if tx.send(RawResult { result, tier_done: None }).is_err() {
                    return;
                }
                count += 1;
            }
        }
        // Signal tier completion
        let _ = tx.send(RawResult {
            result: SearchResult {
                doc_id: 0,
                score: 0.0,
                section_idx: 0,
                tier: 1,
                match_type: MatchType::Content,
            },
            tier_done: Some(1),
        });
    }

    /// Stream T2 prefix matches to channel.
    #[cfg(feature = "rayon")]
    fn stream_tier2(&self, query: &str, limit: usize, tx: Sender<RawResult>) {
        let prefix_matches = prefix_search_vocabulary(
            &self.inner.suffix_array,
            &self.inner.vocabulary,
            query,
        );

        let mut count = 0;
        for vocab_idx in prefix_matches {
            if count >= limit {
                break;
            }
            if let Some(postings) = self.inner.postings.get(vocab_idx) {
                for entry in postings {
                    if count >= limit {
                        break;
                    }
                    if self.inner.docs.get(entry.doc_id as usize).is_none() {
                        continue;
                    }
                    let result = SearchResult {
                        doc_id: entry.doc_id as usize,
                        score: T2_PREFIX_SCORE,
                        section_idx: entry.section_idx,
                        tier: 2,
                        match_type: MatchType::from_heading_level(entry.heading_level),
                    };
                    if tx.send(RawResult { result, tier_done: None }).is_err() {
                        return;
                    }
                    count += 1;
                }
            }
        }
        // Signal tier completion
        let _ = tx.send(RawResult {
            result: SearchResult {
                doc_id: 0,
                score: 0.0,
                section_idx: 0,
                tier: 2,
                match_type: MatchType::Content,
            },
            tier_done: Some(2),
        });
    }

    /// Stream T3 fuzzy matches to channel.
    #[cfg(feature = "rayon")]
    fn stream_tier3(&self, query: &str, limit: usize, tx: Sender<RawResult>) {
        let dfa = match &self.inner.lev_dfa {
            Some(dfa) => dfa,
            None => {
                // Signal tier completion even if no DFA
                let _ = tx.send(RawResult {
                    result: SearchResult {
                        doc_id: 0,
                        score: 0.0,
                        section_idx: 0,
                        tier: 3,
                        match_type: MatchType::Content,
                    },
                    tier_done: Some(3),
                });
                return;
            }
        };

        let matcher = QueryMatcher::new(dfa, query);

        // Parallel vocabulary scan using rayon
        let fuzzy_matches: Vec<FuzzyMatch> = self.inner.vocabulary
            .par_iter()
            .enumerate()
            .filter_map(|(term_idx, term)| {
                matcher.matches(term).and_then(|distance| {
                    // Only include fuzzy matches (distance > 0), not exact (T1's job)
                    if distance <= 2 && distance > 0 {
                        Some(FuzzyMatch { term_idx, distance })
                    } else {
                        None
                    }
                })
            })
            .collect();

        let mut count = 0;
        for FuzzyMatch { term_idx, distance } in fuzzy_matches {
            if count >= limit {
                break;
            }
            if let Some(postings) = self.inner.postings.get(term_idx) {
                for entry in postings {
                    if count >= limit {
                        break;
                    }
                    if self.inner.docs.get(entry.doc_id as usize).is_none() {
                        continue;
                    }
                    let score = match distance {
                        1 => T3_FUZZY_DISTANCE_1_SCORE,
                        2 => T3_FUZZY_DISTANCE_2_SCORE,
                        _ => T3_FUZZY_DISTANCE_3_SCORE,
                    };
                    let result = SearchResult {
                        doc_id: entry.doc_id as usize,
                        score,
                        section_idx: entry.section_idx,
                        tier: 3,
                        match_type: MatchType::from_heading_level(entry.heading_level),
                    };
                    if tx.send(RawResult { result, tier_done: None }).is_err() {
                        return;
                    }
                    count += 1;
                }
            }
        }
        // Signal tier completion
        let _ = tx.send(RawResult {
            result: SearchResult {
                doc_id: 0,
                score: 0.0,
                section_idx: 0,
                tier: 3,
                match_type: MatchType::Content,
            },
            tier_done: Some(3),
        });
    }

    /// Dedup worker: runs on separate thread, maintains ordered heap, forwards unique results.
    ///
    /// CRITICAL: Results are emitted in RANKED ORDER (highest score first).
    /// Since T1 (100) > T2 (50) > T3 (30), we emit tier-by-tier:
    /// - Buffer all results as they arrive
    /// - When T1 finishes → emit T1 results (guaranteed highest scores)
    /// - When T2 finishes → emit T2 results (next highest)
    /// - When T3 finishes → emit T3 results + final sorted list
    ///
    /// This ensures UI always receives results in ranked order.
    #[cfg(feature = "rayon")]
    fn dedup_worker(
        raw_rx: Receiver<RawResult>,
        ui_tx: Sender<UIMessage>,
        limit: usize,
        docs: Vec<SearchDoc>,
    ) {
        // Dedup lookup: doc_id → (score, tier, section_idx) for removal
        // Using doc_id only ensures each document appears at most once
        let mut seen: HashMap<usize, (OrderedFloat, u8, u32)> = HashMap::new();

        // Ordered heap: sorted by (score DESC, tier ASC, doc_id, section_idx)
        // BTreeMap with Reverse<score> so iteration gives highest score first
        type HeapKey = (Reverse<OrderedFloat>, u8, usize, u32);
        let mut heap: BTreeMap<HeapKey, SearchResult> = BTreeMap::new();

        // Track which tiers are done
        let mut t1_done = false;
        let mut t2_done = false;
        let mut t3_done = false;

        let mut emitted = 0usize;

        for raw in raw_rx {
            // Check for tier completion signal
            if let Some(tier) = raw.tier_done {
                match tier {
                    1 => t1_done = true,
                    2 => t2_done = true,
                    3 => t3_done = true,
                    _ => {}
                }

                // EMIT results in ranked order when tier completes
                if t1_done && emitted < limit {
                    // Emit all T1 results (score 100, guaranteed highest)
                    emitted += Self::emit_tier_results(&mut heap, 1, limit - emitted, &ui_tx);
                }
                if t1_done && t2_done && emitted < limit {
                    // Emit T2 results (score 50, next highest)
                    emitted += Self::emit_tier_results(&mut heap, 2, limit - emitted, &ui_tx);
                }
                if t1_done && t2_done && t3_done {
                    break;
                }
                continue;
            }

            let result = raw.result;
            let doc_id = result.doc_id;
            let score = OrderedFloat(result.score);

            // Dedupe by doc_id only: keep result with best (score, tier)
            match seen.entry(doc_id) {
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert((score, result.tier, result.section_idx));
                    let heap_key = (Reverse(score), result.tier, result.doc_id, result.section_idx);
                    heap.insert(heap_key, result);
                }
                std::collections::hash_map::Entry::Occupied(mut e) => {
                    let (old_score, old_tier, old_section_idx) = *e.get();
                    // Better if higher score, or same score but lower tier
                    if score > old_score || (score == old_score && result.tier < old_tier) {
                        // Remove old entry using stored section_idx
                        let old_key = (Reverse(old_score), old_tier, doc_id, old_section_idx);
                        heap.remove(&old_key);
                        // Insert new
                        e.insert((score, result.tier, result.section_idx));
                        let heap_key = (Reverse(score), result.tier, result.doc_id, result.section_idx);
                        heap.insert(heap_key, result);
                    }
                }
            }
        }

        // Emit remaining T3 results
        if emitted < limit {
            let _ = Self::emit_tier_results(&mut heap, 3, limit - emitted, &ui_tx);
        }

        // Final sorted results (for onFinish callback)
        let mut final_results: Vec<SearchResult> = heap.into_values().collect();
        final_results.sort_by(|a, b| compare_results(a, b, &docs));
        final_results.truncate(limit);

        let _ = ui_tx.send(UIMessage::Finished(final_results));
    }

    /// Emit results from a specific tier in score order.
    /// Returns number of results emitted.
    #[cfg(feature = "rayon")]
    fn emit_tier_results(
        heap: &mut BTreeMap<(Reverse<OrderedFloat>, u8, usize, u32), SearchResult>,
        tier: u8,
        max: usize,
        tx: &Sender<UIMessage>,
    ) -> usize {
        let mut emitted = 0;
        let mut to_remove = Vec::new();

        // Collect tier results to emit (already in score order)
        for (key, result) in heap.iter() {
            if key.1 == tier && emitted < max {
                if tx.send(UIMessage::Result(result.clone())).is_err() {
                    return emitted;
                }
                to_remove.push(*key);
                emitted += 1;
            }
        }

        // Remove emitted entries
        for key in to_remove {
            heap.remove(&key);
        }

        emitted
    }
}

/// Prefix search over vocabulary suffix array (O(log k)).
///
/// Uses binary search (partition_point) to find all terms that start with the prefix.
///
/// The suffix array is sorted lexicographically, allowing binary search for the
/// first suffix >= prefix. Then we scan forward collecting all matching suffixes.
///
/// # Arguments
/// * `suffix_array` - Sorted array of (term_idx, offset) pairs
/// * `vocabulary` - Vocabulary terms (indexed by term_idx)
/// * `prefix` - Search prefix (assumed lowercase)
///
/// # Returns
/// Vector of term indices whose vocabulary entries start with prefix.
pub fn prefix_search_vocabulary(
    suffix_array: &[(u32, u32)],
    vocabulary: &[String],
    prefix: &str,
) -> Vec<usize> {
    if suffix_array.is_empty() || prefix.is_empty() {
        return Vec::new();
    }

    let mut matches = HashSet::new();

    // Binary search for first suffix starting with prefix
    let start = suffix_array.partition_point(|(term_idx, offset)| {
        let term_idx = *term_idx as usize;
        let offset = *offset as usize;

        // Bounds check before accessing vocabulary
        if term_idx >= vocabulary.len() {
            return false; // term_idx out of bounds, treat as >= prefix
        }

        let term = &vocabulary[term_idx];
        if offset >= term.len() {
            return false; // offset out of bounds, treat as >= prefix
        }

        // Check if offset is at a valid UTF-8 character boundary
        if !term.is_char_boundary(offset) {
            return false; // invalid byte offset, skip
        }

        let suffix = &term[offset..];
        suffix < prefix
    });

    // If partition_point returns end of array, no matches
    if start >= suffix_array.len() {
        return Vec::new();
    }

    // Collect all matching suffixes
    for i in start..suffix_array.len() {
        let (term_idx, offset) = suffix_array[i];
        let term_idx = term_idx as usize;

        // Bounds check
        if term_idx >= vocabulary.len() {
            break; // Suffix array contains invalid index, stop processing
        }

        let term = &vocabulary[term_idx];
        let offset_usize = offset as usize;

        // Check bounds before creating suffix
        if offset_usize >= term.len() {
            break;
        }

        // Check if offset is at a valid UTF-8 character boundary
        if !term.is_char_boundary(offset_usize) {
            continue; // Skip invalid byte offsets
        }

        let suffix = &term[offset_usize..];

        // Use SIMD-accelerated prefix check for faster matching
        if starts_with_simd(suffix.as_bytes(), prefix.as_bytes()) {
            // Only count if prefix matches at word start (offset == 0)
            if offset == 0 {
                matches.insert(term_idx);
            }
        } else {
            break; // Past all prefix matches (sorted array)
        }
    }

    // Sort to ensure deterministic order (HashSet iteration is non-deterministic)
    let mut result: Vec<usize> = matches.into_iter().collect();
    result.sort_unstable();
    result
}

/// Fuzzy search using Levenshtein DFA (O(vocabulary), ~8ns per term).
///
/// Uses precomputed Levenshtein automaton tables (Schulz-Mihov 2002).
/// DFA loaded from embedded bytes; query matcher build is pure table lookups (~1μs).
///
/// # Arguments
/// * `vocabulary` - Vocabulary terms to search
/// * `lev_dfa` - Precomputed Levenshtein DFA (if None, returns empty results)
/// * `query` - Search query (assumed lowercase)
/// * `max_distance` - Maximum edit distance (typically 2)
///
/// # Returns
/// Vector of FuzzyMatch results (term_idx, distance), sorted by distance ascending.
pub fn fuzzy_search_vocabulary(
    vocabulary: &[String],
    lev_dfa: Option<&ParametricDFA>,
    query: &str,
    max_distance: u8,
) -> Vec<FuzzyMatch> {
    // Need precomputed DFA for fuzzy search
    let dfa = match lev_dfa {
        Some(dfa) => dfa,
        None => return Vec::new(),
    };

    if vocabulary.is_empty() {
        return Vec::new();
    }

    // Build query-specific matcher from precomputed DFA tables (~1μs)
    let matcher = QueryMatcher::new(dfa, query);

    let mut matches = Vec::new();

    // Iterate vocabulary and check each term against the matcher
    // For ~6000 terms, this is faster than FST setup overhead (~0.3µs per match)
    for (term_idx, term) in vocabulary.iter().enumerate() {
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

// ============================================================================
// MULTI-TERM QUERY TESTS
// ============================================================================

#[cfg(test)]
mod multi_term_tests {
    use super::*;

    fn create_test_searcher() -> TierSearcher {
        // Create a simple index with known documents for testing multi-term queries
        let docs = vec![
            SearchDoc {
                id: 0,
                title: "Rust Programming Guide".to_string(),
                excerpt: "Learn Rust programming".to_string(),
                href: "/doc1".to_string(),
                kind: "article".to_string(),
                category: Some("programming".to_string()),
                author: None,
                tags: vec![],
            },
            SearchDoc {
                id: 1,
                title: "Optimization Techniques".to_string(),
                excerpt: "Learn optimization".to_string(),
                href: "/doc2".to_string(),
                kind: "article".to_string(),
                category: Some("programming".to_string()),
                author: None,
                tags: vec![],
            },
            SearchDoc {
                id: 2,
                title: "Rust Optimization".to_string(),
                excerpt: "Rust optimization guide".to_string(),
                href: "/doc3".to_string(),
                kind: "article".to_string(),
                category: Some("programming".to_string()),
                author: None,
                tags: vec![],
            },
            SearchDoc {
                id: 3,
                title: "Python Programming".to_string(),
                excerpt: "Learn Python".to_string(),
                href: "/doc4".to_string(),
                kind: "article".to_string(),
                category: Some("programming".to_string()),
                author: None,
                tags: vec![],
            },
        ];

        // Build vocabulary (sorted for suffix array)
        let vocabulary = vec![
            "guide".to_string(),
            "optimization".to_string(),
            "programming".to_string(),
            "python".to_string(),
            "rust".to_string(),
        ];

        // Build inverted index
        let mut inverted_index: HashMap<String, Vec<PostingEntry>> = HashMap::new();

        // "rust" appears in doc0, doc2 (titles)
        inverted_index.insert(
            "rust".to_string(),
            vec![
                PostingEntry {
                    doc_id: 0,
                    section_idx: 0,
                    heading_level: 0,
                },
                PostingEntry {
                    doc_id: 2,
                    section_idx: 0,
                    heading_level: 0,
                },
            ],
        );

        // "programming" appears in doc0, doc3 (titles)
        inverted_index.insert(
            "programming".to_string(),
            vec![
                PostingEntry {
                    doc_id: 0,
                    section_idx: 0,
                    heading_level: 0,
                },
                PostingEntry {
                    doc_id: 3,
                    section_idx: 0,
                    heading_level: 0,
                },
            ],
        );

        // "optimization" appears in doc1, doc2 (titles)
        inverted_index.insert(
            "optimization".to_string(),
            vec![
                PostingEntry {
                    doc_id: 1,
                    section_idx: 0,
                    heading_level: 0,
                },
                PostingEntry {
                    doc_id: 2,
                    section_idx: 0,
                    heading_level: 0,
                },
            ],
        );

        // "guide" appears in doc0, doc2 (both in section 0 for multi-term test)
        inverted_index.insert(
            "guide".to_string(),
            vec![
                PostingEntry {
                    doc_id: 0,
                    section_idx: 0,
                    heading_level: 0,
                },
                PostingEntry {
                    doc_id: 2,
                    section_idx: 0, // Same section as rust/optimization for multi-term summing
                    heading_level: 0,
                },
            ],
        );

        // "python" appears in doc3
        inverted_index.insert(
            "python".to_string(),
            vec![PostingEntry {
                doc_id: 3,
                section_idx: 0,
                heading_level: 0,
            }],
        );

        // Build suffix array: (vocab_index, char_offset) sorted by suffix
        // For simplicity, we just use word-start suffixes (offset=0)
        let mut suffix_array: Vec<(u32, u32)> = vocabulary
            .iter()
            .enumerate()
            .map(|(i, _)| (i as u32, 0))
            .collect();
        suffix_array.sort_by(|&(a, _), &(b, _)| vocabulary[a as usize].cmp(&vocabulary[b as usize]));

        // Build postings indexed by vocabulary position
        let postings: Vec<Vec<PostingEntry>> = vocabulary
            .iter()
            .map(|term| inverted_index.get(term).cloned().unwrap_or_default())
            .collect();

        let inner = TierSearcherInner {
            docs,
            vocabulary,
            inverted_index,
            suffix_array,
            postings,
            section_table: vec![],
            lev_dfa: None,
        };

        TierSearcher {
            inner: Arc::new(inner),
        }
    }

    #[test]
    fn test_t1_multiterm_score_summing() {
        let searcher = create_test_searcher();

        // "rust optimization" should match doc2 which has both terms
        let results = searcher.search_tier1_exact("rust optimization", 10);

        // Should find doc2 (has both rust + optimization)
        assert!(!results.is_empty(), "Should find multi-term matches");
        assert_eq!(results[0].doc_id, 2, "Doc2 should be first (has both terms)");

        // Score should be ~200 (100 per term)
        assert!(
            results[0].score >= 180.0,
            "Multi-term score should be summed (~200), got {}",
            results[0].score
        );

        // Single-term "rust" should return doc0 and doc2
        let single_results = searcher.search_tier1_exact("rust", 10);
        assert!(single_results.len() >= 2, "Single term should match more docs");
        assert!(
            single_results[0].score < 150.0,
            "Single-term score should be ~100, got {}",
            single_results[0].score
        );
    }

    #[test]
    fn test_t1_multiterm_and_semantics() {
        let searcher = create_test_searcher();

        // "rust python" should match nothing (no doc has both)
        let results = searcher.search_tier1_exact("rust python", 10);
        assert!(
            results.is_empty(),
            "AND semantics: no doc has both rust and python"
        );

        // "rust programming" should match doc0 (has both in title)
        let results = searcher.search_tier1_exact("rust programming", 10);
        assert!(!results.is_empty(), "Should find doc with both terms");
        assert_eq!(results[0].doc_id, 0, "Doc0 has both rust and programming");
    }

    #[test]
    fn test_t2_multiterm_score_summing() {
        let searcher = create_test_searcher();
        let exclude = HashSet::new();

        // "rus opt" (prefixes) should match doc2 which has both rust + optimization
        let results = searcher.search_tier2_prefix("rus opt", &exclude, 10);

        // Should find doc2 (matches both prefixes)
        assert!(!results.is_empty(), "Should find multi-term prefix matches");
        assert_eq!(results[0].doc_id, 2, "Doc2 should match both prefixes");

        // Score should be ~100+ (50 per term, possibly with title boost)
        assert!(
            results[0].score >= 90.0,
            "Multi-term prefix score should be summed (~100+), got {}",
            results[0].score
        );
    }

    #[test]
    fn test_t2_multiterm_and_semantics() {
        let searcher = create_test_searcher();
        let exclude = HashSet::new();

        // "rus pyt" should match nothing (no doc has both rust and python)
        let results = searcher.search_tier2_prefix("rus pyt", &exclude, 10);
        assert!(
            results.is_empty(),
            "AND semantics: no doc has both rust and python prefixes"
        );
    }

    #[test]
    fn test_single_term_unchanged() {
        let searcher = create_test_searcher();

        // Single-term queries should work as before
        let results = searcher.search_tier1_exact("rust", 10);
        assert!(!results.is_empty(), "Single term should work");
        assert!(
            results[0].score >= 90.0 && results[0].score <= 120.0,
            "Single-term T1 score should be ~100, got {}",
            results[0].score
        );

        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("rus", &exclude, 10);
        assert!(!results.is_empty(), "Single prefix should work");
        assert!(
            results[0].score >= 45.0 && results[0].score <= 70.0,
            "Single-term T2 score should be ~50-60, got {}",
            results[0].score
        );
    }

    #[test]
    fn test_empty_query() {
        let searcher = create_test_searcher();

        let results = searcher.search_tier1_exact("", 10);
        assert!(results.is_empty(), "Empty query should return empty");

        let results = searcher.search_tier1_exact("   ", 10);
        assert!(results.is_empty(), "Whitespace-only query should return empty");

        let exclude = HashSet::new();
        let results = searcher.search_tier2_prefix("", &exclude, 10);
        assert!(results.is_empty(), "Empty prefix query should return empty");
    }

    #[test]
    fn test_multiterm_three_terms() {
        let searcher = create_test_searcher();

        // "rust optimization guide" - doc2 has all three (rust, optimization, guide)
        let results = searcher.search_tier1_exact("rust optimization guide", 10);

        // Should find doc2
        assert!(!results.is_empty(), "Should find doc with all 3 terms");
        assert_eq!(results[0].doc_id, 2, "Doc2 should have all 3 terms");

        // Score should be ~300 (100 per term)
        assert!(
            results[0].score >= 250.0,
            "Three-term score should be summed (~300), got {}",
            results[0].score
        );
    }

    #[test]
    fn test_t3_multiterm_without_dfa() {
        let searcher = create_test_searcher();
        let exclude = HashSet::new();

        // Without DFA, T3 should return empty
        let results = searcher.search_tier3_fuzzy("ruzt optimizaton", &exclude, 10);
        assert!(
            results.is_empty(),
            "T3 without DFA should return empty results"
        );
    }
}
