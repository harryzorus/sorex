# Search Library Benchmark Results

Generated: 2026-01-05T20:37:47.318Z
Platform: darwin arm64
Node: v24.3.0

## Index Building Time

Lower is better. Shows time to build search index from scratch.

### Small Blog (~20 posts)

| Library | ops/sec | Mean (ms) |
|---------|---------|-----------|
| fuse.js | 7552 | 0.132 |
| lunr.js | 194 | 5.156 |
| flexsearch | 1771 | 0.565 |
| minisearch | 491 | 2.036 |

### Medium Blog (~100 posts)

| Library | ops/sec | Mean (ms) |
|---------|---------|-----------|
| fuse.js | 731 | 1.368 |
| lunr.js | 20 | 50.583 |
| flexsearch | 187 | 5.337 |
| minisearch | 51 | 19.424 |

## Search Query Performance

Higher ops/sec is better. Measured on medium blog (100 posts).

### Single word (common)
Query: `rust`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1404 | 712 |
| lunr.js | 48396 | 20.7 |
| flexsearch | 1763312 | 0.6 |
| minisearch | 68927 | 14.5 |

### Multi-word query
Query: `rust async programming`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 838 | 1192.7 |
| lunr.js | 16098 | 62.1 |
| flexsearch | 450281 | 2.2 |
| minisearch | 30987 | 32.3 |

### Word prefix
Query: `perf`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1787 | 559.5 |
| lunr.js | 44383 | 22.5 |
| flexsearch | 3344713 | 0.3 |
| minisearch | 339919 | 2.9 |

### Substring: "script" in typescript/javascript
Query: `script`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1490 | 671.3 |
| lunr.js | 1003964 | 1 |
| flexsearch | 3265495 | 0.3 |
| minisearch | 399091 | 2.5 |

### Substring: "netes" in kubernetes
Query: `netes`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1384 | 722.8 |
| lunr.js | 951605 | 1.1 |
| flexsearch | 3352694 | 0.3 |
| minisearch | 345411 | 2.9 |

### Substring: "chron" in asynchronous
Query: `chron`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1528 | 654.6 |
| lunr.js | 1024115 | 1 |
| flexsearch | 3357785 | 0.3 |
| minisearch | 385022 | 2.6 |

### Typo: "ruts" for rust (1 edit)
Query: `ruts`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1417 | 705.8 |
| lunr.js | 1059064 | 0.9 |
| flexsearch | 3417264 | 0.3 |
| minisearch | 376043 | 2.7 |

### Typo: transposition in javascript
Query: `javasrcript`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1447 | 691.1 |
| lunr.js | 720122 | 1.4 |
| flexsearch | 3217699 | 0.3 |
| minisearch | 69958 | 14.3 |

### Typo: missing letter in typescript
Query: `typscript`

| Library | ops/sec | Mean (us) |
|---------|---------|-----------|
| fuse.js | 1206 | 829.2 |
| lunr.js | 820772 | 1.2 |
| flexsearch | 3249011 | 0.3 |
| minisearch | 66391 | 15.1 |

## Memory Usage

Index memory consumption in KB.

### Small Blog (~20 posts)

| Library | Index (KB) | Raw Data (KB) |
|---------|------------|---------------|
| fuse.js | 0 | 83 |
| lunr.js | 0 | 83 |
| flexsearch | 0 | 83 |
| minisearch | 0 | 83 |

### Medium Blog (~100 posts)

| Library | Index (KB) | Raw Data (KB) |
|---------|------------|---------------|
| fuse.js | 0 | 796 |
| lunr.js | 0 | 796 |
| flexsearch | 0 | 796 |
| minisearch | 0 | 796 |
