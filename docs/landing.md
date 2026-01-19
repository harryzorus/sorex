---
version: "1.0.0"
sizes:
  wasm: "153KB"
  js: "17KB"
format: "v12"

hero:
  tagline: "Search that actually finds things."
  subtitle: "Most search libraries can't find \"auth\" in \"authentication.\" Sorex can. It handles typos, finds substrings, and proves its ranking correct."

features:
  streaming:
    title: "Progressive Search"
    description: "All tiers run in parallel. Results stream as each completes."
    tiers:
      - name: "Exact"
        time: "~5μs"
        description: "Hash lookup, instant results"
      - name: "Prefix"
        time: "~10μs"
        description: "Suffix array binary search"
      - name: "Fuzzy"
        time: "~200μs"
        description: "Levenshtein DFA traversal"

  parallel:
    title: "Parallel Loading"
    description: "WASM compiles while index downloads. Threading when available."
    modes:
      - browser: "Chrome 89+"
        mode: "Parallel"
        workers: "4 workers"
      - browser: "Firefox 79+"
        mode: "Parallel"
        workers: "4 workers"
      - browser: "Safari"
        mode: "Serial"
        workers: "Graceful fallback"
      - browser: "Edge 89+"
        mode: "Parallel"
        workers: "Chromium-based"

  typo:
    title: "Typo Tolerance"
    description: "Levenshtein DFA with edit distance ≤ 2."
    example:
      query: "epilouge"
      result: "epilogue"

  substring:
    title: "Substring Search"
    description: "Find terms inside words, not just at boundaries."
    example:
      query: "sync"
      results:
        - "async"
        - "__syncthreads"
        - "synchronize"

  hierarchical:
    title: "Hierarchical Ranking"
    description: "Section headings rank above content. Mathematically proven."
    buckets:
      - level: 1
        name: "Title"
        description: "Document title or h1"
      - level: 2
        name: "h2 Section"
        description: "Main section heading"
      - level: 3
        name: "h3 Subsection"
        description: "Nested heading"
      - level: 4
        name: "Content"
        description: "Regular content text"

  nonblocking:
    title: "Non-Blocking"
    description: "Runs in a Web Worker. UI stays at 60fps."

  quality:
    title: "Quality"
    points:
      - "Formally verified in Lean 4"
      - "Property tested with proptest fuzzing"
      - "25 languages via Unicode segmentation"

benchmarks:
  cutlass:
    name: "NVIDIA CUTLASS"
    pages: 70
    description: "GPU kernel documentation"
    queries:
      - query: "gemm"
        description: "Matrix multiply API"
      - query: "sync"
        description: "Substring in async, __syncthreads"
      - query: "epilouge"
        description: "Typo for epilogue"

  pytorch:
    name: "PyTorch"
    pages: 300
    description: "ML framework documentation"
    queries:
      - query: "tensor"
        description: "Core data structure"
      - query: "autograd"
        description: "Automatic differentiation"
      - query: "tensro"
        description: "Typo for tensor"

useWhen:
  - "Users search for substrings (auth → authentication)"
  - "Typos are common (epilouge → epilogue)"
  - "You need section-aware ranking"
  - "You need provably correct ranking"

dontUseWhen:
  - "Minimal bundle size is critical (Sorex: ~153KB, FlexSearch: 6.6KB)"
  - "You only need exact word matching"
  - "Dataset is under 10 docs (use array.filter())"
  - "You need server-side search (try Meilisearch)"

specs:
  format: ".sorex v12 (self-contained)"
  maxEditDistance: 2
  license: "Apache-2.0"

quickstart:
  install: "cargo install sorex"
  build: "sorex index --input ./docs --output ./search"
  search: "sorex search --wasm ./search/index.sorex \"tensor\""
  code: |
    import { loadSorex } from './sorex.js';
    const searcher = await loadSorex('./index.sorex');
    searcher.search('tensor', 10, ui_callback);

links:
  github: "https://github.com/harryzorus/sorex"
  docs: "/projects/sorex/docs/index"
---

# Sorex

Search that actually finds things.

Most search libraries can't find "auth" in "authentication." Sorex can. It handles typos, finds substrings, and proves its ranking correct.

## Why Sorex?

### Different Tradeoffs

Each library makes different tradeoffs. FlexSearch optimizes for speed and bundle size. lunr.js brings Lucene-style stemming. fuse.js prioritizes zero-configuration fuzzy matching. Sorex trades bundle size for substring search and proven ranking.

### What Sorex Does Differently

1. **Three-tier progressive search**: Exact matches return in ~5μs. Prefix matches add more results at ~10μs. Fuzzy matches complete the picture at ~200μs. Your UI updates after each tier.

2. **Parallel loading**: The .sorex v12 format puts WASM first, so compilation starts while index data downloads. On Chrome/Firefox, 4 workers compile in parallel.

3. **Proven ranking**: The field hierarchy (title > heading > content) is mathematically proven in Lean 4, not just tested.

## Try It

Build an index with the demo flag:

```bash
sorex index --input ./docs --output ./search --demo
```

Open `search/demo.html` in your browser to search your own content.

## Performance

On NVIDIA CUTLASS docs (70 pages):

- **"sync"** finds 28 results (async, __syncthreads, synchronize...)
- FlexSearch: 4 results. lunr.js: 3 results. fuse.js: 1 result.

On PyTorch docs (300 pages):

- **"tensro"** (typo) finds 10 precise matches via Levenshtein DFA
- fuse.js: 21 results (over-inclusive). FlexSearch/lunr.js: 0 results.

## Learn More

- [Quick Start](/projects/sorex/docs/quickstart) - Get started in 5 minutes
- [Architecture](/projects/sorex/docs/architecture) - How the three-tier pipeline works
- [Benchmarks](/projects/sorex/docs/benchmarks) - Performance comparisons
- [Verification](/projects/sorex/docs/verification) - Formal proofs in Lean 4
