// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! The building blocks of a search index.
//!
//! These types define how documents, field boundaries, and suffix entries fit together.
//! Every struct here has a corresponding Lean specification in `SearchVerified/Types.lean`,
//! so if something seems overly constrained, there's probably a theorem depending on it.
//!
//! # Lean Correspondence
//!
//! | Rust Type        | Lean Type        | Purpose                          |
//! |------------------|------------------|----------------------------------|
//! | `SearchDoc`      | `SearchDoc`      | Document metadata for results    |
//! | `FieldBoundary`  | `FieldBoundary`  | Title/heading/content regions    |
//! | `FieldType`      | `FieldType`      | Title > Heading > Content        |
//! | `SuffixEntry`    | `SuffixEntry`    | Pointer into the suffix array    |
//! | `SearchIndex`    | `SearchIndex`    | The complete searchable index    |
//!
//! # Invariants (the stuff that breaks if you ignore it)
//!
//! - **SuffixEntry**: `doc_id < texts.len() ∧ offset < texts[doc_id].len()`
//!   Every suffix points somewhere valid. Strict inequality because suffixes are non-empty.
//!
//! - **SearchIndex**: `docs.len() = texts.len() ∧ lcp.len() = suffix_array.len()`
//!   The arrays must line up. Off-by-one here means garbage results.
//!
//! - **FieldBoundary**: `doc_id < texts.len() ∧ start < end ∧ end ≤ texts[doc_id].len()`
//!   Non-empty, non-overlapping regions within valid documents.
//!
//! Rather than trusting yourself to remember these, use `ValidatedSuffixEntry` and
//! `WellFormedIndex` from `verify` - they enforce invariants at the type level.

use serde::{Deserialize, Serialize};

#[cfg(feature = "lean")]
use sorex_lean_macros::{LeanProptest, LeanSpec};

// =============================================================================
// NEWTYPES: Type-safe indices and offsets
// =============================================================================

/// Type-safe document identifier.
///
/// Prevents accidentally passing a character offset where a document ID is expected.
/// Use `DocId::new()` for runtime-validated construction, or `.into()` for trusted sources.
///
/// **Lean Correspondence**: `doc_id` fields in `Types.lean`
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct DocId(pub u32);

impl DocId {
    /// Create a new DocId, validating it's within bounds.
    #[inline]
    pub fn new(id: u32, num_docs: usize) -> Option<Self> {
        if (id as usize) < num_docs {
            Some(DocId(id))
        } else {
            None
        }
    }

    /// Get the underlying value.
    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }

    /// Convert to usize for array indexing.
    #[inline]
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for DocId {
    fn from(id: u32) -> Self {
        DocId(id)
    }
}

impl From<DocId> for usize {
    fn from(id: DocId) -> Self {
        id.0 as usize
    }
}

/// Character offset within normalized document text.
///
/// This is an offset into the Unicode scalar values of the text, NOT a byte offset.
/// The distinction matters for UTF-8 text where byte position ≠ character position.
///
/// **Lean Correspondence**: `offset` fields in `Types.lean`
/// **Invariant**: `offset < text.chars().count()` (for non-empty suffixes)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct CharOffset(pub u32);

impl CharOffset {
    /// Create a new CharOffset, validating it's within text bounds.
    #[inline]
    pub fn new(offset: u32, text_len: usize) -> Option<Self> {
        if (offset as usize) < text_len {
            Some(CharOffset(offset))
        } else {
            None
        }
    }

    /// Get the underlying value.
    #[inline]
    pub fn get(self) -> u32 {
        self.0
    }

    /// Convert to usize for string slicing.
    #[inline]
    pub fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl From<u32> for CharOffset {
    fn from(offset: u32) -> Self {
        CharOffset(offset)
    }
}

impl From<CharOffset> for usize {
    fn from(offset: CharOffset) -> Self {
        offset.0 as usize
    }
}

/// Byte offset within UTF-8 encoded text.
///
/// This is a raw byte position in the UTF-8 representation.
/// Must be used with care to ensure it falls on a valid UTF-8 boundary.
///
/// Prefer `CharOffset` for search-related operations since the suffix array
/// and field boundaries work with character positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(transparent)]
pub struct ByteOffset(pub usize);

impl ByteOffset {
    /// Create a new ByteOffset, validating it's within byte bounds.
    #[inline]
    pub fn new(offset: usize, byte_len: usize) -> Option<Self> {
        if offset <= byte_len {
            Some(ByteOffset(offset))
        } else {
            None
        }
    }

    /// Get the underlying value.
    #[inline]
    pub fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for ByteOffset {
    fn from(offset: usize) -> Self {
        ByteOffset(offset)
    }
}

impl From<ByteOffset> for usize {
    fn from(offset: ByteOffset) -> Self {
        offset.0
    }
}

// =============================================================================
// DOCUMENT TYPES
// =============================================================================

/// What users see when they get a search result.
///
/// The `id` field indexes into the texts array - everything else is metadata
/// for displaying and filtering results. We keep this lean (pun intended)
/// because it gets serialized into the index and loaded on every search.
///
/// **Lean Specification**: `SearchDoc` in `Types.lean`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "lean", derive(LeanSpec, LeanProptest))]
#[cfg_attr(feature = "lean", lean(name = "SearchDoc"))]
#[serde(rename_all = "camelCase")]
pub struct SearchDoc {
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..100"))]
    pub id: usize,
    #[cfg_attr(feature = "lean", lean(pattern = "[a-zA-Z0-9 ]{1,50}"))]
    pub title: String,
    #[cfg_attr(feature = "lean", lean(pattern = "[a-zA-Z0-9 ]{0,100}"))]
    pub excerpt: String,
    #[cfg_attr(feature = "lean", lean(pattern = "/[a-z0-9/-]{1,50}"))]
    pub href: String,
    #[serde(rename = "type")]
    #[cfg_attr(
        feature = "lean",
        lean(strategy = "proptest::strategy::Just(\"post\".to_string())")
    )]
    pub kind: String,
    /// Category for client-side filtering (e.g., "engineering", "adventures")
    #[serde(default)]
    pub category: Option<String>,
    /// Author name (for multi-author blogs)
    #[serde(default)]
    pub author: Option<String>,
    /// Tags/labels for categorization
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Where in a document did the match occur?
///
/// Title matches beat heading matches beat content matches. The gap between
/// tiers is deliberately large (100 vs 10 vs 1) so position bonuses can't
/// accidentally promote a content match above a title match.
///
/// **Lean Specification**: `FieldType` in `Types.lean`
/// - Theorem `title_beats_heading` proves the gap is sufficient
///
/// **Gotcha**: The derived `Ord` is lexicographic (Title < Heading < Content),
/// which is backwards from score order. Don't use `Ord` for ranking - use
/// `field_type_score()` instead. We keep `Ord` for deterministic serialization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Title,
    Heading,
    Content,
}

impl FieldType {
    /// Convert to lowercase string representation.
    ///
    /// Matches the serde `rename_all = "lowercase"` convention.
    pub fn as_str(&self) -> &'static str {
        match self {
            FieldType::Title => "title",
            FieldType::Heading => "heading",
            FieldType::Content => "content",
        }
    }
}

/// Hierarchical bucket for ranking based on where in the document structure a match occurred.
///
/// This is the primary sort key for results. Within each bucket, numeric scores
/// break ties, but a Section match will never outrank a Title match regardless
/// of how good the content score looks.
///
/// The hierarchy: Title > Section > Subsection > Subsubsection > Content
///
/// **Lean Specification**: `MatchType` in `SearchVerified/MatchType.lean`
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    Title,         // Document title (heading_level=0, e.g. <title> or front-matter)
    Section,       // h1 or h2 heading (heading_level=1-2)
    Subsection,    // h3 heading (heading_level=3)
    Subsubsection, // h4 heading (heading_level=4)
    Content,       // h5+, or implicit content section (heading_level=5+)
}

impl MatchType {
    /// Convert heading level (0-6) to MatchType bucket.
    ///
    /// - 0: Document title (highest rank) - the `<title>` or front-matter title
    /// - 1-2: Section heading (h1/h2) - major section headings
    /// - 3: Subsection (h3)
    /// - 4: Subsubsection (h4)
    /// - 5+: Content (lowest rank)
    ///
    /// This ensures document titles always rank above content headings,
    /// even if both are h1-level structurally.
    #[inline]
    pub fn from_heading_level(level: u8) -> Self {
        match level {
            0 => MatchType::Title,
            1 | 2 => MatchType::Section,
            3 => MatchType::Subsection,
            4 => MatchType::Subsubsection,
            _ => MatchType::Content,
        }
    }

    /// Convert MatchType to numeric value for JavaScript serialization.
    /// - 0: Title
    /// - 1: Section
    /// - 2: Subsection
    /// - 3: Subsubsection
    /// - 4: Content
    pub fn to_u8(self) -> u8 {
        match self {
            MatchType::Title => 0,
            MatchType::Section => 1,
            MatchType::Subsection => 2,
            MatchType::Subsubsection => 3,
            MatchType::Content => 4,
        }
    }
}

/// A contiguous region of text with a specific field type.
///
/// Documents are divided into non-overlapping boundaries: title, headings, and content.
/// Each boundary knows its byte range and can optionally link to a section anchor for
/// deep linking (so search results can jump directly to `#optimization` instead of
/// just the page).
///
/// **Lean Specification**: `FieldBoundary` in `Types.lean`
/// - Invariant: `start < end ∧ end ≤ text_len` (non-empty, in bounds)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "lean", derive(LeanSpec, LeanProptest))]
#[cfg_attr(
    feature = "lean",
    lean(name = "FieldBoundary", invariant = "start < end")
)]
#[serde(rename_all = "camelCase")]
pub struct FieldBoundary {
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..100"))]
    pub doc_id: usize,
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..1000"))]
    pub start: usize,
    #[cfg_attr(feature = "lean", lean(bounds = "1usize..1001"))]
    pub end: usize,
    #[cfg_attr(
        feature = "lean",
        lean(
            strategy = "proptest::sample::select(vec![FieldType::Title, FieldType::Heading, FieldType::Content])"
        )
    )]
    pub field_type: FieldType,
    /// Section ID for deep linking to headings
    /// - None: title field (links to top of page)
    /// - Some: heading/content (links to #section-id anchor)
    ///
    /// **Lean Specification**: `section_id` field in `Types.lean`
    /// **Verified by**: `prop_title_has_no_section_id`, `prop_content_inherits_section`
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// Heading level for hierarchical ranking (v8 bucketing)
    /// - 0 or 1: Title/h1
    /// - 2: h2 section
    /// - 3: h3 subsection
    /// - 4: h4 subsubsection
    /// - 5+: h5+ / content
    ///
    /// Populated by: HarryZorus build pipeline (src/build/scripts/search.ts)
    /// Default: 0 (title level, for backward compatibility)
    #[serde(default)]
    #[cfg_attr(feature = "lean", lean(bounds = "0u8..10"))]
    pub heading_level: u8,
}

/// A pointer to a suffix in the document corpus.
///
/// Every position in every document gets a suffix entry. When sorted lexicographically,
/// these form a suffix array that enables O(log n) binary search for any prefix.
/// It's a classic data structure, but the "classic" papers don't mention how annoying
/// the edge cases are.
///
/// **Lean Specification**: `SuffixEntry` in `Types.lean`
/// - Invariant: `doc_id < texts.size ∧ offset < texts[doc_id].length`
/// - Strict `<` for offset because we index non-empty suffixes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "lean", derive(LeanSpec, LeanProptest))]
#[cfg_attr(
    feature = "lean",
    lean(
        name = "SuffixEntry",
        invariant = "doc_id < texts.size ∧ offset < texts[doc_id].length"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct SuffixEntry {
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..100"))]
    pub doc_id: usize,
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..10000"))]
    pub offset: usize,
}

/// The complete search index: suffix array, LCP array, docs, and field boundaries.
///
/// This is the basic index type - good for learning the codebase or small datasets.
/// For production use, consider `HybridIndex` (adds fuzzy search) or `UnionIndex`
/// (multi-site search).
///
/// **Lean Specification**: `SearchIndex` in `Types.lean`
/// - Invariant: `docs.size = texts.size ∧ lcp.size = suffix_array.size`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "lean", derive(LeanSpec))]
#[cfg_attr(
    feature = "lean",
    lean(
        name = "SearchIndex",
        invariant = "docs.size = texts.size ∧ lcp.size = suffix_array.size"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndex {
    pub docs: Vec<SearchDoc>,
    pub texts: Vec<String>,
    pub suffix_array: Vec<SuffixEntry>,
    pub lcp: Vec<usize>,
    pub field_boundaries: Vec<FieldBoundary>,
    /// Index format version (default: 4)
    #[serde(default = "default_version")]
    pub version: u32,
}

fn default_version() -> u32 {
    4
}

/// A scored document result (internal use).
#[derive(Debug, Clone)]
pub(crate) struct ScoredDoc {
    pub doc: SearchDoc,
    pub score: f64,
}

// =============================================================================
// INVERTED INDEX TYPES (Hybrid Search Extension)
// =============================================================================

/// A single occurrence of a term in the corpus.
///
/// Every time a word appears, we record where (doc, offset), what kind of field
/// (title/heading/content), and precompute a score. Postings are sorted by score
/// descending, so retrieving the top-k results for a single term is O(k).
///
/// **Lean Specification**: `Posting` in `InvertedIndex.lean`
/// - Invariant: `doc_id < texts.size ∧ offset + term_len ≤ texts[doc_id].length`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[cfg_attr(feature = "lean", derive(LeanSpec, LeanProptest))]
#[cfg_attr(
    feature = "lean",
    lean(
        name = "Posting",
        invariant = "doc_id < texts.size ∧ offset + term_len ≤ texts[doc_id].length"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct Posting {
    /// Document containing this term occurrence
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..100"))]
    pub doc_id: usize,
    /// Character offset within the document text
    #[cfg_attr(feature = "lean", lean(bounds = "0usize..10000"))]
    pub offset: usize,
    /// Field type at this position (for scoring)
    #[cfg_attr(
        feature = "lean",
        lean(
            strategy = "proptest::sample::select(vec![FieldType::Title, FieldType::Heading, FieldType::Content])"
        )
    )]
    pub field_type: FieldType,
    /// Section ID for deep linking (None for title, Some for heading/content)
    /// Looked up from FieldBoundary.section_id at index time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// Heading level for hierarchical ranking (v8 bucketing)
    /// - 0 or 1: Title/h1
    /// - 2: h2 section
    /// - 3: h3 subsection
    /// - 4: h4 subsubsection
    /// - 5+: h5+ / content
    ///
    /// Copied from FieldBoundary.heading_level at index time.
    #[serde(default)]
    #[cfg_attr(feature = "lean", lean(bounds = "0u8..10"))]
    pub heading_level: u8,
    /// Precomputed relevance score for fast top-k retrieval.
    /// Computed at index time from field_type and position_bonus.
    /// Posting lists are sorted by score DESC for O(k) single-term queries.
    #[serde(default)]
    pub score: f64,
}

/// All occurrences of a single term across the corpus.
///
/// Sorted by score descending for O(k) top-k retrieval. The `doc_freq` is cached
/// because counting unique doc_ids in a sorted list is surprisingly expensive
/// when you're doing it thousands of times per search.
///
/// **Lean Specification**: `PostingList` in `InvertedIndex.lean`
/// - Invariant: postings sorted by (score DESC, doc_id ASC)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "lean", derive(LeanSpec))]
#[cfg_attr(
    feature = "lean",
    lean(
        name = "PostingList",
        invariant = "postings_sorted_by_score ∧ doc_freq = unique_doc_ids.size"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct PostingList {
    /// Sorted list of postings by (score DESC, doc_id ASC) for O(k) top-k retrieval
    pub postings: Vec<Posting>,
    /// Number of unique documents containing this term
    pub doc_freq: usize,
}

/// The inverted index: term → posting list.
///
/// O(1) exact term lookup via HashMap. This is the workhorse for single-word queries
/// and the foundation for boolean AND/OR. The `total_docs` field is cached for
/// IDF calculations.
///
/// **Lean Specification**: `InvertedIndex` in `InvertedIndex.lean`
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "lean", derive(LeanSpec))]
#[cfg_attr(
    feature = "lean",
    lean(
        name = "InvertedIndex",
        invariant = "∀ (term, pl). term_exists ∧ PostingList.WellFormed pl"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct InvertedIndex {
    /// Map from normalized term to posting list
    #[serde(flatten)]
    pub terms: std::collections::HashMap<String, PostingList>,
    /// Total number of documents indexed
    pub total_docs: usize,
}

/// Index mode selected at build time based on content characteristics.
///
/// **Decision criteria** (evaluated at build time):
/// - `SuffixArrayOnly`: Small blogs (<100 docs), need prefix/fuzzy matching
/// - `InvertedIndexOnly`: Large blogs, primarily exact word matching
/// - `Hybrid`: Large blogs needing both capabilities
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub enum IndexMode {
    /// Use only suffix array (current behavior, good for prefix/fuzzy)
    #[default]
    SuffixArrayOnly,
    /// Use only inverted index (O(1) exact word lookup)
    InvertedIndexOnly,
    /// Use both indexes (suffix for prefix, inverted for exact)
    Hybrid,
}

/// Unified search index that can operate in different modes.
///
/// At build time, we analyze content characteristics and choose the best mode:
/// - Small blogs: SuffixArrayOnly (simpler, good enough)
/// - Large blogs with exact queries: InvertedIndexOnly (faster word lookup)
/// - Large blogs with mixed queries: Hybrid (best of both)
///
/// **Lean Specification**: `UnifiedIndex` in `InvertedIndex.lean`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "lean", derive(LeanSpec))]
#[cfg_attr(
    feature = "lean",
    lean(name = "UnifiedIndex", invariant = "mode_matches_available_indexes")
)]
#[serde(rename_all = "camelCase")]
pub struct UnifiedIndex {
    /// Document metadata (shared between both indexes)
    pub docs: Vec<SearchDoc>,
    /// Document texts (shared between both indexes)
    pub texts: Vec<String>,
    /// Field boundaries (shared)
    pub field_boundaries: Vec<FieldBoundary>,
    /// Mode selected at build time
    pub mode: IndexMode,
    /// Suffix array (populated in SuffixArrayOnly or Hybrid mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix_array: Option<Vec<SuffixEntry>>,
    /// LCP array for suffix array (populated when suffix_array is Some)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lcp: Option<Vec<usize>>,
    /// Inverted index (populated in InvertedIndexOnly or Hybrid mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inverted_index: Option<InvertedIndex>,
}

// =============================================================================
// HYBRID INDEX: Suffix Array over Vocabulary Keys
// =============================================================================

/// Entry in the vocabulary suffix array.
///
/// Points to a position within a term in the vocabulary.
/// This enables O(log k) prefix search over the vocabulary,
/// where k = number of unique terms (typically much smaller than full text).
///
/// **Lean Specification**: `VocabSuffixEntry` in `HybridIndex.lean`
/// - Well-formedness: `term_idx < vocabulary.size ∧ offset < vocabulary[term_idx].length`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VocabSuffixEntry {
    /// Index into the vocabulary array
    pub term_idx: usize,
    /// Character offset within the term
    pub offset: usize,
}

/// The production search index: inverted index + vocabulary suffix array.
///
/// Combines the best of both worlds:
/// - **Exact**: O(1) via inverted index HashMap
/// - **Prefix**: O(log k) via suffix array over vocabulary terms
/// - **Fuzzy**: O(vocabulary) via Levenshtein DFA traversal
///
/// The trick is building the suffix array over vocabulary terms (10K unique words)
/// instead of full text (500KB). Same algorithmic complexity, 100x smaller index.
///
/// **Lean Specification**: `HybridIndex` in `HybridIndex.lean`
/// - Invariant: All vocabulary terms exist in the inverted index
/// - Invariant: `vocab_suffix_array` is sorted lexicographically
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HybridIndex {
    /// Document metadata
    pub docs: Vec<SearchDoc>,
    /// Document texts (needed for snippet extraction)
    pub texts: Vec<String>,
    /// Field boundaries for scoring
    pub field_boundaries: Vec<FieldBoundary>,
    /// Inverted index: term → posting list (for O(1) exact lookup)
    pub inverted_index: InvertedIndex,
    /// Vocabulary: all unique terms (sorted for binary search fallback)
    pub vocabulary: Vec<String>,
    /// Suffix array over vocabulary (for O(log k) prefix search)
    /// Each entry points to a suffix of a vocabulary term
    pub vocab_suffix_array: Vec<VocabSuffixEntry>,
}

// =============================================================================
// UNION INDEX: Multiple Indexes by Content Type
// =============================================================================

/// Source of a search result (which index it came from).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum SearchSource {
    /// Match found in post/page title
    Title,
    /// Match found in section heading (h2, h3, etc.)
    Heading,
    /// Match found in body content
    Content,
}

/// A search result with its source and score.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    /// Document ID (for efficient storage - avoids cloning SearchDoc)
    pub doc_id: usize,
    /// Which index the match came from
    pub source: SearchSource,
    /// Relevance score (higher is better)
    pub score: f64,
    /// Section ID for deep linking (None for title matches, Some for heading/content)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
    /// Search tier: 1=exact, 2=prefix, 3=fuzzy
    #[serde(default = "default_tier")]
    pub tier: u8,
}

fn default_tier() -> u8 {
    1
}

/// Union index combining separate indexes for titles, headings, and content.
///
/// This structure enables:
/// - **Faster searches**: Smaller indexes = fewer comparisons
/// - **Source attribution**: Know if match was in title vs content
/// - **Early termination**: Stop after finding title matches
/// - **Grouped results**: Display "Found in title" vs "Found in content"
///
/// Each sub-index is a HybridIndex (inverted index + vocabulary suffix array).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnionIndex {
    /// Shared document metadata (same across all indexes)
    pub docs: Vec<SearchDoc>,

    /// Index over post/page titles only
    /// Matches here get highest ranking boost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titles: Option<HybridIndex>,

    /// Index over section headings (h2, h3, etc.)
    /// Matches here get medium ranking boost
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headings: Option<HybridIndex>,

    /// Index over body content
    /// Matches here get base ranking
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HybridIndex>,
}

// =============================================================================
// SECTION NAVIGATION (Deep Linking)
// =============================================================================

/// A heading and its content region, for deep-linking search results.
///
/// Instead of linking to `/posts/rust-search`, we can link directly to
/// `/posts/rust-search#optimization`. Users land exactly where the match is,
/// not at the top of a 5000-word post.
///
/// Sections must be non-overlapping and cover the entire document. If that
/// sounds like a partition, it is - and the Lean proofs verify it.
///
/// **Lean Specification**: `Section` in `Section.lean`
/// - Invariant: `start_offset < end_offset` (non-empty)
/// - Invariant: Sections are non-overlapping and cover [0, doc_length)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Section {
    /// Heading ID for anchor links (e.g., "introduction", "performance-optimization")
    /// Generated by rehype-slug from the heading text
    pub id: String,
    /// Starting character offset in the document (inclusive)
    pub start_offset: usize,
    /// Ending character offset (exclusive) - where next section starts or EOF
    pub end_offset: usize,
    /// Heading level (1-6 for h1-h6, 0 for implicit top section before first heading)
    pub level: u8,
}

impl Section {
    /// Check if an offset falls within this section.
    ///
    /// **Lean Specification**: `Section.contains` in `Section.lean`
    #[inline]
    pub fn contains(&self, offset: usize) -> bool {
        self.start_offset <= offset && offset < self.end_offset
    }

    /// Check if this section is well-formed (non-empty).
    ///
    /// **Lean Specification**: `Section.WellFormed` in `Section.lean`
    #[inline]
    pub fn is_well_formed(&self) -> bool {
        self.start_offset < self.end_offset
    }

    /// Check if two sections are non-overlapping.
    ///
    /// **Lean Specification**: `Section.NonOverlapping` in `Section.lean`
    pub fn non_overlapping(&self, other: &Section) -> bool {
        self.end_offset <= other.start_offset || other.end_offset <= self.start_offset
    }

    /// Check if a section ID is valid for use as a URL anchor.
    ///
    /// Valid characters: alphanumeric, hyphen, underscore
    /// Generated IDs from rehype-slug should always be valid.
    ///
    /// **Lean Specification**: `Section.validId` in `Section.lean`
    pub fn is_valid_id(&self) -> bool {
        !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    }
}

/// Validate that a list of sections satisfies all invariants.
///
/// **Lean Specification**: `validSectionList` in `Section.lean`
/// - All sections are well-formed
/// - All pairs of sections are non-overlapping
/// - Sections are sorted by start offset
pub fn validate_sections(sections: &[Section], doc_length: usize) -> Result<(), String> {
    // Check all sections are well-formed
    for (i, section) in sections.iter().enumerate() {
        if !section.is_well_formed() {
            return Err(format!(
                "Section {} is not well-formed: start={} >= end={}",
                i, section.start_offset, section.end_offset
            ));
        }
        if !section.is_valid_id() {
            return Err(format!("Section {} has invalid ID: '{}'", i, section.id));
        }
    }

    // Check sortedness and non-overlapping
    for i in 1..sections.len() {
        if sections[i - 1].start_offset > sections[i].start_offset {
            return Err(format!(
                "Sections not sorted: section {} starts at {} but section {} starts at {}",
                i - 1,
                sections[i - 1].start_offset,
                i,
                sections[i].start_offset
            ));
        }
        if !sections[i - 1].non_overlapping(&sections[i]) {
            return Err(format!(
                "Sections overlap: section {} [{}, {}) and section {} [{}, {})",
                i - 1,
                sections[i - 1].start_offset,
                sections[i - 1].end_offset,
                i,
                sections[i].start_offset,
                sections[i].end_offset
            ));
        }
    }

    // Check coverage if sections exist
    if !sections.is_empty() {
        if sections[0].start_offset != 0 {
            return Err(format!(
                "First section doesn't start at 0: starts at {}",
                sections[0].start_offset
            ));
        }
        if sections.last().unwrap().end_offset != doc_length {
            return Err(format!(
                "Last section doesn't end at doc_length: ends at {} but doc_length is {}",
                sections.last().unwrap().end_offset,
                doc_length
            ));
        }
    }

    Ok(())
}

/// Find the section containing a given offset.
///
/// Returns the section ID if found, or None if offset is out of bounds
/// or no sections cover that position.
///
/// **Lean Specification**: Follows from `offset_maps_to_unique_section` theorem
/// in `Section.lean` - given non-overlapping sections, at most one section
/// contains any given offset.
pub fn find_section_at_offset(sections: &[Section], offset: usize) -> Option<&str> {
    // Binary search since sections are sorted by start_offset
    let idx = sections.partition_point(|s| s.start_offset <= offset);
    if idx > 0 {
        let section = &sections[idx - 1];
        if section.contains(offset) {
            return Some(&section.id);
        }
    }
    None
}
