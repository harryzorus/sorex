# Overview

Sorex is an attempt to bring database-class search to the browser, with a formal verification twist.

Most client-side search libraries make tradeoffs that hurt relevance. They tokenize aggressively, losing substring matches. They skip fuzzy matching for speed. They rank by term frequency alone, ignoring document structure. The result: users search for "auth" and don't find "authentication."

The project started from a personal frustration: searching "auth" and not finding "authentication", or making a typo and getting zero results. As a non-native English speaker, I wanted search that tolerates the mistakes I actually make.

Database search engines like Elasticsearch and Meilisearch solve these problems with suffix arrays, inverted indexes, and sophisticated ranking. But they require servers. Sorex asks: what if we brought those techniques to a 153KB WASM binary that runs entirely in the browser?

The formal verification twist: search ranking is notoriously hard to get right. Sorex encodes its ranking invariants in Lean 4 and proves them mathematically correct. When we say "title matches rank above content matches," that's not just tested. It's proven.

---

## Reading Paths

**New to Sorex?** Start here:
1. [Quick Start](quickstart.md) - Get search running in 5 minutes
2. [Integration](integration.md) - Framework examples (React, vanilla JS)
3. [Troubleshooting](troubleshooting.md) - When things go wrong

**Building an API?**
- [TypeScript API](typescript.md) - Browser WASM bindings
- [Rust API](rust.md) - Library for index building
- [CLI Reference](cli.md) - Command-line tools

**Understanding the internals?**
- [Runtime](runtime.md) - Browser execution model
- [Architecture](architecture.md) - System design
- [Binary Format](binary-format.md) - .sorex file specification
- [Algorithms](algorithms.md) - Suffix arrays, Levenshtein DFA

**Evaluating performance?**
- [Benchmarks](benchmarks.md) - Comparisons with other libraries

**Contributing?**
- [Verification](verification.md) - Formal verification rules
- [Contributing](contributing.md) - Development workflow

---

## Documentation

### Getting Started

| Guide | Description |
|-------|-------------|
| [Quick Start](quickstart.md) | Get search running in 5 minutes |
| [Integration](integration.md) | Framework examples for React, Svelte, vanilla JS |
| [Troubleshooting](troubleshooting.md) | Solutions to common issues |

### API Reference

| Reference | Description |
|-----------|-------------|
| [TypeScript API](typescript.md) | Browser WASM bindings: `loadSorex`, `SorexSearcher` |
| [Rust API](rust.md) | Library API: `build_index`, verification types |
| [CLI Reference](cli.md) | Build with `sorex index`, inspect with `sorex inspect` |

### Internals

| Guide | Description |
|-------|-------------|
| [Runtime](runtime.md) | Streaming compilation, threading, progressive search |
| [Architecture](architecture.md) | System design, three-tier search, formal verification |
| [Binary Format](binary-format.md) | .sorex v12 wire format specification |
| [Algorithms](algorithms.md) | Suffix arrays, Levenshtein automata, Block PFOR |

### Evidence & Contributing

| Guide | Description |
|-------|-------------|
| [Benchmarks](benchmarks.md) | Performance comparisons with other search libraries |
| [Verification](verification.md) | How Lean 4 proofs guarantee ranking correctness |
| [Contributing](contributing.md) | Development workflow, verification checklist |

---

## Quick Start

### 1. Install the CLI

```bash
cargo install sorex
```

### 2. Build an index

```bash
sorex index --input ./docs --output ./search
```

### 3. Search in the browser

```typescript
import { loadSorex } from './sorex.js';

const searcher = await loadSorex('./index.sorex');
searcher.search('query', 10, {
  onUpdate: (results) => console.log(results),  // Progressive updates
  onFinish: (results) => console.log(results)   // Final results
});
```

---

## Project Structure

```
sorex/
├── src/
│   ├── lib.rs              # Library entry point
│   ├── main.rs             # CLI entry point
│   ├── types.rs            # Core data structures
│   ├── binary/             # .sorex format encoding/decoding
│   ├── build/              # Index construction pipeline
│   ├── cli/                # CLI display and output
│   ├── fuzzy/              # Levenshtein DFA, edit distance
│   ├── index/              # Suffix arrays, inverted index
│   ├── runtime/            # WASM bindings, Deno runtime
│   ├── scoring/            # Ranking (Lean-verified)
│   ├── search/             # Three-tier search
│   ├── util/               # SIMD, compression
│   └── verify/             # Runtime contracts
├── lean/                   # Lean 4 formal specifications
│   └── SearchVerified/     # Proofs for ranking, binary search
├── data/
│   ├── datasets/           # Benchmark datasets (CUTLASS, PyTorch)
│   └── e2e/                # End-to-end tests
├── benches/                # Criterion benchmarks
├── tests/                  # Integration and property tests
├── fuzz/                   # Fuzz testing targets
└── docs/                   # This documentation
```
