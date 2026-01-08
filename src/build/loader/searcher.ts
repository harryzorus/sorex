/**
 * SorexSearcher - WASM-backed search interface
 *
 * Wraps the Rust SorexSearcher via wasm-bindgen, providing
 * a type-safe JavaScript API for searching .sorex indexes.
 *
 * @module searcher
 */

import type { WasmState } from "./wasm-state";

/**
 * A single search result returned by SorexSearcher.search()
 */
export interface SearchResult {
  /** URL path to the document */
  href: string;
  /** Document title */
  title: string;
  /** Short excerpt from the matching content */
  excerpt: string;
  /** Section ID for deep linking (e.g., "installation"), or null */
  sectionId: string | null;
}

// FinalizationRegistry for automatic cleanup
const SorexSearcherFinalization =
  typeof FinalizationRegistry === "undefined"
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry((prevent: () => void) => prevent());

/**
 * Search index loaded from a .sorex file.
 * Each instance is bound to a specific WASM version.
 */
export class SorexSearcher {
  #state: WasmState;
  #ptr: number;

  constructor(state: WasmState, indexBytes: Uint8Array) {
    this.#state = state;
    const wasm = state.wasm;

    const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
    try {
      const ptr0 = state.passArrayToWasm(indexBytes, wasm.__wbindgen_export);
      const len0 = state.vectorLen;
      wasm.sorexsearcher_new(retptr, ptr0, len0);

      const r0 = state.getDataView().getInt32(retptr + 0, true);
      const r1 = state.getDataView().getInt32(retptr + 4, true);
      const r2 = state.getDataView().getInt32(retptr + 8, true);

      if (r2) {
        throw state.takeObject(r1) as Error;
      }

      this.#ptr = r0 >>> 0;
      SorexSearcherFinalization.register(this, () => this.free(), this);
    } finally {
      wasm.__wbindgen_add_to_stack_pointer(16);
    }
  }

  /**
   * Release WASM memory. Call when done with the searcher.
   */
  free(): void {
    if (this.#ptr === 0) return;
    const ptr = this.#ptr;
    this.#ptr = 0;
    SorexSearcherFinalization.unregister(this);
    this.#state.wasm.__wbg_sorexsearcher_free(ptr, 0);
  }

  /**
   * Search with three-tier strategy: exact -> prefix -> fuzzy.
   * @param query - Search query
   * @param limit - Maximum results (default: 10)
   * @returns Array of search results
   */
  search(query: string, limit?: number): SearchResult[] {
    const state = this.#state;
    const wasm = state.wasm;
    const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);

    try {
      const ptr0 = state.passStringToWasm(
        query,
        wasm.__wbindgen_export,
        wasm.__wbindgen_export2
      );
      const len0 = state.vectorLen;

      wasm.sorexsearcher_search(
        retptr,
        this.#ptr,
        ptr0,
        len0,
        state.isLikeNone(limit) ? 0x100000001 : (limit as number) >>> 0
      );

      const r0 = state.getDataView().getInt32(retptr + 0, true);
      const r1 = state.getDataView().getInt32(retptr + 4, true);
      const r2 = state.getDataView().getInt32(retptr + 8, true);

      if (r2) {
        throw state.takeObject(r1) as Error;
      }

      return state.takeObject(r0) as SearchResult[];
    } finally {
      wasm.__wbindgen_add_to_stack_pointer(16);
    }
  }

  /**
   * Check if documents are loaded.
   */
  has_docs(): boolean {
    return this.#state.wasm.sorexsearcher_has_docs(this.#ptr) !== 0;
  }

  /**
   * Get the number of indexed documents.
   */
  doc_count(): number {
    return this.#state.wasm.sorexsearcher_doc_count(this.#ptr) >>> 0;
  }

  /**
   * Get the vocabulary size (number of unique terms).
   */
  vocab_size(): number {
    return this.#state.wasm.sorexsearcher_vocab_size(this.#ptr) >>> 0;
  }

  /**
   * Check if vocabulary is loaded.
   */
  has_vocabulary(): boolean {
    return this.#state.wasm.sorexsearcher_has_vocabulary(this.#ptr) !== 0;
  }
}
