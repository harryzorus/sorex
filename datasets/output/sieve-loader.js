/**
 * Sieve Loader - Extract WASM and index data from a .sieve file
 *
 * Minimal loader that parses the v7 binary format header and extracts sections.
 * Designed to be extensible as the format evolves.
 */

const HEADER_SIZE = 52;
const MAGIC = [0x53, 0x49, 0x46, 0x54]; // "SIFT"

/**
 * Parse .sieve header and extract all sections
 * @param {ArrayBuffer} buffer - The .sieve file contents
 * @returns {{ header: Object, wasm: Uint8Array, index: Uint8Array }}
 */
export function parseSieve(buffer) {
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  // Validate magic
  for (let i = 0; i < 4; i++) {
    if (view.getUint8(i) !== MAGIC[i]) {
      throw new Error('Invalid .sieve file');
    }
  }

  const version = view.getUint8(4);
  if (version < 7) {
    throw new Error(`Sieve v${version} does not embed WASM, need v7+`);
  }

  // Parse header (little-endian u32 fields)
  const header = {
    version,
    docCount: view.getUint32(6, true),
    termCount: view.getUint32(10, true),
    vocabLen: view.getUint32(14, true),
    saLen: view.getUint32(18, true),
    postingsLen: view.getUint32(22, true),
    skipLen: view.getUint32(26, true),
    sectionTableLen: view.getUint32(30, true),
    levDfaLen: view.getUint32(34, true),
    docsLen: view.getUint32(38, true),
    wasmLen: view.getUint32(42, true),
    dictTableLen: view.getUint32(46, true),
  };

  // Calculate section offsets
  const wasmOffset = HEADER_SIZE + header.vocabLen + header.saLen +
    header.postingsLen + header.skipLen + header.sectionTableLen +
    header.levDfaLen + header.docsLen;
  const dictTableOffset = wasmOffset + header.wasmLen;
  const footerOffset = dictTableOffset + header.dictTableLen;

  // Extract WASM binary
  const wasm = bytes.slice(wasmOffset, wasmOffset + header.wasmLen);

  // Build index: header + sections before WASM + dict_tables + footer (WASM stripped)
  // The index is what the WASM module loads - it includes dict_tables but not the WASM itself
  const indexLen = wasmOffset + header.dictTableLen + 8; // +8 for footer
  const index = new Uint8Array(indexLen);

  // Copy header + all sections up to WASM
  index.set(bytes.slice(0, wasmOffset), 0);

  // Copy dict_tables (placed right after docs, since we're skipping WASM)
  index.set(bytes.slice(dictTableOffset, dictTableOffset + header.dictTableLen), wasmOffset);

  // Zero out wasmLen in copied header (WASM is extracted separately)
  new DataView(index.buffer).setUint32(42, 0, true);

  // Copy footer
  index.set(bytes.slice(footerOffset, footerOffset + 8), wasmOffset + header.dictTableLen);

  return { header, wasm, index };
}

/**
 * Load .sieve from URL and initialize search
 * @param {string} url - URL to .sieve file
 * @param {Function} initWasm - WASM init function (from sieve.js)
 * @param {Function} createSearcher - Searcher constructor (SieveSearcher.fromBytes)
 * @returns {Promise<Object>} Initialized searcher
 */
export async function loadSieve(url, initWasm, createSearcher) {
  const res = await fetch(url);
  if (!res.ok) throw new Error(`Failed to fetch: ${res.status}`);

  const { wasm, index } = parseSieve(await res.arrayBuffer());
  await initWasm(wasm);
  return createSearcher(index);
}
