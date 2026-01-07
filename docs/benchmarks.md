# Benchmarks

Performance comparisons between Sieve and popular JavaScript search libraries. Benchmarks use the European Countries dataset (30 full Wikipedia-style articles, ~3,500 words each) and Tinybench with 5-second measurement windows.

**Test environment:** Apple M5, 32GB RAM, macOS 26.2, Bun 1.3.5
**Last run:** 2026-01-05

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
| **Capability** (Sieve) | Substring + fuzzy + correctness | Bundle size |

Sieve deliberately trades payload size for search capability and correctness. The WASM bundle is larger than pure JS alternatives, but it enables features they cannot provide: true substring search, Levenshtein-based typo tolerance, and formally verified ranking.

---

## The Tradeoff in Practice

Suffix arrays enable true substring and fuzzy search that inverted indexes cannot do. The cost is a larger WASM bundle.

| Library | Substring Search | Typo Tolerance | Bundle (gzip) |
|---------|------------------|----------------|---------------|
| **Sieve (WASM)** | Yes | Yes (Levenshtein) | 153 KB |
| FlexSearch | No | No | 6.6 KB |
| Fuse.js | Via fuzzy only | Yes (slow) | 15.3 KB |
| MiniSearch | Prefix only | Partial | 17.9 KB |
| Lunr.js | No | No | 24.3 KB |

---

## Time to First Search

How long from loading until search is ready? This is what users experience.

**Sieve** loads a pre-built binary index (`.sieve` file). **JS libraries** build indexes from documents at runtime.

| Library | Time | Notes |
|---------|------|-------|
| **Sieve (WASM)** | 0.12ms | Load pre-built .sieve binary |
| fuse.js | 0.05ms | No index (stores raw data) |
| FlexSearch | 0.32ms | Build from documents |
| MiniSearch | 0.85ms | Build from documents |
| lunr.js | 2.61ms | Build from documents |

*Measured on European Countries dataset (30 documents).*

Sieve's load time is predictable because it doesn't build anything at runtime. JS libraries scale with document count. Lunr.js takes 50ms+ on 100 documents.

### Offline Index Build (Sieve)

Sieve builds indexes ahead of time using the `sieve index` CLI or Rust API. This happens at deploy time, not in the browser.

```bash
# Build index from documents (WASM is embedded by default)
sieve index --input ./docs --output ./search-index
```

For native Rust build times, run `cargo bench -- index_build`.

---

## Query Latency

Search performance on European Countries dataset (30 documents). Sieve shows tier latencies: **T1** (exact), **T2** (prefix), **All** (including fuzzy).

### Exact Word Queries

Query: `"history"` - Common word (11+ results expected)

| Library | Results | T1 (us) | T2 (us) | All (us) | ops/sec |
|---------|---------|---------|---------|----------|---------|
| **Sieve** | 10 | 2.1 | 3.8 | 6.6 | 151,158 |
| FlexSearch | 15 | - | - | 0.4 | 2,426,517 |
| lunr.js | 11 | - | - | 6.5 | 153,586 |
| MiniSearch | 11 | - | - | 7.7 | 130,599 |
| fuse.js | 14 | - | - | 555.8 | 1,799 |

FlexSearch is fastest for exact matches because it has the tightest inner loop. Fuse.js is 1000x slower because it scans all documents and computes fuzzy scores. **Sieve shows first results (T1) in 2.1us** - users see results immediately while fuzzy search continues.

### Rare Terms

Query: `"fjords"` (appears in Norway only)

| Library | Results | T1 (us) | T2 (us) | All (us) | ops/sec |
|---------|---------|---------|---------|----------|---------|
| **Sieve** | 1 | 0.4 | 0.8 | 64.4 | 15,524 |
| FlexSearch | 0 | - | - | 0.3 | 3,659,951 |
| lunr.js | 0 | - | - | 1.1 | 876,883 |
| MiniSearch | 0 | - | - | 5.1 | 194,748 |
| fuse.js | 0 | - | - | 527.0 | 1,898 |

**Only Sieve finds the result.** Inverted indexes don't index rare terms - they require minimum document frequency thresholds. Sieve's suffix array finds any string, rare or common. The 64us total is dominated by fuzzy search, but T1/T2 complete in under 1us.

---

## The Killer Feature: Substring Search

This is why Sieve exists. Query: `"land"` (to find "Iceland", "Finland", "landlocked", etc.)

| Library | Results | T1 (us) | T2 (us) | All (us) | Notes |
|---------|---------|---------|---------|----------|-------|
| **Sieve** | **10** | 0.5 | 5.2 | 59.8 | Finds substring matches |
| MiniSearch | 30 | - | - | 13.2 | Fuzzy mode (over-inclusive) |
| fuse.js | 30 | - | - | 387.9 | Fuzzy matches everything |
| lunr.js | **0** | - | - | 1.1 | No substring support |
| FlexSearch | **0** | - | - | 0.3 | No substring support |

**Key insight:** Sieve returns 8 prefix matches (T2) in just 5.2us - before the 60us fuzzy search even starts. Users see "Finland", "Iceland", "landlocked" immediately.

### Why Result Counts Differ

The result count differences reveal fundamental architectural differences:

**Sieve (10 results):** Finds documents containing actual substring matches like "Iceland", "Finland", "landlocked", "landscape", "mainland". Returns precise matches only.

**MiniSearch (30 results):** With `fuzzy: 0.2`, the query "land" fuzzy-expands to match "and" (edit distance 1). Since every document contains the word "and", all 30 documents match. These are false positives.

**Fuse.js (30 results):** Similar issue - its fuzzy threshold is permissive enough to match nearly everything for short queries.

**Lunr.js / FlexSearch (0 results):** Inverted indexes tokenize by whole words. "land" is not a token in the vocabulary (only "Poland", "Finland", etc. as complete words), so no results are returned.

### Stop Word Filtering

Sieve filters common stop words (like "and", "the", "is") at index construction time. This:

1. **Prevents false positives**: "land" won't fuzzy-match to "and"
2. **Reduces index size**: Vocabulary dropped from 740 to 700 terms
3. **Improves relevance**: Results contain meaningful matches only

Stop words are defined in `data/stop_words.json` and cover 20+ languages including English, Spanish, French, German, Portuguese, Italian, Dutch, Russian, Polish, Nordic languages, Turkish, and Indonesian.

### More Substring Tests

| Query | Target | Sieve | lunr.js | FlexSearch |
|-------|--------|-------|---------|------------|
| `"burg"` | Luxembourg, Hamburg | **10** | 0 | 0 |
| `"ian"` | Italian, Croatian, Romanian | **4** | 0 | 0 |

Inverted indexes tokenize by words - they cannot match substrings within words. When users search "land", they expect Iceland and Finland. Only suffix arrays (Sieve) or full-text fuzzy (fuse.js, very slow) find them.

---

## Typo Tolerance

Query: `"popultion"` (typo for "population", edit distance 1)

| Library | Results | T1 (us) | T2 (us) | All (us) | Notes |
|---------|---------|---------|---------|----------|-------|
| **Sieve** | **10** | 0.4 | 0.9 | 65.1 | Levenshtein automata (precise) |
| MiniSearch | 30 | - | - | 44.6 | Fuzzy mode (over-inclusive) |
| fuse.js | 29 | - | - | 522.8 | Fuzzy matching (very slow) |
| lunr.js | 0 | - | - | 1.4 | No fuzzy support |
| FlexSearch | 0 | - | - | 0.3 | No fuzzy support |

Query: `"mediteranean"` (typo for "mediterranean", edit distance 1)

| Library | Results | T1 (us) | T2 (us) | All (us) | Notes |
|---------|---------|---------|---------|----------|-------|
| **Sieve** | **8** | 0.4 | 1.0 | 41.7 | Correct matches only |
| MiniSearch | 8 | - | - | 69.4 | Fuzzy mode |
| fuse.js | 8 | - | - | 779.3 | Fuzzy matching (slow) |
| lunr.js | 0 | - | - | 1.6 | No fuzzy support |
| FlexSearch | 0 | - | - | 0.3 | No fuzzy support |

**Key difference:** Sieve uses Levenshtein automata for true edit-distance matching within distance 2. MiniSearch's fuzzy mode uses prefix expansion (generates all possible prefixes), which produces false positives and is not true edit-distance matching. Note that T1/T2 return quickly (no matches for typos), while all results come from T3 fuzzy search.

---

## Streaming Search: Progressive Results

Sieve's streaming API returns results in three tiers, enabling progressive UX where users see results faster:

- **Tier 1 (Exact)**: O(1) inverted index lookup - show immediately
- **Tier 2 (Prefix)**: O(log k) binary search on suffix array
- **Tier 3 (Fuzzy)**: O(vocabulary) Levenshtein DFA scan

### Common Query: `"European"`

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sieve (streaming)** | 30 | **9.1us** | 19.8us | 103.9us | 228.6us | 10+10+10 |
| FlexSearch | 31 | - | - | 0.7us | 4.7us | 31 |
| lunr.js | 29 | - | - | 12.4us | 26.9us | 29 |
| MiniSearch | 30 | - | - | 30.7us | 44.3us | 30 |
| fuse.js | 30 | - | - | 307.3us | 396.1us | 30 |

**Sieve shows 10 exact matches in 9.1us** - users see results 10x faster than waiting for all 30.

### Substring Query: `"land"`

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sieve (streaming)** | 18 | 0.5us | **5.2us** | 64.9us | 91.9us | 0+8+10 |
| FlexSearch | 0 | - | - | 0.3us | 0.8us | 0 |
| lunr.js | 0 | - | - | 1.3us | 3.2us | 0 |
| MiniSearch | 30 | - | - | 13.7us | 19.3us | 30 |
| fuse.js | 30 | - | - | 398.2us | 500.2us | 30 |

**No exact match for "land"** (tier 1 returns 0), but **8 prefix matches in 5.2us** from words like "landlocked", "landscape". Tier 3 adds 10 fuzzy matches.

*Note: FlexSearch/lunr.js return 0 results - they don't support substring search. MiniSearch/fuse.js return 30 because their fuzzy matching is over-inclusive (see "Why Result Counts Differ" above).*

### Fuzzy Query: `"mediteranean"` (typo)

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sieve (streaming)** | 8 | 0.4us | 1.0us | **43.1us** | 60.0us | 0+0+8 |
| FlexSearch | 0 | - | - | 0.4us | 1.0us | 0 |
| lunr.js | 0 | - | - | 1.9us | 5.3us | 0 |
| MiniSearch | 8 | - | - | 69.0us | 95.4us | 8 |
| fuse.js | 8 | - | - | 791.5us | 916.2us | 8 |

**All results come from fuzzy tier** (typo doesn't match exactly or as prefix). Sieve's Levenshtein DFA is faster than fuse.js (~17x) and competitive with MiniSearch.

---

## Index Sizes

Serialized index size for European Countries dataset (30 documents, 23KB raw):

| Library | Raw | Gzipped | Notes |
|---------|-----|---------|-------|
| Raw Data | 23.4 KB | 6.5 KB | - |
| **Sieve (.sieve)** | 29.0 KB | 15.6 KB | Binary format |
| fuse.js | 23.4 KB | 6.5 KB | No index |
| FlexSearch | 22.4 KB | 6.8 KB | - |
| MiniSearch | 33.4 KB | 7.8 KB | - |
| lunr.js | 68.4 KB | 12.8 KB | - |

Sieve's binary format is slightly larger than raw data but includes:
- Suffix array for substring search
- Vocabulary (700 terms after stop word filtering)
- Posting lists for fast exact matching
- Document metadata

### Sieve Binary Format Components

| Component | Size | Notes |
|-----------|------|-------|
| Header | 52 bytes | Magic, version, counts |
| Vocabulary | ~500 bytes | Sorted term list |
| Suffix Array | ~8 KB | Delta + varint encoded |
| Postings | ~15 KB | Block PFOR compressed |
| Levenshtein DFA | ~1.2 KB | Precomputed automaton |
| Documents | varies | Metadata for results |
| Dictionary Tables | varies | Parquet-style compression (v7) |
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
| **Sieve (WASM + loader)** | 346 KB | 153 KB |

The Sieve bundle consists of:
- `sieve_bg.wasm`: 329 KB raw, 153 KB gzipped (embedded in .sieve file)
- `sieve-loader.js`: 17 KB raw, 4.5 KB gzipped (self-contained, no dependencies)
- `sieve-loader.js.map`: 40 KB (optional, for debugging)

Sieve's WASM bundle is larger because it includes:
- Suffix array construction and search
- Levenshtein automata (precomputed DFA)
- Block PFOR compression/decompression
- Binary format parsing

---

## Feature Comparison

| Feature | Sieve | FlexSearch | MiniSearch | lunr.js | fuse.js |
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

**Use Sieve when:**
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

# European Countries benchmark (includes Sieve)
bun run bench:eu

# NVIDIA CUTLASS documentation benchmark
bun run crawl:cutlass  # Crawl docs.nvidia.com (outputs to datasets/cutlass/)
sieve build --input datasets/cutlass --output datasets/cutlass
bun run bench:cutlass  # Run benchmarks, outputs to RESULTS-CUTLASS.md

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

**Dataset:** NVIDIA CUTLASS 4.3.4 documentation
- 70 pages covering CUDA, tensor cores, matrix multiplication
- Technical vocabulary: "gemm", "tensor", "warp", "synchronize", "epilogue"
- Mix of API reference, tutorials, and architecture docs

### Substring Search: Query "sync"

Finding all mentions of synchronization-related terms.

| Library | Results | Notes |
|---------|---------|-------|
| **Sieve** | **28** | Finds "synchronize", "async", "__syncthreads", "sync_warp" |
| FlexSearch | 4 | Only exact "sync" token matches |
| MiniSearch | 4 | Only exact "sync" token matches |
| lunr.js | 3 | Only exact "sync" token matches |
| fuse.js | 1 | Fuzzy matching too imprecise |

**Key insight:** In technical documentation, substring search is essential. Users searching "sync" expect to find all synchronization primitives, not just documents with "sync" as a standalone word.

### Typo Tolerance: Query "epilouge"

Common typo for "epilogue" (a CUTLASS concept for post-GEMM operations).

| Library | Results | Notes |
|---------|---------|-------|
| **Sieve** | **12** | Levenshtein distance 1, finds all epilogue docs |
| fuse.js | 0 | Fuzzy threshold too strict for this typo |
| FlexSearch | 0 | No fuzzy support |
| lunr.js | 0 | No fuzzy support |
| MiniSearch | 0 | Fuzzy mode doesn't catch this |

**Key insight:** Technical terms like "epilogue" are easy to misspell. Sieve's Levenshtein automata handle this naturally.

### Time to First Result: Query "tensor"

Common query in GPU documentation.

| Library | Latency | Notes |
|---------|---------|-------|
| FlexSearch | 7 μs | Fastest (inverted index only) |
| MiniSearch | 14 μs | Good balance |
| **Sieve T1** | 18 μs | First results stream immediately |
| lunr.js | 37 μs | Stemming overhead |
| fuse.js | 870 μs | Full document scan |

**Key insight:** Sieve's T1 results arrive in 18μs while T2/T3 compute in background. Users see results immediately.

### Fuzzy Match Latency: Query "syncronize" (typo)

Typo for "synchronize" (missing 'h').

| Library | Latency | Results |
|---------|---------|---------|
| **Sieve** | 52 μs | 15 (all synchronize variants) |
| fuse.js | 415 μs | 15 (8x slower) |
| FlexSearch | 0 μs | 0 (no fuzzy) |
| lunr.js | 0 μs | 0 (no fuzzy) |
| MiniSearch | 0 μs | 0 (fuzzy didn't catch it) |

---

## Related Documentation

- [Architecture](./architecture.md) - How Sieve's index structures work
- [Algorithms](./algorithms.md) - Suffix arrays, Levenshtein automata
- [Integration](./integration.md) - WASM setup, browser integration
- [Verification](./verification.md) - Formal verification approach and limits
- [Contributing](./contributing.md) - How to contribute safely
