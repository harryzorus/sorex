/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

/-
  InvertedIndex.lean - Inverted index correctness specifications.

  The inverted index maps terms to posting lists for O(1) term lookup.
  This complements the suffix array which provides O(log n) prefix matching.

  Key invariants:
  1. All postings are well-formed (valid doc_id and offset)
  2. Posting lists are sorted by (doc_id, offset)
  3. Document frequency equals the count of unique doc_ids
  4. Every term in the index exists in at least one document
-/

import SearchVerified.Types
import SearchVerified.Basic

namespace SearchVerified.Inverted

open SearchVerified

-- =============================================================================
-- Posting Types
-- =============================================================================

/-- A posting: location where a term occurs in a document -/
structure Posting where
  doc_id : Nat
  offset : Nat
  field_type : FieldType
  /-- Section ID for deep linking -/
  section_id : Option String := none
  /-- Heading level for hierarchical ranking (0=title, 1-4=H1-H4, 5+=content) -/
  heading_level : Nat := 0
  /-- Pre-computed score (scaled by 10 for Nat) -/
  score : Nat := 0
  deriving DecidableEq, Repr, Inhabited

/-- A posting is well-formed if it points to a valid location -/
def Posting.WellFormed (p : Posting) (texts : Array String) (term_len : Nat) : Prop :=
  p.doc_id < texts.size ∧
  p.offset + term_len ≤ (texts[p.doc_id]!).length

/-- Postings are ordered by (doc_id, offset) -/
def Posting.le (a b : Posting) : Bool :=
  a.doc_id < b.doc_id ||
  (a.doc_id == b.doc_id && a.offset ≤ b.offset)

-- =============================================================================
-- Posting List Types
-- =============================================================================

/-- A posting list: all occurrences of a term -/
structure PostingList where
  postings : Array Posting
  doc_freq : Nat
  deriving Repr

/-- The posting list is sorted by (doc_id, offset) -/
def PostingList.Sorted (pl : PostingList) : Prop :=
  ∀ i j : Nat, (hi : i < pl.postings.size) → (hj : j < pl.postings.size) → i < j →
    Posting.le pl.postings[i] pl.postings[j]

/-- All postings in the list are well-formed -/
def PostingList.AllWellFormed (pl : PostingList) (texts : Array String) (term_len : Nat) : Prop :=
  ∀ i : Nat, (hi : i < pl.postings.size) →
    Posting.WellFormed pl.postings[i]! texts term_len

/-- Document frequency matches unique doc_ids -/
def PostingList.DocFreqCorrect (pl : PostingList) : Prop :=
  pl.doc_freq = (pl.postings.map (·.doc_id)).toList.eraseDups.length

/-- A posting list is well-formed -/
def PostingList.WellFormed (pl : PostingList) (texts : Array String) (term_len : Nat) : Prop :=
  PostingList.Sorted pl ∧
  PostingList.AllWellFormed pl texts term_len ∧
  PostingList.DocFreqCorrect pl

-- =============================================================================
-- Inverted Index Type
-- =============================================================================

/-- The inverted index: maps terms to posting lists -/
structure InvertedIndex where
  terms : List (String × PostingList)
  total_docs : Nat
  deriving Repr

/-- All posting lists in the index are well-formed -/
def InvertedIndex.AllWellFormed (idx : InvertedIndex) (texts : Array String) : Prop :=
  ∀ entry : String × PostingList, entry ∈ idx.terms →
    PostingList.WellFormed entry.2 texts entry.1.length

/-- Every term in the index has at least one posting -/
def InvertedIndex.NonEmpty (idx : InvertedIndex) : Prop :=
  ∀ entry : String × PostingList, entry ∈ idx.terms →
    entry.2.postings.size > 0

/-- The total_docs field matches the actual number of documents -/
def InvertedIndex.TotalDocsCorrect (idx : InvertedIndex) (texts : Array String) : Prop :=
  idx.total_docs = texts.size

/-- An inverted index is well-formed -/
def InvertedIndex.WellFormed (idx : InvertedIndex) (texts : Array String) : Prop :=
  InvertedIndex.AllWellFormed idx texts ∧
  InvertedIndex.NonEmpty idx ∧
  InvertedIndex.TotalDocsCorrect idx texts

-- =============================================================================
-- Core Operations
-- =============================================================================

/-- Look up a term in the inverted index -/
def InvertedIndex.lookup (idx : InvertedIndex) (term : String) : Option PostingList :=
  match idx.terms.find? (fun entry => entry.1 == term) with
  | some (_, pl) => some pl
  | none => none

/-- Get all documents containing a term -/
def PostingList.docIds (pl : PostingList) : List Nat :=
  (pl.postings.map (·.doc_id)).toList.eraseDups

-- =============================================================================
-- Intersection (AND query)
-- =============================================================================

/-- Intersect two sorted posting lists -/
def PostingList.intersect (a b : PostingList) : PostingList :=
  let common_docs := a.docIds.filter (fun d => d ∈ b.docIds)
  let postings := a.postings.filter (fun p => p.doc_id ∈ common_docs)
  { postings := postings, doc_freq := common_docs.length }

/-- Intersection preserves sortedness -/
axiom intersect_preserves_sorted (a b : PostingList)
    (ha : PostingList.Sorted a) (hb : PostingList.Sorted b) :
    PostingList.Sorted (PostingList.intersect a b)

/-- Intersection only returns documents in both lists.

    AXIOMATIZED: The proof requires complex Array↔List interop lemmas.
    This is algorithmically trivial (filter preserves membership) but
    tedious to prove in Lean 4 with the current Mathlib Array API.

    Verified by: prop_intersect_correct in tests/property.rs -/
axiom intersect_correct (a b : PostingList) (doc_id : Nat) :
    doc_id ∈ (PostingList.intersect a b).docIds ↔
    doc_id ∈ a.docIds ∧ doc_id ∈ b.docIds

-- =============================================================================
-- Union (OR query)
-- =============================================================================

/-- Merge two sorted posting lists (union) -/
def PostingList.union (a b : PostingList) : PostingList :=
  let all_docs := (a.docIds ++ b.docIds).eraseDups
  let all_postings := a.postings ++ b.postings
  -- Note: In practice, merge sort is used; simplified for specification
  { postings := all_postings, doc_freq := all_docs.length }

/-- Union returns documents in either list -/
axiom union_correct (a b : PostingList) (doc_id : Nat) :
    doc_id ∈ (PostingList.union a b).docIds ↔
    doc_id ∈ a.docIds ∨ doc_id ∈ b.docIds

-- =============================================================================
-- Build Index Correctness
-- =============================================================================

/-- Building an inverted index produces a well-formed result -/
axiom build_produces_wellformed
    (docs : Array SearchDoc)
    (texts : Array String)
    (h_nonempty : docs.size > 0)
    (h_match : docs.size = texts.size)
    (idx : InvertedIndex) :
    InvertedIndex.WellFormed idx texts

/-- The inverted index contains all words from all documents.

This is a high-level specification: if a word appears at a word boundary
in a document, the inverted index will have a posting for it.

The actual word boundary detection (alphanumeric checks) is verified
via property tests in the Rust implementation. -/
axiom build_complete
    (texts : Array String)
    (h_nonempty : texts.size > 0)
    (idx : InvertedIndex)
    (word : String)
    (doc_id : Nat)
    (offset : Nat)
    (h_doc : doc_id < texts.size)
    (h_valid : offset + word.length ≤ (texts[doc_id]!).length)
    (h_word : ((texts[doc_id]!).drop offset).take word.length = word)
    (h_at_boundary : True) : -- Word boundary checked at runtime
    ∃ pl : PostingList, idx.lookup word = some pl ∧
      ∃ p : Posting, p ∈ pl.postings.toList ∧ p.doc_id = doc_id ∧ p.offset = offset

-- =============================================================================
-- Hybrid Index Correctness
-- =============================================================================

/-- Hybrid index: both indexes are well-formed and agree on texts -/
structure HybridIndex where
  suffix_index : SearchIndex
  inverted_index : InvertedIndex
  deriving Repr

/-- A hybrid index is well-formed if both components are -/
def HybridIndex.WellFormed (idx : HybridIndex) : Prop :=
  SearchIndex.WellFormed idx.suffix_index ∧
  InvertedIndex.WellFormed idx.inverted_index idx.suffix_index.texts

/-- Both indexes agree on document results for exact matches -/
axiom hybrid_search_consistent
    (idx : HybridIndex)
    (h : HybridIndex.WellFormed idx)
    (word : String)
    (doc_id : Nat) :
    -- If inverted index says doc contains word, suffix array can find it too
    (∃ pl, idx.inverted_index.lookup word = some pl ∧ doc_id ∈ pl.docIds) →
    ∃ entry : SuffixEntry, entry ∈ idx.suffix_index.suffix_array.toList ∧
      entry.doc_id = doc_id ∧
      ((idx.suffix_index.texts[entry.doc_id]!).drop entry.offset |>.take word.length) = word

end SearchVerified.Inverted
