---
title: Rust API
description: Library API for building and searching Sorex indexes programmatically
order: 5
---

# Rust API

Sorex's Rust API provides building blocks for search indexes with formal verification.

## Installation

```toml
[dependencies]
sorex = "0.2.5"
```

## Core Types

### SearchDoc

Document metadata stored in the index:

```rust
pub struct SearchDoc {
    pub id: usize,
    pub title: String,
    pub excerpt: String,
    pub href: String,
    pub kind: String,          // "post", "page", etc.
    pub category: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
}
```

### FieldBoundary

Marks field boundaries in document text for scoring:

```rust
pub struct FieldBoundary {
    pub doc_id: usize,
    pub start: usize,          // Character offset
    pub end: usize,
    pub field_type: FieldType, // Title, Heading, or Content
    pub section_id: Option<String>, // For deep linking
}

pub enum FieldType {
    Title,   // 100x boost
    Heading, // 10x boost
    Content, // 1x boost
}
```

### SearchResult

Search result with source attribution:

```rust
pub struct SearchResult {
    pub doc_id: usize,
    pub score: f64,
    pub section_idx: u32,          // 0 = no section, >0 = section_table[idx-1]
    pub tier: u8,                  // 1=exact, 2=prefix, 3=fuzzy
    pub match_type: MatchType,     // Title, Section, Heading, or Content
    pub matched_term: Option<u32>, // Vocabulary index of matched term
}
```

**`matched_term`**: Index into the vocabulary of the term that matched. Useful for:
- Highlighting the matched term in results
- Showing fuzzy match expansions (e.g., query "ruts" matched vocabulary term "rust")
- Prefix expansion display (e.g., query "typ" matched "typescript")

### SearchOptions

Configuration for search behavior:

```rust
pub struct SearchOptions {
    pub dedup_sections: bool,  // Default: true
}

impl SearchOptions {
    pub fn new() -> Self;                    // Default settings
    pub fn without_section_dedup() -> Self;  // Disable section dedup
}
```

**`dedup_sections`** (default: `true`):
- When `true`: Returns one result per document. The best matching section (by match_type, then score) is used for deep linking.
- When `false`: Returns multiple results per document if different sections match.

## Building Indexes

### Suffix Array Index

Basic suffix array for prefix search:

```rust
use sorex::{build_index, search, SearchDoc, FieldBoundary};

let docs = vec![
    SearchDoc {
        id: 0,
        title: "Getting Started".into(),
        excerpt: "Learn the basics".into(),
        href: "/docs/getting-started".into(),
        kind: "page".into(),
        category: None,
        author: None,
        tags: vec![],
    },
];

let texts = vec!["getting started with rust programming".into()];

let boundaries = vec![
    FieldBoundary {
        doc_id: 0,
        start: 0,
        end: 15,
        field_type: FieldType::Title,
        section_id: None,
    },
];

let index = build_index(docs, texts, boundaries);
let results = search(&index, "rust");
```

### Hybrid Index (Recommended)

Combines inverted index (exact) + suffix array (prefix) + Levenshtein (fuzzy):

```rust
use sorex::{build_hybrid_index, search_hybrid};

let index = build_hybrid_index(docs, texts, boundaries);
let results = search_hybrid(&index, "rust"); // All three tiers
```

### Parallel Index Building

For large document sets, use Rayon parallelism:

```rust
use sorex::build_hybrid_index_parallel;

// Builds vocabulary and suffix array in parallel
let index = build_hybrid_index_parallel(docs, texts, boundaries);
```

## Search Functions

### Basic Search

```rust
use sorex::search;

let results = search(&index, "query");
for result in results {
    println!("{}: {} (score: {})", result.doc.title, result.doc.href, result.score);
}
```

### Hybrid Search

Three-tier strategy with fuzzy matching:

```rust
use sorex::{search_hybrid, search_exact, search_expanded};

// Full search (all tiers)
let all_results = search_hybrid(&index, "query");

// Streaming: exact first, then expand
let exact = search_exact(&index, "query");
let exclude_ids: Vec<usize> = exact.iter().map(|d| d.id).collect();
let expanded = search_expanded(&index, "query", &exclude_ids);
```

### Fuzzy Search

Direct fuzzy search with edit distance:

```rust
use sorex::{search_fuzzy, levenshtein_within};

// Fuzzy search with max edit distance 2
let fuzzy_results = search_fuzzy(&index, "query", 2);

// Check if two strings are within edit distance
if levenshtein_within("rust", "ruts", 1) {
    // Edit distance <= 1
}
```

## Levenshtein DFA

Precomputed automaton for O(1) fuzzy matching:

```rust
use sorex::{ParametricDFA, QueryMatcher};

// Build DFA once (expensive, ~10ms)
let dfa = ParametricDFA::new(2); // max edit distance 2

// Build query matcher (~1μs per query)
let matcher = QueryMatcher::new(&dfa, "rust");

// Match against vocabulary (~8ns per term)
for term in vocabulary {
    if let Some(distance) = matcher.matches(term) {
        println!("{} matches with distance {}", term, distance);
    }
}
```

## Verification

Sorex includes type-level invariants for correctness:

```rust
use sorex::{WellFormedIndex, SortedSuffixArray, ValidatedSuffixEntry};

// Validated index with compile-time guarantees
let validated: WellFormedIndex = index.validate()?;

// Sorted suffix array (type-level proof)
let sorted: SortedSuffixArray = validated.suffix_array();

// Each entry has valid bounds
for entry: ValidatedSuffixEntry in sorted.entries() {
    assert!(entry.doc_id() < validated.doc_count());
    assert!(entry.offset() <= validated.text_len(entry.doc_id()));
}
```

### Verification Report

Check index invariants at runtime:

```rust
use sorex::VerificationReport;

let report = VerificationReport::from_index(&index);
assert!(report.suffix_array_sorted);
assert!(report.suffix_array_complete);
assert!(report.lcp_correct);
assert!(report.field_boundaries_valid);
```

## Binary Format

Serialize indexes to compact binary format:

```rust
use sorex::binary::LoadedLayer;

// Deserialize from bytes
let layer = LoadedLayer::from_bytes(&bytes)?;

// Access components
let vocabulary: &[String] = &layer.vocabulary;
let suffix_array: &[(u32, u32)] = &layer.suffix_array;
let postings: &[Vec<PostingEntry>] = &layer.postings;
let section_table: &[String] = &layer.section_table;
```

## Custom Scoring (Deno Runtime)

With the `deno-runtime` feature, you can use custom JavaScript/TypeScript scoring functions:

```rust
use sorex::runtime::ScoringEvaluator;

// Load scoring function from file
let mut evaluator = ScoringEvaluator::from_file(Path::new("scoring.ts"))?;

// Or from inline code
let mut evaluator = ScoringEvaluator::from_code(r#"
    export default function score(ctx) {
        return ctx.match.fieldType === "title" ? 1000 : 100;
    }
"#)?;

// Evaluate single context
let score = evaluator.evaluate(&context)?;

// Batch evaluation (more efficient)
let scores = evaluator.evaluate_batch(&contexts)?;
```

See [CLI Reference](./cli.md#custom-scoring-functions) for the `ScoringContext` interface.

## Feature Flags

```toml
[dependencies]
sorex = { version = "0.2.5", features = ["wasm", "serde_json"] }
```

| Feature | Description |
|---------|-------------|
| `wasm` | WebAssembly bindings (SorexSearcher with callback-based search) |
| `serde_json` | JSON serialization for build pipeline integration |
| `embed-wasm` | Embed WASM runtime in CLI binary |
| `deno-runtime` | Deno V8 runtime for custom ranking functions |

## See Also

- [CLI Reference](./cli.md) - Build indexes with `sorex index`
- [TypeScript API](./typescript.md) - Browser WASM bindings
- [Architecture](./architecture.md) - Index format details
- [Algorithms](./algorithms.md) - Search algorithm explanations
- [Verification](./verification.md) - Lean 4 proofs and type-level invariants
