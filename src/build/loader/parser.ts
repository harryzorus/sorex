/**
 * .sieve file parsing and CRC32 validation
 */

const HEADER_SIZE = 52;
const MAGIC = [0x53, 0x49, 0x46, 0x54]; // "SIFT"
const FOOTER_MAGIC = [0x54, 0x46, 0x49, 0x53]; // "TFIS"

// CRC32 lookup table (IEEE polynomial)
const CRC32_TABLE = new Uint32Array(256);
for (let i = 0; i < 256; i++) {
  let crc = i;
  for (let j = 0; j < 8; j++) {
    crc = crc & 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1;
  }
  CRC32_TABLE[i] = crc >>> 0;
}

export function computeCrc32(data: Uint8Array): number {
  let crc = 0xffffffff;
  for (let i = 0; i < data.length; i++) {
    crc = CRC32_TABLE[(crc ^ data[i]) & 0xff] ^ (crc >>> 8);
  }
  return (crc ^ 0xffffffff) >>> 0;
}

export interface SieveHeader {
  version: number;
  docCount: number;
  termCount: number;
  vocabLen: number;
  saLen: number;
  postingsLen: number;
  skipLen: number;
  sectionTableLen: number;
  levDfaLen: number;
  docsLen: number;
  wasmLen: number;
  dictTableLen: number;
}

export interface ParsedSieve {
  wasm: Uint8Array;
  index: Uint8Array;
}

/**
 * Parse a .sieve file and extract WASM + index sections.
 */
export function parseSieve(buffer: ArrayBuffer): ParsedSieve {
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);

  // Validate magic
  for (let i = 0; i < 4; i++) {
    if (view.getUint8(i) !== MAGIC[i]) {
      throw new Error("Invalid .sieve file");
    }
  }

  const version = view.getUint8(4);
  if (version < 7) {
    throw new Error(`Sieve v${version} does not embed WASM, need v7+`);
  }

  // Parse header (little-endian u32 fields)
  const header: SieveHeader = {
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
  const wasmOffset =
    HEADER_SIZE +
    header.vocabLen +
    header.saLen +
    header.postingsLen +
    header.skipLen +
    header.sectionTableLen +
    header.levDfaLen +
    header.docsLen;
  const dictTableOffset = wasmOffset + header.wasmLen;

  // Extract WASM binary
  const wasm = bytes.slice(wasmOffset, wasmOffset + header.wasmLen);

  // Build index: header + sections before WASM + dict_tables + new footer
  const contentLen = wasmOffset + header.dictTableLen;
  const index = new Uint8Array(contentLen + 8); // +8 for footer

  // Copy header + all sections up to WASM
  index.set(bytes.slice(0, wasmOffset), 0);

  // Copy dict_tables (placed right after docs, since we're skipping WASM)
  index.set(
    bytes.slice(dictTableOffset, dictTableOffset + header.dictTableLen),
    wasmOffset
  );

  // Zero out wasmLen in copied header (WASM is extracted separately)
  const indexView = new DataView(index.buffer);
  indexView.setUint32(42, 0, true);

  // Compute new CRC32 over the modified content
  const newCrc32 = computeCrc32(index.subarray(0, contentLen));
  indexView.setUint32(contentLen, newCrc32, true);

  // Write footer magic
  for (let i = 0; i < 4; i++) {
    index[contentLen + 4 + i] = FOOTER_MAGIC[i];
  }

  return { wasm, index };
}
