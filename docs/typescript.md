---
title: TypeScript API
description: WASM bindings for browser-based search
order: 20
---

# TypeScript API

This is the complete reference for Sorex's browser API. The surface is intentionally small: one loader function, one searcher class with four methods. Everything else is handled internally.

The key design choice is callback-based progressive search. Instead of awaiting a single result, you receive updates after each search tier completes. This lets you show fast exact matches immediately while slower fuzzy matches continue in the background. The `SearchResult` type includes tier information so you can style results differently by match quality.

For how the loader works internally (streaming compilation, thread pool setup), see [Runtime](runtime.md). For framework patterns and race condition handling, see [Integration](integration.md).

---

## Installation

The WASM module is embedded in `.sorex` files built with `sorex index`. The generated `sorex.js` handles initialization:

```typescript
import { loadSorex } from './sorex.js';

const searcher = await loadSorex('/search/index.sorex');

// Search with callbacks for progressive updates
searcher.search('query', 10, {
  onUpdate: (results) => renderResults(results),  // Called after each tier
  onFinish: (results) => renderResults(results)   // Called when complete
});
```

---

## Public API

The loader exports only what you need:

```typescript
export { loadSorex, type SorexSearcher, type SearchResult };
```

---

## loadSorex

Loads a `.sorex` file and returns a ready-to-use searcher.

```typescript
async function loadSorex(url: string): Promise<SorexSearcher>
```

**Features:**
- Extracts embedded WASM from `.sorex` file
- Compiles WASM during download (streaming compilation)
- Initializes thread pool for parallel search (when available)
- Falls back gracefully to single-threaded mode in Safari

**Example:**

```typescript
const searcher = await loadSorex('/search/index.sorex');
```

---

## SorexSearcher

The main search interface.

### search

Three-tier progressive search: exact -> prefix -> fuzzy.

```typescript
search(
  query: string,
  limit: number,
  callback?: {
    onUpdate?: (results: SearchResult[]) => void,
    onFinish?: (results: SearchResult[]) => void
  }
): void
```

**Parameters:**
- `query` - Search query string
- `limit` - Maximum number of results
- `callback.onUpdate` - Called after each tier completes with accumulated results
- `callback.onFinish` - Called when all tiers complete with final sorted results

**Example:**

```typescript
// Basic usage - just get final results
searcher.search('auto-tuning', 10, {
  onFinish: (results) => console.log(results)
});

// Progressive UI updates
searcher.search('kernel', 10, {
  onUpdate: (results) => {
    // Called after T1, T2, T3 with accumulated results
    renderResults(results);
  },
  onFinish: (results) => {
    // Final sorted results
    renderResults(results);
  }
});
```

### docCount

Returns the number of indexed documents.

```typescript
docCount(): number
```

### vocabSize

Returns the number of unique terms in the vocabulary.

```typescript
vocabSize(): number
```

### free

Releases WASM memory. Call when done with the searcher (important in SPAs).

```typescript
free(): void
```

---

## SearchResult

```typescript
interface SearchResult {
  href: string;              // URL path (e.g., "/posts/2024/01/my-post")
  title: string;             // Document title
  excerpt: string;           // Short description
  sectionId: string | null;  // Section ID for deep linking
  tier: 1 | 2 | 3;           // Match tier (1=exact, 2=prefix, 3=fuzzy)
  matchType: number;         // Match type (0=title, 1=section, 2+=content)
}
```

---

## Complete Example

```typescript
import { loadSorex } from './sorex.js';

class SearchController {
  private searcher: SorexSearcher | null = null;
  private currentSearchId = 0;

  async init(indexUrl: string) {
    this.searcher = await loadSorex(indexUrl);
    console.log(`Loaded ${this.searcher.docCount()} documents`);
  }

  search(query: string) {
    if (!this.searcher || query.length < 2) {
      this.renderResults([]);
      return;
    }

    // Increment search ID to handle race conditions
    const searchId = ++this.currentSearchId;

    this.searcher.search(query, 10, {
      onUpdate: (results) => {
        if (searchId !== this.currentSearchId) return;
        this.renderResults(results);
      },
      onFinish: (results) => {
        if (searchId !== this.currentSearchId) return;
        this.renderResults(results);
      }
    });
  }

  private renderResults(results: SearchResult[]) {
    const html = results.map(r => {
      const url = r.sectionId ? `${r.href}#${r.sectionId}` : r.href;
      return `<a href="${url}"><h3>${r.title}</h3><p>${r.excerpt}</p></a>`;
    }).join('');
    document.getElementById('results')!.innerHTML = html;
  }

  destroy() {
    this.searcher?.free();
    this.searcher = null;
  }
}
```

---

## API Summary

| Method | Description |
|--------|-------------|
| `loadSorex(url)` | Load .sorex file, returns Promise<SorexSearcher> |
| `search(query, limit, callback?)` | Progressive search with callbacks |
| `docCount()` | Number of indexed documents |
| `vocabSize()` | Number of vocabulary terms |
| `free()` | Release WASM memory |

---

## See Also

- [Runtime](runtime.md) - Streaming compilation, threading, progressive search internals
- [Integration](integration.md) - React, Svelte, vanilla JS examples
- [CLI Reference](cli.md) - Building indexes with `sorex index`
- [Troubleshooting](troubleshooting.md) - Common issues
