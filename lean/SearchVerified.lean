/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

/-
  Formal specifications for the search engine.

  These Lean modules mirror the Rust implementation. Where possible, properties
  are proven outright. Where that gets tedious (array indexing, mostly), we
  axiomatize and verify via property tests in Rust. The goal isn't 100% formal
  coverage. It's catching the bugs that matter before they ship.

  ## Modules

  - Types: Core data structures mirroring Rust structs (MatchType, FieldBoundary, etc.)
  - SuffixArray: Sorting invariant and completeness proofs
  - BinarySearch: Search correctness theorems + prefix search via vocabulary suffix array
  - Levenshtein: Edit distance bounds for fuzzy search (Tier 3)
  - Scoring: Field type + MatchType dominance and ranking proofs
  - InvertedIndex: Posting list properties and hybrid index specs
  - TieredSearch: Three-tier search architecture + fuzzy scores + multi-term AND
  - Section: Deep-linking section navigation correctness
  - Binary: Binary format (.sorex) roundtrip correctness
  - Streaming: Dedup worker correctness (no duplicates, ordering)
  - Accumulation: Multi-term score accumulation + best section selection

  ## Three-Tier Search Architecture

  The search algorithm uses three tiers with decreasing precision:
  1. **Tier 1 (Exact)**: O(1) inverted index lookup for exact matches
  2. **Tier 2 (Prefix)**: O(log k) suffix array binary search for prefix matches
  3. **Tier 3 (Fuzzy)**: O(vocabulary) Levenshtein DFA for typo tolerance

  See `TieredSearch.lean` for complete specifications.

  ## Proof Status

  ✓ Proven (native_decide/rfl):
  - title_beats_heading, heading_beats_content, field_type_dominance
  - title_matchType_beats_section, section_matchType_beats_subsection
  - subsection_matchType_beats_subsubsection, subsubsection_matchType_beats_content
  - tier1_dominates_tier2, tier2_dominates_tier3
  - fuzzy_bounded_by_prefix, fuzzy_distance1_beats_distance2, fuzzy_distance2_beats_distance3

  ✓ Proven (tactic):
  - tier1_subset_full, tier_completeness_helper
  - matchType_ordering_transitive
  - positionBoost_range, positionBoost_monotone
  - finalScore_positive, finalScore_respects_hierarchy
  - aggregateScores_eq_sum

  ◐ Partially proven:
  - sorted_iff_consecutive (one direction)
  - sorted_transitive (uses hypothesis)
  - withinBounds_sound, early_exit_correct (use length_diff_lower_bound)

  ○ Axiomatized (verified by property tests):
  - Suffix array: build_produces_sorted, build_produces_complete, build_produces_correct_lcp
  - Inverted index: build_produces_wellformed, build_complete
  - Tiered search: tier_completeness, tier_ordering, streaming_ranking, tiered_pipeline_correct
  - Tier exclusion: tier2_disjoint_tier1, tier3_disjoint_tier1_tier2, tier3_excludes_exact_matches
  - Fuzzy score: fuzzy_score_monotone
  - MatchType: fromHeadingLevel_monotone
  - Prefix search: prefix_search_lower_bound, prefix_search_finds_match, prefix_search_complete
  - Multi-term AND: merge_and_requires_all, merge_and_complete, merge_and_score_sum
  - Binary format: varint_roundtrip, varint_prefix_free, section_table_roundtrip,
                   postings_roundtrip, sorex_roundtrip
  - Streaming dedup: dedup_no_duplicates, dedup_score_ordering, dedup_tier_ordering,
                     dedup_tier_complete, dedup_preserves_best
  - Accumulation: rust_accumulator_matches_spec, dedup_uses_total_score
-/

import SearchVerified.Types
import SearchVerified.SuffixArray
import SearchVerified.BinarySearch
import SearchVerified.Levenshtein
import SearchVerified.Scoring
import SearchVerified.InvertedIndex
import SearchVerified.TieredSearch
import SearchVerified.Section
import SearchVerified.Binary
import SearchVerified.Streaming
import SearchVerified.Oracle
import SearchVerified.Accumulation
