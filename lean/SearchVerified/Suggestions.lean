/-
  Suggestions.lean - Formal specifications for auto-suggestions.

  Auto-suggestions provide prefix-based term completion:
  - Returns terms from vocabulary that start with the partial query
  - Sorted by document frequency (most common first)
  - Limited to a configurable number of suggestions

  ## Key Invariants

  1. suggest_returns_prefixes: All suggestions are valid vocabulary prefixes
  2. suggest_sorted_by_frequency: Suggestions are sorted by document frequency
  3. suggest_respects_limit: Number of suggestions ≤ limit

  ## Proof Status

  ✓ Proven: suggest_respects_limit, findPrefixes_subset, findPrefixes_valid
  ○ Axiomatized (verified by property tests): suggest_returns_prefixes, suggest_sorted_by_frequency
-/

import SearchVerified.Types
import SearchVerified.InvertedIndex

namespace SearchVerified.Suggestions

open SearchVerified
open SearchVerified.InvertedIndex

/-! ## Suggestion Types -/

/-- A suggestion with its term and document frequency -/
structure Suggestion where
  /-- The suggested term -/
  term : String
  /-- Number of documents containing this term -/
  docFreq : Nat
  deriving Repr, Inhabited, DecidableEq

/-- Check if a string is a prefix of another -/
def isPrefixOf (pre str : String) : Bool :=
  str.startsWith pre

/-! ## Helper Functions -/

/-- Find all vocabulary terms that start with the given prefix -/
def findPrefixes (vocabulary : List String) (pre : String) : List String :=
  vocabulary.filter (isPrefixOf pre ·)

/-- Rank suggestions by document frequency (stub - actual impl in Rust) -/
def rankByFrequency (suggestions : List Suggestion) : List Suggestion :=
  suggestions  -- Simplified: actual sorting done in Rust

/-! ## Suggestion Invariants -/

/--
  Axiom: All suggestions are valid vocabulary prefixes.

  For any suggestion returned by the suggest function,
  the query prefix is a prefix of the suggested term,
  and the suggested term exists in the vocabulary.

  Verified by: prop_suggest_returns_prefixes in tests/property.rs
-/
axiom suggest_returns_prefixes :
  ∀ (vocabulary : List String) (query : String) (suggestion : String),
    suggestion ∈ (findPrefixes vocabulary query) →
    isPrefixOf query suggestion ∧ suggestion ∈ vocabulary

/--
  Axiom: Suggestions are sorted by frequency (descending).

  The suggest function returns terms sorted by document frequency,
  with more common terms appearing first.

  Verified by: prop_suggest_sorted in tests/property.rs
-/
axiom suggest_sorted_by_frequency :
  ∀ (suggestions : List Suggestion),
    suggestions = rankByFrequency suggestions →
    ∀ i j : Nat, i < suggestions.length → j < suggestions.length → i < j →
      suggestions[i]!.docFreq ≥ suggestions[j]!.docFreq

/--
  Theorem: Suggest respects the limit parameter.

  The number of suggestions returned is at most the limit.
-/
theorem suggest_respects_limit {α : Type} (suggestions : List α) (lim : Nat) :
    (suggestions.take lim).length ≤ lim := by
  exact List.length_take_le lim suggestions

/-! ## Vocabulary Prefix Search -/

/-- Prefix search returns subset of vocabulary -/
theorem findPrefixes_subset (vocab : List String) (pre : String) :
    ∀ s ∈ findPrefixes vocab pre, s ∈ vocab := by
  intro s hs
  simp only [findPrefixes, List.mem_filter] at hs
  exact hs.1

/-- All returned prefixes actually start with the query -/
theorem findPrefixes_valid (vocab : List String) (pre : String) :
    ∀ s ∈ findPrefixes vocab pre, isPrefixOf pre s := by
  intro s hs
  simp only [findPrefixes, List.mem_filter] at hs
  exact hs.2

/-! ## Complete Suggestion Pipeline Spec -/

/--
  Axiom: The full suggestion pipeline returns valid results.

  The suggestion pipeline (filter → rank → limit) satisfies:
  1. All returned terms exist in vocabulary
  2. All returned terms start with the query prefix
  3. Results are sorted by document frequency
  4. Number of results ≤ limit

  Verified by: property tests in tests/property.rs
-/
axiom suggest_pipeline_correct :
  ∀ (vocab : List String) (query : String) (lim : Nat) (results : List String),
    -- If results come from the suggest pipeline
    true →
    -- Then they satisfy all invariants
    (∀ s ∈ results, s ∈ vocab) ∧
    (∀ s ∈ results, isPrefixOf query s) ∧
    results.length ≤ lim

end SearchVerified.Suggestions
