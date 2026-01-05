//! Search index construction.
//!
//! # Lean Correspondence
//!
//! The index construction corresponds to specifications in:
//! - `SearchVerified/SuffixArray.lean` - Sorted suffix array properties
//! - `SearchVerified/BinarySearch.lean` - LCP array correctness
//!
//! # INVARIANTS (DO NOT VIOLATE)
//!
//! 1. **SUFFIX_ARRAY_SORTED**: After `build_index`, suffix array is lexicographically sorted
//! 2. **SUFFIX_ARRAY_COMPLETE**: Every position in every document has an entry
//! 3. **LCP_CORRECT**: `lcp[i]` = common prefix length of consecutive suffixes
//! 4. **ENTRY_WELLFORMED**: Every entry has valid `doc_id` and `offset`
//!
//! # Unicode Support
//!
//! Offsets in the suffix array are **character offsets** (not byte offsets).
//! This ensures compatibility with JavaScript's UTF-16 string indexing.
//! - Rust iterates over `text.chars()` to generate character positions
//! - JavaScript's `text.slice(offset)` uses the same character semantics
//! - Non-ASCII characters (Telugu, accents, emoji) are fully supported

use crate::types::{FieldBoundary, SearchDoc, SearchIndex, SuffixEntry};
use crate::utils::common_prefix_len_chars;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[cfg(feature = "lean")]
use sieve_lean_macros::{lean_proptest_verify, lean_verify};

/// Pre-computed character-to-byte offset mapping for a text.
/// Enables O(1) slicing by character offset.
struct CharOffsets {
    /// char_to_byte[i] = byte index where character i starts
    /// char_to_byte[char_count] = text.len() (sentinel for end slicing)
    char_to_byte: Vec<usize>,
}

impl CharOffsets {
    fn new(text: &str) -> Self {
        let mut char_to_byte: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        char_to_byte.push(text.len()); // Sentinel for slicing to end
        Self { char_to_byte }
    }

    /// Get the suffix starting at character offset `char_offset`
    fn suffix_at<'a>(&self, text: &'a str, char_offset: usize) -> &'a str {
        let byte_start = self
            .char_to_byte
            .get(char_offset)
            .copied()
            .unwrap_or(text.len());
        &text[byte_start..]
    }

    /// Number of characters in the text
    fn char_count(&self) -> usize {
        self.char_to_byte.len().saturating_sub(1)
    }
}

/// Get the suffix at a given entry in the suffix array (character-based).
///
/// This is the fundamental operation that defines suffix array ordering.
/// The suffix array is sorted by lexicographic order of these suffixes.
///
/// **Important**: `entry.offset` is a character offset, not a byte offset.
/// This function handles the conversion internally.
///
/// # Lean Specification
///
/// Corresponds to `suffixAt` in `Types.lean`:
/// ```lean
/// def suffixAt (texts : Array String) (e : SuffixEntry) : String :=
///   if h : e.doc_id < texts.size then (texts[e.doc_id]).drop e.offset
///   else ""
/// ```
#[cfg_attr(
    feature = "lean",
    lean_proptest_verify(
        spec = "suffix_at_valid",
        requires = "entry.doc_id < texts.len()",
        ensures = "result.chars().count() <= texts[entry.doc_id].chars().count()",
        cases = 100
    )
)]
pub fn suffix_at(texts: &[String], entry: &SuffixEntry) -> String {
    texts
        .get(entry.doc_id)
        .map(|t| t.chars().skip(entry.offset).collect())
        .unwrap_or_default()
}

/// Get the suffix using pre-computed character offsets (O(1) lookup).
fn suffix_at_fast<'a>(text: &'a str, char_offsets: &CharOffsets, char_offset: usize) -> &'a str {
    char_offsets.suffix_at(text, char_offset)
}

/// Check if the suffix array is sorted.
///
/// This is the key invariant from `SuffixArray.Sorted` in the Lean specs.
///
/// # Lean Specification
///
/// Corresponds to `SuffixArray.Sorted` in `SuffixArray.lean`:
/// ```lean
/// def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop :=
///   ∀ i j : Nat, (hi : i < sa.size) → (hj : j < sa.size) → i < j →
///     SuffixLe texts sa[i] sa[j]
/// ```
pub fn is_suffix_array_sorted(texts: &[String], suffix_array: &[SuffixEntry]) -> bool {
    suffix_array.windows(2).all(|pair| {
        let prev = suffix_at(texts, &pair[0]);
        let curr = suffix_at(texts, &pair[1]);
        prev <= curr
    })
}

/// Build a search index from documents.
///
/// Creates a sorted suffix array enabling O(log n) binary search.
///
/// # Lean Specification
///
/// The following properties are guaranteed (see `SuffixArray.lean`):
/// - `suffix_array_sorted`: Suffix array is sorted lexicographically
/// - `suffix_array_complete`: All suffixes of all documents are included
/// - `lcp_correct`: LCP[i] = common prefix length of consecutive suffixes
#[cfg_attr(
    feature = "lean",
    lean_verify(
        spec = "build_index",
        requires = "docs.len() > 0 ∧ docs.len() = texts.len()",
        ensures = "∀i j. i < j → suffix_at texts result.suffix_array[i] ≤ suffix_at texts result.suffix_array[j]",
        properties = ["suffix_array_sorted", "suffix_array_complete", "lcp_correct"]
    )
)]
pub fn build_index(
    docs: Vec<SearchDoc>,
    texts: Vec<String>,
    field_boundaries: Vec<FieldBoundary>,
) -> SearchIndex {
    // Pre-compute character offset mappings for O(1) suffix lookups
    let char_offsets: Vec<CharOffsets> = texts.iter().map(|t| CharOffsets::new(t)).collect();

    // Generate suffix entries with CHARACTER offsets (not byte offsets)
    // This ensures compatibility with JavaScript's UTF-16 string indexing
    let suffixes: Vec<SuffixEntry> = {
        #[cfg(feature = "parallel")]
        {
            docs.par_iter()
                .flat_map_iter(|doc| {
                    char_offsets.get(doc.id).into_iter().flat_map(move |co| {
                        (0..co.char_count()).map(move |char_offset| SuffixEntry {
                            doc_id: doc.id,
                            offset: char_offset,
                        })
                    })
                })
                .collect()
        }
        #[cfg(not(feature = "parallel"))]
        {
            let mut tmp = Vec::new();
            for doc in &docs {
                if let Some(co) = char_offsets.get(doc.id) {
                    for char_offset in 0..co.char_count() {
                        tmp.push(SuffixEntry {
                            doc_id: doc.id,
                            offset: char_offset,
                        });
                    }
                }
            }
            tmp
        }
    };

    let mut suffixes = suffixes;

    // INVARIANT: SUFFIX_ARRAY_SORTED
    // This sort establishes the key invariant that enables O(log n) binary search.
    // The comparator uses lexicographic ordering of the actual suffix strings.
    // Character offsets are converted to byte slices using pre-computed mappings.
    // DO NOT modify this comparator without updating Lean proofs in SuffixArray.lean.
    #[cfg(feature = "parallel")]
    {
        suffixes.par_sort_by(|a, b| {
            let sa = suffix_at_fast(&texts[a.doc_id], &char_offsets[a.doc_id], a.offset);
            let sb = suffix_at_fast(&texts[b.doc_id], &char_offsets[b.doc_id], b.offset);
            sa.cmp(sb)
        });
    }
    #[cfg(not(feature = "parallel"))]
    {
        suffixes.sort_by(|a, b| {
            let sa = suffix_at_fast(&texts[a.doc_id], &char_offsets[a.doc_id], a.offset);
            let sb = suffix_at_fast(&texts[b.doc_id], &char_offsets[b.doc_id], b.offset);
            sa.cmp(sb)
        });
    }

    // INVARIANT: LCP_CORRECT
    // lcp[0] = 0, lcp[i] = common prefix length (in characters) of consecutive suffixes
    // This enables early termination in binary search. DO NOT modify without
    // updating check_lcp_correct in contracts.rs.
    let mut lcp: Vec<usize> = vec![0; suffixes.len()];
    if suffixes.len() > 1 {
        #[cfg(feature = "parallel")]
        {
            lcp.par_iter_mut()
                .enumerate()
                .skip(1)
                .for_each(|(i, slot)| {
                    let prev = suffix_at_fast(
                        &texts[suffixes[i - 1].doc_id],
                        &char_offsets[suffixes[i - 1].doc_id],
                        suffixes[i - 1].offset,
                    );
                    let curr = suffix_at_fast(
                        &texts[suffixes[i].doc_id],
                        &char_offsets[suffixes[i].doc_id],
                        suffixes[i].offset,
                    );
                    *slot = common_prefix_len_chars(prev, curr);
                });
        }

        #[cfg(not(feature = "parallel"))]
        {
            for i in 1..suffixes.len() {
                let prev = suffix_at_fast(
                    &texts[suffixes[i - 1].doc_id],
                    &char_offsets[suffixes[i - 1].doc_id],
                    suffixes[i - 1].offset,
                );
                let curr = suffix_at_fast(
                    &texts[suffixes[i].doc_id],
                    &char_offsets[suffixes[i].doc_id],
                    suffixes[i].offset,
                );
                lcp[i] = common_prefix_len_chars(prev, curr);
            }
        }
    }

    SearchIndex {
        docs,
        texts,
        suffix_array: suffixes,
        lcp,
        field_boundaries,
        version: 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc(id: usize) -> SearchDoc {
        SearchDoc {
            id,
            title: format!("Doc {}", id),
            excerpt: "".to_string(),
            href: format!("/doc/{}", id),
            kind: "post".to_string(),
        }
    }

    #[test]
    fn test_suffix_at() {
        let texts = vec!["hello".to_string()];
        let entry = SuffixEntry {
            doc_id: 0,
            offset: 2,
        };
        assert_eq!(suffix_at(&texts, &entry), "llo");
    }

    #[test]
    fn test_suffix_at_unicode() {
        // "café" has 4 characters but 5 bytes (é is 2 bytes in UTF-8)
        let texts = vec!["café".to_string()];
        let entry = SuffixEntry {
            doc_id: 0,
            offset: 2, // Character offset, not byte offset
        };
        // Should get "fé" (characters 2 and 3), not "f" (byte 2)
        assert_eq!(suffix_at(&texts, &entry), "fé");
    }

    #[test]
    fn test_build_index_sorted() {
        let docs = vec![make_doc(0), make_doc(1)];
        let texts = vec!["banana".to_string(), "apple".to_string()];
        let index = build_index(docs, texts.clone(), vec![]);

        assert!(is_suffix_array_sorted(&texts, &index.suffix_array));
    }

    #[test]
    fn test_build_index_sorted_unicode() {
        // Test with non-ASCII characters
        let docs = vec![make_doc(0)];
        let texts = vec!["tummalachērla hello".to_string()];
        let index = build_index(docs, texts.clone(), vec![]);

        assert!(is_suffix_array_sorted(&texts, &index.suffix_array));
    }

    #[test]
    fn test_build_index_complete() {
        let docs = vec![make_doc(0)];
        let texts = vec!["abc".to_string()];
        let index = build_index(docs, texts, vec![]);

        // Should have 3 suffixes: "abc", "bc", "c"
        assert_eq!(index.suffix_array.len(), 3);
    }

    #[test]
    fn test_build_index_complete_unicode() {
        let docs = vec![make_doc(0)];
        // "café" has 4 characters (not 5 bytes)
        let texts = vec!["café".to_string()];
        let index = build_index(docs, texts, vec![]);

        // Should have 4 suffixes (one per character), not 5 (one per byte)
        assert_eq!(index.suffix_array.len(), 4);
    }

    #[test]
    fn test_lcp_correct() {
        let docs = vec![make_doc(0)];
        let texts = vec!["banana".to_string()];
        let index = build_index(docs, texts, vec![]);

        // LCP[0] should be 0
        assert_eq!(index.lcp[0], 0);

        // Verify other LCP values using character-based comparison
        for i in 1..index.lcp.len() {
            let prev = suffix_at(&index.texts, &index.suffix_array[i - 1]);
            let curr = suffix_at(&index.texts, &index.suffix_array[i]);
            assert_eq!(index.lcp[i], common_prefix_len_chars(&prev, &curr));
        }
    }
}
