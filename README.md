# Sorex

Full-text search that runs in the browser. Handles typos, finds substrings, proves its ranking correct.

## Why This Exists

The JavaScript ecosystem has excellent search libraries. [Lunr](https://lunrjs.com/) pioneered client-side search with a clean API and tiny footprint. [Fuse.js](https://fusejs.io/) handles fuzzy matching elegantly. [FlexSearch](https://github.com/nextapps-de/flexsearch) is remarkably fast for its size. [MiniSearch](https://lucaong.github.io/minisearch/) balances features and bundle weight thoughtfully. [Pagefind](https://pagefind.app/) solves the static site case beautifully.

These libraries make smart tradeoffs. They prioritize small bundles, zero build steps, broad compatibility, and simple APIs. Bundle size matters, build complexity has costs, and most sites don't need substring search.

I wanted something different for my static site. Two things kept bothering me. First, I wanted "auth" to find "authentication": substring matching, not just word boundaries. Second, I mistype words constantly. English isn't my first language, and I'm forever writing "optimzer" instead of "optimizer" or "asyncronous" instead of "asynchronous". One or two characters off, and search returns nothing, even when the answer is right there.

I've used Lean 4 at work for database internals, the kind of code where a bug means corrupted data. It got me thinking: what if I applied the same approach to smaller projects? Write specifications, let AI agents generate implementations, prove the results correct. Move fast and prove things. Sorex is my test case for that idea. The core invariants (ranking, suffix array ordering, binary search bounds, section boundaries) are specified in Lean. When an agent generates code, the proofs constrain what it can produce. If the implementation violates an invariant, it won't compile. The specifications become guardrails.

Sorex trades simplicity for capability. It requires a build step. It uses WebAssembly. The index format is custom. In exchange: suffix arrays for true substring search, a Levenshtein DFA that tolerates my typos, and mathematical guarantees that title matches always rank above content matches.

Different problems, different tools. If Lunr or MiniSearch solve your problem, use them. They're battle-tested and dependency-free. Sorex exists for the cases where substring search and typo tolerance matter more than simplicity.

## What It Does

Search runs in three tiers:

```
Tier 1: Exact     →  Hash lookup          →  ~2μs
Tier 2: Prefix    →  Suffix array search  →  ~10μs
Tier 3: Fuzzy     →  Levenshtein DFA      →  ~50μs
```

Results stream as each tier completes. Users see exact matches immediately while fuzzy search runs in the background. The index is a single `.sorex` file (~150KB gzipped) containing the WASM runtime and all the search data. Load it, call `search()`, done.

```javascript
import { loadSorex } from './sorex.js';

const searcher = await loadSorex('./index.sorex');
searcher.search('authentication', 10, {
  onUpdate: (results) => renderResults(results),
  onFinish: (results) => console.log('done:', results.length)
});
```

## The Verification Story

Proofs are only as good as their axioms. I learned this the hard way.

- **Lean proofs** for core invariants: ranking, search bounds, section boundaries. If it compiles, it's correct. No amount of clever testing beats that.
- **Kani proofs** for panic-freedom: the varint parser is proven safe for ANY input, not just tested inputs. Malformed `.sorex` files can't crash the runtime.
- **Oracle tests** for complex algorithms: SAIS, binary search, and Levenshtein are compared against simple reference implementations. If they disagree, the simple one is right.
- **Property tests** to check the implementation matches the spec. The proofs assume suffix arrays are sorted; proptest makes sure they actually are.
- **Mutation testing** to verify tests catch bugs. CI fails if detection rate drops below 60%.
- **Fuzz tests** for the bugs that would otherwise wake you at 3am: overflows, malformed input, the parser choking on bytes that should never exist.

Each layer catches what the others miss. The combination sleeps better than I do.

## Installation

```bash
cargo install sorex
```

## Usage

Build an index from JSON documents:

```bash
sorex index --input ./docs --output ./search
sorex inspect ./search/index-*.sorex
```

Input format is one JSON file per document:

```json
{
  "id": 0,
  "title": "Getting Started",
  "href": "/docs/getting-started",
  "text": "Getting Started This guide covers...",
  "fieldBoundaries": [
    { "start": 0, "end": 15, "fieldType": "title" },
    { "start": 16, "end": 100, "fieldType": "content", "sectionId": "intro" }
  ]
}
```

The `fieldBoundaries` tell the ranker where titles, headings, and content live in the text. Section IDs enable deep linking. Search results can point to `#intro` instead of just the page.

## Architecture

```
src/
├── binary/         .sorex format (varint, delta encoding, CRC32)
├── index/          Suffix array (SA-IS), inverted index
├── search/         Three-tier pipeline, deduplication
├── scoring/        Field weights, match type ranking
├── fuzzy/          Levenshtein DFA for typo tolerance
└── verify/         Runtime contracts, type-level invariants

lean/SearchVerified/
├── Scoring.lean    Field dominance proofs
├── TieredSearch.lean
└── ...

kani-proofs/        Panic-freedom proofs (varint parsing)

tests/property/
├── oracles.rs      Reference implementations for differential testing
└── ...

fuzz/fuzz_targets/
├── search_queries.rs
├── binary_parsing.rs
└── ...
```

The Lean modules mirror the Rust structure. When I change a scoring constant in Rust, the build fails until I update the corresponding Lean definition and verify the proofs still hold. Kani proves the binary parser can't panic on any input. Oracle tests compare optimized algorithms against simple references.

## Performance

On a typical docs site (~300 pages, ~500KB text):

| Operation | Time |
|-----------|------|
| Index load | ~5ms |
| Exact search | ~2μs |
| Prefix search | ~10μs |
| Fuzzy search | ~50μs |

The index stays under 200KB gzipped for most sites. WASM adds ~150KB, but that's cached after first load.

## Running the Verification

```bash
cargo xtask verify    # Full suite: Lean + tests + mutations + E2E (11 steps)
cargo xtask check     # Quick check: tests + clippy (no Lean/mutations)
cargo xtask kani      # Kani model checking (~5 min, proves panic-freedom)

# Fuzz testing
cargo +nightly fuzz run search_queries -- -max_total_time=60
```

## Limitations

The fuzzy search maxes out at edit distance 2. Three-character typos are on their own. The suffix array is built at index time; no incremental updates, so you rebuild when content changes. Rebuilds are fast though. The indexer parallelizes with map-reduce, so a 300-page site indexes in ~50ms. See [benchmarks](docs/benchmarks.md).

Index size scales better than you'd expect. Vocabulary grows sublinearly ([Zipf's law](https://en.wikipedia.org/wiki/Zipf%27s_law)): most new documents reuse existing words, and postings compress well with PFOR and Brotli. A 1,000-page site isn't 10x the size of a 100-page site.

The Lean proofs cover ranking, not completeness. I haven't proven "every matching document appears in results" - that's a harder theorem, and the property tests haven't found a counterexample yet. Good enough for a blog search. Probably don't use this for medical records.

## License

Apache-2.0
