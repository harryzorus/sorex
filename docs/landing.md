# Sieve

**Search that actually finds things.**

Most search libraries can't find "auth" in "authentication." Sieve can. It handles typos, finds substrings, and proves its ranking correct.

- **v0.3.0** · 153 KB WASM · 4.5 KB JS
- [GitHub](https://github.com/harryzorus/sieve) · [Documentation](index.md)

## Features

### Three-Tier Search

Results stream progressively as each tier completes:

| Tier | Type | Latency |
|------|------|---------|
| T1 | Exact match | ~2μs |
| T2 | Substring | ~10μs |
| T3 | Fuzzy | ~50μs |

### Non-Blocking

Runs in a Web Worker. UI stays at 60fps while search executes in the background.

### Typo Tolerance

`"epilouge"` → `epilogue`

Levenshtein DFA with edit distance ≤2.

### Substring Search

`"sync"` finds `async`, `__syncthreads`, and any word containing "sync".

### Quality

- **Formally verified** in [Lean 4](verification.md)
- **Property tested** with proptest fuzzing
- **25 languages** via Unicode segmentation

## When to Use Sieve

**Use Sieve when:**
- Users search for substrings (`auth` → `authentication`)
- Typos are common (`epilouge` → `epilogue`)
- You need provably correct ranking
- 153KB WASM is acceptable

**Consider alternatives when:**
- Bundle size under 20KB is required
- You only need exact word matching
- Dataset is under 10 docs (just use `filter()`)
- You need server-side search

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

## Specifications

- Index format: `.sieve` v7
- Max edit distance: 2
- License: Apache-2.0

## Learn More

- [Integration Guide](integration.md) - Get started in 5 minutes
- [Architecture](architecture.md) - How the three-tier pipeline works
- [Benchmarks](benchmarks.md) - Performance comparisons
- [Verification](verification.md) - Formal proofs in Lean 4

### API Reference

- [CLI Reference](cli.md) - Build and inspect indexes
- [TypeScript API](typescript.md) - Browser WASM bindings
- [Rust API](rust.md) - Library for programmatic use
