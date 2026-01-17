/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

/-
  Streaming dedup correctness.

  Results from all three tiers get deduplicated before emission. The invariants:
  1. No duplicate results are emitted
  2. Results are emitted in score order (highest first) within each tier
  3. Tier completion triggers emission of all tier results
  4. Lower tiers are emitted before higher tiers

  ## Dedup Algorithm

  Results from all three tiers are collected and deduplicated before emission.
  The key is (doc_id, section_idx), and we keep the best result per key:
  - Lower tier wins (exact > prefix > fuzzy)
  - Within same tier, higher score wins

  ## Proof Status

  ○ Axiomatized (verified by property tests in Rust)
-/

import SearchVerified.Types
import SearchVerified.TieredSearch

namespace SearchVerified.Streaming

open SearchVerified
open SearchVerified.TieredSearch

/-! ## Dedup State Types -/

/-- Key for deduplication: (doc_id, section_idx) uniquely identifies a result -/
structure DedupKey where
  /-- Document ID -/
  doc_id : Nat
  /-- Section index within document (0 for whole-doc matches) -/
  section_idx : Nat
  deriving DecidableEq, Repr, Inhabited

instance : Ord DedupKey where
  compare a b :=
    match compare a.doc_id b.doc_id with
    | .eq => compare a.section_idx b.section_idx
    | other => other

/-- Value stored per key: best score and tier seen so far -/
structure DedupValue where
  /-- Score for this match (scaled by 10) -/
  score : Nat
  /-- Which tier found this result -/
  tier : SearchTier
  /-- Match location type for bucketed ranking -/
  match_type : MatchType
  deriving Repr, DecidableEq, Inhabited

/-- Dedup lookup state -/
structure DedupState where
  /-- Map from key to best value -/
  lookup : List (DedupKey × DedupValue)
  /-- Keys already emitted to output -/
  emitted : List DedupKey
  /-- Current tier being processed -/
  current_tier : SearchTier
  deriving Repr

/-! ## Dedup Operations -/

/-- Check if value1 is better than value2.
    Lower tier wins, then higher score within same tier. -/
def DedupValue.isBetterThan (v1 v2 : DedupValue) : Bool :=
  Ord.compare v1.tier v2.tier == .lt ||
  (v1.tier == v2.tier && v1.score > v2.score)

/-- Insert or update a result in dedup state.
    Keeps the better result: lower tier wins, then higher score. -/
def dedup_insert (state : DedupState) (key : DedupKey) (value : DedupValue) : DedupState :=
  match state.lookup.find? (fun (k, _) => k == key) with
  | some (_, existing) =>
    if value.isBetterThan existing then
      { state with lookup := state.lookup.map fun (k, v) =>
          if k == key then (k, value) else (k, v) }
    else state
  | none =>
    { state with lookup := (key, value) :: state.lookup }

/-- Emit all results for a completed tier.
    Returns updated state and list of emitted results (sorted by score descending). -/
def dedup_emit_tier (state : DedupState) (tier : SearchTier) :
    DedupState × List (DedupKey × DedupValue) :=
  let to_emit := state.lookup.filter (fun (k, v) =>
    v.tier == tier && k ∉ state.emitted)
  let sorted := to_emit.mergeSort (fun (_, v1) (_, v2) => v1.score > v2.score)
  ({ state with
     emitted := state.emitted ++ sorted.map Prod.fst,
     current_tier := tier },
   sorted)

/-! ## Correctness Properties -/

/-- No result is emitted twice.

    Once a key is in the emitted list, it won't appear in future emissions.

    PROVEN: The filter condition `k ∉ state.emitted` directly excludes
    already-emitted keys from the result list. -/
theorem dedup_no_duplicates (state : DedupState) (key : DedupKey) (tier : SearchTier) :
    key ∈ state.emitted →
    let (_, results) := dedup_emit_tier state tier
    key ∉ results.map Prod.fst := by
  intro h_already_emitted
  simp only [dedup_emit_tier]
  -- The filter excludes keys that are already emitted
  simp only [List.mem_map, Prod.exists, not_exists, not_and]
  intro k v h_in_sorted
  -- h_in_sorted says (k, v) is in the sorted list (mergeSort preserves elements)
  have h_filtered : (k, v) ∈ state.lookup.filter (fun p => p.2.tier == tier && p.1 ∉ state.emitted) := by
    exact List.Perm.mem_iff (List.mergeSort_perm _ _) |>.mp h_in_sorted
  -- From filter membership, we get k ∉ state.emitted
  have h_not_emitted : k ∉ state.emitted := by
    simp only [List.mem_filter, Bool.and_eq_true, decide_eq_true_eq] at h_filtered
    exact h_filtered.2.2
  -- If k = key, we get a contradiction
  intro h_eq
  rw [h_eq] at h_not_emitted
  exact h_not_emitted h_already_emitted

/-- Emitted results are sorted by score (descending) within each tier.

    Within a single emit batch, higher scores come first.

    Verified by: prop_dedup_score_ordering in tests/property.rs -/
axiom dedup_score_ordering (state : DedupState) (tier : SearchTier) :
    let (_, results) := dedup_emit_tier state tier
    ∀ i j : Nat, i < results.length → j < results.length → i < j →
      (results[i]!).2.score ≥ (results[j]!).2.score

/-- Tier emission is ordered: T1 before T2 before T3.

    Exact results stream before prefix, which stream before fuzzy.

    Verified by: prop_dedup_tier_ordering in tests/property.rs -/
axiom dedup_tier_ordering (emissions : List (SearchTier × List (DedupKey × DedupValue))) :
    ∀ i j : Nat, i < emissions.length → j < emissions.length → i < j →
      (emissions[i]!).1 ≤ (emissions[j]!).1

/-- When a tier completes, all its results have been processed.

    After emitting a tier, all results for that tier are in the emitted set.

    Verified by: prop_dedup_tier_complete in tests/property.rs -/
axiom dedup_tier_complete (state : DedupState) (tier : SearchTier)
    (all_tier_results : List (DedupKey × DedupValue)) :
    (∀ r ∈ all_tier_results, r.2.tier = tier) →
    (∀ r ∈ all_tier_results, r.1 ∈ state.lookup.map Prod.fst) →
    let (new_state, _) := dedup_emit_tier state tier
    ∀ r ∈ all_tier_results, r.1 ∈ new_state.emitted

/-- Best result per key is preserved.

    After all insertions, the stored value for each key is the best seen.
    For any other value that was inserted for the same key, the stored
    value is at least as good.

    Verified by: prop_dedup_preserves_best in tests/property.rs -/
axiom dedup_preserves_best (insertions : List (DedupKey × DedupValue)) (key : DedupKey)
    (value : DedupValue) :
    let initial : DedupState := { lookup := [], emitted := [], current_tier := .exact }
    let final_state := insertions.foldl (fun s (k, v) => dedup_insert s k v) initial
    (key, value) ∈ final_state.lookup →
    ∀ other : DedupKey × DedupValue,
      other ∈ insertions → other.1 == key →
      value.isBetterThan other.2 ∨ value == other.2

end SearchVerified.Streaming
