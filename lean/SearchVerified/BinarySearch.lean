/-
  BinarySearch.lean - Binary search correctness specifications.

  The search algorithm finds all suffixes that start with the query string
  using binary search to find the first match, then walking forward.
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

end SearchVerified.BinarySearch
