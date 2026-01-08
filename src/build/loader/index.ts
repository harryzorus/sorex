/**
 * Sorex Loader - Self-contained search loader for .sorex files
 *
 * This module provides a complete, self-contained loader for .sorex files.
 * All wasm-bindgen glue code is inlined - no external dependencies required.
 *
 * Features:
 * - Automatic WASM extraction from .sorex files
 * - Per-version isolation (multiple WASM versions can coexist)
 * - CRC32 validation after index reconstruction
 * - FinalizationRegistry for automatic cleanup
 *
 * @module sorex-loader
 *
 * @example
 * ```typescript
 * import { loadSorex } from './sorex-loader.js';
 *
 * // Load and search
 * const searcher = await loadSorex('./index.sorex');
 * const results = searcher.search('query', 10);
 *
 * // Results include deep linking info
 * results.forEach(r => {
 *   const url = r.sectionId ? `${r.href}#${r.sectionId}` : r.href;
 *   console.log(r.title, url);
 * });
 *
 * // Free when done (optional - uses FinalizationRegistry)
 * searcher.free();
 * ```
 */

import { WasmState } from "./wasm-state";
import { SorexSearcher } from "./searcher";
import { parseSorex } from "./parser";

export type { SearchResult } from "./searcher";
export { SorexSearcher } from "./searcher";

// Instance cache for WASM version isolation
const instances = new Map<number, WasmState>();

/**
 * Simple hash for WASM deduplication.
 * Samples every 1024th byte for speed.
 */
function hashBytes(bytes: Uint8Array): number {
  let hash = 2166136261; // FNV-1a offset basis
  for (let i = 0; i < bytes.length; i += 1024) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 16777619); // FNV-1a prime
  }
  return hash >>> 0;
}

/**
 * Get or create a WASM instance for the given bytes.
 * Instances are cached by hash to allow reuse.
 */
function getOrCreateInstance(wasmBytes: Uint8Array): WasmState {
  const hash = hashBytes(wasmBytes);
  let state = instances.get(hash);
  if (!state) {
    state = new WasmState(wasmBytes);
    instances.set(hash, state);
  }
  return state;
}

/**
 * Load a .sorex file and create a searcher.
 *
 * Multiple .sorex files with different WASM versions can coexist on the same page.
 * Each unique WASM version gets its own isolated instance.
 *
 * @param url - URL to the .sorex file
 * @returns Promise that resolves to an initialized searcher
 *
 * @example
 * ```typescript
 * const searcher = await loadSorex('./index.sorex');
 * const results = searcher.search('hello world', 10);
 * ```
 */
export async function loadSorex(url: string): Promise<SorexSearcher> {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status}`);
  }

  const buffer = await response.arrayBuffer();
  return loadSorexSync(buffer);
}

/**
 * Load from an ArrayBuffer (for use with pre-fetched data).
 *
 * @param buffer - The .sorex file contents
 * @returns Initialized searcher
 *
 * @example
 * ```typescript
 * const response = await fetch('./index.sorex');
 * const buffer = await response.arrayBuffer();
 * const searcher = loadSorexSync(buffer);
 * ```
 */
export function loadSorexSync(buffer: ArrayBuffer): SorexSearcher {
  const { wasm: wasmBytes, index } = parseSorex(buffer);
  const state = getOrCreateInstance(wasmBytes);
  return new SorexSearcher(state, index);
}
