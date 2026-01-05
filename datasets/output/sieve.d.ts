/* tslint:disable */
/* eslint-disable */

export class SieveProgressiveIndex {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Search using only inverted index (O(1) exact word matches).
   *
   * Returns results from exact word matches only. This is the fast path
   * that provides first results immediately.
   *
   * Use this for the first phase of streaming search.
   */
  search_exact(query: string, options?: any | null): any;
  /**
   * Get list of loaded layer names.
   */
  loaded_layers(): string[];
  /**
   * Check if all layers are loaded.
   */
  is_fully_loaded(): boolean;
  /**
   * Search using suffix array, excluding already-found IDs (O(log k)).
   *
   * Returns additional results not found by exact search.
   * Pass the doc IDs from search_exact() as exclude_ids.
   *
   * Use this for the second phase of streaming search.
   */
  search_expanded(query: string, exclude_ids: any, options?: any | null): any;
  /**
   * Load a specific layer from binary format.
   *
   * Valid layer names: "titles", "headings", "content"
   *
   * Binary format (.sieve files) uses:
   * - FST vocabulary (5-10x smaller than JSON)
   * - Block PFOR postings (Lucene-style 128-doc blocks)
   * - Skip lists for large posting lists
   *
   * This method is ~3-5x faster to decode than JSON format.
   */
  load_layer_binary(layer_name: string, layer_bytes: Uint8Array): void;
  /**
   * Create a new progressive index from a manifest.
   *
   * The manifest contains only document metadata, no search data.
   * Layers must be loaded separately via `load_layer()`.
   */
  constructor(manifest: any);
  /**
   * Search across all loaded layers and return results.
   *
   * Results include source attribution (title, heading, or content).
   * For documents matching in multiple layers, the highest-scoring source is used.
   *
   * Options (all optional):
   * - `limit`: Maximum results (default: 10)
   * - `fuzzy`: Enable fuzzy matching (default: true)
   * - `prefix`: Enable prefix matching (default: true)
   * - `boost`: Custom field boosts `{title: 100, heading: 10, content: 1}`
   */
  search(query: string, options?: any | null): any;
  /**
   * Get suggestions for a partial query (prefix search on vocabulary).
   *
   * Returns terms from the index that start with the given prefix,
   * sorted by document frequency (most common first).
   */
  suggest(partial: string, limit?: number | null): any;
  /**
   * Get the total number of documents.
   */
  doc_count(): number;
  /**
   * Check if a specific layer is loaded.
   */
  has_layer(layer_name: string): boolean;
}

export class SieveSearcher {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get the number of terms in the vocabulary.
   */
  vocab_size(): number;
  /**
   * Check if vocabulary is available for fuzzy search.
   */
  has_vocabulary(): boolean;
  /**
   * Tier 1: Exact word match only (O(1) inverted index lookup).
   * Returns results immediately for fast first-result display.
   */
  search_tier1_exact(query: string, limit?: number | null): any;
  /**
   * Tier 3: Fuzzy match only (O(vocabulary) via Levenshtein DFA).
   * Pass doc IDs from tier1+tier2 as exclude_ids to avoid duplicates.
   */
  search_tier3_fuzzy(query: string, exclude_ids: any, limit?: number | null): any;
  /**
   * Tier 2: Prefix match only (O(log k) binary search).
   * Pass doc IDs from tier1 as exclude_ids to avoid duplicates.
   */
  search_tier2_prefix(query: string, exclude_ids: any, limit?: number | null): any;
  /**
   * Create a new searcher from binary .sieve format.
   *
   * The binary format is 5-7x smaller than JSON and loads ~3-5x faster.
   * Since v5, document metadata is embedded in the binary (no separate load_docs call needed).
   * Since v6, section_ids are stored per-posting for deep linking to specific sections.
   */
  constructor(bytes: Uint8Array);
  /**
   * Search with three-tier strategy: exact → prefix → fuzzy.
   *
   * Returns JSON array of SearchResult objects with section_ids for deep linking.
   *
   * Tier 1 (O(1)): Exact word match via inverted index
   * Tier 2 (O(log k)): Prefix match via vocabulary suffix array
   * Tier 3 (O(FST)): Fuzzy match via FST + Levenshtein DFA
   */
  search(query: string, limit?: number | null): any;
  /**
   * Check if document metadata is loaded (either embedded or via load_docs).
   */
  has_docs(): boolean;
  /**
   * Get the number of documents.
   */
  doc_count(): number;
  /**
   * Load document metadata (for backward compatibility with v4 files).
   *
   * Not needed for v5+ files where docs are embedded in the binary.
   */
  load_docs(docs: any): void;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_sieveprogressiveindex_free: (a: number, b: number) => void;
  readonly __wbg_sievesearcher_free: (a: number, b: number) => void;
  readonly sieveprogressiveindex_doc_count: (a: number) => number;
  readonly sieveprogressiveindex_has_layer: (a: number, b: number, c: number) => number;
  readonly sieveprogressiveindex_is_fully_loaded: (a: number) => number;
  readonly sieveprogressiveindex_load_layer_binary: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly sieveprogressiveindex_loaded_layers: (a: number, b: number) => void;
  readonly sieveprogressiveindex_new: (a: number, b: number) => void;
  readonly sieveprogressiveindex_search: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly sieveprogressiveindex_search_exact: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly sieveprogressiveindex_search_expanded: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly sieveprogressiveindex_suggest: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly sievesearcher_doc_count: (a: number) => number;
  readonly sievesearcher_has_docs: (a: number) => number;
  readonly sievesearcher_has_vocabulary: (a: number) => number;
  readonly sievesearcher_load_docs: (a: number, b: number, c: number) => void;
  readonly sievesearcher_new: (a: number, b: number, c: number) => void;
  readonly sievesearcher_search: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly sievesearcher_search_tier1_exact: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly sievesearcher_search_tier2_prefix: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly sievesearcher_search_tier3_fuzzy: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly sievesearcher_vocab_size: (a: number) => number;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export3: (a: number) => void;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
