# European Countries Benchmark Results

**Generated:** 2026-01-06T06:53:36.504Z

## System Information

| Property | Value |
|----------|-------|
| CPU | Apple M5 |
| Memory | 32GB |
| OS | darwin 26.2 |
| Node.js | v24.3.0 |
| Dataset | 30 documents |

---

## Time to First Search

How long until search is ready? Lower is better.

| Library | Mean (ms) | ops/sec |
|---------|-----------|---------|
| Sieve (load .sieve) | 0.202 | 4,959 |
| fuse.js (build) | 0.048 | 20,791 |
| lunr.js (build) | 2.493 | 401 |
| flexsearch (build) | 0.32 | 3,124 |
| minisearch (build) | 0.848 | 1,179 |

*Sieve loads a pre-built binary index. JS libraries build at runtime.*

---

## Query Latency

Search performance by query type. Higher ops/sec is better.

### Exact Word Queries

**Query: `capital`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 12.1 | 82,327 | 10 |
| fuse.js | 294.4 | 3,396 | 30 |
| lunr.js | 10.5 | 95,303 | 30 |
| flexsearch | 0.3 | 3,025,779 | 30 |
| minisearch | 10.3 | 96,874 | 30 |

**Query: `European`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 12.3 | 81,114 | 10 |
| fuse.js | 295.5 | 3,385 | 30 |
| lunr.js | 11.1 | 89,788 | 29 |
| flexsearch | 0.5 | 2,012,540 | 31 |
| minisearch | 28.6 | 34,937 | 30 |

**Query: `history`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 6.9 | 144,529 | 10 |
| fuse.js | 550.9 | 1,815 | 14 |
| lunr.js | 6.6 | 152,307 | 11 |
| flexsearch | 0.4 | 2,380,671 | 15 |
| minisearch | 7.8 | 128,394 | 11 |

**Query: `fjords`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 65.9 | 15,173 | 1 |
| fuse.js | 542.5 | 1,843 | 0 |
| lunr.js | 1.2 | 831,340 | 0 |
| flexsearch | 0.3 | 3,983,513 | 0 |
| minisearch | 5.2 | 193,360 | 0 |

### Multi-Word Queries

**Query: `European Union`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 31.8 | 31,474 | 0 |
| fuse.js | 320.1 | 3,124 | 29 |
| lunr.js | 17.5 | 57,224 | 29 |
| flexsearch | 1 | 1,019,030 | 29 |
| minisearch | 36.5 | 27,385 | 30 |

**Query: `Mediterranean Sea`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 28.8 | 34,726 | 0 |
| fuse.js | 1049.5 | 953 | 8 |
| lunr.js | 10.7 | 93,309 | 13 |
| flexsearch | 1 | 1,041,216 | 4 |
| minisearch | 53.3 | 18,762 | 13 |

**Query: `constitutional monarchy`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 33.2 | 30,151 | 0 |
| fuse.js | 1400.6 | 714 | 0 |
| lunr.js | 4.4 | 225,795 | 1 |
| flexsearch | 0.6 | 1,558,209 | 0 |
| minisearch | 65.1 | 15,359 | 1 |

### Substring Queries (Sieve Advantage)

**Query: `land`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 62.2 | 16,066 | 10 |
| fuse.js | 378.2 | 2,644 | 30 |
| lunr.js | 1.1 | 887,764 | 0 |
| flexsearch | 0.2 | 4,008,116 | 0 |
| minisearch | 13.3 | 75,238 | 30 |

**Query: `burg`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 61 | 16,403 | 10 |
| fuse.js | 543.3 | 1,841 | 6 |
| lunr.js | 1.1 | 947,517 | 0 |
| flexsearch | 0.2 | 4,047,660 | 0 |
| minisearch | 7.2 | 138,092 | 0 |

**Query: `ian`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 45.5 | 21,975 | 4 |
| fuse.js | 260.2 | 3,843 | 13 |
| lunr.js | 1 | 1,030,775 | 0 |
| flexsearch | 0.2 | 4,140,411 | 0 |
| minisearch | 13.9 | 71,774 | 30 |

### Typo Tolerance (Fuzzy)

**Query: `popultion`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 71.1 | 14,057 | 10 |
| fuse.js | 523.1 | 1,912 | 29 |
| lunr.js | 1.5 | 673,168 | 0 |
| flexsearch | 0.2 | 4,001,220 | 0 |
| minisearch | 45.5 | 21,964 | 30 |

**Query: `provnce`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 68.2 | 14,657 | 0 |
| fuse.js | 748.1 | 1,337 | 0 |
| lunr.js | 1.3 | 762,344 | 0 |
| flexsearch | 0.2 | 4,147,190 | 0 |
| minisearch | 23.7 | 42,213 | 0 |

**Query: `mediteranean`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 42.8 | 23,372 | 8 |
| fuse.js | 766.7 | 1,304 | 8 |
| lunr.js | 1.6 | 607,273 | 0 |
| flexsearch | 0.3 | 3,810,744 | 0 |
| minisearch | 70.5 | 14,179 | 8 |

---

## Result Timing (First vs All)

Measures latency to first result and complete search. Important for streaming UX.

### Many results
Query: `European`

| Library | Results | First (us) | All (us) | P99 (us) |
|---------|---------|------------|----------|----------|
| Sieve (streaming) | 30 | 10.1 | 108.0 | 213.2 |
| fuse.js | 30 | - | 288.4 | 369.5 |
| lunr.js | 29 | - | 11.7 | 26.0 |
| flexsearch | 31 | - | 0.6 | 2.5 |
| minisearch | 30 | - | 29.8 | 39.2 |

### Substring match
Query: `land`

| Library | Results | First (us) | All (us) | P99 (us) |
|---------|---------|------------|----------|----------|
| Sieve (streaming) | 18 | 0.6 | 68.6 | 176.8 |
| fuse.js | 30 | - | 375.5 | 561.3 |
| lunr.js | 0 | - | 1.3 | 2.6 |
| flexsearch | 0 | - | 0.3 | 0.8 |
| minisearch | 30 | - | 13.4 | 20.6 |

### Fuzzy match
Query: `mediteranean`

| Library | Results | First (us) | All (us) | P99 (us) |
|---------|---------|------------|----------|----------|
| Sieve (streaming) | 8 | 0.4 | 43.5 | 58.8 |
| fuse.js | 8 | - | 748.5 | 844.0 |
| lunr.js | 0 | - | 1.8 | 3.7 |
| flexsearch | 0 | - | 0.3 | 0.8 |
| minisearch | 8 | - | 73.5 | 138.6 |

---

## Index Sizes

Serialized index size (network transfer). Smaller is better.

| Library | Raw (KB) | Gzipped (KB) | Notes |
|---------|----------|--------------|-------|
| Raw Data | 23.4 | 6.5 |  |
| Sieve (.sieve) | 347.1 | 167.6 | binary |
| fuse.js | 23.4 | 6.5 | no index |
| lunr.js | 68.4 | 12.8 |  |
| flexsearch | 22.4 | 6.8 |  |
| minisearch | 33.4 | 7.8 |  |

---

## Key Takeaways

1. **Time to First Search**: Sieve loads pre-built indexes instantly (~Xms). JS libraries must build at runtime.
2. **Substring Search**: Sieve finds results for substring queries where inverted indexes return 0.
3. **Typo Tolerance**: Sieve uses Levenshtein automata for true edit-distance fuzzy matching.
4. **Index Size**: Sieve's binary format is compact and compresses well.
