---
title: CLI Reference
description: Command-line interface for building and inspecting Sieve search indexes
order: 3
---

# CLI Reference

Sieve provides a command-line interface for building search indexes and inspecting `.sieve` files.

## Installation

```bash
cargo install sieve-search
```

## Commands

### `sieve index`

Build a search index from a directory of JSON document files.

```bash
sieve index --input <INPUT_DIR> --output <OUTPUT_DIR> [--demo]
```

**Arguments:**

| Flag | Description |
|------|-------------|
| `-i, --input <DIR>` | Input directory containing `manifest.json` and document files |
| `-o, --output <DIR>` | Output directory for `.sieve` files |
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

- `index-{hash}.sieve` - Binary search index with embedded WASM runtime
- `sieve-loader.js` - JavaScript loader for browser integration
- `sieve-loader.js.map` - Source map for debugging
- `demo.html` (if `--demo` flag) - Integration example

**Example:**

```bash
# Build index from markdown-derived JSON
sieve index --input .build-input --output dist/search

# With demo page
sieve index --input .build-input --output dist/search --demo
```

### `sieve inspect`

Inspect the structure of a `.sieve` file.

```bash
sieve inspect <FILE>
```

**Arguments:**

| Argument | Description |
|----------|-------------|
| `<FILE>` | Path to `.sieve` file to inspect |

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
sieve inspect dist/search/index-868342ec.sieve
```

Output:
```
Sieve Index v7
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

Sieve respects standard Rust environment variables:

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Set logging level (`debug`, `info`, `warn`, `error`) |

## See Also

- [Integration Guide](./integration.md) - How to integrate Sieve into your site
- [TypeScript API](./typescript.md) - Browser WASM bindings reference
- [Rust API](./rust.md) - Library API for programmatic use
- [Architecture](./architecture.md) - How the index format works
- [Benchmarks](./benchmarks.md) - Index build performance
