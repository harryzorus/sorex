# Integration Guide

This guide covers integrating Sorex into your frontend application. Sorex compiles to WebAssembly, so it runs entirely in the browser - no backend required.

---

## Quick Start

### 1. Generate a Search Index

```bash
# Build index from a directory of JSON documents
sorex index --input ./docs --output ./search-output
```

This produces:
- `index-{hash}.sorex` - Self-contained binary with embedded WASM (~153KB gzipped)
- `sorex-loader.js` - JavaScript loader module (17KB)
- `sorex-loader.js.map` - Source map for debugging (optional, 40KB)

### 2. Load and Search

```typescript
import { loadSorex } from './sorex-loader.js';

// Load index (extracts and initializes WASM automatically)
const searcher = await loadSorex('./index-a1b2c3d4.sorex');

// Search!
const results = searcher.search('query', 10);

// Results include section IDs for deep linking
results.forEach(r => {
  const url = r.sectionId ? `${r.href}#${r.sectionId}` : r.href;
  console.log(`${r.title}: ${url}`);
});
```

That's it! The loader handles WASM extraction and initialization automatically.

---

## Index Formats

Sorex supports two search modes, depending on your needs:

### SorexSearcher (Recommended)

A single `.sorex` binary file containing everything. Best for most use cases.

```typescript
const searcher = new SorexSearcher(bytes);
const results = searcher.search('query');
```

**When to use**: You want simplicity. One file, one load, done.

### SorexProgressiveIndex

Three separate layer files loaded incrementally. Best for very large sites where you want results before the full index loads.

```typescript
const index = new SorexProgressiveIndex(manifest);
await index.load_layer_binary('titles', titleBytes);
// User can start searching now, with title-only results

await index.load_layer_binary('headings', headingBytes);
await index.load_layer_binary('content', contentBytes);
// Now all layers are loaded
```

**When to use**: You have >100 documents and want sub-100ms time to first result.

---

## Input Format

### JSON Payload Structure

```json
{
  "docs": [
    {
      "id": 0,
      "title": "Getting Started",
      "excerpt": "Learn how to set up your first project...",
      "href": "/docs/getting-started",
      "type": "doc"
    },
    {
      "id": 1,
      "title": "API Reference",
      "excerpt": "Complete API documentation for all endpoints...",
      "href": "/docs/api",
      "type": "doc"
    }
  ],
  "texts": [
    "Getting Started\n\nLearn how to set up your first project. This guide covers installation, configuration, and your first search query.",
    "API Reference\n\nComplete API documentation for all endpoints. Covers authentication, rate limits, and error handling."
  ],
  "fieldBoundaries": [
    { "docId": 0, "start": 0, "end": 15, "fieldType": "title" },
    { "docId": 0, "start": 17, "end": 150, "fieldType": "content" },
    { "docId": 1, "start": 0, "end": 13, "fieldType": "title" },
    { "docId": 1, "start": 15, "end": 120, "fieldType": "content" }
  ],
  "sectionBoundaries": [
    { "docId": 0, "start": 0, "end": 150, "sectionId": "getting-started" },
    { "docId": 1, "start": 0, "end": 120, "sectionId": "api-reference" }
  ]
}
```

### Field Types

| Field Type | Scoring Weight | Use For |
|------------|----------------|---------|
| `title` | 100.0 | Document titles |
| `heading` | 10.0 | Section headings (h2, h3, etc.) |
| `content` | 1.0 | Body text |

Matches in higher-weighted fields always rank above lower-weighted fields, regardless of position. This is mathematically proven - see [Verification](./verification.md).

### Section IDs for Deep Linking

Section IDs enable linking directly to a heading within a document. When a user clicks a search result, they land at the relevant section, not the top of the page.

```json
{
  "sectionBoundaries": [
    {
      "docId": 0,
      "start": 0,
      "end": 50,
      "sectionId": "introduction"
    },
    {
      "docId": 0,
      "start": 50,
      "end": 150,
      "sectionId": "installation"
    }
  ]
}
```

Results include `sectionId` in the response:

```typescript
const results = searcher.search('install');
// results[0].sectionId === "installation"

// Build the deep link URL
const url = result.sectionId
  ? `${result.href}#${result.sectionId}`
  : result.href;
```

---

## JavaScript API

### SorexSearcher

```typescript
class SorexSearcher {
  constructor(bytes: Uint8Array);

  search(query: string, limit?: number): SearchResult[];

  has_docs(): boolean;
  has_vocabulary(): boolean;
  vocab_size(): number;
  doc_count(): number;

  free(): void;  // Release WASM memory
}
```

### SorexProgressiveIndex

```typescript
class SorexProgressiveIndex {
  constructor(manifest: SearchDoc[]);

  load_layer_binary(name: 'titles' | 'headings' | 'content', bytes: Uint8Array): void;

  has_layer(name: string): boolean;
  loaded_layers(): string[];
  is_fully_loaded(): boolean;
  doc_count(): number;

  search(query: string, options?: SearchOptions): SearchResult[];
  search_exact(query: string, options?: SearchOptions): SearchResult[];
  search_expanded(query: string, excludeIds: number[], options?: SearchOptions): SearchResult[];

  suggest(partial: string, limit?: number): string[];
}
```

### Types

```typescript
interface SearchResult {
  href: string;
  title: string;
  excerpt: string;
  sectionId: string | null;  // For deep linking
}

interface SearchOptions {
  limit?: number;    // Max results (default: 10)
  fuzzy?: boolean;   // Enable fuzzy matching (default: true)
  prefix?: boolean;  // Enable prefix matching (default: true)
  boost?: BoostOptions;
}

interface BoostOptions {
  title?: number;    // Title field weight (default: 100)
  heading?: number;  // Heading field weight (default: 10)
  content?: number;  // Content field weight (default: 1)
}
```

---

## Framework Integration

### React

```tsx
// useSearch.ts
import { useState, useEffect, useCallback } from 'react';
import { loadSorex, SorexSearcher, SearchResult } from './sorex-loader.js';

export function useSearch() {
  const [searcher, setSearcher] = useState<SorexSearcher | null>(null);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    let mounted = true;
    loadSorex('/search/index.sorex').then(s => {
      if (mounted) {
        setSearcher(s);
        setIsLoading(false);
      }
    });
    return () => {
      mounted = false;
      searcher?.free();
    };
  }, []);

  const search = useCallback((query: string) => {
    if (!searcher) return;
    setResults(searcher.search(query, 10));
  }, [searcher]);

  return { results, search, isLoading };
}
```

```tsx
// SearchModal.tsx
import { useSearch } from './useSearch';

function buildResultUrl(result: SearchResult): string {
  return result.sectionId
    ? `${result.href}#${result.sectionId}`
    : result.href;
}

export function SearchModal() {
  const { results, search, isLoading } = useSearch();

  if (isLoading) return <div>Loading search...</div>;

  return (
    <>
      <input
        type="search"
        onChange={(e) => search(e.target.value)}
      />
      {results.map((result) => (
        <a key={result.href} href={buildResultUrl(result)}>
          <h3>{result.title}</h3>
          <p>{result.excerpt}</p>
        </a>
      ))}
    </>
  );
}
```

### Vanilla JavaScript

```html
<script type="module">
  import { loadSorex } from './sorex-loader.js';

  let searcher;

  async function initSearch() {
    searcher = await loadSorex('/search/index.sorex');
    document.getElementById('search-input')
      .addEventListener('input', handleSearch);
  }

  function handleSearch(e) {
    const results = searcher.search(e.target.value, 10);
    renderResults(results);
  }

  function buildResultUrl(result) {
    return result.sectionId
      ? `${result.href}#${result.sectionId}`
      : result.href;
  }

  function renderResults(results) {
    const container = document.getElementById('search-results');
    container.innerHTML = results.map(r => `
      <a href="${buildResultUrl(r)}">
        <h3>${r.title}</h3>
        <p>${r.excerpt}</p>
      </a>
    `).join('');
  }

  initSearch();
</script>

<input type="search" id="search-input" placeholder="Search...">
<div id="search-results"></div>
```

---

## Progressive Loading

For large sites, load index layers incrementally to show results faster:

```typescript
async function initProgressiveSearch() {
  await init();

  // Load manifest (document metadata only)
  const manifest = await fetch('/search/manifest.json').then(r => r.json());
  const index = new SorexProgressiveIndex(manifest);

  // Load titles layer first (~5KB) - enables title-only search
  const titlesBytes = await fetch('/search/titles.sorex')
    .then(r => r.arrayBuffer())
    .then(b => new Uint8Array(b));
  index.load_layer_binary('titles', titlesBytes);

  // User can start searching now with title results
  showSearchUI();

  // Load remaining layers in background
  const [headingsBytes, contentBytes] = await Promise.all([
    fetch('/search/headings.sorex').then(r => r.arrayBuffer()).then(b => new Uint8Array(b)),
    fetch('/search/content.sorex').then(r => r.arrayBuffer()).then(b => new Uint8Array(b))
  ]);

  index.load_layer_binary('headings', headingsBytes);
  index.load_layer_binary('content', contentBytes);

  // Now fully loaded
  return index;
}
```

### Streaming Search API

For even faster perceived performance, use the two-phase streaming API:

```typescript
async function streamingSearch(index: SorexProgressiveIndex, query: string) {
  // Phase 1: Exact matches (O(1), instant)
  const exactResults = index.search_exact(query);
  renderResults(exactResults);  // Show immediately

  // Phase 2: Expanded matches (O(log k), slightly slower)
  const exactIds = exactResults.map(r => r.doc.id);
  const expandedResults = index.search_expanded(query, exactIds);

  // Merge and re-render
  renderResults([...exactResults, ...expandedResults]);
}
```

---

## Suggestions / Autocomplete

Get term suggestions as the user types:

```typescript
const suggestions = index.suggest('auth', 5);
// ["authentication", "authorization", "author", "authenticate", "authored"]
```

Suggestions are sorted by document frequency (most common terms first).

---

## Performance Tips

<aside class="callout callout-success">
<div class="callout-title">Pro Tip</div>

Index loading is the expensive operation. Do it once at app startup, not per search.

</aside>

### 1. Initialize Once

```typescript
// Good: Initialize once
const searcher = await loadSorex('/search/index.sorex');
document.addEventListener('keydown', (e) => {
  if (e.key === '/') searchModal.open(searcher);
});

// Bad: Initialize per search
searchButton.onclick = async () => {
  const searcher = await loadSorex('/search/index.sorex');  // Slow!
  searcher.search(query);
};
```

### 2. Debounce Input

For live search, debounce to avoid excessive WASM calls:

```typescript
let debounceTimer: number;

function handleInput(query: string) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    const results = searcher.search(query, 10);
    renderResults(results);
  }, 100);  // 100ms debounce
}
```

### 3. Limit Results

Fetching more results than you display wastes cycles:

```typescript
// Good: Request only what you need
const results = searcher.search(query, 10);

// Bad: Request everything, slice later
const results = searcher.search(query, 1000).slice(0, 10);
```

### 4. Preload Index

Load the index before the user opens search:

```typescript
// Preload on page load
let searcherPromise = loadSorex('/search/index.sorex');

// Use when needed
async function openSearch() {
  const searcher = await searcherPromise;  // Already loaded!
  showModal(searcher);
}
```

---

## Troubleshooting

<aside class="callout callout-warning">
<div class="callout-title">Important</div>

Always call `searcher.free()` when done with a searcher in SPAs to prevent memory leaks. See the examples below.

</aside>

### "Failed to parse binary" Error

The index file is corrupted or in the wrong format. Regenerate it:

```bash
sorex index --input ./docs --output ./search-output
```

### Empty Results

1. Check that field boundaries are correct (`start < end`)
2. Verify text offsets match the actual text content
3. Ensure section IDs are valid (alphanumeric, hyphens, underscores only)

### Search Returns Wrong Section

Section boundaries must cover the entire document without overlapping:

```json
// Correct: continuous, non-overlapping
[
  { "start": 0, "end": 50, "sectionId": "intro" },
  { "start": 50, "end": 100, "sectionId": "setup" }
]

// Wrong: gap between 50 and 60
[
  { "start": 0, "end": 50, "sectionId": "intro" },
  { "start": 60, "end": 100, "sectionId": "setup" }
]
```

### WASM Memory Leak

Call `free()` when done with a searcher (especially in SPAs):

```typescript
// On component unmount
onUnmount(() => {
  searcher.free();
});
```

---

## Related Documentation

- [CLI Reference](./cli.md): Build indexes with `sorex index`
- [TypeScript API](./typescript.md): Full API reference for WASM bindings
- [Rust API](./rust.md): Library API for programmatic use
- [Architecture](./architecture.md): Binary format, algorithm details
- [Benchmarks](./benchmarks.md): Performance comparisons with other libraries
