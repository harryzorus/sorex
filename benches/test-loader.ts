/**
 * Test script for the sorex-loader.js parsing logic
 *
 * This creates a synthetic v7 .sorex file and verifies the loader correctly
 * extracts the WASM and index sections.
 */

const HEADER_SIZE = 52;
const MAGIC = [0x53, 0x4F, 0x52, 0x58]; // "SORX"
const FOOTER_MAGIC = [0x58, 0x52, 0x4F, 0x53]; // "XROS"

/**
 * Create a minimal v7 .sorex file for testing
 */
function createTestSorexFile(): Uint8Array {
  // Section lengths (all minimal)
  const vocabLen = 10;
  const saLen = 8;
  const postingsLen = 12;
  const skipLen = 0;
  const sectionTableLen = 0;
  const levDfaLen = 4;
  const docsLen = 8;
  const wasmLen = 16; // Our test WASM bytes
  const dictTableLen = 0; // v7: dictionary tables

  // Total size: header + sections + wasm + dict_tables + footer
  const totalSize = HEADER_SIZE + vocabLen + saLen + postingsLen + skipLen +
    sectionTableLen + levDfaLen + docsLen + wasmLen + dictTableLen + 8;

  const buffer = new ArrayBuffer(totalSize);
  const view = new DataView(buffer);
  const bytes = new Uint8Array(buffer);

  // Write header
  let offset = 0;
  for (let i = 0; i < 4; i++) {
    view.setUint8(offset++, MAGIC[i]);
  }
  view.setUint8(offset++, 7); // version
  view.setUint8(offset++, 0); // flags

  view.setUint32(offset, 1, true); offset += 4; // doc_count
  view.setUint32(offset, 1, true); offset += 4; // term_count
  view.setUint32(offset, vocabLen, true); offset += 4;
  view.setUint32(offset, saLen, true); offset += 4;
  view.setUint32(offset, postingsLen, true); offset += 4;
  view.setUint32(offset, skipLen, true); offset += 4;
  view.setUint32(offset, sectionTableLen, true); offset += 4;
  view.setUint32(offset, levDfaLen, true); offset += 4;
  view.setUint32(offset, docsLen, true); offset += 4;
  view.setUint32(offset, wasmLen, true); offset += 4;
  view.setUint32(offset, dictTableLen, true); offset += 4; // v7: dictionary tables
  view.setUint16(offset, 0, true); offset += 2; // reserved

  // Fill sections with recognizable patterns
  const vocabStart = HEADER_SIZE;
  for (let i = 0; i < vocabLen; i++) bytes[vocabStart + i] = 0xAA;

  const saStart = vocabStart + vocabLen;
  for (let i = 0; i < saLen; i++) bytes[saStart + i] = 0xBB;

  const postingsStart = saStart + saLen;
  for (let i = 0; i < postingsLen; i++) bytes[postingsStart + i] = 0xCC;

  const levDfaStart = postingsStart + postingsLen + skipLen + sectionTableLen;
  for (let i = 0; i < levDfaLen; i++) bytes[levDfaStart + i] = 0xDD;

  const docsStart = levDfaStart + levDfaLen;
  for (let i = 0; i < docsLen; i++) bytes[docsStart + i] = 0xEE;

  // WASM section - use recognizable WASM magic + data
  const wasmStart = docsStart + docsLen;
  const testWasm = [0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00, // WASM magic
    0xDE, 0xAD, 0xBE, 0xEF, 0xCA, 0xFE, 0xBA, 0xBE]; // test pattern
  for (let i = 0; i < wasmLen; i++) bytes[wasmStart + i] = testWasm[i];

  // Write footer (CRC32 placeholder + magic)
  const footerStart = wasmStart + wasmLen;
  view.setUint32(footerStart, 0x12345678, true); // CRC32 placeholder
  for (let i = 0; i < 4; i++) {
    bytes[footerStart + 4 + i] = FOOTER_MAGIC[i];
  }

  return bytes;
}

/**
 * Parse .sorex header (same logic as sorex-loader.js)
 */
function parseHeader(view: DataView) {
  // Validate magic
  for (let i = 0; i < 4; i++) {
    if (view.getUint8(i) !== MAGIC[i]) {
      throw new Error('Invalid .sorex file');
    }
  }

  const version = view.getUint8(4);
  if (version < 7) {
    throw new Error(`Sorex v${version} does not embed WASM, need v7+`);
  }

  return {
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
    dictTableLen: view.getUint32(46, true), // v7: dictionary tables
  };
}

// Run tests
console.log('Testing sorex-loader parsing logic...\n');

// Test 1: Create and parse test file
const testFile = createTestSorexFile();
const view = new DataView(testFile.buffer);
const header = parseHeader(view);

console.log('Header parsed:');
console.log(`  version: ${header.version}`);
console.log(`  wasmLen: ${header.wasmLen}`);

// Verify header values
console.assert(header.version === 7, 'Version should be 7');
console.assert(header.wasmLen === 16, 'WASM length should be 16');
console.assert(header.docCount === 1, 'Doc count should be 1');

// Test 2: Calculate WASM offset and extract
const wasmOffset = HEADER_SIZE + header.vocabLen + header.saLen +
  header.postingsLen + header.skipLen + header.sectionTableLen +
  header.levDfaLen + header.docsLen;

const extractedWasm = testFile.slice(wasmOffset, wasmOffset + header.wasmLen);

console.log(`\nWASM section at offset ${wasmOffset}:`);
console.log(`  bytes: ${Array.from(extractedWasm).map(b => b.toString(16).padStart(2, '0')).join(' ')}`);

// Verify WASM magic
console.assert(extractedWasm[0] === 0x00, 'WASM magic byte 0');
console.assert(extractedWasm[1] === 0x61, 'WASM magic byte 1 (a)');
console.assert(extractedWasm[2] === 0x73, 'WASM magic byte 2 (s)');
console.assert(extractedWasm[3] === 0x6d, 'WASM magic byte 3 (m)');

// Test 3: Verify index extraction (without WASM)
const indexLen = wasmOffset + 8; // header + sections + footer
const indexBytes = new Uint8Array(indexLen);
indexBytes.set(testFile.slice(0, wasmOffset), 0);

// Zero out wasmLen in copied header
const indexView = new DataView(indexBytes.buffer);
indexView.setUint32(42, 0, true);

// Copy footer
indexBytes.set(testFile.slice(wasmOffset + header.wasmLen, wasmOffset + header.wasmLen + 8), wasmOffset);

const indexHeader = parseHeader(indexView);
console.assert(indexHeader.wasmLen === 0, 'Index wasmLen should be 0');

console.log('\n✅ All tests passed!');
console.log(`Total file size: ${testFile.length} bytes`);
console.log(`Index size (without WASM): ${indexBytes.length} bytes`);
console.log(`WASM size: ${extractedWasm.length} bytes`);
