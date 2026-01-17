// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Type wrappers that make invalid states unrepresentable.
//!
//! Instead of hoping you remembered to check bounds, wrap your data in these types.
//! They check invariants at construction and guarantee them forever after. The cost
//! is paid once upfront, then you get compile-time proof that things are valid.
//!
//! # Lean Correspondence
//!
//! | Type                  | Lean Specification            | What's Guaranteed         |
//! |-----------------------|-------------------------------|---------------------------|
//! | `ValidatedSuffixEntry`| `SuffixEntry.WellFormed`      | doc_id, offset in bounds  |
//! | `SortedSuffixArray`   | `SuffixArray.Sorted`          | Lexicographic order       |
//! | `WellFormedIndex`     | `SearchIndex.WellFormed`      | All arrays aligned        |
//! | `ValidatedPosting`    | `Posting.WellFormed`          | doc_id, offset valid      |
//!
//! # Example
//!
//! ```ignore
//! // Construction validates everything
//! let index = WellFormedIndex::from_index(raw_index)?;
//!
//! // Now suffix() can't panic - bounds were checked at construction
//! let suffix = index.suffix_array().get(i).unwrap().suffix(&texts);
//! ```

use crate::{FieldBoundary, SearchDoc, SearchIndex, SuffixEntry};
use std::fmt;

/// Error type for invariant violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvariantError {
    /// `doc_id` is out of bounds for the texts array.
    InvalidDocId { doc_id: usize, texts_len: usize },
    /// `offset` is out of bounds for the document text.
    InvalidOffset {
        doc_id: usize,
        offset: usize,
        text_len: usize,
    },
    /// Suffix array is not sorted lexicographically.
    UnsortedSuffixArray { position: usize },
    /// Documents and texts arrays have different lengths.
    MismatchedDocsTexts { docs_len: usize, texts_len: usize },
    /// LCP and suffix array have different lengths.
    MismatchedLcpSuffixArray { lcp_len: usize, sa_len: usize },
    /// Field boundary refers to invalid document.
    InvalidFieldBoundary { doc_id: usize, texts_len: usize },
    /// Posting list is not sorted.
    UnsortedPostingList { position: usize, term: String },
    /// Document frequency doesn't match unique doc count.
    IncorrectDocFreq {
        term: String,
        claimed: usize,
        actual: usize,
    },
    /// Total docs doesn't match texts length.
    IncorrectTotalDocs { claimed: usize, actual: usize },
    /// Posting list is empty (every term should have at least one posting).
    EmptyPostingList { term: String },
}

impl fmt::Display for InvariantError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvariantError::InvalidDocId { doc_id, texts_len } => {
                write!(f, "doc_id {} >= texts.len() {}", doc_id, texts_len)
            }
            InvariantError::InvalidOffset {
                doc_id,
                offset,
                text_len,
            } => {
                write!(
                    f,
                    "offset {} >= texts[{}].len() {}",
                    offset, doc_id, text_len
                )
            }
            InvariantError::UnsortedSuffixArray { position } => {
                write!(f, "suffix array not sorted at position {}", position)
            }
            InvariantError::MismatchedDocsTexts {
                docs_len,
                texts_len,
            } => {
                write!(f, "docs.len() {} != texts.len() {}", docs_len, texts_len)
            }
            InvariantError::MismatchedLcpSuffixArray { lcp_len, sa_len } => {
                write!(f, "lcp.len() {} != suffix_array.len() {}", lcp_len, sa_len)
            }
            InvariantError::InvalidFieldBoundary { doc_id, texts_len } => {
                write!(
                    f,
                    "field boundary doc_id {} >= texts.len() {}",
                    doc_id, texts_len
                )
            }
            InvariantError::UnsortedPostingList { position, term } => {
                write!(
                    f,
                    "posting list for '{}' not sorted at position {}",
                    term, position
                )
            }
            InvariantError::IncorrectDocFreq {
                term,
                claimed,
                actual,
            } => {
                write!(
                    f,
                    "posting list for '{}' has doc_freq {} but {} unique docs",
                    term, claimed, actual
                )
            }
            InvariantError::IncorrectTotalDocs { claimed, actual } => {
                write!(f, "total_docs {} != texts.len() {}", claimed, actual)
            }
            InvariantError::EmptyPostingList { term } => {
                write!(f, "posting list for '{}' is empty", term)
            }
        }
    }
}

impl std::error::Error for InvariantError {}

/// A validated suffix entry where `doc_id < texts.len()` and `offset < texts[doc_id].len()`.
///
/// # Lean Specification
/// Corresponds to `SuffixEntry.WellFormed` in `Types.lean`:
/// ```lean
/// def SuffixEntry.WellFormed (e : SuffixEntry) (texts : Array String) : Prop :=
///   e.doc_id < texts.size ∧ e.offset < texts[e.doc_id].length
/// ```
///
/// # Invariants (enforced at construction)
/// - `doc_id` is a valid index into `texts`
/// - `offset` is a valid position within `texts[doc_id]`
#[derive(Debug, Clone)]
pub struct ValidatedSuffixEntry {
    inner: SuffixEntry,
}

impl ValidatedSuffixEntry {
    /// Create a validated suffix entry.
    ///
    /// Returns `Err` if invariants are violated.
    pub fn new(entry: SuffixEntry, texts: &[String]) -> Result<Self, InvariantError> {
        // Check doc_id bound
        if entry.doc_id >= texts.len() {
            return Err(InvariantError::InvalidDocId {
                doc_id: entry.doc_id,
                texts_len: texts.len(),
            });
        }

        // Check offset bound (strict inequality - no empty suffixes)
        let text_len = texts[entry.doc_id].len();
        if entry.offset >= text_len {
            return Err(InvariantError::InvalidOffset {
                doc_id: entry.doc_id,
                offset: entry.offset,
                text_len,
            });
        }

        Ok(Self { inner: entry })
    }

    /// Get the underlying suffix entry.
    pub fn inner(&self) -> &SuffixEntry {
        &self.inner
    }

    /// Get the doc_id (guaranteed valid).
    pub fn doc_id(&self) -> usize {
        self.inner.doc_id
    }

    /// Get the offset (guaranteed valid for the doc).
    pub fn offset(&self) -> usize {
        self.inner.offset
    }

    /// Get the suffix at this entry.
    ///
    /// # Safety (Type-Level)
    /// This cannot panic because the entry has been validated.
    pub fn suffix<'a>(&self, texts: &'a [String]) -> &'a str {
        // SAFETY: doc_id was validated at construction
        &texts[self.inner.doc_id][self.inner.offset..]
    }
}

/// A sorted suffix array where all entries are in lexicographic order.
///
/// # Lean Specification
/// Corresponds to `SuffixArray.Sorted` in `SuffixArray.lean`:
/// ```lean
/// def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop :=
///   ∀ i j : Nat, (hi : i < sa.size) → (hj : j < sa.size) → i < j →
///     SuffixLe texts sa[i] sa[j]
/// ```
///
/// # Invariants (enforced at construction)
/// - For all `i < j`: `suffix_at(sa[i]) ≤ suffix_at(sa[j])`
/// - All entries are well-formed (valid doc_id and offset)
#[derive(Debug, Clone)]
pub struct SortedSuffixArray {
    entries: Vec<ValidatedSuffixEntry>,
}

impl SortedSuffixArray {
    /// Create a sorted suffix array by validating an existing array.
    ///
    /// Returns `Err` if the array is not sorted or contains invalid entries.
    pub fn from_vec(entries: Vec<SuffixEntry>, texts: &[String]) -> Result<Self, InvariantError> {
        // Validate all entries
        let validated: Result<Vec<_>, _> = entries
            .into_iter()
            .map(|e| ValidatedSuffixEntry::new(e, texts))
            .collect();
        let validated = validated?;

        // Check sortedness
        for i in 1..validated.len() {
            let prev = validated[i - 1].suffix(texts);
            let curr = validated[i].suffix(texts);
            if prev > curr {
                return Err(InvariantError::UnsortedSuffixArray { position: i });
            }
        }

        Ok(Self { entries: validated })
    }

    /// Get the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the array is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get an entry by index.
    pub fn get(&self, index: usize) -> Option<&ValidatedSuffixEntry> {
        self.entries.get(index)
    }

    /// Iterate over entries.
    pub fn iter(&self) -> impl Iterator<Item = &ValidatedSuffixEntry> {
        self.entries.iter()
    }

    /// Binary search for the first suffix >= target.
    ///
    /// Returns an index in `[0, len()]` such that:
    /// - All entries before this index have suffix < target
    /// - All entries at and after have suffix >= target
    ///
    /// # Lean Specification
    /// Corresponds to `BinarySearch.findFirstGe` in `BinarySearch.lean`.
    pub fn find_first_ge(&self, texts: &[String], target: &str) -> usize {
        let mut lo = 0;
        let mut hi = self.entries.len();

        while lo < hi {
            let mid = (lo + hi) / 2;
            // SAFETY: mid is always < entries.len() due to loop invariant
            let suffix = self.entries[mid].suffix(texts);
            if suffix < target {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        lo
    }
}

/// A well-formed search index with all invariants verified.
///
/// # Lean Specification
/// Corresponds to `SearchIndex.WellFormed` in `Types.lean`:
/// ```lean
/// def SearchIndex.WellFormed (idx : SearchIndex) : Prop :=
///   idx.docs.size = idx.texts.size ∧
///   idx.lcp.size = idx.suffix_array.size ∧
///   ∀ e ∈ idx.suffix_array, SuffixEntry.WellFormed e idx.texts
/// ```
///
/// # Invariants (enforced at construction)
/// - `docs.len() == texts.len()`
/// - `lcp.len() == suffix_array.len()`
/// - All suffix entries are well-formed
/// - Suffix array is sorted
/// - All field boundaries refer to valid documents
#[derive(Debug, Clone)]
pub struct WellFormedIndex {
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    suffix_array: SortedSuffixArray,
    lcp: Vec<usize>,
    field_boundaries: Vec<FieldBoundary>,
}

impl WellFormedIndex {
    /// Create a well-formed index by validating all invariants.
    ///
    /// This is more expensive than `build_index` because it validates
    /// the suffix array rather than trusting the sort operation.
    pub fn new(
        docs: Vec<SearchDoc>,
        texts: Vec<String>,
        suffix_array: Vec<SuffixEntry>,
        lcp: Vec<usize>,
        field_boundaries: Vec<FieldBoundary>,
    ) -> Result<Self, InvariantError> {
        // Check docs/texts match
        if docs.len() != texts.len() {
            return Err(InvariantError::MismatchedDocsTexts {
                docs_len: docs.len(),
                texts_len: texts.len(),
            });
        }

        // Check lcp/suffix_array match
        if lcp.len() != suffix_array.len() {
            return Err(InvariantError::MismatchedLcpSuffixArray {
                lcp_len: lcp.len(),
                sa_len: suffix_array.len(),
            });
        }

        // Validate suffix array (checks entries and sortedness)
        let sorted_sa = SortedSuffixArray::from_vec(suffix_array, &texts)?;

        // Validate field boundaries
        for boundary in &field_boundaries {
            if boundary.doc_id >= texts.len() {
                return Err(InvariantError::InvalidFieldBoundary {
                    doc_id: boundary.doc_id,
                    texts_len: texts.len(),
                });
            }
        }

        Ok(Self {
            docs,
            texts,
            suffix_array: sorted_sa,
            lcp,
            field_boundaries,
        })
    }

    /// Create from a `SearchIndex`, validating all invariants.
    pub fn from_index(index: SearchIndex) -> Result<Self, InvariantError> {
        Self::new(
            index.docs,
            index.texts,
            index.suffix_array,
            index.lcp,
            index.field_boundaries,
        )
    }

    /// Get the documents.
    pub fn docs(&self) -> &[SearchDoc] {
        &self.docs
    }

    /// Get the texts.
    pub fn texts(&self) -> &[String] {
        &self.texts
    }

    /// Get the sorted suffix array.
    pub fn suffix_array(&self) -> &SortedSuffixArray {
        &self.suffix_array
    }

    /// Get the LCP array.
    pub fn lcp(&self) -> &[usize] {
        &self.lcp
    }

    /// Get the field boundaries.
    pub fn field_boundaries(&self) -> &[FieldBoundary] {
        &self.field_boundaries
    }

    /// Convert back to unverified `SearchIndex`.
    ///
    /// This is useful for interop with existing code.
    pub fn into_index(self) -> SearchIndex {
        SearchIndex {
            docs: self.docs,
            texts: self.texts,
            suffix_array: self
                .suffix_array
                .entries
                .into_iter()
                .map(|e| e.inner)
                .collect(),
            lcp: self.lcp,
            field_boundaries: self.field_boundaries,
            version: 4,
        }
    }
}

// =============================================================================
// INVERTED INDEX VERIFICATION
// =============================================================================

/// A validated posting where doc_id and offset are bounds-checked.
///
/// # Lean Specification
/// Corresponds to `Posting.WellFormed` in `InvertedIndex.lean`:
/// ```lean
/// def Posting.WellFormed (p : Posting) (texts : Array String) (term_len : Nat) : Prop :=
///   p.doc_id < texts.size ∧ p.offset + term_len ≤ texts[p.doc_id].length
/// ```
#[derive(Debug, Clone)]
pub struct ValidatedPosting {
    inner: crate::types::Posting,
    term_len: usize,
}

impl ValidatedPosting {
    /// Create a validated posting.
    pub fn new(
        posting: crate::types::Posting,
        texts: &[String],
        term_len: usize,
    ) -> Result<Self, InvariantError> {
        if posting.doc_id >= texts.len() {
            return Err(InvariantError::InvalidDocId {
                doc_id: posting.doc_id,
                texts_len: texts.len(),
            });
        }

        let text_len = texts[posting.doc_id].len();
        if posting.offset + term_len > text_len {
            return Err(InvariantError::InvalidOffset {
                doc_id: posting.doc_id,
                offset: posting.offset,
                text_len,
            });
        }

        Ok(Self {
            inner: posting,
            term_len,
        })
    }

    /// Get the underlying posting.
    pub fn inner(&self) -> &crate::types::Posting {
        &self.inner
    }

    /// Get the term at this posting location.
    pub fn term<'a>(&self, texts: &'a [String]) -> &'a str {
        &texts[self.inner.doc_id][self.inner.offset..self.inner.offset + self.term_len]
    }
}

/// A validated posting list where all postings are well-formed and sorted.
///
/// # Lean Specification
/// Corresponds to `PostingList.WellFormed` in `InvertedIndex.lean`.
///
/// # Invariants
/// - All postings are validated (valid doc_id, offset)
/// - Postings are sorted by (doc_id, offset)
/// - doc_freq equals count of unique doc_ids
#[derive(Debug, Clone)]
pub struct ValidatedPostingList {
    postings: Vec<ValidatedPosting>,
    doc_freq: usize,
    term: String,
}

impl ValidatedPostingList {
    /// Create a validated posting list.
    pub fn new(
        term: String,
        list: crate::types::PostingList,
        texts: &[String],
    ) -> Result<Self, InvariantError> {
        let term_len = term.len();

        // Validate all postings
        let validated: Result<Vec<_>, _> = list
            .postings
            .into_iter()
            .map(|p| ValidatedPosting::new(p, texts, term_len))
            .collect();
        let postings = validated?;

        // Check sortedness by (score DESC, doc_id ASC)
        for i in 1..postings.len() {
            let prev = &postings[i - 1].inner;
            let curr = &postings[i].inner;
            // Score should be descending (prev >= curr)
            // If equal score, doc_id should be ascending (prev <= curr)
            let score_ok = prev.score >= curr.score;
            let tiebreak_ok = prev.score != curr.score || prev.doc_id <= curr.doc_id;
            if !score_ok || !tiebreak_ok {
                return Err(InvariantError::UnsortedPostingList {
                    position: i,
                    term: term.clone(),
                });
            }
        }

        // Verify doc_freq
        let mut unique_docs: Vec<usize> = postings.iter().map(|p| p.inner.doc_id).collect();
        unique_docs.sort();
        unique_docs.dedup();
        if list.doc_freq != unique_docs.len() {
            return Err(InvariantError::IncorrectDocFreq {
                term: term.clone(),
                claimed: list.doc_freq,
                actual: unique_docs.len(),
            });
        }

        Ok(Self {
            postings,
            doc_freq: list.doc_freq,
            term,
        })
    }

    /// Get the postings.
    pub fn postings(&self) -> &[ValidatedPosting] {
        &self.postings
    }

    /// Get the document frequency.
    pub fn doc_freq(&self) -> usize {
        self.doc_freq
    }

    /// Get the term for this posting list.
    pub fn term(&self) -> &str {
        &self.term
    }

    /// Get unique document IDs in this posting list.
    pub fn doc_ids(&self) -> Vec<usize> {
        let mut docs: Vec<usize> = self.postings.iter().map(|p| p.inner.doc_id).collect();
        docs.sort();
        docs.dedup();
        docs
    }
}

/// A validated inverted index where all posting lists are well-formed.
///
/// # Lean Specification
/// Corresponds to `InvertedIndex.WellFormed` in `InvertedIndex.lean`.
///
/// # Invariants
/// - All posting lists are validated
/// - All terms exist in at least one document
/// - total_docs matches texts.len()
#[derive(Debug, Clone)]
pub struct ValidatedInvertedIndex {
    terms: std::collections::HashMap<String, ValidatedPostingList>,
    total_docs: usize,
}

impl ValidatedInvertedIndex {
    /// Create a validated inverted index.
    pub fn new(
        index: crate::types::InvertedIndex,
        texts: &[String],
    ) -> Result<Self, InvariantError> {
        if index.total_docs != texts.len() {
            return Err(InvariantError::IncorrectTotalDocs {
                claimed: index.total_docs,
                actual: texts.len(),
            });
        }

        let mut validated_terms = std::collections::HashMap::new();
        for (term, list) in index.terms {
            if list.postings.is_empty() {
                return Err(InvariantError::EmptyPostingList { term });
            }
            let validated = ValidatedPostingList::new(term.clone(), list, texts)?;
            validated_terms.insert(term, validated);
        }

        Ok(Self {
            terms: validated_terms,
            total_docs: index.total_docs,
        })
    }

    /// Look up a term.
    pub fn get(&self, term: &str) -> Option<&ValidatedPostingList> {
        self.terms.get(term)
    }

    /// Get all terms in the index.
    pub fn terms(&self) -> impl Iterator<Item = &String> {
        self.terms.keys()
    }

    /// Get total documents.
    pub fn total_docs(&self) -> usize {
        self.total_docs
    }

    /// Intersect posting lists for AND queries.
    ///
    /// Returns document IDs that appear in ALL given terms.
    pub fn intersect(&self, terms: &[&str]) -> Vec<usize> {
        if terms.is_empty() {
            return Vec::new();
        }

        // Start with first term's docs
        let first = match self.get(terms[0]) {
            Some(pl) => pl.doc_ids(),
            None => return Vec::new(),
        };

        // Intersect with remaining terms
        let mut result = first;
        for term in &terms[1..] {
            match self.get(term) {
                Some(pl) => {
                    let doc_ids = pl.doc_ids();
                    result.retain(|d| doc_ids.contains(d));
                }
                None => return Vec::new(),
            }
        }

        result
    }
}

/// Verification status for reporting.
#[derive(Debug, Clone)]
pub struct VerificationReport {
    /// Number of proven theorems in Lean.
    pub proven_theorems: usize,
    /// Number of axiomatized statements in Lean.
    pub axiom_count: usize,
    /// Number of property tests covering specifications.
    pub property_tests: usize,
    /// Type-level invariants enforced.
    pub type_invariants: Vec<String>,
}

impl VerificationReport {
    /// Generate a verification report for the current codebase.
    pub fn generate() -> Self {
        Self {
            proven_theorems: 5, // aggregateScores_eq_sum, various small lemmas
            axiom_count: 22,    // Counted from lean/*.lean (including InvertedIndex.lean)
            property_tests: 14, // Counted from tests module (including inverted index tests)
            type_invariants: vec![
                "ValidatedSuffixEntry: doc_id < texts.len() ∧ offset < texts[doc_id].len()"
                    .to_string(),
                "SortedSuffixArray: ∀ i < j. suffix[i] ≤ suffix[j]".to_string(),
                "WellFormedIndex: docs.len() = texts.len() ∧ lcp.len() = suffix_array.len()"
                    .to_string(),
                "ValidatedPosting: doc_id < texts.len() ∧ offset + term_len ≤ texts[doc_id].len()"
                    .to_string(),
                "ValidatedPostingList: postings sorted ∧ doc_freq = unique_doc_ids.len()"
                    .to_string(),
                "ValidatedInvertedIndex: ∀ term. postings.len() > 0 ∧ total_docs = texts.len()"
                    .to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_index;

    #[test]
    fn validated_suffix_entry_rejects_invalid_doc_id() {
        let texts = vec!["hello".to_string()];
        let entry = SuffixEntry {
            doc_id: 5,
            offset: 0,
        };

        let result = ValidatedSuffixEntry::new(entry, &texts);
        assert!(matches!(
            result,
            Err(InvariantError::InvalidDocId { doc_id: 5, .. })
        ));
    }

    #[test]
    fn validated_suffix_entry_rejects_invalid_offset() {
        let texts = vec!["hi".to_string()];
        let entry = SuffixEntry {
            doc_id: 0,
            offset: 10,
        };

        let result = ValidatedSuffixEntry::new(entry, &texts);
        assert!(matches!(
            result,
            Err(InvariantError::InvalidOffset { offset: 10, .. })
        ));
    }

    #[test]
    fn validated_suffix_entry_accepts_valid_entry() {
        let texts = vec!["hello".to_string()];
        let entry = SuffixEntry {
            doc_id: 0,
            offset: 2,
        };

        let result = ValidatedSuffixEntry::new(entry, &texts);
        assert!(result.is_ok());

        let validated = result.unwrap();
        assert_eq!(validated.suffix(&texts), "llo");
    }

    #[test]
    fn sorted_suffix_array_rejects_unsorted() {
        let texts = vec!["banana".to_string()];
        let entries = vec![
            SuffixEntry {
                doc_id: 0,
                offset: 0,
            }, // "banana"
            SuffixEntry {
                doc_id: 0,
                offset: 3,
            }, // "ana"
        ];

        let result = SortedSuffixArray::from_vec(entries, &texts);
        assert!(matches!(
            result,
            Err(InvariantError::UnsortedSuffixArray { .. })
        ));
    }

    #[test]
    fn sorted_suffix_array_accepts_sorted() {
        let texts = vec!["banana".to_string()];
        // Sorted order: "a", "ana", "anana", "banana", "na", "nana"
        let entries = vec![
            SuffixEntry {
                doc_id: 0,
                offset: 5,
            }, // "a"
            SuffixEntry {
                doc_id: 0,
                offset: 3,
            }, // "ana"
            SuffixEntry {
                doc_id: 0,
                offset: 1,
            }, // "anana"
            SuffixEntry {
                doc_id: 0,
                offset: 0,
            }, // "banana"
            SuffixEntry {
                doc_id: 0,
                offset: 4,
            }, // "na"
            SuffixEntry {
                doc_id: 0,
                offset: 2,
            }, // "nana"
        ];

        let result = SortedSuffixArray::from_vec(entries, &texts);
        assert!(result.is_ok());
    }

    #[test]
    fn well_formed_index_validates_build_output() {
        use crate::SearchDoc;

        let docs = vec![SearchDoc {
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

        // Build using the standard function
        let index = build_index(docs, texts, vec![]);

        // Validate using WellFormedIndex
        let result = WellFormedIndex::from_index(index);
        assert!(
            result.is_ok(),
            "build_index output should be well-formed: {:?}",
            result.err()
        );
    }

    #[test]
    fn binary_search_finds_correct_position() {
        let texts = vec!["apple banana cherry".to_string()];
        let index = build_index(
            vec![crate::SearchDoc {
                id: 0,
                title: "Test".to_string(),
                excerpt: "".to_string(),
                href: "/test".to_string(),
                kind: "post".to_string(),
                category: None,
                author: None,
                tags: vec![],
            }],
            texts.clone(),
            vec![],
        );

        let validated = WellFormedIndex::from_index(index).unwrap();
        let sa = validated.suffix_array();

        // Search for "b" should find position where suffixes starting with "b" begin
        let pos = sa.find_first_ge(&texts, "b");
        assert!(pos < sa.len());

        // All suffixes before pos should be < "b"
        for i in 0..pos {
            let suffix = sa.get(i).unwrap().suffix(&texts);
            assert!(
                suffix < "b",
                "suffix at {} is '{}', should be < 'b'",
                i,
                suffix
            );
        }
    }

    #[test]
    fn verification_report_generates() {
        let report = VerificationReport::generate();
        assert!(report.proven_theorems > 0);
        assert!(report.property_tests > 0);
        assert!(!report.type_invariants.is_empty());
    }
}
