/**
 * Sieve Loader - Self-contained search loader
 *
 * AUTO-GENERATED FILE - Do not edit directly!
 * Source: src/build/loader/*.ts
 * Rebuild: cd src/build/loader && bun run build.ts
 *
 * Usage:
 *   import { loadSieve, SieveSearcher } from './sieve-loader.js';
 *   const searcher = await loadSieve('./index.sieve');
 *   const results = searcher.search('query');
 */

// imports.ts
function createImports(state) {
  const imports = { wbg: {} };
  const wbg = imports.wbg;
  wbg.__wbg_Error_52673b7de5a0ca89 = (arg0, arg1) => state.addHeapObject(Error(state.getStringFromWasm(arg0, arg1)));
  wbg.__wbg_Number_2d1dcfcf4ec51736 = (arg0) => Number(state.getObject(arg0));
  wbg.__wbg_String_8f0eb39a4a4c2f66 = (arg0, arg1) => {
    const ret = String(state.getObject(arg1));
    const ptr1 = state.passStringToWasm(ret, state.wasm.__wbindgen_export, state.wasm.__wbindgen_export2);
    state.getDataView().setInt32(arg0 + 4, state.vectorLen, true);
    state.getDataView().setInt32(arg0, ptr1, true);
  };
  wbg.__wbg___wbindgen_bigint_get_as_i64_6e32f5e6aff02e1d = (arg0, arg1) => {
    const v = state.getObject(arg1);
    const ret = typeof v === "bigint" ? v : undefined;
    state.getDataView().setBigInt64(arg0 + 8, state.isLikeNone(ret) ? BigInt(0) : ret, true);
    state.getDataView().setInt32(arg0, state.isLikeNone(ret) ? 0 : 1, true);
  };
  wbg.__wbg___wbindgen_boolean_get_dea25b33882b895b = (arg0) => {
    const v = state.getObject(arg0);
    const ret = typeof v === "boolean" ? v : undefined;
    return state.isLikeNone(ret) ? 16777215 : ret ? 1 : 0;
  };
  wbg.__wbg___wbindgen_debug_string_adfb662ae34724b6 = (arg0, arg1) => {
    const ret = state.debugString(state.getObject(arg1));
    const ptr1 = state.passStringToWasm(ret, state.wasm.__wbindgen_export, state.wasm.__wbindgen_export2);
    state.getDataView().setInt32(arg0 + 4, state.vectorLen, true);
    state.getDataView().setInt32(arg0, ptr1, true);
  };
  wbg.__wbg___wbindgen_in_0d3e1e8f0c669317 = (arg0, arg1) => (state.getObject(arg0) in state.getObject(arg1));
  wbg.__wbg___wbindgen_is_bigint_0e1a2e3f55cfae27 = (arg0) => typeof state.getObject(arg0) === "bigint";
  wbg.__wbg___wbindgen_is_function_8d400b8b1af978cd = (arg0) => typeof state.getObject(arg0) === "function";
  wbg.__wbg___wbindgen_is_object_ce774f3490692386 = (arg0) => {
    const val = state.getObject(arg0);
    return typeof val === "object" && val !== null;
  };
  wbg.__wbg___wbindgen_is_undefined_f6b95eab589e0269 = (arg0) => state.getObject(arg0) === undefined;
  wbg.__wbg___wbindgen_jsval_eq_b6101cc9cef1fe36 = (arg0, arg1) => state.getObject(arg0) === state.getObject(arg1);
  wbg.__wbg___wbindgen_jsval_loose_eq_766057600fdd1b0d = (arg0, arg1) => state.getObject(arg0) == state.getObject(arg1);
  wbg.__wbg___wbindgen_number_get_9619185a74197f95 = (arg0, arg1) => {
    const obj = state.getObject(arg1);
    const ret = typeof obj === "number" ? obj : undefined;
    state.getDataView().setFloat64(arg0 + 8, state.isLikeNone(ret) ? 0 : ret, true);
    state.getDataView().setInt32(arg0, state.isLikeNone(ret) ? 0 : 1, true);
  };
  wbg.__wbg___wbindgen_string_get_a2a31e16edf96e42 = (arg0, arg1) => {
    const obj = state.getObject(arg1);
    const ret = typeof obj === "string" ? obj : undefined;
    const ptr1 = state.isLikeNone(ret) ? 0 : state.passStringToWasm(ret, state.wasm.__wbindgen_export, state.wasm.__wbindgen_export2);
    state.getDataView().setInt32(arg0 + 4, state.vectorLen, true);
    state.getDataView().setInt32(arg0, ptr1, true);
  };
  wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = (arg0, arg1) => {
    throw new Error(state.getStringFromWasm(arg0, arg1));
  };
  wbg.__wbg_call_abb4ff46ce38be40 = function() {
    return state.handleError(function(arg0, arg1) {
      return state.addHeapObject(state.getObject(arg0).call(state.getObject(arg1)));
    }, Array.from(arguments));
  };
  wbg.__wbg_done_62ea16af4ce34b24 = (arg0) => state.getObject(arg0).done;
  wbg.__wbg_get_6b7bd52aca3f9671 = (arg0, arg1) => state.addHeapObject(state.getObject(arg0)[arg1 >>> 0]);
  wbg.__wbg_get_af9dab7e9603ea93 = function() {
    return state.handleError(function(arg0, arg1) {
      return state.addHeapObject(Reflect.get(state.getObject(arg0), state.getObject(arg1)));
    }, Array.from(arguments));
  };
  wbg.__wbg_get_with_ref_key_1dc361bd10053bfe = (arg0, arg1) => state.addHeapObject(state.getObject(arg0)[state.getObject(arg1)]);
  wbg.__wbg_instanceof_ArrayBuffer_f3320d2419cd0355 = (arg0) => {
    try {
      return state.getObject(arg0) instanceof ArrayBuffer;
    } catch {
      return false;
    }
  };
  wbg.__wbg_instanceof_Uint8Array_da54ccc9d3e09434 = (arg0) => {
    try {
      return state.getObject(arg0) instanceof Uint8Array;
    } catch {
      return false;
    }
  };
  wbg.__wbg_isArray_51fd9e6422c0a395 = (arg0) => Array.isArray(state.getObject(arg0));
  wbg.__wbg_isSafeInteger_ae7d3f054d55fa16 = (arg0) => Number.isSafeInteger(state.getObject(arg0));
  wbg.__wbg_iterator_27b7c8b35ab3e86b = () => state.addHeapObject(Symbol.iterator);
  wbg.__wbg_length_22ac23eaec9d8053 = (arg0) => state.getObject(arg0).length;
  wbg.__wbg_length_d45040a40c570362 = (arg0) => state.getObject(arg0).length;
  wbg.__wbg_new_1ba21ce319a06297 = () => state.addHeapObject({});
  wbg.__wbg_new_25f239778d6112b9 = () => state.addHeapObject([]);
  wbg.__wbg_new_6421f6084cc5bc5a = (arg0) => state.addHeapObject(new Uint8Array(state.getObject(arg0)));
  wbg.__wbg_next_138a17bbf04e926c = (arg0) => state.addHeapObject(state.getObject(arg0).next);
  wbg.__wbg_next_3cfe5c0fe2a4cc53 = function() {
    return state.handleError(function(arg0) {
      return state.addHeapObject(state.getObject(arg0).next());
    }, Array.from(arguments));
  };
  wbg.__wbg_prototypesetcall_dfe9b766cdc1f1fd = (arg0, arg1, arg2) => {
    Uint8Array.prototype.set.call(state.getArrayFromWasm(arg0, arg1), state.getObject(arg2));
  };
  wbg.__wbg_set_3f1d0b984ed272ed = (arg0, arg1, arg2) => {
    state.getObject(arg0)[state.takeObject(arg1)] = state.takeObject(arg2);
  };
  wbg.__wbg_set_7df433eea03a5c14 = (arg0, arg1, arg2) => {
    state.getObject(arg0)[arg1 >>> 0] = state.takeObject(arg2);
  };
  wbg.__wbg_value_57b7b035e117f7ee = (arg0) => state.addHeapObject(state.getObject(arg0).value);
  wbg.__wbindgen_cast_2241b6af4c4b2941 = (arg0, arg1) => state.addHeapObject(state.getStringFromWasm(arg0, arg1));
  wbg.__wbindgen_cast_4625c577ab2ec9ee = (arg0) => state.addHeapObject(BigInt.asUintN(64, arg0));
  wbg.__wbindgen_cast_d6cd19b81560fd6e = (arg0) => state.addHeapObject(arg0);
  wbg.__wbindgen_object_clone_ref = (arg0) => state.addHeapObject(state.getObject(arg0));
  wbg.__wbindgen_object_drop_ref = (arg0) => {
    state.takeObject(arg0);
  };
  wbg.__wbindgen_object_is_undefined = (arg0) => state.getObject(arg0) === undefined;
  return imports;
}

// wasm-state.ts
class WasmState {
  wasm;
  heap;
  heapNext;
  cachedDataView = null;
  cachedUint8Array = null;
  textDecoder;
  textEncoder;
  numBytesDecoded = 0;
  vectorLen = 0;
  constructor(wasmBytes) {
    this.heap = new Array(128).fill(undefined);
    this.heap.push(undefined, null, true, false);
    this.heapNext = this.heap.length;
    this.textDecoder = new TextDecoder("utf-8", {
      ignoreBOM: true,
      fatal: true
    });
    this.textDecoder.decode();
    this.textEncoder = new TextEncoder;
    const imports = createImports(this);
    const module = new WebAssembly.Module(wasmBytes);
    const instance = new WebAssembly.Instance(module, imports);
    this.wasm = instance.exports;
  }
  addHeapObject(obj) {
    if (this.heapNext === this.heap.length) {
      this.heap.push(this.heap.length + 1);
    }
    const idx = this.heapNext;
    this.heapNext = this.heap[idx];
    this.heap[idx] = obj;
    return idx;
  }
  dropObject(idx) {
    if (idx < 132)
      return;
    this.heap[idx] = this.heapNext;
    this.heapNext = idx;
  }
  getObject(idx) {
    return this.heap[idx];
  }
  takeObject(idx) {
    const ret = this.getObject(idx);
    this.dropObject(idx);
    return ret;
  }
  getDataView() {
    if (this.cachedDataView === null || this.cachedDataView.buffer.detached === true || this.cachedDataView.buffer.detached === undefined && this.cachedDataView.buffer !== this.wasm.memory.buffer) {
      this.cachedDataView = new DataView(this.wasm.memory.buffer);
    }
    return this.cachedDataView;
  }
  getUint8Array() {
    if (this.cachedUint8Array === null || this.cachedUint8Array.byteLength === 0) {
      this.cachedUint8Array = new Uint8Array(this.wasm.memory.buffer);
    }
    return this.cachedUint8Array;
  }
  getStringFromWasm(ptr, len) {
    ptr = ptr >>> 0;
    this.numBytesDecoded += len;
    if (this.numBytesDecoded >= 2146435072) {
      this.textDecoder = new TextDecoder("utf-8", {
        ignoreBOM: true,
        fatal: true
      });
      this.textDecoder.decode();
      this.numBytesDecoded = len;
    }
    return this.textDecoder.decode(this.getUint8Array().subarray(ptr, ptr + len));
  }
  passStringToWasm(arg, malloc, realloc) {
    if (realloc === undefined) {
      const buf = this.textEncoder.encode(arg);
      const ptr2 = malloc(buf.length, 1) >>> 0;
      this.getUint8Array().subarray(ptr2, ptr2 + buf.length).set(buf);
      this.vectorLen = buf.length;
      return ptr2;
    }
    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;
    const mem = this.getUint8Array();
    let offset = 0;
    for (;offset < len; offset++) {
      const code = arg.charCodeAt(offset);
      if (code > 127)
        break;
      mem[ptr + offset] = code;
    }
    if (offset !== len) {
      if (offset !== 0)
        arg = arg.slice(offset);
      ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
      const view = this.getUint8Array().subarray(ptr + offset, ptr + len);
      const ret = this.textEncoder.encodeInto(arg, view);
      offset += ret.written;
      ptr = realloc(ptr, len, offset, 1) >>> 0;
    }
    this.vectorLen = offset;
    return ptr;
  }
  passArrayToWasm(arg, malloc) {
    const ptr = malloc(arg.length, 1) >>> 0;
    this.getUint8Array().set(arg, ptr);
    this.vectorLen = arg.length;
    return ptr;
  }
  getArrayFromWasm(ptr, len) {
    ptr = ptr >>> 0;
    return this.getUint8Array().subarray(ptr, ptr + len);
  }
  isLikeNone(x) {
    return x === undefined || x === null;
  }
  debugString(val) {
    const type = typeof val;
    if (type === "number" || type === "boolean" || val == null)
      return `${val}`;
    if (type === "string")
      return `"${val}"`;
    if (type === "symbol")
      return val.description ? `Symbol(${val.description})` : "Symbol";
    if (type === "function")
      return val.name ? `Function(${val.name})` : "Function";
    if (Array.isArray(val)) {
      let debug = "[";
      if (val.length > 0)
        debug += this.debugString(val[0]);
      for (let i = 1;i < val.length; i++)
        debug += ", " + this.debugString(val[i]);
      return debug + "]";
    }
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    const className = builtInMatches && builtInMatches.length > 1 ? builtInMatches[1] : toString.call(val);
    if (className === "Object") {
      try {
        return "Object(" + JSON.stringify(val) + ")";
      } catch {
        return "Object";
      }
    }
    if (val instanceof Error)
      return `${val.name}: ${val.message}
${val.stack}`;
    return className;
  }
  handleError(f, args) {
    try {
      return f.apply(this, args);
    } catch (e) {
      this.wasm.__wbindgen_export3(this.addHeapObject(e));
    }
  }
}

// searcher.ts
var SieveSearcherFinalization = typeof FinalizationRegistry === "undefined" ? { register: () => {}, unregister: () => {} } : new FinalizationRegistry((prevent) => prevent());

class SieveSearcher {
  #state;
  #ptr;
  constructor(state, indexBytes) {
    this.#state = state;
    const wasm = state.wasm;
    const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
    try {
      const ptr0 = state.passArrayToWasm(indexBytes, wasm.__wbindgen_export);
      const len0 = state.vectorLen;
      wasm.sievesearcher_new(retptr, ptr0, len0);
      const r0 = state.getDataView().getInt32(retptr + 0, true);
      const r1 = state.getDataView().getInt32(retptr + 4, true);
      const r2 = state.getDataView().getInt32(retptr + 8, true);
      if (r2) {
        throw state.takeObject(r1);
      }
      this.#ptr = r0 >>> 0;
      SieveSearcherFinalization.register(this, () => this.free(), this);
    } finally {
      wasm.__wbindgen_add_to_stack_pointer(16);
    }
  }
  free() {
    if (this.#ptr === 0)
      return;
    const ptr = this.#ptr;
    this.#ptr = 0;
    SieveSearcherFinalization.unregister(this);
    this.#state.wasm.__wbg_sievesearcher_free(ptr, 0);
  }
  search(query, limit) {
    const state = this.#state;
    const wasm = state.wasm;
    const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
    try {
      const ptr0 = state.passStringToWasm(query, wasm.__wbindgen_export, wasm.__wbindgen_export2);
      const len0 = state.vectorLen;
      wasm.sievesearcher_search(retptr, this.#ptr, ptr0, len0, state.isLikeNone(limit) ? 4294967297 : limit >>> 0);
      const r0 = state.getDataView().getInt32(retptr + 0, true);
      const r1 = state.getDataView().getInt32(retptr + 4, true);
      const r2 = state.getDataView().getInt32(retptr + 8, true);
      if (r2) {
        throw state.takeObject(r1);
      }
      return state.takeObject(r0);
    } finally {
      wasm.__wbindgen_add_to_stack_pointer(16);
    }
  }
  has_docs() {
    return this.#state.wasm.sievesearcher_has_docs(this.#ptr) !== 0;
  }
  doc_count() {
    return this.#state.wasm.sievesearcher_doc_count(this.#ptr) >>> 0;
  }
  vocab_size() {
    return this.#state.wasm.sievesearcher_vocab_size(this.#ptr) >>> 0;
  }
  has_vocabulary() {
    return this.#state.wasm.sievesearcher_has_vocabulary(this.#ptr) !== 0;
  }
}

// parser.ts
var HEADER_SIZE = 52;
var MAGIC = [83, 73, 70, 84];
var FOOTER_MAGIC = [84, 70, 73, 83];
var CRC32_TABLE = new Uint32Array(256);
for (let i = 0;i < 256; i++) {
  let crc = i;
  for (let j = 0;j < 8; j++) {
    crc = crc & 1 ? 3988292384 ^ crc >>> 1 : crc >>> 1;
  }
  CRC32_TABLE[i] = crc >>> 0;
}
function computeCrc32(data) {
  let crc = 4294967295;
  for (let i = 0;i < data.length; i++) {
    crc = CRC32_TABLE[(crc ^ data[i]) & 255] ^ crc >>> 8;
  }
  return (crc ^ 4294967295) >>> 0;
}
function parseSieve(buffer) {
  const bytes = new Uint8Array(buffer);
  const view = new DataView(buffer);
  for (let i = 0;i < 4; i++) {
    if (view.getUint8(i) !== MAGIC[i]) {
      throw new Error("Invalid .sieve file");
    }
  }
  const version = view.getUint8(4);
  if (version < 7) {
    throw new Error(`Sieve v${version} does not embed WASM, need v7+`);
  }
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
    dictTableLen: view.getUint32(46, true)
  };
  const wasmOffset = HEADER_SIZE + header.vocabLen + header.saLen + header.postingsLen + header.skipLen + header.sectionTableLen + header.levDfaLen + header.docsLen;
  const dictTableOffset = wasmOffset + header.wasmLen;
  const wasm = bytes.slice(wasmOffset, wasmOffset + header.wasmLen);
  const contentLen = wasmOffset + header.dictTableLen;
  const index = new Uint8Array(contentLen + 8);
  index.set(bytes.slice(0, wasmOffset), 0);
  index.set(bytes.slice(dictTableOffset, dictTableOffset + header.dictTableLen), wasmOffset);
  const indexView = new DataView(index.buffer);
  indexView.setUint32(42, 0, true);
  const newCrc32 = computeCrc32(index.subarray(0, contentLen));
  indexView.setUint32(contentLen, newCrc32, true);
  for (let i = 0;i < 4; i++) {
    index[contentLen + 4 + i] = FOOTER_MAGIC[i];
  }
  return { wasm, index };
}

// index.ts
var instances = new Map;
function hashBytes(bytes) {
  let hash = 2166136261;
  for (let i = 0;i < bytes.length; i += 1024) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 16777619);
  }
  return hash >>> 0;
}
function getOrCreateInstance(wasmBytes) {
  const hash = hashBytes(wasmBytes);
  let state = instances.get(hash);
  if (!state) {
    state = new WasmState(wasmBytes);
    instances.set(hash, state);
  }
  return state;
}
async function loadSieve(url) {
  const response = await fetch(url);
  if (!response.ok) {
    throw new Error(`Failed to fetch ${url}: ${response.status}`);
  }
  const buffer = await response.arrayBuffer();
  return loadSieveSync(buffer);
}
function loadSieveSync(buffer) {
  const { wasm: wasmBytes, index } = parseSieve(buffer);
  const state = getOrCreateInstance(wasmBytes);
  return new SieveSearcher(state, index);
}
export {
  loadSieveSync,
  loadSieve,
  SieveSearcher
};
