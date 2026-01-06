//! Core type definitions for the Sieve search index.
//!
//! # Lean Correspondence
//!
//! These types correspond to specifications in `SearchVerified/Types.lean`:
//! - `SearchDoc` → `SearchDoc` structure
//! - `FieldBoundary` → `FieldBoundary` structure
//! - `FieldType` → `FieldType` inductive
//! - `SuffixEntry` → `SuffixEntry` structure
//! - `SearchIndex` → `SearchIndex` structure
//!
//! # INVARIANTS
//!
//! These types have well-formedness conditions that MUST be maintained:
//!
//! - **SuffixEntry**: `doc_id < texts.len() ∧ offset < texts[doc_id].len()`
//! - **SearchIndex**: `docs.len() = texts.len() ∧ lcp.len() = suffix_array.len()`
//! - **FieldBoundary**: `doc_id < texts.len() ∧ start < end ∧ end ≤ texts[doc_id].len()`
//!
//! Use `ValidatedSuffixEntry` and `WellFormedIndex` from `verified.rs` for
//! compile-time enforcement of these invariants.

use serde::{Deserialize, Serialize};

#[cfg(feature = "lean")]
use sieve_lean_macros::{LeanProptest, LeanSpec};

/// Document metadata for search results.
///
/// **Lean Specification**: `SearchDoc` in `Types.lean`
/// - `id`: indexes into the texts array
/// - All string fields are searchable metadata
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

/// Field type within a document.
///
/// **Lean Specification**: `FieldType` in `Types.lean`
/// - Title: highest priority (base score 100)
/// - Heading: medium priority (base score 10)
/// - Content: lowest priority (base score 1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum FieldType {
    Title,
    Heading,
    Content,
}

/// Field boundary within a document text.
///
/// **Lean Specification**: `FieldBoundary` in `Types.lean`
/// - Well-formedness: `start < end ∧ end ≤ text_len`
/// - Represents a contiguous region of a specific field type
/// - `section_id`: Optional section ID for deep linking (None for title, Some for heading/content)
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
}

/// Suffix array entry pointing into document text.
///
/// **Lean Specification**: `SuffixEntry` in `Types.lean`
/// - Well-formedness: `doc_id < texts.size ∧ offset < texts[doc_id].length`
/// - The suffix array is sorted lexicographically by `suffixAt texts entry`
/// - This enables O(log n) binary search for prefix matching
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

/// The complete search index containing suffix array and metadata.
///
/// **Lean Specification**: `SearchIndex` in `Types.lean`
/// - Well-formedness: `docs.size = texts.size ∧ lcp.size = suffix_array.size`
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

/// A posting in the inverted index: location where a term occurs.
///
/// **Lean Specification**: `Posting` in `InvertedIndex.lean`
/// - Well-formedness: `doc_id < texts.size ∧ offset + term_len ≤ texts[doc_id].length`
/// - Postings for the same term are sorted by (doc_id, offset) for efficient intersection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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
}

/// A posting list for a single term: all documents containing that term.
///
/// **Lean Specification**: `PostingList` in `InvertedIndex.lean`
/// - Well-formedness: `∀ p ∈ postings. Posting.WellFormed p texts`
/// - Invariant: postings are sorted by (doc_id, offset)
/// - Document frequency is always equal to the count of unique doc_ids
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "lean", derive(LeanSpec))]
#[cfg_attr(
    feature = "lean",
    lean(
        name = "PostingList",
        invariant = "postings_sorted ∧ doc_freq = unique_doc_ids.size"
    )
)]
#[serde(rename_all = "camelCase")]
pub struct PostingList {
    /// Sorted list of postings (by doc_id, then offset)
    pub postings: Vec<Posting>,
    /// Number of unique documents containing this term
    pub doc_freq: usize,
}

/// The inverted index: maps terms to their posting lists.
///
/// **Lean Specification**: `InvertedIndex` in `InvertedIndex.lean`
/// - Well-formedness: `∀ (term, pl) ∈ index. PostingList.WellFormed pl texts ∧ term_exists_in_docs`
/// - This enables O(1) term lookup and efficient boolean queries
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

/// Hybrid search index: inverted index + suffix array over vocabulary.
///
/// This combines the best of both approaches:
/// - **Exact word lookup**: O(1) via inverted index hash map
/// - **Prefix search**: O(log k) via suffix array over vocabulary keys
/// - **Fuzzy search**: O(vocabulary) via Levenshtein distance iteration
/// - **Posting list intersection**: Efficient AND queries
///
/// The key insight is that the suffix array is built over the vocabulary
/// (unique terms) rather than the full text. For a typical blog:
/// - Full text: ~500KB (would need ~500K suffix entries)
/// - Vocabulary: ~10K unique words (needs only ~50K suffix entries)
///
/// **Lean Specification**: `HybridIndex` in `HybridIndex.lean`
/// - Invariant: All vocabulary terms exist as keys in the inverted index
/// - Invariant: vocab_suffix_array is sorted lexicographically
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
    /// The matched document
    pub doc: SearchDoc,
    /// Which index the match came from
    pub source: SearchSource,
    /// Relevance score (higher is better)
    pub score: f64,
    /// Section ID for deep linking (None for title matches, Some for heading/content)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section_id: Option<String>,
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

/// A section represents a heading and its content region in a document.
///
/// **Lean Specification**: `Section` in `Section.lean`
/// - Well-formedness: `start_offset < end_offset` (non-empty region)
/// - Invariant: Sections in a document are non-overlapping
/// - Invariant: Every text offset maps to exactly one section
///
/// Used for deep-linking search results to specific headings within a document.
/// For example, searching for "optimization" might link to `/posts/2024/03/rust-search#optimization`
/// instead of just `/posts/2024/03/rust-search`.
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
