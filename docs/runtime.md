---
title: Browser Runtime
description: Streaming compilation, threading, and progressive search
order: 30
---

# Browser Runtime

How Sorex executes in the browser: streaming WASM compilation, parallel threading with Web Workers, and progressive three-tier search. For API usage, see [TypeScript API](typescript.md). For file format details, see [Binary Format](binary-format.md).

---

## Streaming Loading

The v12 `.sorex` format places WASM at the front of the file, enabling **parallel download and compilation**:

```
┌──────────────────────────────────────────────────────────────────────┐
│                      Streaming Compilation Flow                      │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  TIME ──────────────────────────────────────────────────────────────►│
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │ NETWORK: Header │ WASM ────────────────│ Index data ─────────│    │
│  └──────────────────────────────────────────────────────────────┘    │
│                    │                       │                         │
│                    ▼                       │                         │
│  ┌─────────────────────────────────────┐   │                         │
│  │ COMPILE: WebAssembly.compile()      │   │                         │
│  │ (runs in parallel with download)    │   │                         │
│  └─────────────────────────────────────┘   │                         │
│                                            │                         │
│                                            ▼                         │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │ PARSE: Vocabulary, postings, suffix array                    │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  ┌──────────────────────────────────────────────────────────────┐    │
│  │ READY: SorexSearcher instantiated                            │    │
│  └──────────────────────────────────────────────────────────────┘    │
│                                                                      │
│  Traditional: Download ────────────────────► Compile ──► Parse       │
│  v12 Sorex:   Download + Compile ─────────────────────► Parse        │
│                      ↑ parallel ↑                                    │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

**Why this matters:**
- WASM compilation is CPU-intensive (~50ms for 150KB)
- Index download is network-bound (~100ms on 3G)
- By overlapping these, total load time approaches `max(compile, download)` instead of `compile + download`

The loader handles this automatically via `WebAssembly.compileStreaming()` when available.

---

## Threading Detection

The loader automatically detects browser capabilities and chooses the best execution mode:

```
                    loadSorex(url) Threading Detection
                              │
       SharedArrayBuffer available?
              │
       ┌──────┴──────┐
       │             │
      YES           NO ─────────────────────────────┐
       │                                            │
       ▼                                            │
  COOP/COEP headers?                                │
       │                                            │
  ┌────┴────┐                                       │
  │         │                                       │
 YES       NO ──────────────────────────────────────┤
  │                                                 │
  ▼                                                 │
  Safari?                                           │
  │                                                 │
  ├── YES ──────────────────────────────────────────┤
  │                                                 │
  └── NO                                            ▼
       │                              ┌─────────────────────────┐
       ▼                              │     SERIAL MODE         │
  ┌─────────────────────────┐         │  Single-threaded        │
  │    PARALLEL MODE        │         │  Same API, same results │
  │  Web Workers + Rayon    │         └─────────────────────────┘
  └─────────────────────────┘
```

**You don't need to manage this.** The loader detects capabilities and chooses the best mode. Your code works the same either way:

```typescript
// Works in all browsers - parallel when available, single-threaded otherwise
const searcher = await loadSorex('/search/index.sorex');
searcher.search('query', 10, { onFinish: renderResults });
```

---

## Threading Architecture

### Parallel Mode (Chrome/Firefox)

```
┌──────────────────────────────────────────────────────────────────────┐
│                    PARALLEL MODE (Chrome/Firefox)                    │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐                                                │
│  │   Main Thread    │                                                │
│  │                  │──search()──▶ ┌────────────────────────────┐    │
│  │  (UI responsive) │              │    SharedArrayBuffer       │    │
│  └──────────────────┘              │                            │    │
│          ▲                         │  ┌──────────────────────┐  │    │
│          │                         │  │ Index Data (readonly)│  │    │
│     callbacks                      │  └──────────────────────┘  │    │
│          │                         │  ┌──────────────────────┐  │    │
│          │                         │  │ Result Buffer        │  │    │
│  ┌───────┴────────┐                │  └──────────────────────┘  │    │
│  │   Worker 1     │◀───────────────┤                            │    │
│  ├────────────────┤                │                            │    │
│  │   Worker 2     │◀───────────────┤                            │    │
│  ├────────────────┤                │                            │    │
│  │   Worker 3     │◀───────────────┤                            │    │
│  ├────────────────┤                │                            │    │
│  │   Worker 4     │◀───────────────┘                            │    │
│  └────────────────┘                └────────────────────────────┘    │
│                                                                      │
│  wasm-bindgen-rayon: Parallel suffix array search                    │
└──────────────────────────────────────────────────────────────────────┘
```

**Benefits:**
- Main thread stays responsive (60fps UI)
- Suffix array search parallelized across cores
- Results computed faster on multi-core devices

### Serial Mode (Safari/Fallback)

```
┌──────────────────────────────────────────────────────────────────────┐
│                     SERIAL MODE (Safari / Fallback)                  │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────────┐                                                │
│  │   Main Thread    │                                                │
│  │                  │                                                │
│  │  search() ──▶ process() ──▶ callbacks                             │
│  │                  │                                                │
│  │  All work here   │                                                │
│  └──────────────────┘                                                │
│                                                                      │
│  Same API, same results, sequential execution                        │
│  Graceful fallback - no code changes needed                          │
└──────────────────────────────────────────────────────────────────────┘
```

**Note:** Serial mode is still fast (~200μs for typical queries). The difference is only noticeable for very large indexes or complex fuzzy searches.

---

## Browser Support Matrix

```
┌────────────────────┬─────────────────────┬──────────────────────────┐
│     Browser        │        Mode         │         Notes            │
├────────────────────┼─────────────────────┼──────────────────────────┤
│ Chrome 89+         │ ✅ Parallel         │ Full threading support   │
├────────────────────┼─────────────────────┼──────────────────────────┤
│ Firefox 79+        │ ✅ Parallel         │ Full threading support   │
├────────────────────┼─────────────────────┼──────────────────────────┤
│ Safari (all)       │ ⚠️  Serial          │ wasm-bindgen-rayon issue │
├────────────────────┼─────────────────────┼──────────────────────────┤
│ No COOP/COEP       │ ⚠️  Serial          │ SharedArrayBuffer blocked│
├────────────────────┼─────────────────────┼──────────────────────────┤
│ Edge 89+           │ ✅ Parallel         │ Chromium-based           │
└────────────────────┴─────────────────────┴──────────────────────────┘
```

### Requirements for Parallel Mode

1. **Cross-origin isolation headers:**
   ```
   Cross-Origin-Embedder-Policy: require-corp
   Cross-Origin-Opener-Policy: same-origin
   ```

2. **SharedArrayBuffer support** (all modern browsers except when headers missing)

3. **Not Safari** (known compatibility issues with wasm-bindgen-rayon)

See [Troubleshooting](troubleshooting.md) for server configuration examples.

---

## Progressive Search

Search returns results progressively as each tier completes, enabling immediate UI updates:

```
                          search(query, limit, callback)
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────┐
│  TIER 1: EXACT MATCH                                         ~5μs    │
│  ────────────────────────────────────────────────────────────────    │
│                                                                      │
│  Binary search in vocabulary ─▶ Direct postings lookup               │
│                                                                      │
│  "tensor" ─▶ vocab["tensor"] ─▶ [doc:3, doc:17, doc:42]              │
│                    │                                                 │
│                    └─▶ callback.onUpdate([3 results])                │
└──────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────┐
│  TIER 2: PREFIX MATCH                                       ~10μs    │
│  ────────────────────────────────────────────────────────────────    │
│                                                                      │
│  FST range scan ─▶ All terms starting with query                     │
│                                                                      │
│  "tens" ─▶ ["tensor", "tensors", "tension", "tense"]                 │
│                    │                                                 │
│                    └─▶ callback.onUpdate([12 results])               │
└──────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌──────────────────────────────────────────────────────────────────────┐
│  TIER 3: FUZZY MATCH                                       ~200μs    │
│  ────────────────────────────────────────────────────────────────    │
│                                                                      │
│  Levenshtein DFA ─▶ Terms within edit distance 2                     │
│                                                                      │
│  "tensr" ─▶ ["tensor", "tenser", "tenure"] (edit dist ≤ 2)           │
│                    │                                                 │
│                    ├─▶ callback.onUpdate([18 results])               │
│                    └─▶ callback.onFinish([18 results, sorted])       │
└──────────────────────────────────────────────────────────────────────┘
```

### Perceived Latency Improvement

```
JUST onFinish (simple)                 WITH onUpdate (progressive)
──────────────────────────────         ─────────────────────────────────────

  ┌─────────────────────────┐          ┌────────┐
  │ T1 + T2 + T3            │          │ T1     │──► onUpdate → UI
  │ (all tiers complete     │          └────────┘
  │  before callback)       │          ┌────────┐
  │                         │          │ T2     │──► onUpdate → UI
  │        ~215μs           │          └────────┘
  └────────────┬────────────┘          ┌────────┐
               │                       │ T3     │──► onUpdate + onFinish → UI
               ▼                       └────────┘
         ┌───────────┐
         │ onFinish  │                 Total: ~215μs
         └───────────┘                 First result: ~5μs ✓

  User waits ~215μs                    User sees results in ~5μs
  for any results                      (40x faster perceived latency)
```

This gives **40x faster perceived latency** - users see exact matches in ~5μs instead of waiting ~215μs for all tiers.

---

## Implementation Notes

### Memory Model

The WASM module uses linear memory with:
- **Read-only index data** - Vocabulary, postings, suffix array
- **Mutable result buffer** - For collecting search results
- **No runtime allocations** during search (preallocated buffers)

### Thread Safety

In parallel mode:
- Index data is shared read-only via SharedArrayBuffer
- Each worker has its own result buffer
- Final merge happens on main thread
- No locks needed due to immutable shared data

### Cleanup

Always call `free()` when done with a searcher (especially in SPAs):

```typescript
searcher.free();  // Releases WASM memory
```

---

## Related Documentation

- [TypeScript API](typescript.md) - API reference for `loadSorex` and `SorexSearcher`
- [Binary Format](binary-format.md) - How .sorex files are structured
- [Architecture](architecture.md) - System design and algorithms
- [Troubleshooting](troubleshooting.md) - Parallel mode issues
