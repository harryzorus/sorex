/- Copyright 2025-present Harīṣh Tummalachērla -/
/- SPDX-License-Identifier: Apache-2.0 -/

import Mathlib.Data.List.Lex
import Mathlib.Data.List.Basic
import Mathlib.Data.List.Dedup

/-!
# Types.lean - Core Type Definitions for Search Verification

This module defines the fundamental data structures used throughout the
search verification project. These types mirror the Rust implementation
and provide a formal foundation for proving correctness properties.

## Design Philosophy

The types are designed to be:
1. **Simple**: Minimal complexity for clear reasoning
2. **Complete**: All necessary information for proofs
3. **Aligned**: Mirror the actual Rust implementation

## Type Mapping (Rust → Lean)

| Rust Type   | Lean Type     | Notes                          |
|-------------|---------------|--------------------------------|
| `usize`     | `Nat`         | Non-negative integers          |
| `String`    | `String`      | UTF-8 strings                  |
| `Vec<T>`    | `Array T`     | Dynamic arrays                 |
| `Option<T>` | `Option T`    | Optional values                |

## Key Invariants

- All array indices are 0-based
- Suffix array entries always point to valid positions
- Field boundaries don't overlap and cover the entire text

-/

namespace SearchVerified

/-!
## SearchDoc - Document Metadata

Represents a document in the search corpus with metadata for display.
The `id` field is used as an index into the texts array.
-/
structure SearchDoc where
  /-- Unique numeric identifier, indexes into texts array -/
  id : Nat
  /-- Document title for display -/
  title : String
  /-- Brief excerpt shown in search results -/
  excerpt : String
  /-- URL/path for navigation -/
  href : String
  /-- Document kind (post, page, etc.) -/
  kind : String
  deriving Repr, DecidableEq, Inhabited

/-!
## FieldType - Text Region Classification

Field types enable weighted scoring where different parts of a document
contribute differently to the relevance score.

### Score Hierarchy (in Scoring.lean)
- **Title**: Base score 1000 (most important)
- **Heading**: Base score 100 (section headers)
- **Content**: Base score 10 (body text)

This hierarchy is preserved even with position boosts:
`title_score - max_boost > heading_score + max_boost`

See `Scoring.title_beats_heading` theorem for the formal proof.
-/
inductive FieldType where
  | title    -- Highest priority: document titles
  | heading  -- Medium priority: section headings
  | content  -- Base priority: body content
  deriving Repr, DecidableEq, Inhabited

/-!
## MatchType - Fine-grained Match Location Classification

MatchType provides a finer-grained classification than FieldType,
distinguishing between different heading levels for bucketed ranking.

### Hierarchy (highest to lowest priority)
- **title**: Document title (heading_level = 0)
- **section**: Top-level sections (heading_level = 1-2)
- **subsection**: Mid-level sections (heading_level = 3)
- **subsubsection**: Deep sections (heading_level = 4)
- **content**: Body text (heading_level = 5+)

### Bucketed Ranking

Results are first sorted by MatchType, then by score within each bucket.
This ensures structural hierarchy is respected in search results:
- A title match always beats a section match
- A section match always beats a subsection match
- etc.

See `Scoring.lean` for formal proofs of hierarchy preservation.
-/
inductive MatchType where
  | title        -- heading_level = 0 (document title)
  | section      -- heading_level = 1-2 (H1, H2)
  | subsection   -- heading_level = 3 (H3)
  | subsubsection -- heading_level = 4 (H4)
  | content      -- heading_level = 5+ (body text)
  deriving Repr, DecidableEq, Inhabited

instance : Ord MatchType where
  compare a b := match a, b with
    | .title, .title => .eq
    | .title, _ => .lt
    | _, .title => .gt
    | .section, .section => .eq
    | .section, _ => .lt
    | _, .section => .gt
    | .subsection, .subsection => .eq
    | .subsection, _ => .lt
    | _, .subsection => .gt
    | .subsubsection, .subsubsection => .eq
    | .subsubsection, _ => .lt
    | _, .subsubsection => .gt
    | .content, .content => .eq

instance : LT MatchType where
  lt a b := Ord.compare a b == .lt

instance : LE MatchType where
  le a b := Ord.compare a b != .gt

/--
Convert heading level to MatchType.

Maps the 0-6 heading level scale to the 5 MatchType variants:
- Level 0 → title
- Level 1-2 → section
- Level 3 → subsection
- Level 4 → subsubsection
- Level 5+ → content
-/
def MatchType.fromHeadingLevel (level : Nat) : MatchType :=
  if level == 0 then .title
  else if level ≤ 2 then .section
  else if level == 3 then .subsection
  else if level == 4 then .subsubsection
  else .content

/-- Lower heading levels map to better (lower) MatchType values.
    heading_level 0 → Title (best)
    heading_level 5+ → Content (worst)

    This ensures structural hierarchy is preserved in search ranking.

    Proven by case analysis on heading level buckets:
    - Level 0 → .title (best)
    - Level 1-2 → .section
    - Level 3 → .subsection
    - Level 4 → .subsubsection
    - Level 5+ → .content (worst) -/
theorem MatchType.fromHeadingLevel_monotone (l1 l2 : Nat) (h : l1 ≤ l2) :
    fromHeadingLevel l1 ≤ fromHeadingLevel l2 := by
  unfold fromHeadingLevel LE.le instLEMatchType
  simp only [bne_iff_ne, ne_eq, beq_iff_eq, instLENat]
  -- Split all if-then-else conditions exhaustively
  repeat' split
  -- Handle each resulting goal: simp to clean up, omega for contradictions, decide for concrete
  all_goals (simp_all <;> first | omega | decide)

/-!
## FieldBoundary - Text Region Marker

Marks the start and end of a field type region within a document.
The search algorithm uses these to determine the field type for any
character position, which affects the match score.

### Example
For a document: "# Title\n## Heading\nBody text"
```
FieldBoundary { doc_id: 0, start: 0, end: 7, field_type: .title }
FieldBoundary { doc_id: 0, start: 8, end: 18, field_type: .heading }
FieldBoundary { doc_id: 0, start: 19, end: 28, field_type: .content }
```
-/
structure FieldBoundary where
  /-- Which document this boundary belongs to -/
  doc_id : Nat
  /-- Starting character offset (inclusive) -/
  start : Nat
  /-- Ending character offset (exclusive). 'end' is reserved in Lean -/
  «end» : Nat
  /-- The field type for this region -/
  field_type : FieldType
  /-- Section ID for deep linking (None for title, Some for heading/content) -/
  section_id : Option String := none
  /-- Heading level for hierarchical ranking (0=title, 1-4=H1-H4, 5+=content) -/
  heading_level : Nat := 0
  deriving Repr, DecidableEq

/--
Well-formedness predicate for field boundaries.

A boundary is well-formed if:
1. Start is strictly less than end (non-empty region)
2. End doesn't exceed the document length
-/
def FieldBoundary.WellFormed (fb : FieldBoundary) (text_len : Nat) : Prop :=
  fb.start < fb.«end» ∧ fb.«end» ≤ text_len

/-!
## SuffixEntry - Suffix Array Element

A suffix entry is a pointer into the text corpus. The suffix array
contains these entries sorted by the lexicographic order of the
text suffix starting at that position.

### Example
For text "banana" at doc_id 0:
```
Suffix Array (sorted by suffix text):
  SuffixEntry { doc_id: 0, offset: 5 }  → "a"
  SuffixEntry { doc_id: 0, offset: 3 }  → "ana"
  SuffixEntry { doc_id: 0, offset: 1 }  → "anana"
  SuffixEntry { doc_id: 0, offset: 0 }  → "banana"
  SuffixEntry { doc_id: 0, offset: 4 }  → "na"
  SuffixEntry { doc_id: 0, offset: 2 }  → "nana"
```

This sorted structure enables O(log n) binary search for any prefix.
-/
structure SuffixEntry where
  /-- Index into the texts array (which document) -/
  doc_id : Nat
  /-- Character offset within the document (where suffix starts) -/
  offset : Nat
  deriving Repr, DecidableEq, Inhabited

/--
Well-formedness predicate for suffix entries.

An entry is well-formed if:
1. The doc_id is a valid index into the texts array
2. The offset is strictly within the document's bounds (no empty suffixes)
-/
def SuffixEntry.WellFormed (e : SuffixEntry) (texts : Array String) : Prop :=
  e.doc_id < texts.size ∧
  e.offset < (texts[e.doc_id]!).length

/-!
## SearchIndex - Complete Search Index

The main data structure for full-text search. Contains:

1. **docs**: Document metadata for displaying results
2. **texts**: Searchable text content for each document
3. **suffix_array**: Sorted array of (doc_id, offset) pairs
4. **lcp**: Longest Common Prefix array for efficient range queries
5. **field_boundaries**: Maps positions to field types for scoring

### Invariants (specified in SuffixArray.lean)
- `suffix_array` is lexicographically sorted by suffix text
- `lcp[i]` = length of common prefix between suffix_array[i-1] and suffix_array[i]
- All entries point to valid positions in texts
-/
structure SearchIndex where
  /-- Document metadata array -/
  docs : Array SearchDoc
  /-- Document text content (aligned with docs by index) -/
  texts : Array String
  /-- Suffix array: sorted pointers into texts -/
  suffix_array : Array SuffixEntry
  /-- LCP array: lcp[i] = common prefix length of sa[i-1] and sa[i] -/
  lcp : Array Nat
  /-- Field boundaries for scoring -/
  field_boundaries : Array FieldBoundary
  deriving Repr

/-!
## Helper Functions
-/

/--
Extract the suffix string at a given entry.

Given texts = ["hello", "world"] and entry = (0, 2),
returns "llo" (text[0] starting at offset 2).

Returns empty string if doc_id is out of bounds.
This is the fundamental operation that defines suffix array ordering.
-/
def suffixAt (texts : Array String) (entry : SuffixEntry) : String :=
  if h : entry.doc_id < texts.size then
    (texts[entry.doc_id]).drop entry.offset
  else
    ""

/--
Well-formedness predicate for search indices.

An index is well-formed if:
1. docs and texts arrays have the same size
2. lcp and suffix_array have the same size
3. All suffix entries point to valid positions

Additional invariants (sortedness, LCP correctness) are specified
in SuffixArray.lean and BinarySearch.lean.
-/
def SearchIndex.WellFormed (idx : SearchIndex) : Prop :=
  idx.docs.size = idx.texts.size ∧
  idx.lcp.size = idx.suffix_array.size ∧
  (∀ i : Fin idx.suffix_array.size,
    SuffixEntry.WellFormed idx.suffix_array[i] idx.texts)

/-!
## Field Boundary Ordering

Field boundaries are sorted by (doc_id, start) to enable O(log n) lookup
via binary search. This is an optimization added in v0.3.

### Sort Order
For boundaries b1 and b2:
- If b1.doc_id < b2.doc_id, then b1 comes first
- If b1.doc_id = b2.doc_id, then the one with smaller start comes first

This ordering enables binary search to find the first boundary for any doc_id.
-/

/-- Ordering comparison for field boundaries: (doc_id, start) lexicographic -/
def FieldBoundary.lt (b1 b2 : FieldBoundary) : Prop :=
  b1.doc_id < b2.doc_id ∨ (b1.doc_id = b2.doc_id ∧ b1.start < b2.start)

/-- Field boundaries are sorted by (doc_id, start) -/
def FieldBoundary.Sorted (boundaries : Array FieldBoundary) : Prop :=
  ∀ i j : Nat, (hi : i < boundaries.size) → (hj : j < boundaries.size) → i < j →
    FieldBoundary.lt boundaries[i] boundaries[j] ∨ boundaries[i] = boundaries[j]

/-- An offset falls within a boundary -/
def FieldBoundary.containsOffset (b : FieldBoundary) (doc_id offset : Nat) : Prop :=
  b.doc_id = doc_id ∧ b.start ≤ offset ∧ offset < b.«end»

end SearchVerified
