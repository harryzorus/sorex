---
title: Troubleshooting
description: Solutions to common Sorex issues
order: 12
---

# Troubleshooting

When search behaves unexpectedly, the cause is usually one of three things: a malformed index file, incorrect field boundaries in the input JSON, or server headers blocking parallel mode. This page organizes solutions by symptom so you can diagnose quickly.

The debugging tools section shows how to use `sorex search` and `sorex inspect` from the command line to isolate whether the problem is in your index, your integration code, or your server configuration. Most issues resolve once you identify which layer is at fault.

For runtime behavior details, see [Runtime](runtime.md). For API specifics, see [TypeScript API](typescript.md).

---

## Common Errors

### "Failed to parse binary" Error

The index file is corrupted or in the wrong format. Regenerate it:

```bash
sorex index --input ./docs --output ./search-output
```

If the error persists, verify your input JSON matches the expected schema. See [Integration](integration.md) for the input format.

### "Failed to fetch" Error

The `.sorex` file path is incorrect or the server isn't serving it:

1. Check the file exists at the specified path
2. Ensure your server serves `.sorex` files with correct MIME type (`application/octet-stream`)
3. Check for CORS issues if loading from a different domain

### WASM Compilation Failed

Usually caused by browser incompatibility or corrupt WASM:

1. Regenerate the index: `sorex index --input ./docs --output ./search-output`
2. Verify browser supports WebAssembly (all modern browsers do)
3. Check browser console for specific error messages

---

## Search Issues

### Empty Results

1. **Check field boundaries are correct** - `start` must be less than `end`
2. **Verify text offsets** - Offsets must match actual character positions in the text
3. **Ensure section IDs are valid** - Only alphanumeric characters, hyphens, and underscores allowed
4. **Check minimum query length** - Queries shorter than 2 characters may be ignored

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

// Wrong: overlapping ranges
[
  { "start": 0, "end": 60, "sectionId": "intro" },
  { "start": 50, "end": 100, "sectionId": "setup" }
]
```

### Results Missing Expected Matches

1. **Check if the term is in the index** - Use `sorex search <file> <query>` to test
2. **Verify tokenization** - Sorex uses word boundaries for tokenization
3. **Check for typos in source content** - The index reflects the source exactly

---

## Memory Issues

### WASM Memory Leak

<aside class="callout callout-warning">
<div class="callout-title">Important</div>

Always call `searcher.free()` when done with a searcher in SPAs to prevent memory leaks.

</aside>

```typescript
// React
useEffect(() => {
  const searcher = await loadSorex('/search/index.sorex');
  return () => searcher.free();  // Cleanup on unmount
}, []);

// Vanilla JS
function cleanup() {
  searcher.free();
}
window.addEventListener('beforeunload', cleanup);
```

### Large Index Performance

For indexes over 1MB:

1. **Preload the index** - Don't wait for user to open search
2. **Use streaming compilation** - The v12 format does this automatically
3. **Consider splitting indexes** - Separate by content category if possible

---

## Parallel Mode Issues

### Search Not Using Parallel Mode

Check that your server sends the required headers:

```
Cross-Origin-Embedder-Policy: require-corp
Cross-Origin-Opener-Policy: same-origin
```

**Nginx example:**
```nginx
add_header Cross-Origin-Embedder-Policy require-corp;
add_header Cross-Origin-Opener-Policy same-origin;
```

**Vite dev server:**
```typescript
// vite.config.ts
export default {
  server: {
    headers: {
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Opener-Policy': 'same-origin'
    }
  }
}
```

### Safari Always Uses Serial Mode

This is expected. Safari has compatibility issues with wasm-bindgen-rayon. Serial mode works correctly - it's just single-threaded.

### Parallel Mode Causes Crashes

If you see crashes only in parallel mode:

1. **Check SharedArrayBuffer support** - Some browsers disable it without COOP/COEP headers
2. **Try serial mode** - Set headers to disable parallel mode as a workaround
3. **Report the issue** - File a bug at [github.com/harryzorus/sorex/issues](https://github.com/harryzorus/sorex/issues)

---

## Build Issues

### "Invalid manifest.json" Error

Ensure your manifest.json has the correct structure:

```json
{
  "version": 1,
  "documents": ["0.json", "1.json", "2.json"]
}
```

### "Document parse error" Warning

The CLI continues despite parse errors. Check the specific file mentioned for JSON syntax issues.

### Index Too Large

1. **Filter stop words** - Common words like "the", "a", "is" increase index size
2. **Limit text content** - Only index searchable content, not full HTML
3. **Check for duplicates** - Duplicate documents inflate the index

---

## Debugging Tools

### CLI Search Test

Test queries directly against your index:

```bash
# Native Rust search
sorex search ./index.sorex "your query"

# WASM search (validates WASM parity)
sorex search ./index.sorex "your query" --wasm
```

### Index Inspection

View index structure and statistics:

```bash
sorex inspect ./index.sorex
```

Output shows document count, vocabulary size, and section table.

---

## Getting Help

If your issue isn't covered here:

1. Check the [Architecture](architecture.md) doc for implementation details
2. Search existing issues on [GitHub](https://github.com/harryzorus/sorex/issues)
3. File a new issue with:
   - Browser and version
   - Index size and document count
   - Error message (full stack trace)
   - Minimal reproduction steps
