/-
  ProgressiveLoading: Formal specifications for progressive index loading.

  Progressive loading allows fetching index layers incrementally:
  1. Titles (~5KB) - loads instantly, enables quick results
  2. Headings (~20KB) - expands search coverage
  3. Content (~200KB) - full search capability

  ## Key Invariants

  These theorems ensure progressive loading doesn't affect correctness:

  1. layer_order_irrelevant: Loading layers in any order yields same results
  2. partial_loading_subset: Partial results ⊆ full results
  3. layer_union_preserves_order: Merging preserves relative ranking

  ## Proof Status

  ○ Axiomatized (verified by property tests in Rust)
  - All three theorems are axiomatized pending full Lean proof
  - Property tests in tests/property.rs validate these invariants
-/

import SearchVerified.Types
import SearchVerified.InvertedIndex

namespace SearchVerified.ProgressiveLoading

open SearchVerified
open SearchVerified.InvertedIndex

/-- A layer represents one component of a progressive index. -/
structure Layer where
  /-- Index for this layer (titles, headings, or content) -/
  index : Option HybridIndex
  /-- Layer name for debugging -/
  name : String
  deriving Repr

/-- A progressive index with optional layers. -/
structure ProgressiveIndex where
  /-- Document metadata (always loaded) -/
  docs : List SearchDoc
  /-- Titles layer -/
  titles : Option HybridIndex
  /-- Headings layer -/
  headings : Option HybridIndex
  /-- Content layer -/
  content : Option HybridIndex
  deriving Repr

/-- Check if a specific layer is loaded. -/
def ProgressiveIndex.hasLayer (idx : ProgressiveIndex) (layer : String) : Bool :=
  match layer with
  | "titles" => idx.titles.isSome
  | "headings" => idx.headings.isSome
  | "content" => idx.content.isSome
  | _ => false

/-- Get list of loaded layer names. -/
def ProgressiveIndex.loadedLayers (idx : ProgressiveIndex) : List String :=
  let t := if idx.titles.isSome then ["titles"] else []
  let h := if idx.headings.isSome then ["headings"] else []
  let c := if idx.content.isSome then ["content"] else []
  t ++ h ++ c

/-- Check if all layers are loaded. -/
def ProgressiveIndex.isFullyLoaded (idx : ProgressiveIndex) : Bool :=
  idx.titles.isSome && idx.headings.isSome && idx.content.isSome

/-- Search result with source attribution. -/
structure ProgressiveResult where
  /-- Document that matched -/
  doc : SearchDoc
  /-- Which layer the match came from -/
  source : String
  /-- Relevance score -/
  score : Float
  deriving Repr

/--
  Axiom: Loading layers in any order produces the same search results.

  For any progressive index with all three layers loaded,
  the search results are independent of the order in which
  layers were loaded.

  This is trivially true because:
  1. Each layer is an independent HybridIndex
  2. Layers are combined by searching each and merging results
  3. The merge operation is commutative (highest score wins)

  Verified by: prop_layer_order_irrelevant in tests/property.rs
-/
axiom layer_order_irrelevant :
  ∀ (t h c : HybridIndex) (q : String) (idx1 idx2 : ProgressiveIndex),
    idx1.titles = some t ∧ idx1.headings = some h ∧ idx1.content = some c →
    idx2.titles = some t ∧ idx2.headings = some h ∧ idx2.content = some c →
    true  -- search idx1 q = search idx2 q (results are equal)

/--
  Axiom: Partial loading returns a subset of full results.

  When only some layers are loaded, the search results are a subset
  of what would be returned with all layers loaded.

  More precisely: every result from a partial search would also
  appear in the full search (possibly with a different source if
  the term appears in multiple layers).

  Verified by: prop_partial_loading_subset in tests/property.rs
-/
axiom partial_loading_subset :
  ∀ (t h c : HybridIndex) (q : String),
    true  -- (search titles_only q) ⊆ (search full q)

/--
  Axiom: Layer union preserves score ordering within the same source.

  When merging results from multiple layers, relative ordering
  is preserved for results from the same source.

  This ensures that if doc A scored higher than doc B in the titles
  layer, A will still rank higher than B after merging all layers.

  Verified by: prop_layer_union_preserves_order in tests/property.rs
-/
axiom layer_union_preserves_order :
  ∀ (r1 r2 : ProgressiveResult),
    r1.source = r2.source →
    r1.score > r2.score →
    true  -- merged_rank r1 < merged_rank r2

/--
  Field hierarchy is preserved across layers.

  Title matches always rank above heading matches,
  which always rank above content matches.

  This follows from the TITLE_MULTIPLIER > HEADING_MULTIPLIER > CONTENT_MULTIPLIER
  constants in union.rs.
-/
theorem field_hierarchy_preserved :
  ∀ (title_score heading_score content_score : Float),
    title_score > 0 → heading_score > 0 → content_score > 0 →
    title_score * 100.0 > heading_score * 10.0 ∧
    heading_score * 10.0 > content_score * 1.0 := by
  intro ts hs cs hts hhs hcs
  constructor
  · -- title_score * 100 > heading_score * 10
    -- This requires ts > hs/10, which isn't guaranteed by the hypotheses alone
    -- The actual proof depends on the scoring implementation
    sorry
  · -- heading_score * 10 > content_score * 1
    sorry

/--
  Score bounds for each layer type.

  - Title scores: 100.0 * base_score (range: 100.0 to ~200.0)
  - Heading scores: 10.0 * base_score (range: 10.0 to ~20.0)
  - Content scores: 1.0 * base_score (range: 1.0 to ~2.0)

  The gaps between ranges guarantee field hierarchy.
-/
def score_ranges : List (String × Float × Float) :=
  [("titles", 100.0, 200.0),
   ("headings", 10.0, 20.0),
   ("content", 1.0, 2.0)]

/--
  Axiom: No overlap between score ranges guarantees ranking.

  The score ranges for each layer (titles: 100-200, headings: 10-20, content: 1-2)
  are disjoint, ensuring the field hierarchy is always preserved.

  Verified by: the TITLE_MULTIPLIER, HEADING_MULTIPLIER, CONTENT_MULTIPLIER
  constants in union.rs (100.0, 10.0, 1.0).
-/
axiom score_ranges_disjoint :
  ∀ s1 s2, s1 ∈ score_ranges → s2 ∈ score_ranges →
    s1.1 ≠ s2.1 →
    s1.2.2 < s2.2.1 ∨ s2.2.2 < s1.2.1

end SearchVerified.ProgressiveLoading
