/-
  SearchVerified - Formal verification for suffix array and inverted index search.

  This library provides Lean 4 specifications and proofs for the
  Rust search implementation used in the blog's search feature.

  ## Modules

  - Types: Core data structures mirroring Rust structs
  - SuffixArray: Sorting invariant and completeness proofs
  - BinarySearch: Search correctness theorems
  - Levenshtein: Edit distance bounds for fuzzy search
  - Scoring: Field type dominance and ranking proofs
  - InvertedIndex: Posting list properties and hybrid index specs
  - ProgressiveLoading: Layer loading order independence and subset properties
  - SearchOptions: Boost preservation and limit correctness theorems
  - Suggestions: Prefix suggestion validity and ranking proofs
  - Section: Deep-linking section navigation correctness

  ## Proof Status

  ✓ Proven (native_decide):
  - title_beats_heading
  - heading_beats_content
  - field_type_dominance

  ◐ Partially proven:
  - sorted_iff_consecutive (one direction)
  - sorted_transitive (uses hypothesis)
  - withinBounds_sound (uses length_diff_lower_bound)
  - early_exit_correct (uses length_diff_lower_bound)
  - intersect_correct (axiomatized)
  - union_correct (axiomatized)

  ○ Axiomatized (verified by property tests):
  - build_produces_sorted / build_produces_complete / build_produces_correct_lcp
  - build_produces_wellformed / build_complete (inverted index)
  - hybrid_search_consistent
  - streaming_preserves_ranking / streaming_pipeline_correct
-/

import SearchVerified.Types
import SearchVerified.SuffixArray
import SearchVerified.BinarySearch
import SearchVerified.Levenshtein
import SearchVerified.Scoring
import SearchVerified.InvertedIndex
import SearchVerified.ProgressiveLoading
import SearchVerified.SearchOptions
import SearchVerified.Suggestions
import SearchVerified.StreamingSearch
import SearchVerified.Section
