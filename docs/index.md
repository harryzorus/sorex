# Sieve Documentation

A formally verified search engine for static sites. Substring matching, typo tolerance, and streaming results.

## Reading Order

**Getting started?** Begin with [Integration](integration.md) for setup, then [Architecture](architecture.md) for how it works.

**Contributing?** Read [Verification](verification.md) first, then [Contributing](contributing.md).

## Documentation

| Guide | Description |
|-------|-------------|
| [Integration](integration.md) | WebAssembly setup, browser integration, and Web Worker usage. Get search running in 5 minutes. |
| [Architecture](architecture.md) | Three-tier search pipeline (exact → prefix → fuzzy), binary format, and design decisions. |
| [Benchmarks](benchmarks.md) | Why Sieve finds "auth" in "authentication" when lunr.js, FlexSearch, and Fuse.js can't. |
| [Algorithms](algorithms.md) | Suffix arrays, Levenshtein automata, and vocabulary-based indexing. For the curious. |
| [Verification](verification.md) | How Lean 4 proofs guarantee ranking correctness. What we prove, what we trust. |
| [Contributing](contributing.md) | Development workflow, Lean/Rust synchronization, and the verification checklist. |

### API Reference

| Reference | Description |
|-----------|-------------|
| [CLI](cli.md) | Build indexes with `sieve index`, inspect with `sieve inspect`. |
| [TypeScript](typescript.md) | Browser API: `SieveSearcher`, `SieveProgressiveIndex`, streaming search. |
| [Rust](rust.md) | Library API: `build_index`, `search_hybrid`, verification types. |

## Quick Start

### 1. Install the CLI

```bash
cargo install sieve-search
```

### 2. Build an index

```bash
sieve index --input ./docs --output ./search
```

### 3. Search in the browser

```typescript
import { loadSieve } from './sieve-loader.js';

const searcher = await loadSieve('./index.sieve');
const results = searcher.search('query', 10);
```
