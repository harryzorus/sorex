---
title: Architecture
description: System design, three-tier search, and formal verification
order: 31
---

# Architecture

System design: three-tier search, parallel build, formal verification. For wire format details, see [Binary Format](binary-format.md). For browser runtime, see [Runtime](runtime.md).

---

## Design Principles

Three ideas shape every technical decision:

1. **Precompute everything possible:** Build-time work is free; query-time work is expensive. If you can compute it once, do it at index time.
2. **Compact binary format:** Smaller indices load faster and cache better. Every byte in `.sorex` files earns its place.
3. **Proven correctness:** Formal verification catches bugs that tests miss. The field hierarchy is mathematically proven, not just tested.

The goal: instant search in browsers without sacrificing accuracy or features. A ~150KB WASM bundle shouldn't feel like a compromise.

---

## Visual Overview

### Build and Runtime Flow

```
BUILD TIME                              RUNTIME (Browser)
──────────                              ─────────────────

JSON Documents                          .sorex file
      │                                       │
      v                                       v
+-------------+                         +-------------+
| sorex index |                         | WASM Module |
+-------------+                         +-------------+
      │                                       │
      v                                       v
 .sorex file ───────────────────────▶  +-------------+
                                       | Web Worker  |◀── User Query
                                       +-------------+
                                              │
                                              v
                                       Streaming Results
```

### Web Worker Integration

Search runs in a dedicated Web Worker to keep the main thread free. The UI stays responsive at 60fps even during complex fuzzy searches.

```
┌─────────────────┐                    ┌─────────────────┐
│  Main Thread    │  postMessage(q)    │   Web Worker    │
│                 │ ──────────────────▶│                 │
│  UI Component   │                    │  SorexSearcher  │
│                 │◀── T1: exact ──────│                 │
│                 │◀── T2: prefix ─────│                 │
│                 │◀── T3: fuzzy ──────│                 │
└─────────────────┘                    └─────────────────┘
```

**Why Web Workers?**
- **Non-blocking**: Heavy computation doesn't freeze the UI
- **Streaming**: Results arrive progressively as each tier completes
- **Isolation**: WASM memory is sandboxed in the worker

## System Overview

```
BUILD TIME                                   RUNTIME (WASM)
--------------                               --------------
JSON Payload                                 .sorex binary
    |                                             |
    v                                             v
+---------------------------+              +----------------------------+
| Index Construction        |              | Search Execution           |
|                           |              |                            |
| 1. Tokenize + normalize   |              | 1. Parse query             |
| 2. Build inverted index   |              | 2. Exact match O(1)        |
| 3. Build vocab SA         |              | 3. Prefix match O(log k)   |
| 4. Precompute Lev DFA     |              | 4. Fuzzy match (DFA)       |
| 5. Encode to binary       |              | 5. Score + rank            |
|                           |              | 6. Return results          |
+---------------------------+              +----------------------------+
          |                                             |
          v                                             v
     .sorex file                                 SearchResult[]
     (~15% overhead                              with section_ids
      vs raw text)                               for deep linking
```

---

## Parallel Build (MapReduce)

The `sorex index` CLI uses a MapReduce-style architecture for maximum throughput on multi-core machines:

```
INPUT                          MAP PHASE                         REDUCE PHASE
-----                          ---------                         ------------

manifest.json
     |
     +-----+-----+-----+
     v     v     v     v
+-------+-------+-------+---------+
| 0.json| 1.json| 2.json|  N.json |  Document files
+---+---+---+---+---+---+----+----+
    |       |       |        |
    v       v       v        v
+---------------------------------------------------------------+
| PHASE 1: Parallel Document Loading (Rayon par_iter)           |
|                                                               |
|  Thread 1    Thread 2    Thread 3    Thread N                 |
|  --------    --------    --------    --------                 |
|  read JSON   read JSON   read JSON   read JSON                |
|  parse       parse       parse       parse                    |
|  validate    validate    validate    validate                 |
+---------------------------------------------------------------+
                          |
                          v
                   Vec<Document>
                   (sorted by ID)
                          |
                          v
+-----------------------------------------------------------------+
| PHASE 2: Index Construction                                     |
|                                                                 |
|  +-----------------------------------------------------+        |
|  | Shared: Arc<ParametricDFA> (built once, ~1.2KB)     |        |
|  +-----------------------------------------------------+        |
|                                                                 |
|  All Documents                                                  |
|  -------------                                                  |
|       |                                                         |
|       v                                                         |
|  build_fst_index()   <- Verified suffix array construction      |
|       |                                                         |
|       v                                                         |
|  BinaryLayer::build  <- Encode with embedded WASM               |
+-----------------------------------------------------------------+
                          |
                          v
                   BuiltIndex
                          |
+-----------------------------------------------------------------+
| PHASE 3: Sequential Output (single thread)                      |
|                                                                 |
|  write index.sorex              <- Index + embedded WASM        |
|  write sorex.js          <- Extracts WASM from .sorex    |
|                                                                 |
|  if --demo:                                                     |
|    write demo.html              <- Demo page                    |
+-----------------------------------------------------------------+
                          |
                          v
OUTPUT
------
output/
+-- index.sorex             <- Self-contained (index + WASM)
+-- sorex.js         <- JS loader (embedded in binary)
+-- demo.html               <- (if --demo, embedded in binary)
```

**No external dependencies:** The `sorex` binary embeds all output files at compile time:
- WASM runtime (~150KB)
- JavaScript loader
- Demo HTML template

Running `sorex index` extracts these embedded files to the output directory.

### Why This Architecture

| Phase | Parallelization | Bottleneck |
|-------|-----------------|------------|
| Document loading | Per-file | I/O bound (SSD throughput) |
| Index building | Single index | CPU bound (suffix array construction) |
| Output writing | Sequential | I/O bound (negligible) |

The Levenshtein DFA is built once (~1.2KB precomputed automaton with ~70 states). The index uses the verified `build_fst_index()` function that guarantees suffix array sortedness and index well-formedness.

---

## Search Algorithms

### Three-Tier Query Resolution

All three tiers execute in parallel. Results stream progressively:

```
                               Query: "auth"
                                     |
          +--------------------------+--------------------------+
          |                          |                          |
          v                          v                          v
+-------------------+      +-------------------+      +-------------------+
| T1: Exact         |      | T2: Prefix        |      | T3: Fuzzy         |
| O(1)              |      | O(log k)          |      | O(vocabulary)     |
|                   |      |                   |      |                   |
| Hash lookup in    |      | Binary search     |      | Levenshtein DFA   |
| inverted index    |      | suffix array      |      | traversal         |
|                   |      |                   |      |                   |
| "auth" → postings |      | "auth*" matches:  |      | Edit distance ≤2: |
|                   |      |   authenticate    |      |   auto (d=1)      |
|                   |      |   authentication  |      |   author (d=2)    |
|                   |      |   author          |      |                   |
+-------------------+      +-------------------+      +-------------------+
          |                          |                          |
          | ≈2μs                     | ≈10μs                    | ≈50μs
          |                          |                          |
          +--------------------------+--------------------------+
                                     |
                                     v
                           Streaming Results
                     (arrive as each tier completes)
```

### Vocabulary Suffix Array

Rather than a suffix array over the full text (expensive), Sorex uses a suffix array over the vocabulary, the unique terms:

```
Vocabulary: ["apple", "application", "apply", "banana"]

Suffix Array entries:
  (0, 0) -> "apple"          Points to suffix of vocabulary[0]
  (0, 1) -> "pple"
  (0, 2) -> "ple"
  (1, 5) -> "ation"          Points to suffix of vocabulary[1]
  (1, 0) -> "application"
  ...

Sorted lexicographically by the suffix string.
Binary search finds all terms with a given prefix.
```

For a 100KB blog with 10K unique words, this is ~50K suffix entries (vs ~500K for full text). The vocabulary is typically 10-20% of the full text size.

### Levenshtein DFA (Schulz-Mihov 2002)

Traditional fuzzy search computes edit distance per-term at query time: O(query_len x term_len x vocabulary_size). Sorex precomputes a universal DFA at index time:

```
Parametric DFA Structure:
  ~70 states for k=2
  8 transitions per state (2^(k+1) character classes)
  ~1.2KB total

At query time:
  1. Compute character class for each input character:
     bit i = 1 if input matches query[position + i]
  2. Follow DFA transitions (single array lookup per character)
  3. Accept state indicates edit distance <= k

Result: O(term_len) per term, no distance computation
```

The DFA is query-independent. The same precomputed structure works for any query. Only character class computation depends on the actual query string.

### Scoring and Ranking

Results are ranked by a combination of field type and position:

```
Field Type Base Scores (Lean-verified):
  Title   = 100.0
  Heading =  10.0
  Content =   1.0

Position Boost (within field):
  Earlier matches get up to +0.5 bonus
  MaxBoost = 0.5

Field Type Dominance (mathematically proven):
  Title - MaxBoost > Heading + MaxBoost
  -> 99.5 > 10.5 check

  Heading - MaxBoost > Content + MaxBoost
  -> 9.5 > 1.5 check

This guarantees: ANY title match outranks ANY heading match,
regardless of position. The hierarchy is absolute, not heuristic.
```

---

## Index Types

Sorex supports multiple index configurations:

### Union Index (Default)

Separate indices for titles, headings, and content:

```
UnionIndex
+-- docs: Vec<SearchDoc>          <- Shared metadata
+-- titles: Option<HybridIndex>   <- Title text only
+-- headings: Option<HybridIndex> <- Heading text only
+-- content: Option<HybridIndex>  <- Body text only
```

Benefits:
- Faster search: smaller indices = fewer comparisons
- Source attribution: results indicate where match was found
- Early termination: can stop after finding title matches

### Hybrid Index

Each sub-index combines inverted + suffix array:

```
HybridIndex
+-- inverted_index: HashMap<String, PostingList> <- O(1) exact
+-- vocabulary: Vec<String>                      <- Sorted terms
+-- vocab_suffix_array: Vec<(term_idx, offset)>  <- O(log k) prefix
+-- docs, texts, field_boundaries                <- Metadata
```

---

## Deep Linking (Section IDs)

Search results include section IDs for navigation to specific headings:

```
Search result for "optimization" in document "/posts/rust-search":
  {
    href: "/posts/rust-search",
    section_id: "performance-optimization",  <- From FieldBoundary
    source: "heading"
  }

Frontend builds URL: /posts/rust-search#performance-optimization
```

Section IDs are stored in a deduplicated string table and referenced by index in postings:

```
Section Table: ["introduction", "setup", "performance-optimization"]

Posting entry:
  doc_id: 5
  section_idx: 3  <- Points to "performance-optimization"

Resolution:
  section_idx 0 = None (title, no anchor)
  section_idx N = section_table[N-1]
```

---

## Formal Verification

<aside class="skip-note">

*Proof engineering details. [Skip to WASM Compilation](#wasm-compilation) if you just need deployment info.*

</aside>

### Three-Layer Defense

```
+-------------------------------------------------------------+
| LAYER 1: Lean Proofs                                        |
|   Mathematical specifications in lean/SearchVerified/       |
|   5 proven theorems, 18 axioms                              |
|   If Lean builds, the spec is internally consistent         |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
| LAYER 2: Type-Level Wrappers                                |
|   ValidatedSuffixEntry, SortedSuffixArray, WellFormedIndex  |
|   Compile-time enforcement via newtype pattern              |
|   Can't create invalid data without explicit unsafe         |
+-------------------------------------------------------------+
                              |
                              v
+-------------------------------------------------------------+
| LAYER 3: Runtime Contracts                                  |
|   debug_assert! checks in contracts.rs                      |
|   Zero cost in release builds                               |
|   Catch invariant violations during development             |
+-------------------------------------------------------------+
```

### Key Proven Properties

| Property | Lean File | Rust Enforcement |
|----------|-----------|------------------|
| Title beats Heading | Scoring.lean | `check_field_hierarchy` |
| Heading beats Content | Scoring.lean | `check_field_hierarchy` |
| Suffix array sorted | SuffixArray.lean | `SortedSuffixArray` |
| Suffix entries valid | Types.lean | `ValidatedSuffixEntry` |
| Binary search correct | BinarySearch.lean | Property tests |
| Edit distance bounds | Levenshtein.lean | Property tests |

---

## WASM Compilation

The library compiles to WebAssembly with minimal dependencies:

```bash
wasm-pack build --target web --features wasm
```

### Size Optimization

| Technique | Impact |
|-----------|--------|
| `opt-level = 's'` | Optimize for size, not speed |
| `lto = true` | Whole-program optimization |
| `panic = 'abort'` | No unwinding, smaller binary |
| No `serde_json` in WASM | Cuts ~20KB |
| Precomputed DFA | ~1.2KB vs runtime construction |

Output: ~150KB gzipped (WASM + JS loader).

### JavaScript Loader

The `sorex.js` is generated from TypeScript modules in `tools/`:

```
tools/
+-- loader.ts            # Loader template with SorexSearcher wrapper
+-- build.ts             # Bundles to target/loader/
```

Build with `bun tools/build.ts`. Output goes to `target/loader/sorex.js` (and `.map`), which is embedded in the Rust CLI via `include_str!`.

---

## Performance Characteristics

### Time Complexity

| Operation | Complexity | Speed | Notes |
|-----------|------------|-------|-------|
| Index load | O(n) | <span class="complexity complexity-medium">~50ms</span> | Linear scan, one-time cost |
| Exact match | O(1) | <span class="complexity complexity-fast">~2us</span> | Hash table lookup |
| Prefix match | O(log k + m) | <span class="complexity complexity-fast">~10us</span> | k = vocabulary, m = matches |
| Fuzzy match | O(k x t) | <span class="complexity complexity-medium">~50us</span> | k = vocabulary, t = avg term length |
| Posting intersection | O(min(n, m)) | <span class="complexity complexity-fast">~5us</span> | Using skip lists for large lists |

### Space Complexity

| Component | Size | Notes |
|-----------|------|-------|
| Vocabulary | ~15% of text | Deduplicated, length-prefixed |
| Suffix array | ~4 bytes/suffix | Compressed term indices |
| Postings | ~2 bytes/posting | Block PFOR with deltas |
| Levenshtein DFA | ~1.2KB fixed | Independent of vocabulary |
| Skip lists | ~5% of large postings | Only for terms with >1024 docs |

Total index overhead: ~15-20% on top of document metadata.

---

## Related Documentation

- [Binary Format](binary-format.md) - .sorex v12 wire format specification
- [Runtime](runtime.md) - Browser execution model, threading, streaming
- [Algorithms](algorithms.md) - Suffix arrays, Levenshtein automata, Block PFOR
- [TypeScript API](typescript.md) - WASM bindings for browser search
- [CLI Reference](cli.md) - Building indexes with `sorex index`
- [Benchmarks](benchmarks.md) - Performance comparisons
- [Verification](verification.md) - Formal verification guide
