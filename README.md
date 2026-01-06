# Sieve

A full-text search engine with fuzzy and substring search, built to power search on [harryzorus.xyz](https://harryzorus.xyz). Fast enough for real-time search, small enough for browsers, correct enough to prove it.

## What This Is

Sieve builds compact binary search indices (`.sieve` format) that load instantly in WebAssembly. The algorithms are formally verified in Lean 4—not "we wrote some tests" verified, but mathematically proven correct.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                                                                          │
│   ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐            │
│   │ Content │ ──► │  Index  │ ──► │  WASM   │ ──► │ Browser │            │
│   │  .json  │     │  .sieve │     │  153KB  │     │  <1ms   │            │
│   └─────────┘     └─────────┘     └─────────┘     └─────────┘            │
│                                                                          │
│   Your docs       Binary index    Loads anywhere  Instant search         │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

---

## Why This Exists

The client-side search space is crowded—FlexSearch, Lunr.js, Fuse.js, MiniSearch, and others all solve this problem. Sieve makes a deliberate tradeoff: **search capability and correctness over payload size and query syntax**.

Most libraries use inverted indexes that can't find "script" inside "typescript". They're small and fast, but users expect substring matching. Sieve uses suffix arrays and Levenshtein automata to deliver true fuzzy and substring search—at the cost of a larger WASM bundle (153KB gzipped vs 6-25KB for pure JS alternatives).

The other bet: formal verification. Instead of hoping the ranking logic is correct, Sieve proves it in Lean 4.

- **Suffix array** for O(log n) prefix search—find "auth" in "authentication" instantly
- **Levenshtein automata** (Schulz-Mihov 2002) for typo-tolerant fuzzy matching without per-query DFA construction
- **Block PFOR compression** for posting lists—smaller than JSON, faster to decode
- **Formal verification** via Lean 4 proofs—binary search actually works, scoring hierarchy is actually correct

The result: a ~150KB WASM bundle that handles real-time search on thousands of documents.

---

## Features

### Search Capabilities

| Feature | How It Works | Performance |
|---------|--------------|-------------|
| **Exact Match** | Inverted index with skip pointers | O(1) term lookup |
| **Prefix Search** | Binary search on vocabulary suffix array | O(log k) where k = vocabulary size |
| **Fuzzy Search** | Precomputed Levenshtein DFA traversal | ~2ms for d=1, ~5ms for d=2 |
| **Field Ranking** | Title > Heading > Content, mathematically proven | Compile-time verified |
| **Deep Linking** | Section IDs in results for #anchor navigation | Zero overhead |

### Binary Format (`.sieve` v7)

The index format is designed for fast memory-mapped loading. **v7 is self-contained**: a single `.sieve` file includes the search index, document metadata, and the WASM runtime.

- **Vocabulary**: Length-prefixed UTF-8, lexicographically sorted
- **Suffix Array**: Term index + offset pairs for prefix search
- **Inverted Index**: Block PFOR-encoded posting lists with delta compression
- **Levenshtein DFA**: Precomputed automata for d=1 and d=2 (zero runtime cost)
- **Section Table**: IDs for deep linking into document sections
- **CRC32 Footer**: Integrity verification

Total overhead: ~15% on top of raw text (vs 2-3x for JSON-based indices).

### Formal Verification

Three layers of defense against bugs:

```
LEAN PROOFS          ──► Mathematical truth (5 proven theorems, 18 axioms)
TYPE-LEVEL WRAPPERS  ──► Compile-time invariants (ValidatedSuffixEntry, etc.)
RUNTIME CONTRACTS    ──► Debug-mode assertions (zero-cost in release)
```

The field scoring hierarchy is *proven*—Title always beats Heading, Heading always beats Content, regardless of position boosts. Binary search bounds are *proven*—results contain all matches and only matches.

---

## Installation

### Homebrew (macOS)

```bash
brew tap harryzorus/sieve
brew install sieve
```

### Cargo

```bash
cargo install sieve-search
```

### From Source

```bash
git clone https://github.com/harryzorus/sieve.git
cd sieve
cargo build --profile release-native
```

### Debian/Ubuntu

```bash
cargo install cargo-deb
cargo deb --profile release-native --install
```

---

## Usage

### Build an Index

```bash
# Build index from a directory of JSON documents
sieve index --input ./docs --output ./search-output

# Inspect an existing index
sieve inspect ./search-output/index-*.sieve

# Build with demo HTML page
sieve index --input ./docs --output ./search-output --demo
```

### Input Directory Format

Sieve reads a directory containing a `manifest.json` and per-document JSON files:

```
docs/
├── manifest.json          # Index configuration
├── getting-started.json   # Document files
├── installation.json
└── api-reference.json
```

**manifest.json:**
```json
{
  "version": 1,
  "documents": ["getting-started.json", "installation.json", "api-reference.json"]
}
```

**Per-document JSON:**
```json
{
  "id": 0,
  "slug": "getting-started",
  "title": "Getting Started",
  "excerpt": "Learn how to set up...",
  "href": "/docs/getting-started",
  "type": "doc",
  "category": "documentation",
  "text": "Getting Started Learn how to set up your first project...",
  "fieldBoundaries": [
    { "start": 0, "end": 15, "fieldType": "title", "sectionId": null },
    { "start": 16, "end": 100, "fieldType": "content", "sectionId": "intro" }
  ]
}
```

### Browser Integration

The `.sieve` file is self-contained with embedded WASM. Use the generated `sieve-loader.js`:

```javascript
import { loadSieve } from './sieve-loader.js';

// Load index (extracts and initializes WASM automatically)
const searcher = await loadSieve('./index-a1b2c3d4.sieve');

// Search
const results = searcher.search('query', 10);

// Results include sectionId for deep linking
results.forEach(r => {
  const url = r.sectionId ? `${r.href}#${r.sectionId}` : r.href;
  console.log(`${r.title}: ${url}`);
});

// Free when done (optional - uses FinalizationRegistry)
searcher.free();
```

The loader is fully self-contained with no external dependencies. Multiple `.sieve` files with different WASM versions can coexist on the same page.

---

## Performance

Benchmarks on typical blog content (~50 docs, ~100KB text):

| Operation | Time | Notes |
|-----------|------|-------|
| Index build | ~10ms | Includes suffix array, inverted index, DFA |
| Index load (WASM) | ~5ms | Memory-mapped, minimal parsing |
| Exact search | <0.1ms | Inverted index lookup |
| Prefix search | <0.5ms | Binary search on vocabulary |
| Fuzzy search (d=1) | ~2ms | DFA traversal, no construction |
| Fuzzy search (d=2) | ~5ms | Larger automaton |

Comparison with alternatives (100 documents, 500KB text):

| Library | Index Size | Query Time | Bundle Size |
|---------|------------|------------|-------------|
| **Sieve** | 85KB | <1ms | 153KB |
| Lunr.js | 240KB | 15ms | 8KB |
| FlexSearch | 180KB | 3ms | 22KB |
| Fuse.js | N/A | 30ms | 24KB |

---

## Architecture

The codebase is organized around what things *do*, not what they *are*:

```
sieve/
├── src/
│   ├── types.rs          # Core data structures
│   ├── index.rs          # Suffix array construction
│   ├── search.rs         # Binary search implementation
│   ├── scoring.rs        # Field ranking (verified in Lean)
│   ├── levenshtein.rs    # Edit distance computation
│   ├── levenshtein_dfa.rs # Parametric automata (Schulz-Mihov)
│   ├── inverted.rs       # Inverted index + posting lists
│   ├── binary.rs         # .sieve format encoding/decoding
│   ├── wasm.rs           # WebAssembly bindings
│   ├── verified.rs       # Type-level invariant wrappers
│   └── contracts.rs      # Runtime debug assertions
│
├── lean/
│   └── SearchVerified/   # Lean 4 specifications and proofs
│       ├── Types.lean
│       ├── SuffixArray.lean
│       ├── BinarySearch.lean
│       ├── Scoring.lean
│       └── Levenshtein.lean
│
├── tests/
│   ├── invariants.rs     # Lean theorem verification
│   ├── property.rs       # Property-based tests (proptest)
│   └── integration.rs    # End-to-end tests
│
└── docs/
    ├── architecture.md   # Binary format, algorithm details
    ├── algorithms.md     # Deep dive into suffix arrays, DFAs
    └── integration.md    # WASM integration guide
```

---

## Verification

Before committing changes, verify everything works:

```bash
cargo xtask verify    # Full suite: tests + Lean proofs + constant alignment
cargo xtask check     # Quick check: tests + clippy (no Lean)
cargo xtask test      # Just tests
cargo xtask lean      # Just Lean proofs
cargo xtask bench     # Benchmarks
```

See [Verification](docs/verification.md) for the formal verification approach and [CLAUDE.md](CLAUDE.md) for AI agent guidelines.

---

## Documentation

| Document | What's in it |
|----------|--------------|
| [Architecture](docs/architecture.md) | Binary format spec, algorithm overview |
| [Algorithms](docs/algorithms.md) | Suffix arrays, Levenshtein automata, Block PFOR |
| [Benchmarks](docs/benchmarks.md) | Performance comparisons with FlexSearch, lunr.js, etc. |
| [Integration](docs/integration.md) | WASM setup, browser integration, TypeScript types |
| [Verification](docs/verification.md) | Formal verification guide, safe refactoring |
| [Contributing](docs/contributing.md) | How to contribute without breaking proofs |

---

## License

Apache-2.0. See [LICENSE](LICENSE) for details.

---

Built with Rust and verified with Lean 4.
