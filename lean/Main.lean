/-
  Main.lean - Entry point for proof checking.

  Run with: lake build
  Or: lake exe check_proofs

  This executable reports on the proof status of the search verification.
-/

import SearchVerified

open SearchVerified
open SearchVerified.SuffixArray
open SearchVerified.BinarySearch
open SearchVerified.Levenshtein
open SearchVerified.Scoring

/-- Count of theorems that are fully proven (no sorry) -/
def provenTheorems : List String := [
  -- Scoring
  "title_beats_heading",
  "heading_beats_content",
  "field_type_dominance",
  -- SuffixArray
  "sorted_transitive",
  "SuffixLe_trans",  -- NEW: String ordering transitivity via Mathlib
  -- Section
  "offset_maps_to_unique_section"  -- NEW: Deep-link uniqueness
]

/-- Count of theorems with sorry placeholders -/
def pendingTheorems : List String := [
  -- SuffixArray
  "sorted_iff_consecutive (partial)",
  "build_produces_sorted",
  "build_produces_complete",
  "build_produces_correct_lcp",
  -- BinarySearch
  "findFirstGe_bounds",
  "findFirstGe_lower_bound",
  "findFirstGe_upper_bound",
  "collectMatches_sound",
  "collectMatches_complete",
  "search_correct",
  -- Levenshtein
  "length_diff_lower_bound",
  "editDistance_symm",
  "editDistance_triangle",
  "fuzzyScore_exact",
  "fuzzyScore_monotone",
  "editDistance_drop_prefix",
  -- Scoring
  "positionBoost_range",
  "positionBoost_monotone",
  "finalScore_positive",
  "finalScore_respects_hierarchy",
  "aggregate_monotone",
  "termWeight_positive",
  "termWeight_rarer_higher",
  "search_ranking_correct"
]

def main : IO Unit := do
  IO.println "╔══════════════════════════════════════════════════════════════╗"
  IO.println "║          SearchVerified - Proof Status Report                ║"
  IO.println "╠══════════════════════════════════════════════════════════════╣"
  IO.println ""
  IO.println s!"  ✓ Proven theorems:     {provenTheorems.length}"
  IO.println s!"  ○ Pending (sorry):     {pendingTheorems.length}"
  IO.println s!"  Total:                 {provenTheorems.length + pendingTheorems.length}"
  IO.println ""
  IO.println "╠══════════════════════════════════════════════════════════════╣"
  IO.println "║  Proven Theorems                                             ║"
  IO.println "╠══════════════════════════════════════════════════════════════╣"
  for thm in provenTheorems do
    IO.println s!"  ✓ {thm}"
  IO.println ""
  IO.println "╠══════════════════════════════════════════════════════════════╣"
  IO.println "║  Pending Theorems (require Mathlib or manual proof)          ║"
  IO.println "╠══════════════════════════════════════════════════════════════╣"
  for thm in pendingTheorems do
    IO.println s!"  ○ {thm}"
  IO.println ""
  IO.println "╚══════════════════════════════════════════════════════════════╝"
  IO.println ""
  IO.println "To prove pending theorems, consider:"
  IO.println "  • Import Mathlib.Data.List.EditDistance for Levenshtein"
  IO.println "  • Use omega tactic for Nat arithmetic"
  IO.println "  • Use native_decide for decidable propositions"
  IO.println ""
  return ()
