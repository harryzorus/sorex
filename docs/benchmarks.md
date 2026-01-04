# Benchmarks

Performance comparisons against popular JavaScript search libraries. All benchmarks run on a 100-post blog corpus (~100K words) using Tinybench (JavaScript) with 5-second measurement windows.

**Test environment:** Apple M1, macOS, Node.js 25.x

---

## The Tradeoff

Sieve makes a deliberate tradeoff: **capability for bundle size**. Suffix arrays enable true substring and fuzzy search that inverted indexes cannot do. The cost is a larger WASM bundle.

| Library | Substring Search | Typo Tolerance | Bundle (gzip) |
|---------|------------------|----------------|---------------|
| **Sieve (WASM)** | Yes | Yes (Levenshtein) | 153 KB |
| FlexSearch | No | No | 6.6 KB |
| Fuse.js | Via fuzzy only | Yes (slow) | 15.3 KB |
| MiniSearch | No | Partial | 17.9 KB |
| Lunr.js | No | No | 24.3 KB |

---

## Index Build Time

Time to build a search index from scratch (100 posts, ~100K words):

| Library | Time | ops/sec |
|---------|------|---------|
| fuse.js | 980ms | 1,021 |
| FlexSearch | 5.2s | 192 |
| MiniSearch | 21.2s | 47 |
| lunr.js | 49.9s | 20 |

Fuse.js "wins" because it doesn't build an index—it stores raw data and scans on every query. This makes queries 1000× slower.

---

## Query Latency

Single word query ("rust") on medium blog:

| Library | Latency | ops/sec | Results |
|---------|---------|---------|---------|
| FlexSearch | 0.6µs | 1,701,920 | 3 |
| MiniSearch | 21.8µs | 45,944 | 70 |
| lunr.js | 33.6µs | 29,719 | 40 |
| fuse.js | 610ms | 1,637 | 24 |

FlexSearch is blazingly fast but finds fewer results (no stemming, exact match only). Fuse.js is 1000× slower because it scans all documents.

### Multi-word Query

Query: "rust async programming"

| Library | Latency | Results |
|---------|---------|---------|
| FlexSearch | 2.9µs | 0 |
| MiniSearch | 47.1µs | 71 |
| lunr.js | 73.3µs | 71 |
| fuse.js | 1,046ms | 2 |

FlexSearch returns zero results for multi-word queries in default config. MiniSearch and lunr.js handle boolean AND well.

---

## The Killer Feature: Substring Search

Query: "script" (to find "typescript", "javascript")

| Library | Results Found | Latency |
|---------|---------------|---------|
| fuse.js | 10 | 498ms |
| lunr.js | 0 | 0.9µs |
| FlexSearch | 0 | 0.3µs |
| MiniSearch | 0 | 2.5µs |

**This is why Sieve exists.** Inverted indexes tokenize by words—they cannot match substrings within words. When users search "script", they expect TypeScript and JavaScript posts. Only fuzzy search (fuse.js) or suffix arrays (Sieve) find them.

### More Substring Tests

| Query | Target | fuse.js | Others |
|-------|--------|---------|--------|
| "netes" | kubernetes | 8 | 0 |
| "chron" | asynchronous | 0 | 0 |
| "base" | database | varies | 0 |

Even fuse.js misses "chron" in "asynchronous"—its fuzzy algorithm struggles with mid-word matches.

---

## Typo Tolerance

Query: "ruts" (to find "rust", edit distance 1)

| Library | Found "rust"? | Results | Latency |
|---------|---------------|---------|---------|
| fuse.js | Yes | 4 | 660ms |
| MiniSearch (fuzzy) | No | 0 | 2.7µs |
| lunr.js | No | 0 | 1.0µs |
| FlexSearch | No | 0 | 0.3µs |

Only fuse.js handles typos by default. MiniSearch has a `fuzzy` option but it uses prefix matching, not edit distance—it won't find "rust" from "ruts".

### More Typo Tests

| Query | Target | fuse.js | MiniSearch (fuzzy) |
|-------|--------|---------|-------------------|
| typscript | typescript | 8 | 39 |
| kubernates | kubernetes | 10 | 39 |
| programing | programming | 7 | 36 |

MiniSearch's fuzzy mode uses prefix expansion, so it finds results but with lower precision (39 results for a typo query suggests many false positives).

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

## Index Sizes

Serialized index size for a medium blog (100 posts, ~100K words):

| Library | Raw | Gzipped | vs Raw Data |
|---------|-----|---------|-------------|
| FlexSearch | 47 KB | 11 KB | 0.1× |
| lunr.js | 161 KB | 12 KB | 0.2× |
| MiniSearch | 96 KB | 23 KB | 0.1× |
| fuse.js | 738 KB | 172 KB | 1.0× (no index) |
| Raw data | 738 KB | 172 KB | — |

Inverted indexes compress well because they deduplicate terms. Fuse.js stores raw data with no compression.

### Sieve Binary Format

Sieve uses a custom binary format (`.sieve`) optimized for fast loading:

| Component | Size | Notes |
|-----------|------|-------|
| Header | 44 bytes | Magic, version, counts |
| Vocabulary (FST) | ~500 bytes | Compressed term dictionary |
| Suffix Array | ~8 KB | Delta + varint encoded |
| Postings | ~15 KB | Block PFOR compressed |
| Levenshtein DFA | ~1.2 KB | Precomputed automaton |
| Documents | varies | Metadata for results |
| Footer | 8 bytes | CRC32 + magic |

Total for medium blog: ~25 KB (vs 172 KB for JSON).

---

## Feature Comparison

| Feature | Sieve | FlexSearch | MiniSearch | lunr.js | fuse.js |
|---------|-------|------------|------------|---------|---------|
| Exact word match | Yes | Yes | Yes | Yes | Yes |
| Prefix search | Yes | Yes | Yes | Yes | Yes |
| Substring search | **Yes** | No | No | No | Partial |
| Typo tolerance | **Yes** | No | Partial | No | Yes |
| Field weighting | **Proven** | Yes | Yes | Yes | Yes |
| Stemming | No | No | Yes | Yes | No |
| Boolean queries | Yes | Partial | Yes | Yes | No |
| Deep linking | **Yes** | No | No | No | No |
| Binary format | **Yes** | No | No | No | No |
| Progressive loading | **Yes** | No | No | No | No |

---

## When to Use What

**Use Sieve when:**
- Users need substring search ("auth" → "authentication")
- Typo tolerance matters ("typscript" → "typescript")
- You need guaranteed field ranking (proven in Lean)
- Deep linking to sections is important
- You can afford 153 KB bundle

**Use FlexSearch when:**
- Speed is everything (1.7M ops/sec)
- Users search exact words only
- Bundle size must be minimal (6.6 KB)

**Use MiniSearch when:**
- You need good balance of features and size
- Stemming is important
- Boolean queries are common

**Use lunr.js when:**
- You need Lucene-style query syntax
- Stemming and stop words matter
- You're familiar with Lucene/Elasticsearch

**Use fuse.js when:**
- You need fuzzy search without preprocessing
- Dataset is small (<1000 items)
- Query latency of 500ms+ is acceptable

---

## Running Benchmarks

```bash
# JavaScript library comparison
cd benches && npm install && node bench-js.mjs

# Size comparison
node bench-sizes.mjs

# Results saved to benches/RESULTS.md
```

---

## Methodology

**Corpus generation:**
- Technical blog vocabulary (rust, kubernetes, async, etc.)
- Random word selection per post
- Realistic title/content structure

**Measurement:**
- 5-second measurement windows
- 1000+ iterations per benchmark
- Warmup runs excluded
- 99% confidence intervals (not shown in tables)

**Environment:**
- Apple M1 Pro (8 cores)
- macOS 14.x
- Node.js 25.x
- Libraries: fuse.js 7.0, lunr.js 2.3, flexsearch 0.7, minisearch 6.3

---

## Related Documentation

- [Architecture](./architecture.md) — How Sieve's index structures work
- [Algorithms](./algorithms.md) — Suffix arrays, Levenshtein automata
- [Integration](./integration.md) — WASM setup, browser integration
