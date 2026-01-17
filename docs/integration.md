---
title: Integration
description: Framework examples for React, Svelte, and vanilla JavaScript
order: 11
---

# Integration

This page covers the practical details of wiring Sorex into a real application. You will learn the input JSON format, how to structure field boundaries for proper ranking, and how section IDs enable deep linking to specific headings. The examples show React and vanilla JavaScript patterns, including race condition handling for live search.

Most integration issues come from misaligned field boundaries or forgetting to call `free()` in single-page apps. The performance tips section addresses the common mistake of initializing the searcher per query instead of once at startup.

For the minimal "just make it work" path, see [Quick Start](quickstart.md). For the full API surface, see [TypeScript API](typescript.md).

---

## Input Format

### JSON Payload Structure

The `sorex index` command reads per-document JSON files:

```json
{
  "id": 0,
  "slug": "getting-started",
  "title": "Getting Started",
  "excerpt": "Learn how to set up your first project...",
  "href": "/docs/getting-started",
  "type": "doc",
  "text": "Getting Started\n\nLearn how to set up your first project...",
  "fieldBoundaries": [
    { "start": 0, "end": 15, "fieldType": "title", "sectionId": null },
    { "start": 17, "end": 150, "fieldType": "content", "sectionId": "intro" }
  ]
}
```

### Field Types

| Field Type | Scoring Weight | Use For |
|------------|----------------|---------|
| `title` | 100.0 | Document titles |
| `heading` | 10.0 | Section headings (h2, h3, etc.) |
| `content` | 1.0 | Body text |

Matches in higher-weighted fields always rank above lower-weighted fields, regardless of position. This is mathematically proven. See [Verification](verification.md).

### Section IDs for Deep Linking

Section IDs enable linking directly to a heading within a document:

```json
{
  "fieldBoundaries": [
    { "start": 0, "end": 50, "fieldType": "heading", "sectionId": "introduction" },
    { "start": 50, "end": 150, "fieldType": "content", "sectionId": "introduction" },
    { "start": 150, "end": 200, "fieldType": "heading", "sectionId": "installation" },
    { "start": 200, "end": 350, "fieldType": "content", "sectionId": "installation" }
  ]
}
```

Results include `sectionId` in the response:

```typescript
searcher.search('install', 10, {
  onFinish: (results) => {
    // results[0].sectionId === "installation"
    const url = results[0].sectionId
      ? `${results[0].href}#${results[0].sectionId}`
      : results[0].href;
  }
});
```

---

## Framework Examples

### React

#### `useSearch.ts`

```typescript
import { useState, useEffect, useCallback, useRef } from 'react';
import { loadSorex, type SorexSearcher, type SearchResult } from './sorex.js';

export function useSearch() {
  const [results, setResults] = useState<SearchResult[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const searcherRef = useRef<SorexSearcher | null>(null);
  const searchIdRef = useRef(0);

  useEffect(() => {
    let mounted = true;
    loadSorex('/search/index.sorex').then(s => {
      if (mounted) {
        searcherRef.current = s;
        setIsLoading(false);
      }
    });
    return () => {
      mounted = false;
      searcherRef.current?.free();
    };
  }, []);

  const search = useCallback((query: string) => {
    if (!searcherRef.current || query.length < 2) {
      setResults([]);
      return;
    }

    // Handle race conditions with search ID
    const currentSearchId = ++searchIdRef.current;

    searcherRef.current.search(query, 10, {
      onUpdate: (r) => {
        if (currentSearchId === searchIdRef.current) setResults(r);
      },
      onFinish: (r) => {
        if (currentSearchId === searchIdRef.current) setResults(r);
      }
    });
  }, []);

  return { results, search, isLoading };
}
```

#### `SearchModal.tsx`

```typescript
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
  import { loadSorex } from './sorex.js';

  let searcher;
  let currentSearchId = 0;

  async function initSearch() {
    searcher = await loadSorex('/search/index.sorex');
    document.getElementById('search-input')
      .addEventListener('input', handleSearch);
  }

  function handleSearch(e) {
    const query = e.target.value;
    if (query.length < 2) {
      renderResults([]);
      return;
    }

    const searchId = ++currentSearchId;

    searcher.search(query, 10, {
      onUpdate: (results) => {
        if (searchId === currentSearchId) renderResults(results);
      },
      onFinish: (results) => {
        if (searchId === currentSearchId) renderResults(results);
      }
    });
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
  searcher.search(query, 10, { onFinish: render });
};
```

### 2. Debounce Input

For live search, debounce to avoid excessive calls:

```typescript
let debounceTimer: number;

function handleInput(query: string) {
  clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    searcher.search(query, 10, { onFinish: renderResults });
  }, 100);  // 100ms debounce
}
```

### 3. Limit Results

Fetching more results than you display wastes cycles:

```typescript
// Good: Request only what you need
searcher.search(query, 10, { onFinish: render });

// Bad: Request everything
searcher.search(query, 1000, { onFinish: (r) => render(r.slice(0, 10)) });
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

## Related Documentation

- [Quick Start](quickstart.md) - Get running in 5 minutes
- [TypeScript API](typescript.md) - Full API reference
- [Runtime](runtime.md) - Threading and streaming internals
- [Troubleshooting](troubleshooting.md) - Common issues
- [CLI Reference](cli.md) - Building indexes
