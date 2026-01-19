# Benchmarks

Performance comparisons between Sorex and popular JavaScript search libraries. Benchmarks use the European Countries dataset (30 full Wikipedia-style articles, ~3,500 words each) and Tinybench with 5-second measurement windows.

**Test environment:** Apple M5, 32GB RAM, macOS 26.2, Deno 2.6.5
**Last run:** 2026-01-19

---

## The Solution Space

Client-side search is a crowded space. FlexSearch optimizes for raw speed. Lunr.js brings Lucene-style syntax. MiniSearch balances features and size. Fuse.js trades everything for fuzzy matching simplicity.

Each library makes different tradeoffs:

| Priority | Optimizes For | Sacrifices |
|----------|---------------|------------|
| **Speed** (FlexSearch) | Query latency, bundle size | Fuzzy/substring search, features |
| **Syntax** (Lunr.js) | Lucene compatibility, stemming | Bundle size, query speed |
| **Balance** (MiniSearch) | Feature set, reasonable size | True fuzzy matching |
| **Simplicity** (Fuse.js) | Zero-config fuzzy | Speed (1000x slower) |
| **Capability** (Sorex) | Substring + fuzzy + correctness | Bundle size |

Sorex deliberately trades payload size for search capability and correctness. The WASM bundle is larger than pure JS alternatives, but it enables features they cannot provide: true substring search, Levenshtein-based typo tolerance, and formally verified ranking.

---

## The Tradeoff in Practice

Suffix arrays enable true substring and fuzzy search that inverted indexes cannot do. The cost is a larger WASM bundle.

| Library | Substring Search | Typo Tolerance | Bundle (gzip) |
|---------|------------------|----------------|---------------|
| **Sorex (WASM)** | Yes | Yes (Levenshtein) | 153 KB |
| FlexSearch | No | No | 6.6 KB |
| Fuse.js | Via fuzzy only | Yes (slow) | 15.3 KB |
| MiniSearch | Prefix only | Partial | 17.9 KB |
| Lunr.js | No | No | 24.3 KB |

---

## Time to First Search

How long from loading until search is ready? This is what users experience.

**Sorex** loads a pre-built binary index (`.sorex` file). **JS libraries** build indexes from documents at runtime.

| Library | Time | Notes |
|---------|------|-------|
| **Sorex (WASM)** | 0.12ms | Load pre-built .sorex binary |
| fuse.js | 0.05ms | No index (stores raw data) |
| FlexSearch | 0.32ms | Build from documents |
| MiniSearch | 0.85ms | Build from documents |
| lunr.js | 2.61ms | Build from documents |

*Measured on European Countries dataset (30 documents).*

Sorex's load time is predictable because it doesn't build anything at runtime. JS libraries scale with document count. Lunr.js takes 50ms+ on 100 documents.

### Offline Index Build (Sorex)

Sorex builds indexes ahead of time using the `sorex index` CLI or Rust API. This happens at deploy time, not in the browser.

```bash
# Build index from documents (WASM is embedded by default)
sorex index --input ./docs --output ./search-index
```

For native Rust build times, run `cargo bench -- index_build`.

---

## Query Latency

Search performance on European Countries dataset (30 documents). Sorex shows tier latencies: **T1** (exact), **T2** (prefix), **All** (including fuzzy).

### Exact Word Queries

Query: `"history"` - Common word (11+ results expected)

| Library | Results | T1 (us) | T2 (us) | All (us) | ops/sec |
|---------|---------|---------|---------|----------|---------|
| **Sorex** | 10 | 2.1 | 3.8 | 6.6 | 151,158 |
| FlexSearch | 15 | - | - | 0.4 | 2,426,517 |
| lunr.js | 11 | - | - | 6.5 | 153,586 |
| MiniSearch | 11 | - | - | 7.7 | 130,599 |
| fuse.js | 14 | - | - | 555.8 | 1,799 |

FlexSearch is fastest for exact matches because it has the tightest inner loop. Fuse.js is 1000x slower because it scans all documents and computes fuzzy scores. **Sorex shows first results (T1) in 2.1us** - users see results immediately while fuzzy search continues.

### Rare Terms

Query: `"fjords"` (appears in Norway only)

| Library | Results | T1 (us) | T2 (us) | All (us) | ops/sec |
|---------|---------|---------|---------|----------|---------|
| **Sorex** | 1 | 0.4 | 0.8 | 64.4 | 15,524 |
| FlexSearch | 0 | - | - | 0.3 | 3,659,951 |
| lunr.js | 0 | - | - | 1.1 | 876,883 |
| MiniSearch | 0 | - | - | 5.1 | 194,748 |
| fuse.js | 0 | - | - | 527.0 | 1,898 |

**Only Sorex finds the result.** Inverted indexes don't index rare terms - they require minimum document frequency thresholds. Sorex's suffix array finds any string, rare or common. The 64us total is dominated by fuzzy search, but T1/T2 complete in under 1us.

---

## The Killer Feature: Substring Search

This is why Sorex exists. Query: `"land"` (to find "Iceland", "Finland", "landlocked", etc.)

| Library | Results | T1 (us) | T2 (us) | All (us) | Notes |
|---------|---------|---------|---------|----------|-------|
| **Sorex** | **10** | 0.5 | 5.2 | 59.8 | Finds substring matches |
| MiniSearch | 30 | - | - | 13.2 | Fuzzy mode (over-inclusive) |
| fuse.js | 30 | - | - | 387.9 | Fuzzy matches everything |
| lunr.js | **0** | - | - | 1.1 | No substring support |
| FlexSearch | **0** | - | - | 0.3 | No substring support |

**Key insight:** Sorex returns 8 prefix matches (T2) in just 5.2us - before the 60us fuzzy search even starts. Users see "Finland", "Iceland", "landlocked" immediately.

### Why Result Counts Differ

The result count differences reveal fundamental architectural differences:

**Sorex (10 results):** Finds documents containing actual substring matches like "Iceland", "Finland", "landlocked", "landscape", "mainland". Returns precise matches only.

**MiniSearch (30 results):** With `fuzzy: 0.2`, the query "land" fuzzy-expands to match "and" (edit distance 1). Since every document contains the word "and", all 30 documents match. These are false positives.

**Fuse.js (30 results):** Similar issue - its fuzzy threshold is permissive enough to match nearly everything for short queries.

**Lunr.js / FlexSearch (0 results):** Inverted indexes tokenize by whole words. "land" is not a token in the vocabulary (only "Poland", "Finland", etc. as complete words), so no results are returned.

### Stop Word Filtering

Sorex filters common stop words (like "and", "the", "is") at index construction time. This:

1. **Prevents false positives**: "land" won't fuzzy-match to "and"
2. **Reduces index size**: Vocabulary dropped from 740 to 700 terms
3. **Improves relevance**: Results contain meaningful matches only

Stop words are defined in `data/stop_words.json` and cover 20+ languages including English, Spanish, French, German, Portuguese, Italian, Dutch, Russian, Polish, Nordic languages, Turkish, and Indonesian.

### More Substring Tests

| Query | Target | Sorex | lunr.js | FlexSearch |
|-------|--------|-------|---------|------------|
| `"burg"` | Luxembourg, Hamburg | **10** | 0 | 0 |
| `"ian"` | Italian, Croatian, Romanian | **4** | 0 | 0 |

Inverted indexes tokenize by words - they cannot match substrings within words. When users search "land", they expect Iceland and Finland. Only suffix arrays (Sorex) or full-text fuzzy (fuse.js, very slow) find them.

---

## Typo Tolerance

Query: `"popultion"` (typo for "population", edit distance 1)

| Library | Results | T1 (us) | T2 (us) | All (us) | Notes |
|---------|---------|---------|---------|----------|-------|
| **Sorex** | **10** | 0.4 | 0.9 | 65.1 | Levenshtein automata (precise) |
| MiniSearch | 30 | - | - | 44.6 | Fuzzy mode (over-inclusive) |
| fuse.js | 29 | - | - | 522.8 | Fuzzy matching (very slow) |
| lunr.js | 0 | - | - | 1.4 | No fuzzy support |
| FlexSearch | 0 | - | - | 0.3 | No fuzzy support |

Query: `"mediteranean"` (typo for "mediterranean", edit distance 1)

| Library | Results | T1 (us) | T2 (us) | All (us) | Notes |
|---------|---------|---------|---------|----------|-------|
| **Sorex** | **8** | 0.4 | 1.0 | 41.7 | Correct matches only |
| MiniSearch | 8 | - | - | 69.4 | Fuzzy mode |
| fuse.js | 8 | - | - | 779.3 | Fuzzy matching (slow) |
| lunr.js | 0 | - | - | 1.6 | No fuzzy support |
| FlexSearch | 0 | - | - | 0.3 | No fuzzy support |

**Key difference:** Sorex uses Levenshtein automata for true edit-distance matching within distance 2. MiniSearch's fuzzy mode uses prefix expansion (generates all possible prefixes), which produces false positives and is not true edit-distance matching. Note that T1/T2 return quickly (no matches for typos), while all results come from T3 fuzzy search.

---

## Streaming Search: Progressive Results

Sorex's streaming API returns results in three tiers, enabling progressive UX where users see results faster:

- **Tier 1 (Exact)**: O(1) inverted index lookup - show immediately
- **Tier 2 (Prefix)**: O(log k) binary search on suffix array
- **Tier 3 (Fuzzy)**: O(vocabulary) Levenshtein DFA scan

### Common Query: `"European"`

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sorex (streaming)** | 30 | **9.1us** | 19.8us | 103.9us | 228.6us | 10+10+10 |
| FlexSearch | 31 | - | - | 0.7us | 4.7us | 31 |
| lunr.js | 29 | - | - | 12.4us | 26.9us | 29 |
| MiniSearch | 30 | - | - | 30.7us | 44.3us | 30 |
| fuse.js | 30 | - | - | 307.3us | 396.1us | 30 |

**Sorex shows 10 exact matches in 9.1us** - users see results 10x faster than waiting for all 30.

### Substring Query: `"land"`

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sorex (streaming)** | 18 | 0.5us | **5.2us** | 64.9us | 91.9us | 0+8+10 |
| FlexSearch | 0 | - | - | 0.3us | 0.8us | 0 |
| lunr.js | 0 | - | - | 1.3us | 3.2us | 0 |
| MiniSearch | 30 | - | - | 13.7us | 19.3us | 30 |
| fuse.js | 30 | - | - | 398.2us | 500.2us | 30 |

**No exact match for "land"** (tier 1 returns 0), but **8 prefix matches in 5.2us** from words like "landlocked", "landscape". Tier 3 adds 10 fuzzy matches.

*Note: FlexSearch/lunr.js return 0 results - they don't support substring search. MiniSearch/fuse.js return 30 because their fuzzy matching is over-inclusive (see "Why Result Counts Differ" above).*

### Fuzzy Query: `"mediteranean"` (typo)

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sorex (streaming)** | 8 | 0.4us | 1.0us | **43.1us** | 60.0us | 0+0+8 |
| FlexSearch | 0 | - | - | 0.4us | 1.0us | 0 |
| lunr.js | 0 | - | - | 1.9us | 5.3us | 0 |
| MiniSearch | 8 | - | - | 69.0us | 95.4us | 8 |
| fuse.js | 8 | - | - | 791.5us | 916.2us | 8 |

**All results come from fuzzy tier** (typo doesn't match exactly or as prefix). Sorex's Levenshtein DFA is faster than fuse.js (~17x) and competitive with MiniSearch.

---

## Index Sizes

Serialized index size for European Countries dataset (30 documents, 23KB raw):

| Library | Raw | Gzipped | Notes |
|---------|-----|---------|-------|
| Raw Data | 23.4 KB | 6.5 KB | - |
| **Sorex (.sorex)** | 29.0 KB | 15.6 KB | Binary format |
| fuse.js | 23.4 KB | 6.5 KB | No index |
| FlexSearch | 22.4 KB | 6.8 KB | - |
| MiniSearch | 33.4 KB | 7.8 KB | - |
| lunr.js | 68.4 KB | 12.8 KB | - |

Sorex's binary format is slightly larger than raw data but includes:
- Suffix array for substring search
- Vocabulary (700 terms after stop word filtering)
- Posting lists for fast exact matching
- Document metadata

### Sorex Binary Format Components

| Component | Size | Notes |
|-----------|------|-------|
| Header | 52 bytes | Magic, version, counts |
| Vocabulary | ~500 bytes | Sorted term list |
| Suffix Array | ~8 KB | Delta + varint encoded |
| Postings | ~15 KB | Block PFOR compressed |
| Levenshtein DFA | ~1.2 KB | Precomputed automaton |
| Documents | varies | Metadata for results |
| Dictionary Tables | varies | Parquet-style compression |
| Footer | 8 bytes | CRC32 + magic |

---

## Bundle Sizes

Library code size (what users download):

| Library | Raw | Gzipped |
|---------|-----|---------|
| FlexSearch | 16 KB | 6.6 KB |
| fuse.js | 66 KB | 15.3 KB |
| MiniSearch | 76 KB | 17.9 KB |
| lunr.js | 97 KB | 24.3 KB |
| **Sorex (WASM + loader)** | 346 KB | 153 KB |

The Sorex bundle consists of:
- `sorex_bg.wasm`: 329 KB raw, 153 KB gzipped (embedded in .sorex file)
- `sorex.js`: 17 KB raw, 4.5 KB gzipped (self-contained, no dependencies)

Sorex's WASM bundle is larger because it includes:
- Suffix array construction and search
- Levenshtein automata (precomputed DFA)
- Block PFOR compression/decompression
- Binary format parsing

---

## Feature Comparison

| Feature | Sorex | FlexSearch | MiniSearch | lunr.js | fuse.js |
|---------|-------|------------|------------|---------|---------|
| Exact word match | Yes | Yes | Yes | Yes | Yes |
| Prefix search | Yes | Yes | Yes | Yes | Yes |
| Substring search | **Yes** | No | No | No | Partial |
| Typo tolerance | **Yes** | No | Partial | No | Yes |
| Field weighting | **Proven** | Yes | Yes | Yes | Yes |
| Stop word filtering | **Yes** | Configurable | Configurable | Yes | No |
| Stemming | No | No | Yes | Yes | No |
| Boolean queries | Yes | Partial | Yes | Yes | No |
| Deep linking | **Yes** | No | No | No | No |
| Binary format | **Yes** | No | No | No | No |
| Progressive loading | **Yes** | No | No | No | No |

---

## When to Use What

The right choice depends on what you're willing to trade.

**Use Sorex when:**
- Search capability matters more than bundle size
- Users expect substring search ("auth" -> "authentication")
- Typo tolerance is essential ("typscript" -> "typescript")
- Correctness matters (field ranking is proven, not tested)
- You can accept ~150KB for features JS libraries can't provide

**Use FlexSearch when:**
- Bundle size is the constraint (6.6KB)
- Users search exact words only
- Speed is everything (2M+ ops/sec)

**Use MiniSearch when:**
- You need a balance of features and size
- Stemming is important
- Advanced query syntax (boolean, field-specific) is needed

**Use lunr.js when:**
- Lucene-style query syntax is required
- Stemming and stop words matter
- Your team knows Lucene/Elasticsearch

**Use fuse.js when:**
- Zero configuration is the priority
- Dataset is small (<1000 items)
- 500ms+ query latency is acceptable

---

## Running Benchmarks

```bash
cd benches && bun install

# European Countries benchmark (includes Sorex)
bun run bench:eu

# NVIDIA CUTLASS documentation benchmark
bun run crawl:cutlass  # Crawl docs.nvidia.com (outputs to datasets/cutlass/)
sorex index --input datasets/cutlass --output datasets/cutlass
bun run bench:cutlass  # Run benchmarks, outputs to RESULTS-CUTLASS.md

# PyTorch documentation benchmark
bun run crawl:pytorch  # Crawl pytorch.org (outputs to datasets/pytorch/)
sorex index --input datasets/pytorch --output datasets/pytorch
bun run bench:pytorch  # Run benchmarks, outputs to RESULTS-PYTORCH.md

# Synthetic corpus (JS libraries only)
bun run bench

# Memory usage
bun run bench:memory

# Index sizes
bun run bench:sizes

# Results saved to benches/RESULTS-EU.md
```

### Rust Benchmarks

```bash
# Index build time (native)
cargo bench -- index_build

# Search query performance
cargo bench -- search_query

# All benchmarks
cargo bench
```

---

## Methodology

**Dataset:**
- European Countries: 30 full Wikipedia-style articles (~100KB)
- ~3,500 words per country covering history, culture, economy, geography
- Includes proper nouns, technical terms, common words
- Generated by `examples/generate-eu-data.sh`

**Measurement:**
- 5-second measurement windows (1s for quick mode)
- 1000+ iterations per benchmark
- Warmup runs excluded
- 99% confidence intervals

**Environment:**
- Apple M5, 32GB RAM
- macOS 26.2
- Bun 1.3.5
- Libraries: fuse.js 7.0, lunr.js 2.3, flexsearch 0.7, minisearch 7.1

---

## CUTLASS Documentation Benchmark

Real-world benchmark on NVIDIA CUTLASS documentation (70 pages of technical GPU programming documentation).

**Dataset:** NVIDIA CUTLASS documentation
- 70 documents, 6,348 terms
- Index size: 1.1 MB (275.9 KB with Brotli)
- Technical vocabulary: "gemm", "tensor", "warp", "synchronize", "epilogue"
- Mix of API reference, tutorials, and architecture docs

**Datasets available:** [cutlass-dataset.zip](https://assets.harryzorus.xyz/datasets/cutlass-dataset.zip) (352 KB)

### Search Latency: Sorex Three-Tier Architecture

Sorex returns results progressively in three tiers. Users see exact matches instantly while fuzzy search continues in background.

#### Exact Queries (word exists in vocabulary)

| Query | T1 Exact | T2 Prefix | T3 Fuzzy | Total | T1 Results | T2 Results | T3 Results |
|-------|----------|-----------|----------|-------|------------|------------|------------|
| gemm | 6.9 μs | 6.9 μs | 194 μs | 194 μs | 36 | 0 | 19 |
| kernel | 4.8 μs | 4.8 μs | 244 μs | 244 μs | 42 | 0 | 0 |
| tensor | 13.1 μs | 13.1 μs | 263 μs | 263 μs | 48 | 0 | 0 |
| warp | 4.9 μs | 4.9 μs | 201 μs | 201 μs | 24 | 0 | 29 |
| cuda | 3.2 μs | 3.2 μs | 219 μs | 219 μs | 42 | 0 | 27 |
| epilogue | 2.4 μs | 2.4 μs | 266 μs | 266 μs | 18 | 0 | 0 |

**Key insight:** T1 completes in 3-13 μs, showing users 18-48 exact matches instantly. Full fuzzy search completes in ~195-265 μs.

#### Prefix Queries (partial word, finds completions)

| Query | T1 Exact | T2 Prefix | T3 Fuzzy | Total | T1 Results | T2 Results | T3 Results |
|-------|----------|-----------|----------|-------|------------|------------|------------|
| ker | 0.0 μs | 9.7 μs | 189 μs | 189 μs | 0 | 45 | 20 |
| ten | 0.0 μs | 15.2 μs | 209 μs | 209 μs | 0 | 51 | 19 |
| war | 0.0 μs | 7.1 μs | 180 μs | 180 μs | 0 | 45 | 17 |
| gem | 0.0 μs | 8.9 μs | 206 μs | 206 μs | 0 | 37 | 30 |
| mat | 0.0 μs | 7.1 μs | 218 μs | 218 μs | 0 | 45 | 25 |
| epi | 0.0 μs | 4.2 μs | 183 μs | 183 μs | 0 | 13 | 42 |
| sync | 3.3 μs | 3.3 μs | 193 μs | 193 μs | 12 | 0 | 18 |

**Key insight:** Prefix search (T2) completes in 4-15 μs, finding 13-51 matches. No exact matches for partial words.

#### Fuzzy Queries (typos, edit distance 1-2)

| Query | T1 Exact | T2 Prefix | T3 Fuzzy | Total | Results |
|-------|----------|-----------|----------|-------|---------|
| kernal | 0.0 μs | 0.0 μs | 261 μs | 261 μs | 45 |
| tensr | 0.0 μs | 0.0 μs | 258 μs | 258 μs | 56 |
| wrp | 0.0 μs | 0.0 μs | 191 μs | 191 μs | 65 |
| gemn | 0.0 μs | 0.0 μs | 208 μs | 208 μs | 55 |
| matrx | 0.0 μs | 0.0 μs | 226 μs | 226 μs | 42 |
| epilouge | 0.0 μs | 0.0 μs | 270 μs | 270 μs | 18 |
| syncronize | 0.0 μs | 0.0 μs | 257 μs | 257 μs | 0 |

**Key insight:** All results come from T3 fuzzy tier (typos don't match exactly or as prefix). Levenshtein automata find 18-65 matches in ~190-270 μs.

### Library Comparison

How Sorex compares to JavaScript search libraries on the CUTLASS dataset:

| Query Type | Sorex T1 | Sorex Total | FlexSearch | MiniSearch | lunr.js | fuse.js |
|------------|----------|-------------|------------|------------|---------|---------|
| **Exact** (gemm) | **5.8 μs** | 208 μs | 2.6 μs | 84 μs | 41 μs | 12,521 μs |
| **Prefix** (ker) | 0 μs | 190 μs | 0.5 μs (0 results) | 47 μs | 2.2 μs (0 results) | 9,726 μs |
| **Fuzzy** (kernal) | 0 μs | 257 μs | 0.6 μs (0 results) | 70 μs | 1.6 μs (0 results) | 20,781 μs |

**Key findings:**
- **FlexSearch** is fastest but returns **0 results** for prefix/fuzzy queries
- **Sorex** shows first results in 2-10 μs (T1), then completes fuzzy in 180-280 μs
- **MiniSearch** finds fuzzy matches but is slower (17-145 μs)
- **fuse.js** is 50-100x slower than Sorex (9,000-36,000 μs)

See [full comparison report](./comparisons/cutlass.md) for all queries.

---

## PyTorch Documentation Benchmark

Real-world benchmark on PyTorch 2.9 documentation (300 documents covering neural network modules, optimizers, loss functions).

**Dataset:** PyTorch 2.9 API documentation
- 300 documents, 10,837 terms
- Index size: 2.0 MB (Brotli compressed)
- Technical vocabulary: "tensor", "module", "forward", "backward", "autograd", "optim"
- Mix of tutorials, API reference, and examples

**Datasets available:** [pytorch-dataset.zip](https://assets.harryzorus.xyz/datasets/pytorch-dataset.zip) (841 KB)

### Search Latency: Sorex Three-Tier Architecture

#### Exact Queries (word exists in vocabulary)

| Query | T1 Exact | T2 Prefix | T3 Fuzzy | Total | T1 Results | T2 Results | T3 Results |
|-------|----------|-----------|----------|-------|------------|------------|------------|
| tensor | 55.4 μs | 91.7 μs | 491 μs | 491 μs | 248 | 10 | 0 |
| module | 29.7 μs | 29.7 μs | 471 μs | 471 μs | 70 | 0 | 31 |
| forward | 8.5 μs | 8.5 μs | 429 μs | 429 μs | 54 | 0 | 0 |
| backward | 5.7 μs | 5.7 μs | 436 μs | 436 μs | 51 | 0 | 0 |
| autograd | 6.0 μs | 6.0 μs | 443 μs | 443 μs | 79 | 0 | 0 |
| optim | 9.4 μs | 21.1 μs | 394 μs | 394 μs | 21 | 31 | 39 |

**Key insight:** T1 completes in 6-55 μs, showing users 21-248 exact matches instantly. Larger dataset (300 docs) means longer search times but still sub-millisecond.

#### Prefix Queries (partial word, finds completions)

| Query | T1 Exact | T2 Prefix | T3 Fuzzy | Total | T1 Results | T2 Results | T3 Results |
|-------|----------|-----------|----------|-------|------------|------------|------------|
| ten | 0.0 μs | 68.9 μs | 414 μs | 414 μs | 0 | 259 | 34 |
| mod | 0.0 μs | 52.6 μs | 405 μs | 405 μs | 0 | 105 | 93 |
| for | 0.0 μs | 21.2 μs | 381 μs | 381 μs | 0 | 99 | 87 |
| back | 3.6 μs | 21.5 μs | 362 μs | 362 μs | 31 | 51 | 43 |
| auto | 3.3 μs | 22.8 μs | 386 μs | 386 μs | 10 | 91 | 113 |
| opt | 0.0 μs | 40.8 μs | 397 μs | 397 μs | 0 | 186 | 95 |

**Key insight:** Prefix search (T2) completes in 21-69 μs, finding 51-259 matches. Users typing "ten" see tensor-related docs immediately.

#### Fuzzy Queries (typos, edit distance 1-2)

| Query | T1 Exact | T2 Prefix | T3 Fuzzy | Total | Results |
|-------|----------|-----------|----------|-------|---------|
| tensro | 0.0 μs | 0.0 μs | 574 μs | 574 μs | 256 |
| modul | 0.0 μs | 33.8 μs | 440 μs | 440 μs | 106 |
| forwrd | 0.0 μs | 0.0 μs | 435 μs | 435 μs | 56 |
| backwrd | 0.0 μs | 0.0 μs | 463 μs | 463 μs | 73 |
| autogrd | 0.0 μs | 0.0 μs | 458 μs | 458 μs | 79 |
| optimzer | 0.0 μs | 0.0 μs | 448 μs | 448 μs | 41 |

**Key insight:** Typo queries find 41-256 matches via Levenshtein automata in ~435-575 μs. "tensro" (typo for "tensor") finds 256 relevant documents.

### Library Comparison

How Sorex compares to JavaScript search libraries on the PyTorch dataset:

| Query Type | Sorex T1 | Sorex Total | FlexSearch | MiniSearch | lunr.js | fuse.js |
|------------|----------|-------------|------------|------------|---------|---------|
| **Exact** (tensor) | **50.8 μs** | 518 μs | 3.5 μs | 184 μs | 245 μs | 33,403 μs |
| **Prefix** (ten) | 0 μs | 417 μs | 0.5 μs (3 results) | 136 μs | 6.2 μs (4 results) | 30,698 μs |
| **Fuzzy** (tensro) | 0 μs | 563 μs | 0.5 μs (0 results) | 187 μs | 1.4 μs (0 results) | 61,951 μs |

**Key findings:**
- **FlexSearch** is fastest but returns **0-3 results** for prefix/fuzzy queries (vs 259-256 for Sorex)
- **Sorex** shows first results in 5-51 μs (T1), then completes fuzzy in 350-560 μs
- **MiniSearch** finds fuzzy matches and is competitive (35-187 μs)
- **fuse.js** is 50-100x slower than Sorex (27,000-76,000 μs)

See [full comparison report](./comparisons/pytorch.md) for all queries.

---

## Index Size Benchmarks

Measured on synthetic datasets with varying sizes:

### Small Corpus (20 posts, 500 words each = 74KB)

| Library | Raw Size | Gzipped | Ratio | Notes |
|---------|----------|---------|-------|-------|
| Fuse.js | 74.3 KB | 18.3 KB | 1.0x | No index |
| Lunr.js | 35.1 KB | 5.1 KB | 0.5x | Inverted index |
| FlexSearch | 15.3 KB | 4.0 KB | 0.2x | Trie-based |
| MiniSearch | 19.3 KB | 4.0 KB | 0.3x | Compact format |

### Medium Corpus (100 posts, 1000 words each = 740KB)

| Library | Raw Size | Gzipped | Ratio | Notes |
|---------|----------|---------|-------|-------|
| Fuse.js | 740 KB | 172 KB | 1.0x | No index |
| Lunr.js | 162 KB | 12.1 KB | 0.2x | Inverted index |
| FlexSearch | 46.7 KB | 11.2 KB | 0.1x | Trie-based |
| MiniSearch | 96.4 KB | 20.7 KB | 0.1x | Compact format |

### WASM Bundle Comparison

| Component | Raw | Gzipped |
|-----------|-----|---------|
| sorex_bg.wasm | 329 KB | 153 KB |
| sorex.js | 17 KB | 4.5 KB |
| **Total** | **346 KB** | **~153 KB** |

**Context:** Sorex's WASM is larger than pure JS libraries (6-24KB gzipped), but includes capabilities they cannot provide: true substring search, Levenshtein-based fuzzy matching, and formally verified ranking.

---

## Running Benchmarks

Sorex includes a comprehensive benchmark suite that compares against competing libraries.

### Quick Start

```bash
# Full benchmark suite (all datasets + library comparison)
bun run tools/bench.ts

# Quick mode (fewer iterations, faster)
bun run tools/bench.ts --quick

# Single dataset
bun run tools/bench.ts --dataset cutlass
bun run tools/bench.ts --dataset pytorch

# Skip steps
bun run tools/bench.ts --skip-crawl      # Use existing dataset
bun run tools/bench.ts --skip-index      # Use existing index
bun run tools/bench.ts --skip-compare    # Skip library comparison
```

### What the Benchmark Suite Does

1. **Idempotent Dataset Crawling** - Downloads CUTLASS/PyTorch docs if stale (>1 week)
2. **Compilation Check** - Rebuilds Sorex binary/WASM only if source changed
3. **Fresh Indexing** - Reindexes datasets for accurate measurements
4. **Statistical Benchmarking** - Proper warmup, confidence intervals, p99 latency
5. **Library Comparison** - Compares against FlexSearch, MiniSearch, lunr.js, fuse.js

### Generated Reports

| File | Description |
|------|-------------|
| `docs/comparisons/cutlass.md` | Library comparison on CUTLASS dataset |
| `docs/comparisons/pytorch.md` | Library comparison on PyTorch dataset |
| `target/bench-results/*.json` | Raw benchmark data (gitignored) |

### Measurement Methodology

**Adaptive Iteration (like Criterion)**

The benchmark suite uses adaptive iteration similar to Rust's Criterion library:

1. Run minimum iterations (10 quick, 20 full)
2. After each batch, compute confidence interval
3. Stop when CI is within target percentage of mean (10% quick, 5% full)
4. Cap at maximum iterations (100 quick, 500 full)

This ensures:
- Fast benchmarks converge quickly (fewer iterations needed)
- Slow/variable benchmarks get more iterations automatically
- Results marked with `!` suffix didn't converge (hit max iterations)

**Configuration:**

| Mode | Warmup | Min Iters | Max Iters | Target CI | Confidence |
|------|--------|-----------|-----------|-----------|------------|
| Quick | 5 | 10 | 100 | 10% | 95% |
| Full | 10 | 20 | 500 | 5% | 99% |

**Statistics:**
- Mean, stddev, confidence interval using Student's t-distribution
- P99 latency for tail performance
- Environment info (platform/arch) for reproducibility

### Native Rust Benchmarks

For micro-benchmarks of individual algorithms:

```bash
cargo bench                    # All benchmarks
cargo bench -- index_build     # Index building only
cargo bench -- search          # Search algorithms
```

---

## Related Documentation

- [Integration](./integration.md) - Get Sorex running in your project
- [Architecture](./architecture.md) - How Sorex's index structures work
- [Algorithms](./algorithms.md) - Suffix arrays, Levenshtein automata
- [CLI Reference](./cli.md) - Build indexes with `sorex index`
- [TypeScript API](./typescript.md) - Browser WASM bindings
- [Rust API](./rust.md) - Library API for benchmarking
