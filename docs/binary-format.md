---
title: Binary Format
description: .sorex v12 wire format specification
order: 32
---

# Binary Format

This page documents the `.sorex` v12 wire format in full detail. You probably do not need to read this unless you are debugging index corruption, writing tooling that reads `.sorex` files directly, or just curious about how the bytes are laid out.

The key design decision is placing WASM at the front of the file. This enables streaming compilation: browsers start compiling the runtime while the rest of the index is still downloading. The format also embeds everything in a single file (index, metadata, WASM runtime) so deployments never have version mismatches between the runtime and the index it is reading.

For the algorithms behind each section (Block PFOR, suffix arrays, Levenshtein DFA), see [Algorithms](algorithms.md). For the high-level system design, see [Architecture](architecture.md).

---

## Design Goals

The format is designed for streaming initialization and minimal parsing. **WASM comes first** in v12, enabling browsers to start compiling the runtime while the rest of the index downloads. Validation happens once at load time; after that, all operations are direct pointer arithmetic.

**v12 is self-contained** - a single `.sorex` file includes everything needed: the search index, document metadata, and the WASM runtime. No separate JS/WASM files needed.

---

## Why Embed WASM?

Each `.sorex` file embeds its own WASM runtime to avoid backwards compatibility concerns.

The binary format evolves: new compression schemes, additional metadata fields, changed section layouts. If the WASM runtime were separate, every format change would require either:
- Maintaining multiple runtime versions
- Coordinating runtime and index upgrades across deployments
- Version detection logic to load the right runtime

By embedding the runtime, each index is self-contained and frozen in time. A v12 index carries v12-compatible code. A future v13 index carries v13-compatible code. They coexist without conflict. Old indexes keep working indefinitely. New indexes use new features. No migration required.

<aside class="callout callout-neutral">
<div class="callout-title">Size Tradeoff</div>

~150KB (gzipped) added to each index. For most sites with a single search index, this is negligible. If you need multiple indexes without duplicated WASM, [file an issue](https://github.com/harryzorus/sorex/issues).

</aside>

---

## Acknowledgments

Several techniques in this format are inspired by [Apache Lucene](https://lucene.apache.org/), the gold standard for search engine internals:

- **Block PFOR compression** for posting lists (Lucene 4.0+)
- **Skip lists** for large posting traversal (Lucene's multi-level skips)
- **Varint encoding** for compact integer storage
- **Term dictionary** with binary search (similar to Lucene's BlockTree)

The suffix array and Levenshtein DFA components are Sorex-specific additions that enable substring and fuzzy search capabilities beyond traditional inverted indexes.

<aside class="sidenote">

Lucene's approach prioritizes server-side search with large heaps and persistent storage. Sorex inverts this for client-side WASM: everything precomputed, everything in one memory-mapped blob, no runtime allocations during search.

</aside>

---

## Layout

v12 places WASM first to enable **streaming compilation** - browsers can start compiling the runtime while downloading the rest of the index. This eliminates the "download then compile" bottleneck.

```
+---------------------------------------------------------------------+
| HEADER (52 bytes)                                                   |
|   magic: "SORX" (4 bytes) ------------------ Validates file type    |
|   version: u8 = 12                                                  |
|   flags: u8 ---------------------------- HAS_SKIP_LISTS, etc.       |
|   doc_count: u32                                                    |
|   term_count: u32                                                   |
|   vocab_len, sa_len, postings_len, skip_len: u32                    |
|   section_table_len, lev_dfa_len, docs_len, wasm_len: u32           |
|   dict_table_len: u32 --------------------- Dictionary tables       |
|   reserved: 2 bytes                                                 |
+---------------------------------------------------------------------+
| WASM (first for streaming compilation)                              |
|   Embedded WebAssembly runtime (sorex_bg.wasm)                      |
|   ~150KB gzipped, makes .sorex fully self-contained                 |
|   Browser starts compiling while rest of index downloads            |
+---------------------------------------------------------------------+
| VOCABULARY                                                          |
|   Length-prefixed UTF-8 strings, lexicographically sorted           |
|   Format: varint(len) + bytes[len]                                  |
|   Binary search enables O(log k) term lookup                        |
+---------------------------------------------------------------------+
| DICTIONARY TABLES                                                   |
|   Parquet-style string deduplication for repeated fields            |
|   num_tables: u8 (4)                                                |
|   category_table: varint(count) + length-prefixed strings           |
|   author_table: varint(count) + length-prefixed strings             |
|   tags_table: varint(count) + length-prefixed strings               |
|   href_prefix_table: varint(count) + length-prefixed strings        |
|   Reduces wire size by ~45 bytes/doc for large indexes              |
+---------------------------------------------------------------------+
| POSTINGS (Block PFOR, 128-doc blocks)                               |
|   For each term (in vocabulary order):                              |
|     varint(doc_freq)                                                |
|     varint(num_blocks)                                              |
|     For each block: PFOR-encoded deltas (128 docs)                  |
|     varint(tail_count) + varint[tail_count] for remainder           |
|     varint[doc_freq] section_idx values                             |
+---------------------------------------------------------------------+
| SUFFIX ARRAY                                                        |
|   varint(count)                                                     |
|   FOR-encoded (term_idx, char_offset) pairs                         |
|   Delta-encoded term_idx for compression                            |
|   Enables prefix search across entire vocabulary                    |
+---------------------------------------------------------------------+
| DOCUMENTS                                                           |
|   varint(count)                                                     |
|   For each doc:                                                     |
|     type: u8 (0=page, 1=post)                                       |
|     title, excerpt, href: varint_len + utf8                         |
|     category, author: dictionary-indexed                            |
|     tags: array of dictionary indices                               |
+---------------------------------------------------------------------+
| SECTION TABLE                                                       |
|   Deduplicated section_id strings for deep linking                  |
|   varint(count) + length-prefixed strings                           |
|   Postings reference these by index (0 = no section)                |
+---------------------------------------------------------------------+
| SKIP LISTS (for terms with >1024 docs)                              |
|   Multi-level skip pointers for fast posting traversal              |
|   Enables O(log n) seeks within large posting lists                 |
+---------------------------------------------------------------------+
| LEVENSHTEIN DFA                                                     |
|   Precomputed parametric automaton (Schulz-Mihov 2002)              |
|   ~1.2KB for k=2 with transpositions                                |
|   Enables zero-CPU-cost fuzzy matching at query time                |
+---------------------------------------------------------------------+
| FOOTER (8 bytes)                                                    |
|   crc32: u32 ----------------------- Over header + sections         |
|   magic: "XROS" -------------------- Validates complete file        |
+---------------------------------------------------------------------+
```

---

## Streaming Compilation Flow

The v12 layout enables parallel loading:

```
TIME ------------------------------------------------------------------------>

         +--------------------------------------------------------------------+
NETWORK  | Header | WASM bytes -----------------------| Index data -----------|
         +--------+-----------------------------------+------------------------+
                  |                                   |
                  v                                   |
         +--------------------------------------+     |
COMPILE  | WebAssembly.compileStreaming()      |      |
         | (runs in parallel with download)    |      |
         +--------------------------------------+     |
                                                      |
                                                      v
         +--------------------------------------------------------------------+
PARSE    |                Parse vocabulary, postings, suffix array            |
         +--------------------------------------------------------------------+
                                                                              |
                                                                              v
         +--------------------------------------------------------------------+
READY    |                    SorexSearcher ready                             |
         +--------------------------------------------------------------------+

Traditional (WASM at end):   Download ----------------------> Compile --> Parse --> Ready
v12 (WASM first):            Download + Compile -----------------------------> Ready
                                     ^ parallel ^
```

By placing WASM at the front of the file, browsers can use `WebAssembly.compileStreaming()` to compile the runtime **while the rest of the index is still downloading**. This eliminates the sequential "download then compile" bottleneck.

---

## Block PFOR Compression

Posting lists use Block Patched Frame-of-Reference encoding (same as Lucene):

```
Block of 128 doc_id deltas:
  [5, 1, 3, 2, 1, ...]   <-- Differences between consecutive doc_ids

PFOR encoding:
  min_delta: varint      <-- Frame of reference (subtract from all)
  bits_per_value: u8     <-- Bits needed for max(adjusted_deltas)
  packed_data: [u8; ...] <-- Bit-packed values

Example:
  Deltas: [10, 11, 12, 10, 11, 12, ...]  <-- All values 10-12
  min_delta = 10
  bits_per_value = 2  <-- Need 2 bits to represent 0, 1, 2
  Packed: 2 bits x 128 = 32 bytes (vs 512 bytes uncompressed)
```

For uniform distributions (common in rare terms), bits_per_value = 0 and no packed data is written.

---

## Varint Encoding

All variable-length integers use LEB128:

```
Value         Encoded
0             0x00
127           0x7F
128           0x80 0x01
16383         0xFF 0x7F
16384         0x80 0x80 0x01
```

Continuation bit (0x80) indicates more bytes follow. Maximum 10 bytes for u64.

---

## Related Documentation

- [Architecture](architecture.md) - System design and search algorithms
- [Algorithms](algorithms.md) - Data structures (suffix arrays, Levenshtein DFA)
- [Runtime](runtime.md) - Browser execution model
- [CLI Reference](cli.md) - Building indexes with `sorex index`
