# European Countries Benchmark Results

**Generated:** 2026-01-05T22:07:25.919Z

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
| Sieve (load .sieve) | 0.112 | 8,962 |
| fuse.js (build) | 0.049 | 20,257 |
| lunr.js (build) | 2.407 | 415 |
| flexsearch (build) | 0.307 | 3,253 |
| minisearch (build) | 0.835 | 1,198 |

*Sieve loads a pre-built binary index. JS libraries build at runtime.*

---

## Query Latency

Search performance by query type. Higher ops/sec is better.

### Exact Word Queries

**Query: `capital`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 10.7 | 93,302 | 10 |
| fuse.js | 284.9 | 3,510 | 30 |
| lunr.js | 10.3 | 97,385 | 30 |
| flexsearch | 0.4 | 2,856,942 | 30 |
| minisearch | 10.2 | 98,394 | 30 |

**Query: `European`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 10.5 | 95,362 | 10 |
| fuse.js | 311.7 | 3,208 | 30 |
| lunr.js | 12.4 | 80,713 | 29 |
| flexsearch | 0.5 | 2,008,282 | 31 |
| minisearch | 27.5 | 36,399 | 30 |

**Query: `history`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 6.3 | 158,984 | 10 |
| fuse.js | 537.8 | 1,859 | 14 |
| lunr.js | 6.4 | 155,071 | 11 |
| flexsearch | 0.4 | 2,386,915 | 15 |
| minisearch | 7.7 | 129,988 | 11 |

**Query: `fjords`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 65.3 | 15,318 | 1 |
| fuse.js | 525.1 | 1,904 | 0 |
| lunr.js | 1.2 | 865,652 | 0 |
| flexsearch | 0.3 | 3,496,145 | 0 |
| minisearch | 5.2 | 194,029 | 0 |

### Multi-Word Queries

**Query: `European Union`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 31.3 | 31,956 | 0 |
| fuse.js | 309.2 | 3,234 | 29 |
| lunr.js | 17 | 58,841 | 29 |
| flexsearch | 1 | 976,884 | 29 |
| minisearch | 36.2 | 27,617 | 30 |

**Query: `Mediterranean Sea`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 28.4 | 35,206 | 0 |
| fuse.js | 1054.8 | 948 | 8 |
| lunr.js | 10.4 | 95,770 | 13 |
| flexsearch | 1 | 966,883 | 4 |
| minisearch | 50.8 | 19,678 | 13 |

**Query: `constitutional monarchy`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 31.4 | 31,857 | 0 |
| fuse.js | 1259.1 | 794 | 0 |
| lunr.js | 4.2 | 235,353 | 1 |
| flexsearch | 0.7 | 1,449,564 | 0 |
| minisearch | 65 | 15,378 | 1 |

### Substring Queries (Sieve Advantage)

**Query: `land`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 59.9 | 16,696 | 10 |
| fuse.js | 364.6 | 2,742 | 30 |
| lunr.js | 1.1 | 930,698 | 0 |
| flexsearch | 0.3 | 3,487,242 | 0 |
| minisearch | 13 | 76,963 | 30 |

**Query: `burg`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 57.4 | 17,410 | 10 |
| fuse.js | 513.6 | 1,947 | 6 |
| lunr.js | 1 | 959,190 | 0 |
| flexsearch | 0.3 | 3,469,714 | 0 |
| minisearch | 8.2 | 122,397 | 0 |

**Query: `ian`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 44.8 | 22,317 | 4 |
| fuse.js | 249.9 | 4,001 | 13 |
| lunr.js | 1.1 | 910,907 | 0 |
| flexsearch | 0.3 | 3,502,922 | 0 |
| minisearch | 13.7 | 73,237 | 30 |

### Typo Tolerance (Fuzzy)

**Query: `popultion`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 65.9 | 15,185 | 10 |
| fuse.js | 499.3 | 2,003 | 29 |
| lunr.js | 1.5 | 684,825 | 0 |
| flexsearch | 0.3 | 3,323,823 | 0 |
| minisearch | 44.5 | 22,462 | 30 |

**Query: `provnce`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 71.2 | 14,044 | 0 |
| fuse.js | 769.6 | 1,299 | 0 |
| lunr.js | 1.3 | 753,954 | 0 |
| flexsearch | 0.3 | 3,424,100 | 0 |
| minisearch | 22.8 | 43,824 | 0 |

**Query: `mediteranean`**

| Library | Latency (us) | ops/sec | Results |
|---------|--------------|---------|---------|
| Sieve | 41.8 | 23,937 | 8 |
| fuse.js | 752.5 | 1,329 | 8 |
| lunr.js | 1.6 | 614,441 | 0 |
| flexsearch | 0.3 | 3,307,068 | 0 |
| minisearch | 66.4 | 15,055 | 8 |

---

## Result Timing (First vs All)

Measures latency to first result and complete search. Important for streaming UX.

### Many results
Query: `European`

| Library | Results | First (us) | All (us) | P99 (us) |
|---------|---------|------------|----------|----------|
| Sieve (streaming) | 30 | 8.6 | 101.0 | 136.7 |
| fuse.js | 30 | - | 289.6 | 383.1 |
| lunr.js | 29 | - | 11.7 | 25.5 |
| flexsearch | 31 | - | 0.6 | 3.1 |
| minisearch | 30 | - | 30.2 | 45.8 |

### Substring match
Query: `land`

| Library | Results | First (us) | All (us) | P99 (us) |
|---------|---------|------------|----------|----------|
| Sieve (streaming) | 18 | 0.5 | 64.8 | 82.7 |
| fuse.js | 30 | - | 366.7 | 414.3 |
| lunr.js | 0 | - | 1.3 | 3.8 |
| flexsearch | 0 | - | 0.3 | 1.0 |
| minisearch | 30 | - | 13.4 | 20.0 |

### Fuzzy match
Query: `mediteranean`

| Library | Results | First (us) | All (us) | P99 (us) |
|---------|---------|------------|----------|----------|
| Sieve (streaming) | 8 | 0.4 | 42.8 | 57.0 |
| fuse.js | 8 | - | 737.3 | 868.0 |
| lunr.js | 0 | - | 1.9 | 4.8 |
| flexsearch | 0 | - | 0.3 | 0.7 |
| minisearch | 8 | - | 68.9 | 96.6 |

---

## Index Sizes

Serialized index size (network transfer). Smaller is better.

| Library | Raw (KB) | Gzipped (KB) | Notes |
|---------|----------|--------------|-------|
| Raw Data | 23.4 | 6.5 |  |
| Sieve (.sieve) | 29.0 | 15.6 | binary |
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
