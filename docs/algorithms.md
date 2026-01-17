# Algorithms

A detailed look at the data structures and algorithms that make Sorex fast. Skip this if you just want to use the library - read it if you want to understand *why* things work.

---

## Suffix Arrays

Suffix arrays enable O(log n) prefix search. Given a query like "auth", find all terms starting with "auth" without scanning every term.

### The Core Idea

A suffix array is all suffixes of a string, sorted lexicographically. For the vocabulary `["apple", "apply", "banana"]`:

```
Suffixes of "apple":   "apple", "pple", "ple", "le", "e"
Suffixes of "apply":   "apply", "pply", "ply", "ly", "y"
Suffixes of "banana":  "banana", "anana", "nana", "ana", "na", "a"

All suffixes sorted:
  "a"         → banana[5]
  "ana"       → banana[3]
  "anana"     → banana[1]
  "apple"     → apple[0]
  "apply"     → apply[0]
  "banana"    → banana[0]
  "e"         → apple[4]
  "le"        → apple[3]
  "ly"        → apply[3]
  "na"        → banana[4]
  "nana"      → banana[2]
  "ple"       → apple[2]
  "ply"       → apply[2]
  "pple"      → apple[1]
  "pply"      → apply[1]
  "y"         → apply[4]
```

### Vocabulary Suffix Array

Full-text suffix arrays are expensive - O(text_length) entries. Sorex builds suffix arrays over the *vocabulary* instead:

```
Vocabulary: 10,000 unique terms (~50KB)
Full text: 500KB

Full-text SA: ~500,000 entries (expensive)
Vocabulary SA: ~50,000 entries (cheap)
```

Each entry is a `(term_idx, offset)` pair pointing to a suffix within a vocabulary term.

### Binary Search for Prefix Matching

To find all terms starting with "app":

```rust
// Binary search for first suffix ≥ "app"
let start = suffix_array.partition_point(|entry| {
    suffix_at(entry) < "app"
});

// Scan forward while still matching
let mut matches = vec![];
for entry in &suffix_array[start..] {
    let suffix = suffix_at(entry);
    if suffix.starts_with("app") && entry.offset == 0 {
        matches.push(entry.term_idx);  // "apple", "apply"
    } else if !suffix.starts_with("app") {
        break;  // Past all "app*" suffixes
    }
}
```

We only count matches where `offset == 0` because we want terms *starting* with the prefix, not containing it mid-word.

### Complexity

| Operation | Time | Space | Speed |
|-----------|------|-------|-------|
| Build SA | O(n log n) | O(n) | <span class="complexity complexity-medium">Build-time</span> |
| Prefix search | O(log n + k) | O(k) | <span class="complexity complexity-fast">~10μs</span> |

Where n = total suffix count, k = number of matches.

### Lean Verification

The suffix array invariant is specified in `SuffixArray.lean`:

```lean
-- Suffix array is sorted by suffix strings
def Sorted (sa : Array SuffixEntry) (texts : Array String) : Prop :=
  ∀ i j, i < j → i < sa.size → j < sa.size →
    suffixAt texts sa[i] ≤ suffixAt texts sa[j]

-- Binary search correctness depends on sortedness
axiom findFirstGe_correct :
  Sorted sa texts →
  let idx := findFirstGe sa texts target
  ∀ k < idx, suffixAt texts sa[k] < target
```

---

## Levenshtein Automata (Schulz-Mihov 2002)

<aside class="skip-note">

*This section covers automata theory and DFA construction. If you just want to use fuzzy search, [skip to Block PFOR](#block-pfor-compression).*

</aside>

Traditional fuzzy search computes edit distance for every term:

```
Query: "auth"
For each term in vocabulary:
  distance = levenshtein("auth", term)  // O(query × term)
  if distance ≤ 2: add to results
```

This is O(vocabulary × query_len × avg_term_len) - slow for large vocabularies.

### The Insight

Edit distance computation follows a pattern that depends only on:
1. The **structure** of the query (length, character positions)
2. The **character classes** of input characters

The pattern is query-independent. We can precompute a universal automaton that works for *any* query.

### Character Classes

For query "cat" and max distance k=2, we look at k+1=3 characters ahead at each position. The character class encodes which of these match:

```
Query: "cat"
Input character: 'a'

At position 0: looking at "cat"
  Does 'a' match 'c'? No  → bit 0 = 0
  Does 'a' match 'a'? Yes → bit 1 = 1
  Does 'a' match 't'? No  → bit 2 = 0
  Character class = 0b010 = 2

At position 1: looking at "at"
  Does 'a' match 'a'? Yes → bit 0 = 1
  Does 'a' match 't'? No  → bit 1 = 0
  Character class = 0b01 = 1
```

For k=2, there are 2^(k+1) = 8 possible character classes.

### Parametric States

The NFA for Levenshtein distance has states like `(position, edits_used)`. A parametric state is a set of these, normalized to be position-independent:

```
State {(0, 0), (1, 1), (2, 2)}  // At position p, can be:
                                // - exactly at p with 0 edits
                                // - 1 ahead with 1 edit (deleted)
                                // - 2 ahead with 2 edits (2 deletions)
```

For k=2, there are only ~70 unique parametric states. The transitions between them depend only on the character class.

### DFA Construction

Build the DFA by exploring all reachable parametric states:

```rust
pub fn build(with_transpositions: bool) -> ParametricDFA {
    let mut states = Vec::new();
    let mut transitions = Vec::new();
    let mut queue = VecDeque::new();

    // Start state: positions (0,0), (1,1), (2,2)
    let initial = ParametricState::new(vec![
        NfaPos { offset: 0, edits: 0 },
        NfaPos { offset: 1, edits: 1 },
        NfaPos { offset: 2, edits: 2 },
    ]);

    queue.push_back(initial.clone());
    states.push(initial);

    while let Some(state) = queue.pop_front() {
        // For each character class 0-7
        for char_class in 0..8 {
            let next = state.next(char_class, with_transpositions);
            // ... add to states if new, record transition
        }
    }

    ParametricDFA { states, transitions, ... }
}
```

### Query-Specific Matcher

At query time, build a matcher that computes character classes for the specific query:

```rust
impl QueryMatcher {
    pub fn new(dfa: &ParametricDFA, query: &str) -> Self {
        QueryMatcher {
            dfa,
            query: query.chars().collect(),
        }
    }

    pub fn matches(&self, term: &str) -> Option<u8> {
        let mut state = 0;  // Start state

        for ch in term.chars() {
            // Compute character class for this input character
            let char_class = self.char_class_at(state, ch);
            state = self.dfa.transitions[state * 8 + char_class];

            if state == DEAD_STATE {
                return None;  // No match possible
            }
        }

        // Check if final state is accepting
        let distance = self.dfa.accept[state];
        if distance != NOT_ACCEPTING {
            Some(distance)
        } else {
            None
        }
    }
}
```

### Complexity

| Operation | Time | Space | Speed |
|-----------|------|-------|-------|
| Build DFA (once) | O(8^k × k^2) | ~1.2KB for k=2 | <span class="complexity complexity-medium">Build-time</span> |
| Build matcher | O(query_len) | O(query_len) | <span class="complexity complexity-fast">~1μs</span> |
| Match one term | O(term_len) | O(1) | <span class="complexity complexity-fast">~10ns</span> |
| Full fuzzy search | O(vocabulary × avg_term_len) | O(1) | <span class="complexity complexity-medium">~50μs</span> |

The key win: no per-comparison edit distance computation. Just table lookups.

### Performance

Naive Levenshtein: ~10ms for 10K terms
Automaton-based: ~0.1ms for 10K terms

That's 100x faster, and the gap widens with vocabulary size.

### References

- Schulz, K. U., & Mihov, S. (2002). Fast string correction with Levenshtein automata. *International Journal on Document Analysis and Recognition*, 5(1), 67-85.
- Paul Masurel's implementation guide: https://fulmicoton.com/posts/levenshtein/

---

## Block PFOR Compression

<aside class="skip-note">

*Compression internals ahead. [Skip to Inverted Index](#inverted-index) if you just need the API.*

</aside>

Posting lists can be huge. A common term like "the" might appear in every document. Naive storage wastes space:

> **Note:** Block PFOR is a technique popularized by [Apache Lucene](https://lucene.apache.org/) (Lucene 4.0+). Sorex's implementation follows the same principles: 128-document blocks, bit-packing, and exception handling for outliers.

```
Posting list for "the": [0, 1, 2, 3, 4, ...]
Raw storage: 4 bytes × n_docs
```

### Frame of Reference (FOR)

Store deltas instead of absolute values:

```
Doc IDs:  [0, 5, 7, 8, 15, 18]
Deltas:   [0, 5, 2, 1, 7, 3]   // Differences between consecutive IDs
```

Deltas are smaller, need fewer bits.

### Bit Packing

If max delta is 7, we only need 3 bits per value:

```
Deltas:     [5, 2, 1, 7, 3]
3-bit each: [101, 010, 001, 111, 011]
Packed:     101 010 001 111 011 (15 bits = 2 bytes)

vs raw:     5 values × 4 bytes = 20 bytes
Savings:    90%
```

### Block PFOR

Process in 128-document blocks for cache efficiency:

```
For each block of 128 doc_ids:
  1. Compute deltas
  2. Find max delta → determines bits_per_value
  3. Bit-pack all 128 values
  4. Store: min_delta (varint) + bits_per_value (u8) + packed_data
```

Lucene uses this exact scheme. It's fast to decode (single bitshift per value) and compresses well.

### Special Case: Uniform Blocks

If all deltas are identical (common for evenly-spaced documents):

```
Deltas: [1, 1, 1, 1, ...]
min_delta = 1, bits_per_value = 0
No packed data needed!
```

This happens more often than you'd expect in practice.

---

## Inverted Index

The inverted index maps terms to posting lists:

```
"authentication" → [doc_0, doc_5, doc_12]
"authorization"  → [doc_0, doc_3]
"bearer"         → [doc_5, doc_7, doc_12, doc_15]
```

### Structure

```rust
pub struct InvertedIndex {
    terms: HashMap<String, PostingList>,
    total_docs: usize,
}

pub struct PostingList {
    postings: Vec<Posting>,  // Sorted by (doc_id, offset)
    doc_freq: usize,         // Number of unique docs
}

pub struct Posting {
    doc_id: usize,
    offset: usize,           // Position in document
    field_type: FieldType,   // For scoring
    section_id: Option<String>,  // For deep linking
}
```

### Exact Lookup: O(1)

```rust
fn exact_search(index: &InvertedIndex, term: &str) -> Vec<usize> {
    match index.terms.get(term) {
        Some(pl) => pl.postings.iter().map(|p| p.doc_id).collect(),
        None => vec![],
    }
}
```

### Posting List Intersection

For multi-term queries, intersect posting lists:

```
Query: "rust authentication"

"rust"           → [doc_0, doc_3, doc_7, doc_12]
"authentication" → [doc_0, doc_5, doc_12]

Intersection:    → [doc_0, doc_12]  // Both terms present
```

With sorted posting lists, this is O(min(n, m)):

```rust
fn intersect(a: &[usize], b: &[usize]) -> Vec<usize> {
    let mut result = vec![];
    let (mut i, mut j) = (0, 0);

    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            Ordering::Less => i += 1,
            Ordering::Greater => j += 1,
            Ordering::Equal => {
                result.push(a[i]);
                i += 1;
                j += 1;
            }
        }
    }

    result
}
```

### Skip Lists

Skip lists for posting traversal are another classic Lucene technique. For very long posting lists, add skip pointers:

```
Posting list: [0, 5, 12, 18, 25, 33, 41, 50, ...]
Skip pointers (every 8): [0] → [50] → [100] → ...

To find doc_id 45:
  1. Skip to 50 (overshot)
  2. Linear scan from previous skip point
```

Reduces intersection from O(n + m) to O(n log m) for imbalanced lists.

---

## Hybrid Index

Sorex combines inverted index + vocabulary suffix array:

```
HybridIndex
├── inverted_index     // O(1) exact match
├── vocabulary         // Sorted term list
└── vocab_suffix_array // O(log k) prefix match
```

### Search Flow

```
Query: "auth"

1. Exact match: inverted_index.get("auth")
   → Found? Return posting list
   → Not found? Continue...

2. Prefix match: binary search suffix array for "auth*"
   → Find matching term indices
   → Look up each term's posting list
   → Union results

3. Fuzzy match: traverse vocabulary with Levenshtein DFA
   → Find terms within edit distance k
   → Look up posting lists
   → Union results
```

### Why Both?

| Index Type | Best For | Complexity |
|------------|----------|------------|
| Inverted | Exact words | O(1) |
| Suffix Array | Prefixes, substrings | O(log k) |
| Together | Everything | Best of both |

The inverted index handles the common case (exact words) instantly. The suffix array handles the edge cases (partial words, prefixes) without scanning.

---

## Performance Optimizations

Sorex includes several optimizations for large-scale search performance.

### Field Boundary Binary Search

Field boundaries map text offsets to field types (title, heading, content) for scoring. The naive approach is O(n) linear scan:

```rust
// Before: O(n) linear scan
fn get_field_type(boundaries: &[FieldBoundary], doc_id: usize, offset: usize) -> FieldType {
    for b in boundaries {
        if b.doc_id == doc_id && b.start <= offset && offset < b.end {
            return b.field_type.clone();
        }
    }
    FieldType::Content
}
```

Sorex sorts boundaries by `(doc_id, start)` at build time and uses binary search:

```rust
// After: O(log n) binary search + small linear scan
fn get_field_type(boundaries: &[FieldBoundary], doc_id: usize, offset: usize) -> FieldType {
    // Binary search for first boundary with doc_id >= target
    let start = boundaries.partition_point(|b| b.doc_id < doc_id);

    // Linear scan only within this document's boundaries
    for b in &boundaries[start..] {
        if b.doc_id > doc_id { break; }
        if offset >= b.start && offset < b.end {
            return b.field_type.clone();
        }
    }
    FieldType::Content
}
```

**Impact:** 5-9% faster search on 500-document datasets.

### Heap-Based Top-K Selection

For result ranking, the naive approach sorts all results then takes top K:

```rust
// Before: O(n log n) full sort
results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());
results.truncate(k);
```

For large result sets (>200 documents), Sorex uses a min-heap:

```rust
// After: O(n log k) heap selection
let mut heap: BinaryHeap<MinScored> = BinaryHeap::with_capacity(k + 1);
for result in results {
    heap.push(MinScored(result));
    if heap.len() > k {
        heap.pop();  // Remove smallest
    }
}
```

**Impact:** O(n log k) vs O(n log n) - significant for large result sets.

### Score Merging Pre-allocation

Multi-term queries merge scores from each term. Pre-allocating HashMaps reduces allocations:

```rust
// Pre-allocate with capacity hint
let mut doc_scores = HashMap::with_capacity(first_term_results.len());

// Reuse single HashMap for term iteration
let mut term_scores = HashMap::with_capacity(avg_term_results);
for term in &terms[1..] {
    term_scores.clear();  // Reuse, don't reallocate
    // ... merge logic
}
```

### Lean Verification

The binary search optimization requires sorted boundaries. This is specified in Types.lean:

```lean
/-- Field boundaries are sorted by (doc_id, start) -/
def FieldBoundary.Sorted (boundaries : Array FieldBoundary) : Prop :=
  ∀ i j, i < j → i < boundaries.size → j < boundaries.size →
    FieldBoundary.lt boundaries[i] boundaries[j] ∨ boundaries[i] = boundaries[j]

/-- Binary search finds the correct starting point -/
axiom findFirstDocBoundary_lower :
  FieldBoundary.Sorted boundaries →
  ∀ k < findFirstDocBoundary boundaries doc_id,
    boundaries[k].doc_id < doc_id
```

---

## Scoring

### Field Type Hierarchy

Matches in different fields have different importance:

```
Title match:   base = 100.0
Heading match: base = 10.0
Content match: base = 1.0
```

These scores are chosen so any title match beats any heading match, regardless of position:

```
Worst title match:   100.0 - 0.5 = 99.5
Best heading match:  10.0 + 0.5 = 10.5

99.5 > 10.5 ✓
```

### Position Boost

Earlier matches get a small bonus:

```
position_boost = max_boost × (1 - position / field_length)
               = 0.5 × (1 - position / field_length)
```

First word in a title gets +0.5, last word gets +0.

### Lean Verification

The field hierarchy is mathematically proven:

```lean
-- In Scoring.lean
theorem title_beats_heading :
  baseScore .title - maxBoost > baseScore .heading + maxBoost := by
  native_decide  -- 100 - 0.5 > 10 + 0.5, checked at compile time

theorem heading_beats_content :
  baseScore .heading - maxBoost > baseScore .content + maxBoost := by
  native_decide  -- 10 - 0.5 > 1 + 0.5

theorem field_type_dominance :
  (baseScore .title - maxBoost > baseScore .heading + maxBoost) ∧
  (baseScore .heading - maxBoost > baseScore .content + maxBoost) := by
  exact ⟨title_beats_heading, heading_beats_content⟩
```

If you change the constants, the proofs fail. This prevents accidental ranking bugs.

---

## Related Documentation

- [Architecture](./architecture.md): Binary format, system overview
- [Rust API](./rust.md): Library API for building indexes programmatically
- [Verification](./verification.md): Lean 4 proofs for algorithm correctness
- [Benchmarks](./benchmarks.md): Performance comparisons with other libraries
- [Integration](./integration.md): WASM setup, browser integration
