/**
 * Sorex Browser Loader
 *
 * This TypeScript source is compiled to JavaScript and combined with
 * wasm-bindgen output to create the final sorex.js loader.
 *
 * Build: deno task build
 */

// =============================================================================
// Declarations for browser globals and wasm-bindgen (injected at build time)
// =============================================================================

// Browser globals
declare const crossOriginIsolated: boolean | undefined;

declare const VERSION: number;

// wasm-bindgen globals
declare let wasm: WebAssembly.Exports & {
  memory: WebAssembly.Memory;
  [key: string]: unknown;
};
declare let cachedUint8ArrayMemory0: Uint8Array | null;
declare let cachedDataViewMemory0: DataView | null;

// wasm-bindgen functions
declare function initSync(options: { module: BufferSource | Uint8Array; memory?: WebAssembly.Memory }): void;
declare function __wbg_get_imports(memory?: WebAssembly.Memory): WebAssembly.Imports;
declare function __wbg_finalize_init(
  instance: WebAssembly.Instance,
  module: WebAssembly.Module
): void;
declare function initThreadPool(numThreads: number): Promise<void>;

// wasm-bindgen classes
declare class SorexSearcher {
  constructor(bytes: Uint8Array);
  search(
    query: string,
    limit: number,
    onUpdate: (results: SearchResult[]) => void,
    onFinish: (results: SearchResult[]) => void
  ): void;
  searchSync(query: string, limit: number): SearchResult[];
  searchWithTierTiming(query: string, limit: number): TierTimingResult;
  doc_count(): number;
  vocab_size(): number;
  free(): void;
}

declare class SorexIncrementalLoader {
  constructor();
  loadHeader(bytes: Uint8Array): SectionOffsets;
  loadVocabulary(bytes: Uint8Array): void;
  loadDictTables(bytes: Uint8Array): void;
  loadPostings(bytes: Uint8Array): void;
  loadSuffixArray(bytes: Uint8Array): void;
  loadDocs(bytes: Uint8Array): void;
  loadSectionTable(bytes: Uint8Array): void;
  loadSkipLists(bytes: Uint8Array): void;
  loadLevDfa(bytes: Uint8Array): void;
  finalize(): SorexSearcher;
}

// =============================================================================
// Types
// =============================================================================

interface SearchResult {
  id: number;
  slug: string;
  title: string;
  excerpt: string;
  href: string;
  score: number;
  sectionId: string | null;
  matchType: string;
}

interface TierTimingResult {
  results: SearchResult[];
  t1Count: number;
  t2Count: number;
  t3Count: number;
  t1TimeUs: number;
  t2TimeUs: number;
  t3TimeUs: number;
}

interface SectionOffsets {
  vocabularyStart: number;
  vocabularyEnd: number;
  dictTablesStart: number;
  dictTablesEnd: number;
  postingsStart: number;
  postingsEnd: number;
  suffixArrayStart: number;
  suffixArrayEnd: number;
  docsStart: number;
  docsEnd: number;
  sectionTableStart: number;
  sectionTableEnd: number;
  skipListsStart: number;
  skipListsEnd: number;
  levDfaStart: number;
  levDfaEnd: number;
}

interface SearchCallback {
  onUpdate?: (results: SearchResult[]) => void;
  onFinish?: (results: SearchResult[]) => void;
}

// =============================================================================
// Constants
// =============================================================================

const HEADER_SIZE = 52;
const MAGIC = [83, 79, 82, 88]; // "SORX"
const FOOTER_MAGIC = [88, 82, 79, 83]; // "XROS"

// CRC32 lookup table
const CRC32_TABLE = new Uint32Array(256);
for (let i = 0; i < 256; i++) {
  let crc = i;
  for (let j = 0; j < 8; j++) {
    crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
  }
  CRC32_TABLE[i] = crc >>> 0;
}

// =============================================================================
// CRC32
// =============================================================================

function computeCrc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (let i = 0; i < data.length; i++) {
    crc = CRC32_TABLE[(crc ^ data[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

// =============================================================================
// .sorex Parser
// =============================================================================

interface ParsedSorex {
  wasm: Uint8Array;
  index: Uint8Array;
}

function parseSorex(buffer: ArrayBuffer | SharedArrayBuffer): ParsedSorex {
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  // Validate magic
  for (let i = 0; i < 4; i++) {
    if (view.getUint8(i) !== MAGIC[i]) {
      throw new Error("Invalid .sorex file: bad magic");
    }
  }

  const version = view.getUint8(4);
  if (version !== VERSION) {
    throw new Error(`Sorex v${version} not supported, need v${VERSION}`);
  }

  // Parse header fields
  const wasmLen = view.getUint32(42, true);
  const vocabLen = view.getUint32(14, true);
  const saLen = view.getUint32(18, true);
  const postingsLen = view.getUint32(22, true);
  const skipLen = view.getUint32(26, true);
  const sectionTableLen = view.getUint32(30, true);
  const levDfaLen = view.getUint32(34, true);
  const docsLen = view.getUint32(38, true);
  const dictTableLen = view.getUint32(46, true);

  const indexSectionsLen =
    vocabLen + saLen + postingsLen + skipLen + sectionTableLen + levDfaLen + docsLen + dictTableLen;

  const wasmOffset = HEADER_SIZE;
  const sectionsOffset = HEADER_SIZE + wasmLen;

  // Extract WASM bytes
  const wasm = bytes.slice(wasmOffset, wasmOffset + wasmLen);

  // Reconstruct index (header + sections, no WASM)
  const contentLen = HEADER_SIZE + indexSectionsLen;
  const index = new Uint8Array(contentLen + 8);

  // Copy header
  index.set(bytes.slice(0, HEADER_SIZE), 0);

  // Set wasm_len to 0 in reconstructed index
  const indexView = new DataView(index.buffer);
  indexView.setUint32(42, 0, true);

  // Copy index sections
  index.set(bytes.slice(sectionsOffset, sectionsOffset + indexSectionsLen), HEADER_SIZE);

  // Recompute CRC32
  const newCrc32 = computeCrc32(index.subarray(0, contentLen));
  indexView.setUint32(contentLen, newCrc32, true);

  // Add footer magic
  for (let i = 0; i < 4; i++) {
    index[contentLen + 4 + i] = FOOTER_MAGIC[i];
  }

  return { wasm, index };
}

// =============================================================================
// Multi-Instance Wrapper
// =============================================================================

const _originalSorexSearcher = SorexSearcher;

class SorexSearcherWrapper {
  private _wasm: typeof wasm;
  private _instance: SorexSearcher;

  constructor(bytesOrSearcher: Uint8Array | SorexSearcher) {
    this._wasm = wasm;
    if (bytesOrSearcher instanceof _originalSorexSearcher) {
      this._instance = bytesOrSearcher;
    } else {
      this._instance = new _originalSorexSearcher(bytesOrSearcher);
    }
  }

  private _restore(): void {
    wasm = this._wasm;
    cachedUint8ArrayMemory0 = null;
    cachedDataViewMemory0 = null;
  }

  search(query: string, limit: number, callback: SearchCallback = {}): void {
    this._restore();
    const { onUpdate, onFinish } = callback;
    this._instance.search(query, limit, onUpdate || (() => {}), onFinish || (() => {}));
  }

  searchSync(query: string, limit: number): SearchResult[] {
    this._restore();
    return this._instance.searchSync(query, limit);
  }

  searchWithTierTiming(query: string, limit: number): TierTimingResult {
    this._restore();
    return this._instance.searchWithTierTiming(query, limit);
  }

  docCount(): number {
    this._restore();
    return this._instance.doc_count();
  }

  vocabSize(): number {
    this._restore();
    return this._instance.vocab_size();
  }

  free(): void {
    this._restore();
    this._instance.free();
  }
}

// =============================================================================
// Thread Pool
// =============================================================================

let cachedWasmModule: WebAssembly.Module | null = null;
let threadPoolInitialized = false;
let threadPoolFailed = false;
let threadPoolInitPromise: Promise<boolean> | null = null;

function isThreadingAvailable(): boolean {
  return threadPoolInitialized;
}

function isSafari(): boolean {
  if (typeof navigator === "undefined") return false;
  const ua = navigator.userAgent;
  return ua.includes("Safari") && !ua.includes("Chrome") && !ua.includes("Chromium");
}

async function initThreadPoolIfNeeded(): Promise<boolean> {
  if (threadPoolInitialized) return true;
  if (threadPoolFailed) return false;

  if (threadPoolInitPromise) {
    return threadPoolInitPromise;
  }

  if (isSafari()) {
    console.log("[sorex] Safari detected - skipping thread pool");
    threadPoolFailed = true;
    return false;
  }

  if (typeof crossOriginIsolated !== "undefined" && !crossOriginIsolated) {
    console.warn("[sorex] Page not cross-origin isolated - parallel search requires COOP/COEP headers");
    threadPoolFailed = true;
    return false;
  }

  const numThreads = navigator?.hardwareConcurrency || 4;

  threadPoolInitPromise = (async () => {
    try {
      if (typeof initThreadPool === "function") {
        await initThreadPool(numThreads);
        threadPoolInitialized = true;
        return true;
      } else {
        console.warn("[sorex] initThreadPool function not available");
      }
    } catch (e) {
      console.warn("[sorex] Thread pool init failed:", (e as Error).message || e);
      threadPoolFailed = true;
    }
    return false;
  })();

  return threadPoolInitPromise;
}

// =============================================================================
// Utilities
// =============================================================================

function concatenateChunks(chunks: Uint8Array[], length: number): Uint8Array {
  const result = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) {
    const toCopy = Math.min(chunk.length, length - offset);
    result.set(chunk.subarray(0, toCopy), offset);
    offset += toCopy;
    if (offset >= length) break;
  }
  return result;
}

// =============================================================================
// Loaders
// =============================================================================

async function loadSorex(url: string): Promise<SorexSearcherWrapper> {
  const canUseIncremental =
    typeof crossOriginIsolated !== "undefined" &&
    crossOriginIsolated &&
    typeof SorexIncrementalLoader !== "undefined";

  if (!canUseIncremental) {
    console.log("[sorex] Using simple streaming mode");
    return loadSorexSimple(url);
  }

  return loadSorexIncremental(url);
}

async function loadSorexIncremental(url: string): Promise<SorexSearcherWrapper> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Failed to fetch ${url}: ${response.status}`);

  const reader = response.body!.getReader();
  const chunks: Uint8Array[] = [];
  let totalBytes = 0;
  let wasmLen = 0;
  let wasmCompilePromise: Promise<WebAssembly.Module> | null = null;
  let incrementalLoader: SorexIncrementalLoader | null = null;
  let offsets: SectionOffsets | null = null;

  const dispatched = {
    vocabulary: false,
    dictTables: false,
    postings: false,
    suffixArray: false,
    docs: false,
    sectionTable: false,
    skipLists: false,
    levDfa: false,
  };

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    chunks.push(value);
    totalBytes += value.length;

    if (wasmLen === 0 && totalBytes >= HEADER_SIZE) {
      const headerBytes = concatenateChunks(chunks, HEADER_SIZE);
      const view = new DataView(headerBytes.buffer);
      wasmLen = view.getUint32(42, true);
    }

    if (wasmLen > 0 && !wasmCompilePromise && totalBytes >= HEADER_SIZE + wasmLen) {
      const wasmBytes = concatenateChunks(chunks, HEADER_SIZE + wasmLen).slice(HEADER_SIZE);
      wasmCompilePromise = WebAssembly.compile(wasmBytes);
    }

    if (wasmCompilePromise && !incrementalLoader && totalBytes >= HEADER_SIZE + wasmLen) {
      const wasmModule = await wasmCompilePromise;
      cachedWasmModule = wasmModule;

      let sharedMemory: WebAssembly.Memory;
      try {
        sharedMemory = new WebAssembly.Memory({ initial: 18, maximum: 16384, shared: true });
      } catch (e) {
        console.warn("[sorex] Cannot create shared memory:", (e as Error).message);
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          chunks.push(value);
          totalBytes += value.length;
        }
        const buffer = concatenateChunks(chunks, totalBytes).buffer;
        const { index } = parseSorex(buffer);
        return new SorexSearcherWrapper(index);
      }

      const imports = __wbg_get_imports(sharedMemory);
      const instance = await WebAssembly.instantiate(wasmModule, imports);
      __wbg_finalize_init(instance, wasmModule);

      cachedUint8ArrayMemory0 = null;
      cachedDataViewMemory0 = null;

      const threadingOk = await initThreadPoolIfNeeded();
      if (!threadingOk) {
        console.log("[sorex] Thread pool unavailable - using single-threaded mode");
        while (true) {
          const { done, value } = await reader.read();
          if (done) break;
          chunks.push(value);
          totalBytes += value.length;
        }
        const buffer = concatenateChunks(chunks, totalBytes).buffer;
        const { index } = parseSorex(buffer);
        return new SorexSearcherWrapper(index);
      }

      incrementalLoader = new SorexIncrementalLoader();
      const headerBytes = concatenateChunks(chunks, HEADER_SIZE);
      offsets = incrementalLoader.loadHeader(headerBytes);
    }

    if (offsets && incrementalLoader) {
      if (!dispatched.vocabulary && totalBytes >= offsets.vocabularyEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadVocabulary(buffer.slice(offsets.vocabularyStart, offsets.vocabularyEnd));
        dispatched.vocabulary = true;
      }

      if (!dispatched.dictTables && totalBytes >= offsets.dictTablesEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadDictTables(buffer.slice(offsets.dictTablesStart, offsets.dictTablesEnd));
        dispatched.dictTables = true;
      }

      if (!dispatched.postings && totalBytes >= offsets.postingsEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadPostings(buffer.slice(offsets.postingsStart, offsets.postingsEnd));
        dispatched.postings = true;
      }

      if (!dispatched.suffixArray && totalBytes >= offsets.suffixArrayEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadSuffixArray(buffer.slice(offsets.suffixArrayStart, offsets.suffixArrayEnd));
        dispatched.suffixArray = true;
      }

      if (!dispatched.docs && totalBytes >= offsets.docsEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadDocs(buffer.slice(offsets.docsStart, offsets.docsEnd));
        dispatched.docs = true;
      }

      if (!dispatched.sectionTable && totalBytes >= offsets.sectionTableEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadSectionTable(buffer.slice(offsets.sectionTableStart, offsets.sectionTableEnd));
        dispatched.sectionTable = true;
      }

      if (!dispatched.skipLists && totalBytes >= offsets.skipListsEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadSkipLists(buffer.slice(offsets.skipListsStart, offsets.skipListsEnd));
        dispatched.skipLists = true;
      }

      if (!dispatched.levDfa && totalBytes >= offsets.levDfaEnd) {
        const buffer = concatenateChunks(chunks, totalBytes);
        incrementalLoader.loadLevDfa(buffer.slice(offsets.levDfaStart, offsets.levDfaEnd));
        dispatched.levDfa = true;
      }
    }
  }

  const searcher = incrementalLoader!.finalize();
  return new SorexSearcherWrapper(searcher);
}

async function loadSorexSimple(url: string): Promise<SorexSearcherWrapper> {
  const response = await fetch(url);
  if (!response.ok) throw new Error(`Failed to fetch ${url}: ${response.status}`);

  const reader = response.body!.getReader();
  const chunks: Uint8Array[] = [];
  let totalBytes = 0;
  let wasmLen = 0;
  let wasmCompilePromise: Promise<WebAssembly.Module> | null = null;

  while (true) {
    const { done, value } = await reader.read();
    if (done) break;

    chunks.push(value);
    totalBytes += value.length;

    if (wasmLen === 0 && totalBytes >= HEADER_SIZE) {
      const headerBytes = concatenateChunks(chunks, HEADER_SIZE);
      const view = new DataView(headerBytes.buffer);
      wasmLen = view.getUint32(42, true);
    }

    if (wasmLen > 0 && !wasmCompilePromise && totalBytes >= HEADER_SIZE + wasmLen) {
      const wasmBytes = concatenateChunks(chunks, HEADER_SIZE + wasmLen).slice(HEADER_SIZE);
      wasmCompilePromise = WebAssembly.compile(wasmBytes);
    }
  }

  const buffer = concatenateChunks(chunks, totalBytes).buffer;
  const wasmModule = await wasmCompilePromise!;
  cachedWasmModule = wasmModule;

  const { index } = parseSorex(buffer);

  let sharedMemory: WebAssembly.Memory;
  try {
    sharedMemory = new WebAssembly.Memory({ initial: 18, maximum: 16384, shared: true });
  } catch (e) {
    console.warn("[sorex] Cannot create shared memory:", (e as Error).message);
    throw new Error("Sorex requires SharedArrayBuffer. Set COOP/COEP headers.");
  }

  const imports = __wbg_get_imports(sharedMemory);
  const instance = await WebAssembly.instantiate(wasmModule, imports);
  __wbg_finalize_init(instance, wasmModule);

  cachedUint8ArrayMemory0 = null;
  cachedDataViewMemory0 = null;

  await initThreadPoolIfNeeded();

  return new SorexSearcherWrapper(index);
}

function loadSorexSync(buffer: ArrayBuffer): SorexSearcherWrapper {
  const { wasm: wasmBytes, index } = parseSorex(buffer);

  let sharedMemory: WebAssembly.Memory;
  try {
    sharedMemory = new WebAssembly.Memory({ initial: 18, maximum: 16384, shared: true });
  } catch (e) {
    console.warn("[sorex] Cannot create shared memory:", (e as Error).message);
    throw new Error("Sorex requires SharedArrayBuffer. Set COOP/COEP headers.");
  }

  initSync({ module: wasmBytes, memory: sharedMemory });
  cachedUint8ArrayMemory0 = null;
  cachedDataViewMemory0 = null;
  return new SorexSearcherWrapper(index);
}

// =============================================================================
// Exports (for deno bundle to include these - stripped by build.ts)
// =============================================================================

export { loadSorex, loadSorexSync, SorexSearcherWrapper };
