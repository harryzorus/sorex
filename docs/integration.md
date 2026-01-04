# Integration Guide

This guide covers integrating Sieve into your frontend application. Sieve compiles to WebAssembly, so it runs entirely in the browser—no backend required.

---

## Quick Start

### 1. Build the WASM Module

```bash
# From the sieve directory
wasm-pack build --target web --out-dir pkg --features wasm
```

This produces:
- `pkg/sieve.js` — ES module with WASM loader
- `pkg/sieve_bg.wasm` — The WASM binary (~50KB gzipped)
- `pkg/sieve.d.ts` — TypeScript definitions

### 2. Generate a Search Index

```bash
# From your content
cat docs.json | sieve --binary > index.sieve
```

### 3. Load and Search

```typescript
import init, { SieveSearcher } from './pkg/sieve.js';

// Initialize WASM (do this once)
await init();

// Load your index
const response = await fetch('/search/index.sieve');
const bytes = new Uint8Array(await response.arrayBuffer());
const searcher = new SieveSearcher(bytes);

// Search!
const results = searcher.search('query', 10);
```

---

## Index Formats

Sieve supports two search modes, depending on your needs:

### SieveSearcher (Recommended)

A single `.sieve` binary file containing everything. Best for most use cases.

```typescript
const searcher = new SieveSearcher(bytes);
const results = searcher.search('query');
```

**When to use**: You want simplicity. One file, one load, done.

### SieveProgressiveIndex

Three separate layer files loaded incrementally. Best for very large sites where you want results before the full index loads.

```typescript
const index = new SieveProgressiveIndex(manifest);
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

Matches in higher-weighted fields always rank above lower-weighted fields, regardless of position. This is mathematically proven—see [Verification](./verification.md).

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

### SieveSearcher

```typescript
class SieveSearcher {
  constructor(bytes: Uint8Array);

  search(query: string, limit?: number): SearchResult[];

  has_docs(): boolean;
  has_vocabulary(): boolean;
  vocab_size(): number;
  doc_count(): number;

  free(): void;  // Release WASM memory
}
```

### SieveProgressiveIndex

```typescript
class SieveProgressiveIndex {
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

### Svelte / SvelteKit

```typescript
// src/lib/search/SearchState.svelte.ts
import init, { SieveSearcher } from '$lib/wasm/sieve.js';

class SearchState {
  query = $state('');
  results = $state<SearchResult[]>([]);
  isLoading = $state(true);

  private searcher: SieveSearcher | null = null;

  async init() {
    await init();
    const response = await fetch('/search/index.sieve');
    const bytes = new Uint8Array(await response.arrayBuffer());
    this.searcher = new SieveSearcher(bytes);
    this.isLoading = false;
  }

  search(query: string) {
    if (!this.searcher) return;
    this.query = query;
    this.results = this.searcher.search(query, 10);
  }
}

export const searchState = new SearchState();
```

```svelte
<!-- SearchModal.svelte -->
<script lang="ts">
  import { searchState } from '$lib/search/SearchState.svelte';
  import { onMount } from 'svelte';

  onMount(() => searchState.init());

  function handleInput(e: Event) {
    const query = (e.target as HTMLInputElement).value;
    searchState.search(query);
  }

  function buildResultUrl(result: SearchResult): string {
    return result.sectionId
      ? `${result.href}#${result.sectionId}`
      : result.href;
  }
</script>

<input type="search" oninput={handleInput} />

{#each searchState.results as result}
  <a href={buildResultUrl(result)}>
    <h3>{result.title}</h3>
    <p>{result.excerpt}</p>
  </a>
{/each}
```

### React

```tsx
// useSearch.ts
import { useState, useEffect, useCallback } from 'react';
import init, { SieveSearcher } from './pkg/sieve.js';

export function useSearch() {
  const [searcher, setSearcher] = useState<SieveSearcher | null>(null);
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    async function load() {
      await init();
      const response = await fetch('/search/index.sieve');
      const bytes = new Uint8Array(await response.arrayBuffer());
      setSearcher(new SieveSearcher(bytes));
      setIsLoading(false);
    }
    load();

    return () => searcher?.free();
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
  import init, { SieveSearcher } from './pkg/sieve.js';

  let searcher;

  async function initSearch() {
    await init();
    const response = await fetch('/search/index.sieve');
    const bytes = new Uint8Array(await response.arrayBuffer());
    searcher = new SieveSearcher(bytes);

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
  const index = new SieveProgressiveIndex(manifest);

  // Load titles layer first (~5KB) - enables title-only search
  const titlesBytes = await fetch('/search/titles.sieve')
    .then(r => r.arrayBuffer())
    .then(b => new Uint8Array(b));
  index.load_layer_binary('titles', titlesBytes);

  // User can start searching now with title results
  showSearchUI();

  // Load remaining layers in background
  const [headingsBytes, contentBytes] = await Promise.all([
    fetch('/search/headings.sieve').then(r => r.arrayBuffer()).then(b => new Uint8Array(b)),
    fetch('/search/content.sieve').then(r => r.arrayBuffer()).then(b => new Uint8Array(b))
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
async function streamingSearch(index: SieveProgressiveIndex, query: string) {
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

### 1. Initialize Once

WASM initialization and index loading are expensive. Do them once at app startup, not per search.

```typescript
// Good: Initialize once
const searcher = await initSearch();
document.addEventListener('keydown', (e) => {
  if (e.key === '/') searchModal.open(searcher);
});

// Bad: Initialize per search
searchButton.onclick = async () => {
  const searcher = await initSearch();  // Slow!
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
let searcherPromise = initSearch();

// Use when needed
async function openSearch() {
  const searcher = await searcherPromise;  // Already loaded!
  showModal(searcher);
}
```

---

## Troubleshooting

### "Failed to parse binary" Error

The index file is corrupted or in the wrong format. Regenerate it:

```bash
cat docs.json | sieve --binary > index.sieve
```

### Empty Results

1. Check that field boundaries are correct (start < end)
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

- [Architecture](./architecture.md) — Binary format, algorithm details
- [Algorithms](./algorithms.md) — Suffix arrays, Levenshtein automata
- [Benchmarks](./benchmarks.md) — Performance comparisons with other libraries
- [Verification](./verification.md) — Formal verification guide
- [Contributing](./contributing.md) — How to contribute safely
