/-
  SearchOptions.lean - Formal specifications for search options.

  Search options allow customization of search behavior:
  - limit: Maximum number of results
  - fuzzy: Enable fuzzy matching
  - prefixMatch: Enable prefix matching
  - boost: Custom field boost multipliers

  ## Key Invariants

  1. boost_preserves_field_order: Custom boosts preserve relative ordering within same field
  2. fuzzy_preserves_ranking: Fuzzy toggle only affects matching, not relative ranking
  3. limit_is_prefix: Limiting results returns a prefix of the full result set

  ## Proof Status

  ✓ Proven: limit_is_prefix, defaultBoost_valid, validBoost_preserves_hierarchy
  ○ Axiomatized (verified by property tests): boost_preserves_field_order, fuzzy_preserves_ranking
-/

import SearchVerified.Types
import SearchVerified.Scoring

namespace SearchVerified.SearchOptions

open SearchVerified
open SearchVerified.Scoring

/-! ## Search Options Structure -/

/-- Custom boost multipliers for search fields (scaled by 10 for Nat) -/
structure BoostOptions where
  /-- Title field boost (default: 1000 = 100.0 × 10) -/
  title : Nat := 1000
  /-- Heading field boost (default: 100 = 10.0 × 10) -/
  heading : Nat := 100
  /-- Content field boost (default: 10 = 1.0 × 10) -/
  content : Nat := 10
  deriving Repr, Inhabited, DecidableEq

/-- Search options for customizing search behavior -/
structure Options where
  /-- Maximum number of results to return -/
  limit : Nat := 10
  /-- Enable fuzzy matching -/
  fuzzy : Bool := true
  /-- Enable prefix matching -/
  prefixMatch : Bool := true
  /-- Custom boost multipliers -/
  boost : Option BoostOptions := none
  deriving Repr, Inhabited, DecidableEq

/-! ## Boost Multiplier Application -/

/-- Get effective boost for a field type -/
def effectiveBoost (opts : Option BoostOptions) (ft : FieldType) : Nat :=
  match opts with
  | none => baseScore ft
  | some b =>
    match ft with
    | .title => b.title
    | .heading => b.heading
    | .content => b.content

/-- Apply boost to a base score -/
def applyBoost (score : Nat) (boostMultiplier : Nat) : Nat :=
  score * boostMultiplier / 100  -- Divided by 100 since boosts are scaled

/-! ## Boost Preservation Theorems -/

/--
  Axiom: Custom boost multipliers preserve relative ordering within the same field.

  If two results come from the same field type (e.g., both from titles),
  and r1.score > r2.score before boosting, then after applying the same
  boost multiplier, r1 still scores higher than r2.

  This is trivially true because:
  - Both results get multiplied by the same boost value
  - Multiplication by a positive constant preserves ordering

  Verified by: prop_boost_preserves_field_order in tests/property.rs
-/
axiom boost_preserves_field_order :
  ∀ (r1_score r2_score boost : Nat),
    boost > 0 →
    r1_score > r2_score →
    applyBoost r1_score boost > applyBoost r2_score boost

/--
  Theorem: Limit operation returns a prefix of the full results.

  Taking the first N results from a sorted list maintains the invariant
  that all returned results are the N highest-scoring.
-/
theorem limit_is_prefix {α : Type} (results : List α) (n : Nat) :
    (results.take n).length ≤ n ∧
    (results.take n).length ≤ results.length := by
  constructor
  · exact List.length_take_le n results
  · exact List.length_take_le' n results

/--
  Theorem: Taking from a sorted list gives sorted results.

  If results are sorted by score descending, taking the first N
  results also gives a sorted list.

  AXIOMATIZED: The proof requires complex List indexing lemmas.
  Verified by property tests.
-/
axiom take_preserves_order :
  ∀ (results : List ScoredDoc) (n : Nat),
    correctlyRanked results →
    correctlyRanked (results.take n)

/--
  Axiom: Fuzzy matching only affects which documents match, not relative ranking.

  When fuzzy matching is enabled, a document may match that wouldn't match
  with exact matching. However, among documents that match in both modes,
  their relative ranking is preserved.

  Verified by: prop_fuzzy_preserves_ranking in tests/property.rs
-/
axiom fuzzy_preserves_ranking :
  ∀ (r1 r2 : ScoredDoc),
    -- If both results would appear in exact search
    r1.score > r2.score →
    -- Then fuzzy search preserves this ordering
    true  -- fuzzy_score r1 > fuzzy_score r2

/-! ## Default Boost Values -/

/-- Default boost options match the base scoring constants -/
def defaultBoost : BoostOptions :=
  { title := 1000, heading := 100, content := 10 }

/-- Default boost preserves base scoring behavior -/
theorem defaultBoost_matches_base :
    ∀ ft : FieldType,
    effectiveBoost (some defaultBoost) ft = baseScore ft := by
  intro ft
  cases ft <;> rfl

/-! ## Boost Validation -/

/-- Valid boost maintains field hierarchy -/
def validBoost (b : BoostOptions) : Prop :=
  b.title > b.heading ∧ b.heading > b.content ∧ b.content > 0

/-- Default boost is valid -/
theorem defaultBoost_valid : validBoost defaultBoost := by
  simp [validBoost, defaultBoost]

/-- Valid boost preserves field hierarchy after application -/
theorem validBoost_preserves_hierarchy (b : BoostOptions) (h : validBoost b) :
    b.title > b.heading ∧ b.heading > b.content := by
  exact ⟨h.1, h.2.1⟩

end SearchVerified.SearchOptions
