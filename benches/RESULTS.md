# Search Library Benchmark Results

Generated: 2026-01-04T22:33:34.184Z
Platform: darwin arm64
Node: v25.2.1

## Index Building Time

Lower is better. Shows time to build search index from scratch.

### Small Blog (~20 posts)

| Library | ops/sec | Mean (ms) |
|---------|---------|-----------|
| fuse.js | 10052 | 99.48 |
| lunr.js | 184 | 5449.524 |
| flexsearch | 1731 | 577.626 |
| minisearch | 469 | 2130.542 |

### Medium Blog (~100 posts)

| Library | ops/sec | Mean (ms) |
|---------|---------|-----------|
| fuse.js | 1021 | 979.736 |
| lunr.js | 20 | 49869.693 |
| flexsearch | 192 | 5214.82 |
| minisearch | 47 | 21241.124 |

## Search Query Performance

Higher ops/sec is better. Measured on medium blog (100 posts).

### Single word (common)
Query: `rust`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1637 | 610692.4 |
| lunr.js | 29719 | 33648.1 |
| flexsearch | 1701920 | 587.6 |
| minisearch | 45944 | 21765.7 |

### Multi-word query
Query: `rust async programming`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 956 | 1046482.9 |
| lunr.js | 13637 | 73327.4 |
| flexsearch | 344063 | 2906.4 |
| minisearch | 21209 | 47149 |

### Word prefix
Query: `perf`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 2706 | 369540.9 |
| lunr.js | 34842 | 28700.9 |
| flexsearch | 3740566 | 267.3 |
| minisearch | 356681 | 2803.6 |

### Substring: "script" in typescript/javascript
Query: `script`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 2007 | 498211.5 |
| lunr.js | 1144388 | 873.8 |
| flexsearch | 3741914 | 267.2 |
| minisearch | 403311 | 2479.5 |

### Substring: "netes" in kubernetes
Query: `netes`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1815 | 551009.8 |
| lunr.js | 901312 | 1109.5 |
| flexsearch | 3768998 | 265.3 |
| minisearch | 367347 | 2722.2 |

### Substring: "chron" in asynchronous
Query: `chron`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 2146 | 465956.9 |
| lunr.js | 1114387 | 897.4 |
| flexsearch | 3790749 | 263.8 |
| minisearch | 405424 | 2466.6 |

### Typo: "ruts" for rust (1 edit)
Query: `ruts`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1627 | 614470.6 |
| lunr.js | 1006762 | 993.3 |
| flexsearch | 3745352 | 267 |
| minisearch | 381522 | 2621.1 |

### Typo: transposition in javascript
Query: `javasrcript`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 2099 | 476432.9 |
| lunr.js | 809151 | 1235.9 |
| flexsearch | 3417263 | 292.6 |
| minisearch | 52586 | 19016.5 |

### Typo: missing letter in typescript
Query: `typscript`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1560 | 641152.7 |
| lunr.js | 892501 | 1120.4 |
| flexsearch | 3410564 | 293.2 |
| minisearch | 49350 | 20263.4 |

## Memory Usage

Index memory consumption in KB.

### Small Blog (~20 posts)

| Library | Index (KB) | Raw Data (KB) |
|---------|------------|---------------|
| fuse.js | 610 | 83 |
| lunr.js | 11365 | 83 |
| flexsearch | 1226 | 83 |
| minisearch | 1187 | 83 |

### Medium Blog (~100 posts)

| Library | Index (KB) | Raw Data (KB) |
|---------|------------|---------------|
| fuse.js | 6391 | 796 |
| lunr.js | -21925 | 796 |
| flexsearch | 8902 | 796 |
| minisearch | 10718 | 796 |
