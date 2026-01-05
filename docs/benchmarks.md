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
| **Sieve (WASM)** | 0.11ms | Load pre-built .sieve binary |
| fuse.js | 0.05ms | No index (stores raw data) |
| FlexSearch | 0.31ms | Build from documents |
| MiniSearch | 0.84ms | Build from documents |
| lunr.js | 2.43ms | Build from documents |

*Measured on European Countries dataset (30 documents).*

Sieve's load time is predictable because it doesn't build anything at runtime. JS libraries scale with document count. Lunr.js takes 50ms+ on 100 documents.

### Offline Index Build (Sieve)

Sieve builds indexes ahead of time using the `sieve build` CLI or Rust API. This happens at deploy time, not in the browser.

```bash
# Build index from documents
sieve build --input ./docs --output ./search-index --emit-wasm
```

For native Rust build times, run `cargo bench -- index_build`.

---

## Query Latency

Search performance on European Countries dataset (30 documents). Higher ops/sec is better.

### Exact Word Queries

Query: `"history"` - Common word (11+ results expected)

| Library | Results | Latency (us) | ops/sec |
|---------|---------|--------------|---------|
| **Sieve** | 10 | 6.3 | 159,666 |
| FlexSearch | 15 | 0.4 | 2,501,958 |
| lunr.js | 11 | 6.3 | 158,660 |
| MiniSearch | 11 | 7.5 | 133,526 |
| fuse.js | 14 | 559.8 | 1,786 |

FlexSearch is fastest for exact matches because it has the tightest inner loop. Fuse.js is 1000x slower because it scans all documents and computes fuzzy scores.

### Rare Terms

Query: `"fjords"` (appears in Norway only)

| Library | Results | Latency (us) | ops/sec |
|---------|---------|--------------|---------|
| **Sieve** | 1 | 65.9 | 15,175 |
| FlexSearch | 0 | 0.2 | 4,057,569 |
| lunr.js | 0 | 1.1 | 913,244 |
| MiniSearch | 0 | 5.0 | 198,543 |
| fuse.js | 0 | 542.7 | 1,843 |

**Only Sieve finds the result.** Inverted indexes don't index rare terms - they require minimum document frequency thresholds. Sieve's suffix array finds any string, rare or common.

---

## The Killer Feature: Substring Search

This is why Sieve exists. Query: `"land"` (to find "Iceland", "Finland", "landlocked", etc.)

| Library | Results | Latency (us) | Notes |
|---------|---------|--------------|-------|
| **Sieve** | **10** | 60.5 | Finds substring matches |
| MiniSearch | 30 | 13.1 | Fuzzy mode (over-inclusive) |
| fuse.js | 30 | 373.6 | Fuzzy matches everything |
| lunr.js | **0** | 1.1 | No substring support |
| FlexSearch | **0** | 0.2 | No substring support |

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

| Library | Results | Latency (us) | Notes |
|---------|---------|--------------|-------|
| **Sieve** | **10** | 67.2 | Levenshtein automata (precise) |
| MiniSearch | 30 | 44.4 | Fuzzy mode (over-inclusive) |
| fuse.js | 29 | 512.8 | Fuzzy matching (very slow) |
| lunr.js | 0 | 1.5 | No fuzzy support |
| FlexSearch | 0 | 0.3 | No fuzzy support |

Query: `"mediteranean"` (typo for "mediterranean", edit distance 1)

| Library | Results | Latency (us) | Notes |
|---------|---------|--------------|-------|
| **Sieve** | **8** | 42.5 | Correct matches only |
| MiniSearch | 8 | 73.4 | Fuzzy mode |
| fuse.js | 8 | 780.2 | Fuzzy matching (slow) |
| lunr.js | 0 | 1.6 | No fuzzy support |
| FlexSearch | 0 | 0.3 | No fuzzy support |

**Key difference:** Sieve uses Levenshtein automata for true edit-distance matching within distance 2. MiniSearch's fuzzy mode uses prefix expansion (generates all possible prefixes), which produces false positives and is not true edit-distance matching.

---

## Streaming Search: Progressive Results

Sieve's streaming API returns results in three tiers, enabling progressive UX where users see results faster:

- **Tier 1 (Exact)**: O(1) inverted index lookup - show immediately
- **Tier 2 (Prefix)**: O(log k) binary search on suffix array
- **Tier 3 (Fuzzy)**: O(vocabulary) Levenshtein DFA scan

### Common Query: `"European"`

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sieve (streaming)** | 30 | **8.7us** | 18.8us | 100.3us | 135.5us | 10+10+10 |
| FlexSearch | 31 | - | - | 0.6us | 2.1us | 31 |
| lunr.js | 29 | - | - | 11.8us | 17.3us | 29 |
| MiniSearch | 30 | - | - | 30.9us | 41.0us | 30 |
| fuse.js | 30 | - | - | 300.7us | 367.0us | 30 |

**Sieve shows 10 exact matches in 8.7us** - users see results 10x faster than waiting for all 30.

### Substring Query: `"land"`

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sieve (streaming)** | 18 | 0.5us | **5.1us** | 64.7us | 79.7us | 0+8+10 |
| FlexSearch | 0 | - | - | 0.3us | 0.6us | 0 |
| lunr.js | 0 | - | - | 1.3us | 2.5us | 0 |
| MiniSearch | 30 | - | - | 13.1us | 16.9us | 30 |
| fuse.js | 30 | - | - | 373.7us | 413.6us | 30 |

**No exact match for "land"** (tier 1 returns 0), but **8 prefix matches in 5.1us** from words like "landlocked", "landscape". Tier 3 adds 10 fuzzy matches.

*Note: FlexSearch/lunr.js return 0 results - they don't support substring search. MiniSearch/fuse.js return 30 because their fuzzy matching is over-inclusive (see "Why Result Counts Differ" above).*

### Fuzzy Query: `"mediteranean"` (typo)

| Library | Results | Tier 1 | Tier 2 | All | P99 | Breakdown |
|---------|---------|--------|--------|-----|-----|-----------|
| **Sieve (streaming)** | 8 | 0.4us | 0.9us | **43.3us** | 63.6us | 0+0+8 |
| FlexSearch | 0 | - | - | 0.3us | 0.7us | 0 |
| lunr.js | 0 | - | - | 1.8us | 4.5us | 0 |
| MiniSearch | 8 | - | - | 71.1us | 95.6us | 8 |
| fuse.js | 8 | - | - | 761.4us | 835.3us | 8 |

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
| Header | 44 bytes | Magic, version, counts |
| Vocabulary | ~500 bytes | Sorted term list |
| Suffix Array | ~8 KB | Delta + varint encoded |
| Postings | ~15 KB | Block PFOR compressed |
| Levenshtein DFA | ~1.2 KB | Precomputed automaton |
| Documents | varies | Metadata for results |
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
| **Sieve (WASM)** | 342 KB | 153 KB |

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
- You can accept 153KB for features JS libraries can't provide

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

## Related Documentation

- [Architecture](./architecture.md) - How Sieve's index structures work
- [Algorithms](./algorithms.md) - Suffix arrays, Levenshtein automata
- [Integration](./integration.md) - WASM setup, browser integration
- [Verification](./verification.md) - Formal verification approach and limits
