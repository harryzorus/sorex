/**
 * Sorex Loader - Self-contained search loader
 *
 * AUTO-GENERATED FILE - Do not edit directly!
 * Source: target/pkg/sorex.js (wasm-pack output)
 * Generator: scripts/build-loader.ts
 *
 * Usage:
 *   import { loadSorex, loadSorexSync, SorexSearcher } from './sorex-loader.js';
 *   const searcher = await loadSorex('./index.sorex');
 *   const results = searcher.searchSync('query');
 */


// .sorex file parser
// v12 layout order: HEADER | WASM | VOCAB | DICT | POSTINGS | SA | DOCS | SECTION_TABLE | SKIP | LEV_DFA | FOOTER
// See src/binary/header.rs SectionOffsets for canonical definition
const HEADER_SIZE = 52;
const MAGIC = [83, 79, 82, 88]; // "SORX"
const FOOTER_MAGIC = [88, 82, 79, 83]; // "XROS"

const CRC32_TABLE = new Uint32Array(256);
for (let i = 0; i < 256; i++) {
    let crc = i;
    for (let j = 0; j < 8; j++) {
        crc = crc & 1 ? 0xEDB88320 ^ (crc >>> 1) : crc >>> 1;
    }
    CRC32_TABLE[i] = crc >>> 0;
}

function computeCrc32(data) {
    let crc = 0xFFFFFFFF;
    for (let i = 0; i < data.length; i++) {
        crc = CRC32_TABLE[(crc ^ data[i]) & 0xFF] ^ (crc >>> 8);
    }
    return (crc ^ 0xFFFFFFFF) >>> 0;
}

function parseSorex(buffer) {
    const bytes = new Uint8Array(buffer);
    const view = new DataView(buffer);

    // Validate magic
    for (let i = 0; i < 4; i++) {
        if (view.getUint8(i) !== MAGIC[i]) {
            throw new Error("Invalid .sorex file: bad magic");
        }
    }

    const version = view.getUint8(4);
    if (version !== 12) {
        throw new Error(`Sorex v${version} not supported, need v12`);
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

    // Calculate index sections length (everything except WASM)
    const indexSectionsLen = vocabLen + saLen + postingsLen + skipLen +
        sectionTableLen + levDfaLen + docsLen + dictTableLen;

    // v12: WASM is immediately after header
    const wasmOffset = HEADER_SIZE;
    const sectionsOffset = HEADER_SIZE + wasmLen;

    // Extract WASM bytes
    const wasm = bytes.slice(wasmOffset, wasmOffset + wasmLen);

    // Reconstruct index (header + sections, no WASM)
    const contentLen = HEADER_SIZE + indexSectionsLen;
    const index = new Uint8Array(contentLen + 8);

    // Copy header
    index.set(bytes.slice(0, HEADER_SIZE), 0);

    // Set wasm_len to 0
    const indexView = new DataView(index.buffer);
    indexView.setUint32(42, 0, true);

    // Copy index sections (after WASM in v12)
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


let wasm;

function addToExternrefTable0(obj) {
    const idx = wasm.__externref_table_alloc();
    wasm.__wbindgen_externrefs.set(idx, obj);
    return idx;
}

let cachedDataViewMemory0 = null;
function getDataViewMemory0() {
    if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || (cachedDataViewMemory0.buffer.detached === undefined && cachedDataViewMemory0.buffer !== wasm.memory.buffer)) {
        cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
    }
    return cachedDataViewMemory0;
}

function getStringFromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return decodeText(ptr, len);
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function handleError(f, args) {
    try {
        return f.apply(this, args);
    } catch (e) {
        const idx = addToExternrefTable0(e);
        wasm.__wbindgen_exn_store(idx);
    }
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    }
}

let WASM_VECTOR_LEN = 0;

const SorexSearcherFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_sorexsearcher_free(ptr >>> 0, 1));

/**
 * WASM searcher - thin wrapper around TierSearcher.
 */
class SorexSearcher {
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        SorexSearcherFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_sorexsearcher_free(ptr, 0);
    }
    /**
     * Number of vocabulary terms.
     * @returns {number}
     */
    vocab_size() {
        const ret = wasm.sorexsearcher_vocab_size(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Three-tier search: exact → prefix → fuzzy (blocking).
     * For progressive results, use `search()` instead.
     * @param {string} query
     * @param {number | null} [limit]
     * @returns {any}
     */
    searchSync(query, limit) {
        const ptr0 = passStringToWasm0(query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sorexsearcher_searchSync(this.__wbg_ptr, ptr0, len0, isLikeNone(limit) ? 0x100000001 : (limit) >>> 0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return takeFromExternrefTable0(ret[0]);
    }
    /**
     * Create searcher from .sorex binary format.
     * @param {Uint8Array} bytes
     */
    constructor(bytes) {
        const ptr0 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sorexsearcher_new(ptr0, len0);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0] >>> 0;
        SorexSearcherFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * Progressive search with callbacks after each tier.
     *
     * - `on_update`: Called after each tier (1-3 times) with current results
     * - `on_finish`: Called once when search is complete with final results
     *
     * Each callback receives the full deduplicated result set (not deltas).
     *
     * ```js
     * searcher.search(query, 10, onUpdate, onFinish);
     * ```
     *
     * With `wasm-threads` feature, T3 fuzzy search runs in parallel via Web Workers.
     * @param {string} query
     * @param {number} limit
     * @param {Function} on_update
     * @param {Function} on_finish
     */
    search(query, limit, on_update, on_finish) {
        const ptr0 = passStringToWasm0(query, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.sorexsearcher_search(this.__wbg_ptr, ptr0, len0, limit, on_update, on_finish);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Number of documents.
     * @returns {number}
     */
    doc_count() {
        const ret = wasm.sorexsearcher_doc_count(this.__wbg_ptr);
        return ret >>> 0;
    }
}
if (Symbol.dispose) SorexSearcher.prototype[Symbol.dispose] = SorexSearcher.prototype.free;

const EXPECTED_RESPONSE_TYPES = new Set(['basic', 'cors', 'default']);

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = module.ok && EXPECTED_RESPONSE_TYPES.has(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else {
                    throw e;
                }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }
}

function __wbg_get_imports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg_String_8f0eb39a4a4c2f66 = function(arg0, arg1) {
        const ret = String(arg1);
        const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
        getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    };
    imports.wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = function(arg0, arg1) {
        throw new Error(getStringFromWasm0(arg0, arg1));
    };
    imports.wbg.__wbg_call_3020136f7a2d6e44 = function() { return handleError(function (arg0, arg1, arg2) {
        const ret = arg0.call(arg1, arg2);
        return ret;
    }, arguments) };
    imports.wbg.__wbg_new_1ba21ce319a06297 = function() {
        const ret = new Object();
        return ret;
    };
    imports.wbg.__wbg_new_25f239778d6112b9 = function() {
        const ret = new Array();
        return ret;
    };
    imports.wbg.__wbg_set_3f1d0b984ed272ed = function(arg0, arg1, arg2) {
        arg0[arg1] = arg2;
    };
    imports.wbg.__wbg_set_7df433eea03a5c14 = function(arg0, arg1, arg2) {
        arg0[arg1 >>> 0] = arg2;
    };
    imports.wbg.__wbindgen_cast_2241b6af4c4b2941 = function(arg0, arg1) {
        // Cast intrinsic for `Ref(String) -> Externref`.
        const ret = getStringFromWasm0(arg0, arg1);
        return ret;
    };
    imports.wbg.__wbindgen_cast_d6cd19b81560fd6e = function(arg0) {
        // Cast intrinsic for `F64 -> Externref`.
        const ret = arg0;
        return ret;
    };
    imports.wbg.__wbindgen_init_externref_table = function() {
        const table = wasm.__wbindgen_externrefs;
        const offset = table.grow(4);
        table.set(0, undefined);
        table.set(offset + 0, undefined);
        table.set(offset + 1, null);
        table.set(offset + 2, true);
        table.set(offset + 3, false);
    };

    return imports;
}

function __wbg_finalize_init(instance, module) {
    wasm = instance.exports;
    __wbg_init.__wbindgen_wasm_module = module;
    cachedDataViewMemory0 = null;
    cachedUint8ArrayMemory0 = null;


    wasm.__wbindgen_start();
    return wasm;
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (typeof module !== 'undefined') {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

function __wbg_init() { throw new Error('Use initSync instead'); }




// Cached WASM module for subsequent loads
let cachedWasmModule = null;

/**
 * Load a .sorex file asynchronously.
 *
 * v12: WASM is first, so we compile it while extracting index data.
 *
 * @param {string} url - URL to the .sorex file
 * @returns {Promise<SorexSearcher>}
 */
async function loadSorex(url) {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(`Failed to fetch ${url}: ${response.status}`);
    }
    const buffer = await response.arrayBuffer();
    const bytes = new Uint8Array(buffer);
    const view = new DataView(buffer);

    // Read WASM length from header
    const wasmLen = view.getUint32(42, true);

    // v12: WASM is immediately after header
    const wasmOffset = HEADER_SIZE;

    // Extract WASM and compile
    const wasmBytes = bytes.slice(wasmOffset, wasmOffset + wasmLen);
    const wasmModule = cachedWasmModule || await WebAssembly.compile(wasmBytes);
    if (!cachedWasmModule) cachedWasmModule = wasmModule;

    // Reconstruct index bytes (strip WASM section)
    const { index } = parseSorex(buffer);

    // Instantiate WASM
    const imports = __wbg_get_imports();
    const instance = await WebAssembly.instantiate(wasmModule, imports);
    __wbg_finalize_init(instance, wasmModule);

    return new SorexSearcher(index);
}

/**
 * Load synchronously from ArrayBuffer.
 * @param {ArrayBuffer} buffer
 * @returns {SorexSearcher}
 */
function loadSorexSync(buffer) {
    const { wasm, index } = parseSorex(buffer);
    if (!cachedWasmModule) {
        initSync(wasm);
    }
    return new SorexSearcher(index);
}


// ES Module exports
export { loadSorex, loadSorexSync, parseSorex, SorexSearcher, initSync };
