---
title: Quick Start
description: Get Sorex search running in 5 minutes
order: 10
---

# Quick Start

This guide gets you from zero to working search in under five minutes. You will install the CLI, prepare your documents as JSON, build an index, and load it in the browser. The result is a self-contained `.sorex` file with embedded WASM that handles initialization automatically.

---

## 1. Install the CLI

```bash
cargo install sorex
```

## 2. Prepare Your Documents

Create a directory with your content as JSON files:

```
docs/
├── manifest.json
├── 0.json
└── 1.json
```

**manifest.json:**
```json
{
  "version": 1,
  "documents": ["0.json", "1.json"]
}
```

**0.json (example document):**
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
    { "start": 0, "end": 15, "fieldType": "title", "sectionId": null }
  ]
}
```

## 3. Build the Index

```bash
sorex index --input ./docs --output ./search-output
```

This produces:
- `index.sorex` - Self-contained binary with embedded WASM (~153KB gzipped)
- `sorex.js` - JavaScript loader module (17KB)

## 4. Load and Search

```typescript
import { loadSorex } from './sorex.js';

// Load index (handles WASM extraction and threading automatically)
const searcher = await loadSorex('./index.sorex');

// Search with callbacks for progressive updates
searcher.search('query', 10, {
  onUpdate: (results) => renderResults(results),  // Called after each tier
  onFinish: (results) => renderResults(results)   // Called when complete
});
```

That's it! The loader handles WASM extraction, threading detection, and initialization automatically. 

For what happens under the hood during loading, see [Runtime](runtime.md).

For framework-specific patterns (React hooks, race condition handling), see [Integration](integration.md). 

---

## Next Steps

- [Integration](integration.md) - React, vanilla JS examples
- [CLI Reference](cli.md) - Build options and debugging
- [TypeScript API](typescript.md) - Full API reference
- [Troubleshooting](troubleshooting.md) - Common issues
