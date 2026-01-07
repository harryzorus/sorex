---
title: TypeScript API
description: WASM bindings for browser-based search with SieveSearcher and SieveProgressiveIndex
order: 4
---

# TypeScript API

Sieve provides WebAssembly bindings for browser-based search. Two classes are exported:

- **`SieveSearcher`**: Load a `.sieve` file and search immediately (recommended)
- **`SieveProgressiveIndex`**: Progressive layer loading for faster initial results

## Installation

The WASM module is embedded in `.sieve` files built with `sieve index`. The generated `sieve-loader.js` handles initialization:

```typescript
import { loadSieve } from './sieve-loader.js';

const searcher = await loadSieve('/search/index-868342ec.sieve');
const results = searcher.search('query');
```

## SieveSearcher

The main search interface. Load a `.sieve` file and search immediately.

### Constructor

```typescript
new SieveSearcher(bytes: Uint8Array): SieveSearcher
```

Creates a searcher from binary `.sieve` format. Document metadata is embedded in v5+ files.

### Methods

#### `search(query: string, limit?: number): SearchResult[]`

Three-tier search: exact → prefix → fuzzy.

```typescript
const results = searcher.search('auto-tuning', 10);
```

**Returns:** Array of `SearchResult` objects:

```typescript
interface SearchResult {
  href: string;      // URL path (e.g., "/posts/2024/01/my-post")
  title: string;     // Document title
  excerpt: string;   // Short description
  sectionId: string | null;  // Section ID for deep linking (e.g., "introduction")
  tier: 1 | 2 | 3;   // Match tier (1=exact, 2=prefix, 3=fuzzy)
  score: number;     // Ranking score (higher = better)
}
```

**Multi-token queries:**
- `"Auto-tuning"` tokenizes to `["auto", "tuning"]`
- Documents matching all tokens rank higher (2x bonus)
- Scores sum across matching tokens

#### `search_tier1_exact(query: string, limit?: number): SearchResult[]`

Exact word matches only (O(1) inverted index lookup). Use for streaming search first phase.

```typescript
const exactResults = searcher.search_tier1_exact('rust', 10);
```

#### `search_tier2_prefix(query: string, excludeIds: number[], limit?: number): SearchResult[]`

Prefix matches only (O(log k) binary search). Pass doc IDs from tier 1 to avoid duplicates.

```typescript
const tier1Ids = exactResults.map(r => r.docId);
const prefixResults = searcher.search_tier2_prefix('rust', tier1Ids, 10);
```

#### `search_tier3_fuzzy(query: string, excludeIds: number[], limit?: number): SearchResult[]`

Fuzzy matches only (O(vocabulary) via Levenshtein DFA). Pass doc IDs from tiers 1+2.

```typescript
const allIds = [...tier1Ids, ...prefixResults.map(r => r.docId)];
const fuzzyResults = searcher.search_tier3_fuzzy('rust', allIds, 10);
```

#### `doc_count(): number`

Returns the number of indexed documents.

#### `vocab_size(): number`

Returns the number of unique terms in the vocabulary.

#### `has_docs(): boolean`

Returns `true` if document metadata is loaded.

#### `has_vocabulary(): boolean`

Returns `true` if vocabulary is available for fuzzy search.

#### `free(): void`

Releases WASM memory. Also available via `Symbol.dispose` for `using` syntax.

## SieveProgressiveIndex

Progressive layer loading for faster initial results. Load titles first (~5KB), then headings (~20KB), then content (~200KB).

### Constructor

```typescript
new SieveProgressiveIndex(manifest: DocManifest[]): SieveProgressiveIndex
```

Creates an index with document metadata only. Layers must be loaded separately.

### Methods

#### `load_layer_binary(layerName: string, bytes: Uint8Array): void`

Load a search layer from binary format. Valid layer names: `"titles"`, `"headings"`, `"content"`.

```typescript
const titlesBytes = await fetch('/search/titles.bin').then(r => r.arrayBuffer());
index.load_layer_binary('titles', new Uint8Array(titlesBytes));
```

#### `search(query: string, options?: SearchOptions): SearchResult[]`

Search across all loaded layers.

```typescript
interface SearchOptions {
  limit?: number;     // Max results (default: 10)
  fuzzy?: boolean;    // Enable fuzzy matching (default: true)
  prefix?: boolean;   // Enable prefix matching (default: true)
  boost?: {           // Custom field boosts
    title?: number;   // Default: 100
    heading?: number; // Default: 10
    content?: number; // Default: 1
  };
}

const results = index.search('query', { limit: 5, boost: { title: 200 } });
```

#### `search_exact(query: string, options?: SearchOptions): SearchResult[]`

Exact matches only via inverted index (O(1)). Use for streaming search first phase.

#### `search_expanded(query: string, excludeIds: number[], options?: SearchOptions): SearchResult[]`

Prefix/fuzzy matches via suffix array (O(log k)). Pass doc IDs from `search_exact()`.

#### `suggest(partial: string, limit?: number): string[]`

Get autocomplete suggestions for a partial query.

```typescript
const suggestions = index.suggest('aut', 5);
// Returns: ["auto", "automatic", "automate", ...]
```

#### `has_layer(layerName: string): boolean`

Check if a specific layer is loaded.

#### `loaded_layers(): string[]`

Get list of loaded layer names.

#### `is_fully_loaded(): boolean`

Returns `true` if all three layers are loaded.

#### `doc_count(): number`

Returns the number of indexed documents.

## Streaming Search Pattern

For progressive UX, show exact matches immediately, then expand:

```typescript
async function* streamingSearch(searcher: SieveSearcher, query: string) {
  // Phase 1: Exact matches (instant)
  const exact = searcher.search_tier1_exact(query, 10);
  if (exact.length > 0) yield { tier: 1, results: exact };

  // Phase 2: Prefix matches
  const exactIds = exact.map(r => r.docId);
  const prefix = searcher.search_tier2_prefix(query, exactIds, 10);
  if (prefix.length > 0) yield { tier: 2, results: prefix };

  // Phase 3: Fuzzy matches
  const allIds = [...exactIds, ...prefix.map(r => r.docId)];
  const fuzzy = searcher.search_tier3_fuzzy(query, allIds, 10);
  if (fuzzy.length > 0) yield { tier: 3, results: fuzzy };
}
```

## See Also

- [Integration Guide](./integration.md) - Full integration examples with React, Svelte, vanilla JS
- [CLI Reference](./cli.md) - Building indexes with `sieve index`
- [Rust API](./rust.md) - Library API for building indexes programmatically
- [Architecture](./architecture.md) - Index format internals
- [Algorithms](./algorithms.md) - How three-tier search works
