/**
 * WASM instance state management
 *
 * Each WasmState encapsulates a WASM instance with its own:
 * - Heap for JS object references
 * - Memory views (DataView, Uint8Array)
 * - Text encoder/decoder
 *
 * This enables multiple WASM versions to coexist on the same page.
 */

import { createImports } from "./imports";

export interface WasmExports {
  memory: WebAssembly.Memory;
  __wbindgen_add_to_stack_pointer: (delta: number) => number;
  __wbindgen_export: (len: number, align: number) => number;
  __wbindgen_export2: (
    ptr: number,
    oldLen: number,
    newLen: number,
    align: number
  ) => number;
  __wbindgen_export3: (idx: number) => void;
  __wbindgen_export4: (ptr: number, len: number, align: number) => void;
  __wbg_sievesearcher_free: (ptr: number, flag: number) => void;
  sievesearcher_new: (retptr: number, ptr: number, len: number) => void;
  sievesearcher_search: (
    retptr: number,
    ptr: number,
    queryPtr: number,
    queryLen: number,
    limit: number
  ) => void;
  sievesearcher_has_docs: (ptr: number) => number;
  sievesearcher_doc_count: (ptr: number) => number;
  sievesearcher_vocab_size: (ptr: number) => number;
  sievesearcher_has_vocabulary: (ptr: number) => number;
}

export class WasmState {
  wasm: WasmExports;
  heap: unknown[];
  heapNext: number;
  private cachedDataView: DataView | null = null;
  private cachedUint8Array: Uint8Array | null = null;
  private textDecoder: TextDecoder;
  private textEncoder: TextEncoder;
  private numBytesDecoded = 0;
  vectorLen = 0;

  constructor(wasmBytes: Uint8Array) {
    this.heap = new Array(128).fill(undefined);
    this.heap.push(undefined, null, true, false);
    this.heapNext = this.heap.length;
    this.textDecoder = new TextDecoder("utf-8", {
      ignoreBOM: true,
      fatal: true,
    });
    this.textDecoder.decode();
    this.textEncoder = new TextEncoder();

    const imports = createImports(this);
    const module = new WebAssembly.Module(wasmBytes as BufferSource);
    const instance = new WebAssembly.Instance(module, imports);
    this.wasm = instance.exports as unknown as WasmExports;
  }

  // Heap management
  addHeapObject(obj: unknown): number {
    if (this.heapNext === this.heap.length) {
      this.heap.push(this.heap.length + 1);
    }
    const idx = this.heapNext;
    this.heapNext = this.heap[idx] as number;
    this.heap[idx] = obj;
    return idx;
  }

  dropObject(idx: number): void {
    if (idx < 132) return;
    this.heap[idx] = this.heapNext;
    this.heapNext = idx;
  }

  getObject(idx: number): unknown {
    return this.heap[idx];
  }

  takeObject(idx: number): unknown {
    const ret = this.getObject(idx);
    this.dropObject(idx);
    return ret;
  }

  // Memory views
  getDataView(): DataView {
    if (
      this.cachedDataView === null ||
      (this.cachedDataView.buffer as ArrayBuffer & { detached?: boolean }).detached === true ||
      ((this.cachedDataView.buffer as ArrayBuffer & { detached?: boolean }).detached === undefined &&
        this.cachedDataView.buffer !== this.wasm.memory.buffer)
    ) {
      this.cachedDataView = new DataView(this.wasm.memory.buffer);
    }
    return this.cachedDataView;
  }

  getUint8Array(): Uint8Array {
    if (
      this.cachedUint8Array === null ||
      this.cachedUint8Array.byteLength === 0
    ) {
      this.cachedUint8Array = new Uint8Array(this.wasm.memory.buffer);
    }
    return this.cachedUint8Array;
  }

  // String encoding/decoding
  getStringFromWasm(ptr: number, len: number): string {
    ptr = ptr >>> 0;
    this.numBytesDecoded += len;
    // Safari workaround for TextDecoder memory leak
    if (this.numBytesDecoded >= 2146435072) {
      this.textDecoder = new TextDecoder("utf-8", {
        ignoreBOM: true,
        fatal: true,
      });
      this.textDecoder.decode();
      this.numBytesDecoded = len;
    }
    return this.textDecoder.decode(this.getUint8Array().subarray(ptr, ptr + len));
  }

  passStringToWasm(
    arg: string,
    malloc: (len: number, align: number) => number,
    realloc?: (ptr: number, oldLen: number, newLen: number, align: number) => number
  ): number {
    if (realloc === undefined) {
      const buf = this.textEncoder.encode(arg);
      const ptr = malloc(buf.length, 1) >>> 0;
      this.getUint8Array().subarray(ptr, ptr + buf.length).set(buf);
      this.vectorLen = buf.length;
      return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;
    const mem = this.getUint8Array();
    let offset = 0;

    for (; offset < len; offset++) {
      const code = arg.charCodeAt(offset);
      if (code > 0x7f) break;
      mem[ptr + offset] = code;
    }

    if (offset !== len) {
      if (offset !== 0) arg = arg.slice(offset);
      ptr = realloc(ptr, len, (len = offset + arg.length * 3), 1) >>> 0;
      const view = this.getUint8Array().subarray(ptr + offset, ptr + len);
      const ret = this.textEncoder.encodeInto(arg, view);
      offset += ret.written!;
      ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    this.vectorLen = offset;
    return ptr;
  }

  passArrayToWasm(
    arg: Uint8Array,
    malloc: (len: number, align: number) => number
  ): number {
    const ptr = malloc(arg.length, 1) >>> 0;
    this.getUint8Array().set(arg, ptr);
    this.vectorLen = arg.length;
    return ptr;
  }

  getArrayFromWasm(ptr: number, len: number): Uint8Array {
    ptr = ptr >>> 0;
    return this.getUint8Array().subarray(ptr, ptr + len);
  }

  // Utilities
  isLikeNone(x: unknown): boolean {
    return x === undefined || x === null;
  }

  debugString(val: unknown): string {
    const type = typeof val;
    if (type === "number" || type === "boolean" || val == null) return `${val}`;
    if (type === "string") return `"${val}"`;
    if (type === "symbol")
      return (val as symbol).description
        ? `Symbol(${(val as symbol).description})`
        : "Symbol";
    if (type === "function")
      return (val as Function).name
        ? `Function(${(val as Function).name})`
        : "Function";
    if (Array.isArray(val)) {
      let debug = "[";
      if (val.length > 0) debug += this.debugString(val[0]);
      for (let i = 1; i < val.length; i++) debug += ", " + this.debugString(val[i]);
      return debug + "]";
    }
    const builtInMatches = /\[object ([^\]]+)\]/.exec(toString.call(val));
    const className =
      builtInMatches && builtInMatches.length > 1
        ? builtInMatches[1]
        : toString.call(val);
    if (className === "Object") {
      try {
        return "Object(" + JSON.stringify(val) + ")";
      } catch {
        return "Object";
      }
    }
    if (val instanceof Error)
      return `${val.name}: ${val.message}\n${val.stack}`;
    return className;
  }

  handleError(f: Function, args: unknown[]): unknown {
    try {
      return f.apply(this, args);
    } catch (e) {
      this.wasm.__wbindgen_export3(this.addHeapObject(e));
    }
  }
}
