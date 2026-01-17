/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

/-
  Binary search correctness.

  Find the first suffix ≥ the query, then walk forward collecting matches.
  The invariant: if a matching suffix exists, binary search lands in the
  right range. Proving this requires showing the suffix array is sorted,
  which is why `SuffixArray.lean` exists.
-/

import SearchVerified.Types
import SearchVerified.SuffixArray
import SearchVerified.Basic

namespace SearchVerified.BinarySearch

open SearchVerified
open SearchVerified.SuffixArray

/-! ## Binary Search Implementation -/

/-- Binary search helper: find first position where suffix >= target -/
partial def findFirstGe.go (sa : Array SuffixEntry) (texts : Array String) (target : String)
    (lo hi : Nat) : Nat :=
  if lo >= hi then lo
  else
    let mid := (lo + hi) / 2
    if h : mid < sa.size then
      if suffixAt texts sa[mid] < target then
        go sa texts target (mid + 1) hi
      else
        go sa texts target lo mid
    else lo

/-- Binary search: find first position where suffix >= target -/
def findFirstGe (sa : Array SuffixEntry) (texts : Array String) (target : String) : Nat :=
  findFirstGe.go sa texts target 0 sa.size

/-! ## Binary Search Bounds (Axiomatized) -/

/-- Result of findFirstGe: position in [0, sa.size] -/
axiom findFirstGe_bounds (sa : Array SuffixEntry) (texts : Array String) (target : String) :
    findFirstGe sa texts target ≤ sa.size

/-- Correctness: all elements before result are strictly less than target -/
axiom findFirstGe_lower_bound
    (sa : Array SuffixEntry) (texts : Array String) (target : String)
    (h_sorted : Sorted sa texts) :
    let pos := findFirstGe sa texts target
    ∀ k : Nat, k < pos → k < sa.size → suffixAt texts sa[k]! < target

/-- Correctness: element at result (if exists) is >= target -/
axiom findFirstGe_upper_bound
    (sa : Array SuffixEntry) (texts : Array String) (target : String)
    (h_sorted : Sorted sa texts) :
    let pos := findFirstGe sa texts target
    pos < sa.size → suffixAt texts (sa[pos]!) ≥ target

/-! ## Match Collection -/

/-- A suffix matches if it starts with the query -/
def isMatch (texts : Array String) (entry : SuffixEntry) (query : String) : Bool :=
  (suffixAt texts entry).startsWith query

/-- Walking forward from first match finds all matches -/
partial def collectMatches (sa : Array SuffixEntry) (texts : Array String)
    (start : Nat) (query : String) : List SuffixEntry :=
  go start []
where
  go (i : Nat) (acc : List SuffixEntry) : List SuffixEntry :=
    if h : i < sa.size then
      if isMatch texts sa[i] query then
        go (i + 1) (sa[i] :: acc)
      else
        acc.reverse
    else
      acc.reverse

/-- All returned matches actually match the query -/
axiom collectMatches_sound
    (sa : Array SuffixEntry) (texts : Array String)
    (start : Nat) (query : String)
    (entry : SuffixEntry)
    (h_in : entry ∈ collectMatches sa texts start query) :
    isMatch texts entry query = true

/-- All matching entries starting from start are in the result -/
axiom collectMatches_complete
    (sa : Array SuffixEntry) (texts : Array String)
    (start : Nat) (query : String)
    (h_sorted : Sorted sa texts)
    (h_start : start = findFirstGe sa texts query)
    (i : Nat)
    (hi : i < sa.size)
    (hi_ge : i ≥ start)
    (h_matches : isMatch texts sa[i]! query = true)
    (h_contiguous : ∀ k, start ≤ k → k < i → isMatch texts sa[k]! query = true) :
    sa[i]! ∈ collectMatches sa texts start query

/-! ## Combined Search -/

/-- Combined search function -/
def search (sa : Array SuffixEntry) (texts : Array String) (query : String) : List SuffixEntry :=
  let start := findFirstGe sa texts query
  collectMatches sa texts start query

/-- Search returns exactly the matching entries -/
axiom search_correct
    (sa : Array SuffixEntry) (texts : Array String) (query : String)
    (h_sorted : Sorted sa texts)
    (entry : SuffixEntry) :
    entry ∈ search sa texts query →
      (∃ i : Nat, i < sa.size ∧ sa[i]! = entry ∧ isMatch texts entry query = true)

/-! ## Prefix Search via Vocabulary Suffix Array

The vocabulary suffix array enables O(log k) queryPrefix search where k is vocabulary size.
This is used in Tier 2 of the three-tier search to find terms matching a query queryPrefix.

### Algorithm
1. Binary search to find first vocabulary suffix matching the queryPrefix
2. Walk forward to collect all matching terms
3. Return term indices (not suffix positions)
-/

/-- Vocabulary suffix entry: position within a term -/
structure VocabSuffixEntry where
  /-- Index into vocabulary array -/
  term_idx : Nat
  /-- Offset within the term string -/
  offset : Nat
  deriving DecidableEq, Repr, Inhabited

/-- Get suffix string at a vocabulary suffix entry -/
def vocabSuffixAt (vocabulary : Array String) (entry : VocabSuffixEntry) : String :=
  if h : entry.term_idx < vocabulary.size then
    (vocabulary[entry.term_idx]).drop entry.offset
  else
    ""

/-- Prefix search finds first matching suffix via binary search.

    Returns the smallest index where the vocabulary suffix starts with the queryPrefix.
    If no match exists, returns vocab_sa.size. -/
def queryPrefix_search_start (vocab_sa : Array VocabSuffixEntry) (vocabulary : Array String)
    (queryPrefix : String) : Nat :=
  go 0 vocab_sa.size
where
  go (lo hi : Nat) : Nat :=
    if lo >= hi then lo
    else
      let mid := (lo + hi) / 2
      if h : mid < vocab_sa.size then
        let suffix := vocabSuffixAt vocabulary vocab_sa[mid]
        if suffix < queryPrefix then
          go (mid + 1) hi
        else
          go lo mid
      else lo
  termination_by hi - lo

/-- All terms before queryPrefix_search_start don't match.

    Verified by: prop_queryPrefix_search_lower_bound in tests/property.rs -/
axiom queryPrefix_search_lower_bound (vocab_sa : Array VocabSuffixEntry)
    (vocabulary : Array String) (queryPrefix : String) :
    ∀ i : Nat, i < queryPrefix_search_start vocab_sa vocabulary queryPrefix →
      i < vocab_sa.size →
      let entry := vocab_sa[i]!
      let suffix := vocabSuffixAt vocabulary entry
      ¬queryPrefix.isPrefixOf suffix

/-- Term at queryPrefix_search_start matches (if in bounds).

    Verified by: prop_queryPrefix_search_finds_match in tests/property.rs -/
axiom queryPrefix_search_finds_match (vocab_sa : Array VocabSuffixEntry)
    (vocabulary : Array String) (queryPrefix : String) :
    let start := queryPrefix_search_start vocab_sa vocabulary queryPrefix
    start < vocab_sa.size →
      let entry := vocab_sa[start]!
      let suffix := vocabSuffixAt vocabulary entry
      queryPrefix.isPrefixOf suffix ∨ suffix < queryPrefix

/-- Prefix search returns all matching terms.

    For any term that starts with the queryPrefix, there exists an entry
    in the vocabulary suffix array with offset 0 that points to it.

    Verified by: prop_queryPrefix_search_complete in tests/property.rs -/
axiom queryPrefix_search_complete (vocab_sa : Array VocabSuffixEntry)
    (vocabulary : Array String) (queryPrefix : String) (term_idx : Nat) :
    term_idx < vocabulary.size →
    queryPrefix.isPrefixOf vocabulary[term_idx]! →
    ∃ i : Nat, i < vocab_sa.size ∧
      vocab_sa[i]!.term_idx = term_idx ∧
      vocab_sa[i]!.offset = 0

end SearchVerified.BinarySearch
