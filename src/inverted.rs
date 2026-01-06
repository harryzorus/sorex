//! Inverted index construction.
//!
//! # Lean Correspondence
//!
//! The index construction corresponds to specifications in:
//! - `SearchVerified/InvertedIndex.lean` - Posting list properties
//!
//! # INVARIANTS (DO NOT VIOLATE)
//!
//! 1. **POSTING_LIST_SORTED**: Each posting list is sorted by (doc_id, offset)
//! 2. **DOC_FREQ_CORRECT**: doc_freq equals count of unique doc_ids
//! 3. **NON_EMPTY**: Every term has at least one posting
//! 4. **POSTING_WELLFORMED**: Every posting has valid doc_id and offset

use crate::types::{
    FieldBoundary, FieldType, IndexMode, InvertedIndex, Posting, PostingList, SearchDoc,
    UnifiedIndex,
};
use crate::utils::normalize;
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use std::collections::HashMap;

/// Word boundary detection: checks if character is a word separator.
fn is_word_boundary(c: char) -> bool {
    !c.is_alphanumeric()
}

use std::collections::HashSet;
use std::sync::LazyLock;

/// Multilingual stop words loaded from data/stop_words.json.
///
/// These words are:
/// 1. Too common to be useful for search ranking
/// 2. Cause false positives in fuzzy matching (e.g., "land" → "and")
/// 3. Waste index space
///
/// The JSON file contains stop words for 20+ languages including:
/// English, Spanish, French, German, Portuguese, Italian, Dutch, Russian,
/// Polish, Swedish, Norwegian, Danish, Finnish, Turkish, Indonesian,
/// Arabic, Hindi, Chinese, Japanese, Korean (romanized forms).
static STOP_WORDS: LazyLock<HashSet<String>> = LazyLock::new(|| {
    let json_str = include_str!("../data/stop_words.json");
    parse_stop_words_json(json_str)
});

/// Parse stop words from JSON, flattening all language arrays into a single set.
/// Normalizes words to match how input text is normalized (strips diacritics).
fn parse_stop_words_json(json_str: &str) -> HashSet<String> {
    let mut stop_words = HashSet::new();

    // Simple JSON parsing without external dependency
    // The JSON structure is: { "lang": ["word1", "word2", ...], ... }
    let mut in_array = false;
    let mut current_word = String::new();
    let mut in_string = false;

    for ch in json_str.chars() {
        match ch {
            '[' => in_array = true,
            ']' => in_array = false,
            '"' if in_array => {
                if in_string {
                    // End of string, normalize and add word
                    if !current_word.is_empty() {
                        // Normalize to strip diacritics (e.g., "tú" → "tu", "está" → "esta")
                        let normalized = normalize(&current_word);
                        if !normalized.is_empty() {
                            stop_words.insert(normalized);
                        }
                        // Also insert original for non-normalized lookups
                        stop_words.insert(current_word.clone());
                        current_word.clear();
                    }
                }
                in_string = !in_string;
            }
            _ if in_string => {
                current_word.push(ch);
            }
            _ => {}
        }
    }

    stop_words
}

/// Check if a word is a stop word.
///
/// Stop words are filtered during index construction to:
/// 1. Reduce index size
/// 2. Prevent low-quality fuzzy matches (e.g., "land" → "and")
/// 3. Improve search relevance
#[inline]
pub fn is_stop_word(word: &str) -> bool {
    STOP_WORDS.contains(word)
}

/// Tokenize text into (word, offset) pairs.
///
/// Returns normalized words with their byte offsets in the original text.
/// Only returns words at word boundaries (start of text or after non-alphanumeric).
///
/// # Lean Specification
///
/// This corresponds to the word extraction precondition in `build_complete`:
/// ```lean
/// (h_boundary : offset = 0 ∨ ¬ (texts[doc_id]).get! (offset - 1) |>.isAlphaNum)
/// ```
fn tokenize(text: &str) -> Vec<(String, usize)> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut byte_offset = 0;

    while i < chars.len() {
        // Skip non-alphanumeric characters
        while i < chars.len() && is_word_boundary(chars[i]) {
            byte_offset += chars[i].len_utf8();
            i += 1;
        }

        if i >= chars.len() {
            break;
        }

        // Record start of word
        let word_start = byte_offset;
        let word_char_start = i;

        // Collect word characters
        while i < chars.len() && !is_word_boundary(chars[i]) {
            byte_offset += chars[i].len_utf8();
            i += 1;
        }

        // Extract and normalize word
        let word: String = chars[word_char_start..i].iter().collect();
        let normalized = normalize(&word);

        // Skip empty words and stop words
        if !normalized.is_empty() && !is_stop_word(&normalized) {
            tokens.push((normalized, word_start));
        }
    }

    tokens
}

/// Build an inverted index from documents.
///
/// Creates a map from terms to posting lists, enabling O(1) term lookup.
///
/// # Lean Specification
///
/// The following properties are guaranteed (see `InvertedIndex.lean`):
/// - `posting_list_sorted`: Each posting list is sorted
/// - `doc_freq_correct`: Document frequencies are accurate
/// - `build_complete`: All words in documents are indexed
#[cfg_attr(
    feature = "lean",
    lean_verify(
        spec = "build_inverted_index",
        requires = "docs.len() > 0 ∧ docs.len() = texts.len()",
        ensures = "InvertedIndex.WellFormed result texts",
        properties = ["posting_list_sorted", "doc_freq_correct", "build_complete"]
    )
)]
pub fn build_inverted_index(texts: &[String], field_boundaries: &[FieldBoundary]) -> InvertedIndex {
    let mut terms: HashMap<String, Vec<Posting>> = HashMap::new();

    // Process each document
    for (doc_id, text) in texts.iter().enumerate() {
        let tokens = tokenize(text);

        for (word, offset) in tokens {
            // Determine field type and section_id at this position
            let (field_type, section_id) =
                get_field_info_for_inverted(doc_id, offset, field_boundaries);

            let posting = Posting {
                doc_id,
                offset,
                field_type,
                section_id,
            };

            terms.entry(word).or_default().push(posting);
        }
    }

    // INVARIANT: POSTING_LIST_SORTED
    // Sort each posting list by (doc_id, offset)
    for postings in terms.values_mut() {
        postings.sort();
    }

    // Build final posting lists with doc_freq
    let mut final_terms: HashMap<String, PostingList> = HashMap::new();
    for (term, postings) in terms {
        // Calculate doc_freq (unique doc_ids)
        let mut doc_ids: Vec<usize> = postings.iter().map(|p| p.doc_id).collect();
        doc_ids.sort();
        doc_ids.dedup();
        let doc_freq = doc_ids.len();

        final_terms.insert(term, PostingList { postings, doc_freq });
    }

    InvertedIndex {
        terms: final_terms,
        total_docs: texts.len(),
    }
}

/// Get field type and section_id for a position (inverted index version).
/// Returns (field_type, section_id) tuple.
fn get_field_info_for_inverted(
    doc_id: usize,
    offset: usize,
    boundaries: &[FieldBoundary],
) -> (FieldType, Option<String>) {
    for b in boundaries {
        if b.doc_id == doc_id && offset >= b.start && offset < b.end {
            return (b.field_type.clone(), b.section_id.clone());
        }
    }
    (FieldType::Content, None)
}

/// Build an inverted index using parallel map-reduce.
///
/// This is faster for large corpora (100+ documents):
/// 1. **Map phase**: Parallel tokenization (one task per document)
/// 2. **Reduce phase**: Merge per-document posting maps into global index
///
/// For small corpora (<100 docs), use `build_inverted_index` instead.
#[cfg(feature = "parallel")]
pub fn build_inverted_index_parallel(
    texts: &[String],
    field_boundaries: &[FieldBoundary],
) -> InvertedIndex {
    // MAP PHASE: Parallel tokenization
    let per_doc_terms: Vec<HashMap<String, Vec<Posting>>> = texts
        .par_iter()
        .enumerate()
        .map(|(doc_id, text)| {
            let mut doc_terms: HashMap<String, Vec<Posting>> = HashMap::new();
            for (word, offset) in tokenize(text) {
                let (field_type, section_id) =
                    get_field_info_for_inverted(doc_id, offset, field_boundaries);
                doc_terms.entry(word).or_default().push(Posting {
                    doc_id,
                    offset,
                    field_type,
                    section_id,
                });
            }
            doc_terms
        })
        .collect();

    // REDUCE PHASE: Merge all per-document maps
    let mut terms: HashMap<String, Vec<Posting>> = HashMap::new();
    for doc_terms in per_doc_terms {
        for (term, postings) in doc_terms {
            terms.entry(term).or_default().extend(postings);
        }
    }

    // Sort each posting list
    terms
        .par_iter_mut()
        .for_each(|(_, postings)| postings.sort());

    // Build final posting lists with doc_freq
    let final_terms: HashMap<String, PostingList> = terms
        .into_par_iter()
        .map(|(term, postings)| {
            let mut doc_ids: Vec<usize> = postings.iter().map(|p| p.doc_id).collect();
            doc_ids.sort();
            doc_ids.dedup();
            (
                term,
                PostingList {
                    postings,
                    doc_freq: doc_ids.len(),
                },
            )
        })
        .collect();

    InvertedIndex {
        terms: final_terms,
        total_docs: texts.len(),
    }
}

/// Sequential version for non-parallel builds (WASM).
#[cfg(not(feature = "parallel"))]
pub fn build_inverted_index_parallel(
    texts: &[String],
    field_boundaries: &[FieldBoundary],
) -> InvertedIndex {
    let per_doc_terms: Vec<HashMap<String, Vec<Posting>>> = texts
        .iter()
        .enumerate()
        .map(|(doc_id, text)| {
            let mut doc_terms: HashMap<String, Vec<Posting>> = HashMap::new();
            for (word, offset) in tokenize(text) {
                let (field_type, section_id) =
                    get_field_info_for_inverted(doc_id, offset, field_boundaries);
                doc_terms.entry(word).or_default().push(Posting {
                    doc_id,
                    offset,
                    field_type,
                    section_id,
                });
            }
            doc_terms
        })
        .collect();

    let mut terms: HashMap<String, Vec<Posting>> = HashMap::new();
    for doc_terms in per_doc_terms {
        for (term, postings) in doc_terms {
            terms.entry(term).or_default().extend(postings);
        }
    }

    for postings in terms.values_mut() {
        postings.sort();
    }

    let final_terms: HashMap<String, PostingList> = terms
        .into_iter()
        .map(|(term, postings)| {
            let mut doc_ids: Vec<usize> = postings.iter().map(|p| p.doc_id).collect();
            doc_ids.sort();
            doc_ids.dedup();
            (
                term,
                PostingList {
                    postings,
                    doc_freq: doc_ids.len(),
                },
            )
        })
        .collect();

    InvertedIndex {
        terms: final_terms,
        total_docs: texts.len(),
    }
}

/// Threshold configuration for index mode selection.
#[derive(Debug, Clone)]
pub struct IndexThresholds {
    /// Below this doc count, use suffix array only
    pub suffix_only_max_docs: usize,
    /// Below this total text size (bytes), use suffix array only
    pub suffix_only_max_bytes: usize,
    /// Above this doc count, use inverted index only (unless fuzzy needed)
    pub inverted_only_min_docs: usize,
}

impl Default for IndexThresholds {
    fn default() -> Self {
        Self {
            suffix_only_max_docs: 50,
            suffix_only_max_bytes: 100_000, // 100KB
            inverted_only_min_docs: 500,
        }
    }
}

/// Determine the best index mode based on content characteristics.
///
/// This is the build-time decision that determines which index(es) to create.
///
/// # Decision Logic
///
/// - Small content (few docs, small text): Suffix array only
///   - Simpler, good enough performance, supports all query types
/// - Large content with mostly exact queries: Inverted index only
///   - O(1) word lookup, efficient AND intersection
/// - Large content needing prefix/fuzzy: Hybrid
///   - Best of both, but larger index size
pub fn select_index_mode(
    doc_count: usize,
    total_text_bytes: usize,
    needs_prefix_matching: bool,
    needs_fuzzy_matching: bool,
    thresholds: &IndexThresholds,
) -> IndexMode {
    // Small content: suffix array is fine
    if doc_count <= thresholds.suffix_only_max_docs
        && total_text_bytes <= thresholds.suffix_only_max_bytes
    {
        return IndexMode::SuffixArrayOnly;
    }

    // Large content but needs prefix/fuzzy: must have suffix array
    if needs_prefix_matching || needs_fuzzy_matching {
        // Very large content: use hybrid
        if doc_count >= thresholds.inverted_only_min_docs {
            return IndexMode::Hybrid;
        }
        // Medium content: suffix array only is still ok
        return IndexMode::SuffixArrayOnly;
    }

    // Large content, exact matches only: inverted index
    if doc_count >= thresholds.inverted_only_min_docs {
        return IndexMode::InvertedIndexOnly;
    }

    // Medium content, no special needs: suffix array
    IndexMode::SuffixArrayOnly
}

/// Build a unified index based on content characteristics.
///
/// Automatically selects the best index mode and builds the appropriate indexes.
pub fn build_unified_index(
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    field_boundaries: Vec<FieldBoundary>,
    thresholds: &IndexThresholds,
    needs_prefix_matching: bool,
    needs_fuzzy_matching: bool,
) -> UnifiedIndex {
    let total_bytes: usize = texts.iter().map(|t| t.len()).sum();
    let mode = select_index_mode(
        docs.len(),
        total_bytes,
        needs_prefix_matching,
        needs_fuzzy_matching,
        thresholds,
    );

    let (suffix_array, lcp) = match mode {
        IndexMode::SuffixArrayOnly | IndexMode::Hybrid => {
            let suffix_index =
                crate::build_index(docs.clone(), texts.clone(), field_boundaries.clone());
            (Some(suffix_index.suffix_array), Some(suffix_index.lcp))
        }
        IndexMode::InvertedIndexOnly => (None, None),
    };

    let inverted_index = match mode {
        IndexMode::InvertedIndexOnly | IndexMode::Hybrid => {
            Some(build_inverted_index(&texts, &field_boundaries))
        }
        IndexMode::SuffixArrayOnly => None,
    };

    UnifiedIndex {
        docs,
        texts,
        field_boundaries,
        mode,
        suffix_array,
        lcp,
        inverted_index,
    }
}

/// Check if an inverted index is well-formed (debug assertion).
#[cfg(any(debug_assertions, test))]
#[allow(dead_code)]
pub fn check_inverted_index_well_formed(index: &InvertedIndex, texts: &[String]) -> bool {
    // Check total_docs
    if index.total_docs != texts.len() {
        return false;
    }

    for (term, list) in &index.terms {
        // Check non-empty
        if list.postings.is_empty() {
            return false;
        }

        // Check sortedness
        for i in 1..list.postings.len() {
            let prev = &list.postings[i - 1];
            let curr = &list.postings[i];
            if (prev.doc_id, prev.offset) >= (curr.doc_id, curr.offset) {
                return false;
            }
        }

        // Check doc_freq
        let mut doc_ids: Vec<usize> = list.postings.iter().map(|p| p.doc_id).collect();
        doc_ids.sort();
        doc_ids.dedup();
        if list.doc_freq != doc_ids.len() {
            return false;
        }

        // Check posting bounds
        let term_len = term.len();
        for posting in &list.postings {
            if posting.doc_id >= texts.len() {
                return false;
            }
            if posting.offset + term_len > texts[posting.doc_id].len() {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let tokens = tokenize("hello world");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].0, "hello");
        assert_eq!(tokens[0].1, 0);
        assert_eq!(tokens[1].0, "world");
        assert_eq!(tokens[1].1, 6);
    }

    #[test]
    fn test_tokenize_with_punctuation() {
        let tokens = tokenize("hello, world!");
        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].0, "hello");
        assert_eq!(tokens[1].0, "world");
    }

    #[test]
    fn test_tokenize_normalizes() {
        let tokens = tokenize("Hello WORLD");
        assert_eq!(tokens[0].0, "hello");
        assert_eq!(tokens[1].0, "world");
    }

    #[test]
    fn test_build_inverted_index_simple() {
        let texts = vec!["hello world".to_string(), "world peace".to_string()];
        let index = build_inverted_index(&texts, &[]);

        // Check "hello" appears in doc 0 only
        let hello = index.terms.get("hello").unwrap();
        assert_eq!(hello.postings.len(), 1);
        assert_eq!(hello.postings[0].doc_id, 0);
        assert_eq!(hello.doc_freq, 1);

        // Check "world" appears in both docs
        let world = index.terms.get("world").unwrap();
        assert_eq!(world.postings.len(), 2);
        assert_eq!(world.doc_freq, 2);
    }

    #[test]
    fn test_posting_list_sorted() {
        let texts = vec![
            "rust is great".to_string(),
            "rust programming".to_string(),
            "great rust code".to_string(),
        ];
        let index = build_inverted_index(&texts, &[]);

        let rust = index.terms.get("rust").unwrap();
        // Check sorted by doc_id
        for i in 1..rust.postings.len() {
            assert!(rust.postings[i - 1].doc_id <= rust.postings[i].doc_id);
        }
    }

    #[test]
    fn test_index_mode_selection_small() {
        let thresholds = IndexThresholds::default();
        let mode = select_index_mode(10, 5000, false, false, &thresholds);
        assert_eq!(mode, IndexMode::SuffixArrayOnly);
    }

    #[test]
    fn test_index_mode_selection_large_exact() {
        let thresholds = IndexThresholds::default();
        let mode = select_index_mode(1000, 1_000_000, false, false, &thresholds);
        assert_eq!(mode, IndexMode::InvertedIndexOnly);
    }

    #[test]
    fn test_index_mode_selection_large_fuzzy() {
        let thresholds = IndexThresholds::default();
        let mode = select_index_mode(1000, 1_000_000, false, true, &thresholds);
        assert_eq!(mode, IndexMode::Hybrid);
    }

    #[test]
    fn test_check_inverted_index_well_formed() {
        let texts = vec!["hello world".to_string()];
        let index = build_inverted_index(&texts, &[]);
        assert!(check_inverted_index_well_formed(&index, &texts));
    }

    #[test]
    fn test_build_unified_index_suffix_only() {
        let docs = vec![crate::SearchDoc {
            id: 0,
            title: "Test".to_string(),
            excerpt: "".to_string(),
            href: "/test".to_string(),
            kind: "post".to_string(),
            category: None,
            author: None,
            tags: vec![],
        }];
        let texts = vec!["hello world".to_string()];
        let thresholds = IndexThresholds::default();

        let unified = build_unified_index(docs, texts, vec![], &thresholds, false, false);
        assert_eq!(unified.mode, IndexMode::SuffixArrayOnly);
        assert!(unified.suffix_array.is_some());
        assert!(unified.inverted_index.is_none());
    }
}
