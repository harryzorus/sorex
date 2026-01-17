---
title: CLI Reference
description: Command-line interface for building and inspecting Sorex search indexes
order: 3
---

# CLI Reference

The `sorex` command-line tool handles the build side of the search pipeline. You use it to convert JSON documents into `.sorex` index files, test queries against built indexes, and inspect index structure for debugging.

Three commands cover the workflow: `sorex index` builds the index, `sorex search` tests queries with detailed timing breakdowns, and `sorex inspect` shows what is inside a `.sorex` file. The search command is particularly useful for diagnosing ranking issues. It shows per-tier timings and match types so you can see exactly why a result appears where it does.

The `--wasm` flag on search runs queries through the embedded WASM runtime, letting you verify that browser results will match native results.

## Installation

```bash
cargo install sorex
```

## Commands

### `sorex index`

Build a search index from a directory of JSON document files.

```bash
sorex index --input <INPUT_DIR> --output <OUTPUT_DIR> [--demo]
```

**Arguments:**

| Flag | Description |
|------|-------------|
| `-i, --input <DIR>` | Input directory containing `manifest.json` and document files |
| `-o, --output <DIR>` | Output directory for `.sorex` files |
| `--demo` | Generate a demo HTML page showing integration example |

**Input Format:**

The input directory must contain a `manifest.json` file:

```json
{
  "version": 1,
  "documents": ["0.json", "1.json", "2.json"],
  "indexes": {
    "index": { "include": "*" }
  }
}
```

Each document file (e.g., `0.json`) follows this schema:

```json
{
  "id": 0,
  "slug": "my-post",
  "title": "My Post Title",
  "excerpt": "A short description...",
  "href": "/posts/my-post",
  "type": "post",
  "category": "engineering",
  "text": "Normalized searchable text content...",
  "fieldBoundaries": [
    { "start": 0, "end": 13, "fieldType": "title", "sectionId": null },
    { "start": 14, "end": 100, "fieldType": "heading", "sectionId": "introduction" },
    { "start": 101, "end": 500, "fieldType": "content", "sectionId": "introduction" }
  ]
}
```

**Output:**

- `index.sorex` - Binary search index with embedded WASM runtime
- `sorex.js` - JavaScript loader for browser integration
- `demo.html` (if `--demo` flag) - Integration example

**Example:**

```bash
# Build index from markdown-derived JSON
sorex index --input .build-input --output dist/search

# With demo page
sorex index --input .build-input --output dist/search --demo
```

### `sorex search`

Search a `.sorex` file from the command line for testing and debugging.

```bash
sorex search <FILE> <QUERY> [--limit <N>] [--wasm] [--bench] [--confidence <N>]
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to `.sorex` file |
| `<QUERY>` | Search query |
| `-l, --limit <N>` | Maximum results (default: 10) |
| `--wasm` | Use embedded WASM via Deno runtime instead of native Rust |
| `--bench` | Run statistical benchmark with confidence intervals |
| `--confidence <N>` | Target confidence level for benchmark (default: 95%) |

#### Search Flow

```
                              sorex search <file> <query>
                                         │
                                         ▼
┌──────────────────────────────────────────────────────────────────────┐
│  1. LOAD INDEX                                                       │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Read .sorex file ─▶ Parse header ─▶ Load into memory          │  │
│  │  (~12ms for 400KB index)                                       │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
┌──────────────────────────────────────────────────────────────────────┐
│  2. WARM UP (10 iterations)                                          │
│  ┌────────────────────────────────────────────────────────────────┐  │
│  │  Prime CPU branch predictor and caches for accurate timing     │  │
│  └────────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────────┘
                                         │
                                         ▼
┌──────────────────────────────────────────────────────────────────────┐
│  3. TIMED SEARCH                                                     │
│  ┌─────────────────────┐  ┌─────────────────────┐  ┌──────────────┐  │
│  │  T1 Exact (~2μs)    │─▶│  T2 Prefix (~10μs)  │─▶│ T3 Fuzzy     │  │
│  │  Binary search      │  │  FST range scan     │  │ (~50μs)      │  │
│  │  Direct postings    │  │  Exclude T1 results │  │ Lev DFA      │  │
│  └─────────────────────┘  └─────────────────────┘  └──────────────┘  │
│                                                                      │
│  Each tier timed independently with microsecond precision            │
└──────────────────────────────────────────────────────────────────────┘
```

**Example:**

```bash
sorex search dist/search/index.sorex "kernel optimization"
```

**Output:**

```
╔════════════════════════════════════════════════════════════════════╗
║                           SOREX SEARCH                             ║
╠════════════════════════════════════════════════════════════════════╣
║  File:   dist/search/index.sorex                                   ║
║  Query:  "kernel optimization"                                     ║
║  Limit:  10                                                        ║
╚════════════════════════════════════════════════════════════════════╝

┌─ PERFORMANCE ──────────────────────────────────────────────────────┐
│  Index load:         12.34 ms                                      │
│                                                                    │
│  T1 Exact:            2.15 µs  (3 results)                         │
│  T2 Prefix:           8.42 µs  (5 results)                         │
│  T3 Fuzzy:           45.21 µs  (2 results)                         │
│                                                                    │
│  Search total:       55.78 µs                                      │
│  Total:              12.40 ms                                      │
└────────────────────────────────────────────────────────────────────┘

┌─ RESULTS (10) ─────────────────────────────────────────────────────┐
│  #   TIER   MATCH TYPE     SCORE  TITLE                            │
│  1   T1     Title          100.5  Kernel Optimization Guide        │
│  2   T1     Heading         10.3  Performance Tuning               │
│      └─ #kernel-tuning                                             │
│  ...                                                               │
└────────────────────────────────────────────────────────────────────┘
```

#### WASM Parity Testing

Use `--wasm` to test that the embedded WASM produces identical results to native Rust:

```
                    Testing Native/WASM Parity
                              │
           ┌──────────────────┴──────────────────┐
           │                                     │
           ▼                                     ▼
┌─────────────────────────┐         ┌─────────────────────────┐
│  sorex search <file>    │         │  sorex search <file>    │
│      <query>            │         │      <query> --wasm     │
│                         │         │                         │
│  Uses native Rust       │         │  Uses Deno runtime      │
│  TierSearcher           │         │  Embedded WASM          │
└───────────┬─────────────┘         └───────────┬─────────────┘
            │                                   │
            ▼                                   ▼
┌─────────────────────────┐         ┌─────────────────────────┐
│  PERFORMANCE            │         │  PERFORMANCE            │
│  ──────────────         │         │  ──────────────         │
│  T1:   2.15 µs          │         │  WASM init:  45.00 ms   │
│  T2:   8.42 µs          │   vs    │  WASM search: 62.34 µs  │
│  T3:  45.21 µs          │         │  (includes all tiers)   │
│  ─────────────          │         │                         │
│  Total: 55.78 µs        │         │  (warm, TurboFan opt)   │
└─────────────────────────┘         └─────────────────────────┘
```

**Example:**

```bash
# Native Rust search
sorex search index.sorex "matrix"

# WASM search (requires deno-runtime feature)
sorex search index.sorex "matrix" --wasm
```

The `--wasm` flag requires building with `--features deno-runtime`:

```bash
cargo build --release --features deno-runtime
```

Both should return identical results (same titles, same section IDs, same ranking order). If they differ, there's a bug in the WASM implementation.

#### Common Debugging Workflows

**Test a specific query:**
```bash
sorex search ./dist/index.sorex "authentication" --limit 20
```

**Compare native vs WASM results:**
```bash
# Run both and compare output
sorex search ./dist/index.sorex "login"
sorex search ./dist/index.sorex "login" --wasm
```

**Profile tier performance:**
```bash
# Quick look at T1/T2/T3 breakdown to identify bottlenecks
sorex search ./dist/index.sorex "search"
# If T3 is slow, you may have a large vocabulary

# For statistically rigorous measurements with confidence intervals
sorex search ./dist/index.sorex "search" --bench
```

### `sorex inspect`

Inspect the structure of a `.sorex` file.

```bash
sorex inspect <FILE>
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to `.sorex` file to inspect |

**Output:**

Displays index metadata including:
- Binary format version
- Document count
- Vocabulary size
- Suffix array length
- Embedded WASM size
- CRC32 checksum

**Example:**

```bash
sorex inspect dist/search/index.sorex
```

Output:
```
Sorex Index v12
  Documents: 23
  Vocabulary: 4148 terms
  Suffix Array: 12847 entries
  Postings: 8923 entries
  Section Table: 156 entries
  Levenshtein DFA: 48KB
  Embedded WASM: 152KB
  Total Size: 387KB
  CRC32: 868342ec
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Error (missing files, invalid JSON, etc.) |

## Environment Variables

Sorex respects standard Rust environment variables:

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Set logging level (`debug`, `info`, `warn`, `error`) |

## See Also

- [Integration Guide](./integration.md) - How to integrate Sorex into your site
- [TypeScript API](./typescript.md) - Browser WASM bindings reference
- [Rust API](./rust.md) - Library API for programmatic use
- [Architecture](./architecture.md) - How the index format works
- [Benchmarks](./benchmarks.md) - Index build performance
