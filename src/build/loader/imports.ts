/**
 * WASM imports table for wasm-bindgen
 *
 * These functions are called by the WASM module for JS interop.
 * Each function is bound to a WasmState instance for isolation.
 */

import type { WasmState } from "./wasm-state";

export function createImports(state: WasmState): WebAssembly.Imports {
  const imports: WebAssembly.Imports = { wbg: {} };
  const wbg = imports.wbg as Record<string, Function>;

  wbg.__wbg_Error_52673b7de5a0ca89 = (arg0: number, arg1: number) =>
    state.addHeapObject(Error(state.getStringFromWasm(arg0, arg1)));

  wbg.__wbg_Number_2d1dcfcf4ec51736 = (arg0: number) =>
    Number(state.getObject(arg0));

  wbg.__wbg_String_8f0eb39a4a4c2f66 = (arg0: number, arg1: number) => {
    const ret = String(state.getObject(arg1));
    const ptr1 = state.passStringToWasm(
      ret,
      state.wasm.__wbindgen_export,
      state.wasm.__wbindgen_export2
    );
    state.getDataView().setInt32(arg0 + 4, state.vectorLen, true);
    state.getDataView().setInt32(arg0, ptr1, true);
  };

  wbg.__wbg___wbindgen_bigint_get_as_i64_6e32f5e6aff02e1d = (
    arg0: number,
    arg1: number
  ) => {
    const v = state.getObject(arg1);
    const ret = typeof v === "bigint" ? v : undefined;
    state
      .getDataView()
      .setBigInt64(arg0 + 8, state.isLikeNone(ret) ? BigInt(0) : ret!, true);
    state.getDataView().setInt32(arg0, state.isLikeNone(ret) ? 0 : 1, true);
  };

  wbg.__wbg___wbindgen_boolean_get_dea25b33882b895b = (arg0: number) => {
    const v = state.getObject(arg0);
    const ret = typeof v === "boolean" ? v : undefined;
    return state.isLikeNone(ret) ? 0xffffff : ret ? 1 : 0;
  };

  wbg.__wbg___wbindgen_debug_string_adfb662ae34724b6 = (
    arg0: number,
    arg1: number
  ) => {
    const ret = state.debugString(state.getObject(arg1));
    const ptr1 = state.passStringToWasm(
      ret,
      state.wasm.__wbindgen_export,
      state.wasm.__wbindgen_export2
    );
    state.getDataView().setInt32(arg0 + 4, state.vectorLen, true);
    state.getDataView().setInt32(arg0, ptr1, true);
  };

  wbg.__wbg___wbindgen_in_0d3e1e8f0c669317 = (arg0: number, arg1: number) =>
    (state.getObject(arg0) as string | number | symbol) in (state.getObject(arg1) as object);

  wbg.__wbg___wbindgen_is_bigint_0e1a2e3f55cfae27 = (arg0: number) =>
    typeof state.getObject(arg0) === "bigint";

  wbg.__wbg___wbindgen_is_function_8d400b8b1af978cd = (arg0: number) =>
    typeof state.getObject(arg0) === "function";

  wbg.__wbg___wbindgen_is_object_ce774f3490692386 = (arg0: number) => {
    const val = state.getObject(arg0);
    return typeof val === "object" && val !== null;
  };

  wbg.__wbg___wbindgen_is_undefined_f6b95eab589e0269 = (arg0: number) =>
    state.getObject(arg0) === undefined;

  wbg.__wbg___wbindgen_jsval_eq_b6101cc9cef1fe36 = (
    arg0: number,
    arg1: number
  ) => state.getObject(arg0) === state.getObject(arg1);

  wbg.__wbg___wbindgen_jsval_loose_eq_766057600fdd1b0d = (
    arg0: number,
    arg1: number
  ) => state.getObject(arg0) == state.getObject(arg1);

  wbg.__wbg___wbindgen_number_get_9619185a74197f95 = (
    arg0: number,
    arg1: number
  ) => {
    const obj = state.getObject(arg1);
    const ret = typeof obj === "number" ? obj : undefined;
    state
      .getDataView()
      .setFloat64(arg0 + 8, state.isLikeNone(ret) ? 0 : ret!, true);
    state.getDataView().setInt32(arg0, state.isLikeNone(ret) ? 0 : 1, true);
  };

  wbg.__wbg___wbindgen_string_get_a2a31e16edf96e42 = (
    arg0: number,
    arg1: number
  ) => {
    const obj = state.getObject(arg1);
    const ret = typeof obj === "string" ? obj : undefined;
    const ptr1 = state.isLikeNone(ret)
      ? 0
      : state.passStringToWasm(
          ret!,
          state.wasm.__wbindgen_export,
          state.wasm.__wbindgen_export2
        );
    state.getDataView().setInt32(arg0 + 4, state.vectorLen, true);
    state.getDataView().setInt32(arg0, ptr1, true);
  };

  wbg.__wbg___wbindgen_throw_dd24417ed36fc46e = (arg0: number, arg1: number) => {
    throw new Error(state.getStringFromWasm(arg0, arg1));
  };

  wbg.__wbg_call_abb4ff46ce38be40 = function () {
    return state.handleError(function (arg0: number, arg1: number) {
      return state.addHeapObject(
        (state.getObject(arg0) as Function).call(state.getObject(arg1))
      );
    }, Array.from(arguments));
  };

  wbg.__wbg_done_62ea16af4ce34b24 = (arg0: number) =>
    (state.getObject(arg0) as IteratorResult<unknown>).done;

  wbg.__wbg_get_6b7bd52aca3f9671 = (arg0: number, arg1: number) =>
    state.addHeapObject((state.getObject(arg0) as unknown[])[arg1 >>> 0]);

  wbg.__wbg_get_af9dab7e9603ea93 = function () {
    return state.handleError(function (arg0: number, arg1: number) {
      return state.addHeapObject(
        Reflect.get(
          state.getObject(arg0) as object,
          state.getObject(arg1) as string | number | symbol
        )
      );
    }, Array.from(arguments));
  };

  wbg.__wbg_get_with_ref_key_1dc361bd10053bfe = (arg0: number, arg1: number) =>
    state.addHeapObject(
      (state.getObject(arg0) as Record<string, unknown>)[
        state.getObject(arg1) as string
      ]
    );

  wbg.__wbg_instanceof_ArrayBuffer_f3320d2419cd0355 = (arg0: number) => {
    try {
      return state.getObject(arg0) instanceof ArrayBuffer;
    } catch {
      return false;
    }
  };

  wbg.__wbg_instanceof_Uint8Array_da54ccc9d3e09434 = (arg0: number) => {
    try {
      return state.getObject(arg0) instanceof Uint8Array;
    } catch {
      return false;
    }
  };

  wbg.__wbg_isArray_51fd9e6422c0a395 = (arg0: number) =>
    Array.isArray(state.getObject(arg0));

  wbg.__wbg_isSafeInteger_ae7d3f054d55fa16 = (arg0: number) =>
    Number.isSafeInteger(state.getObject(arg0) as number);

  wbg.__wbg_iterator_27b7c8b35ab3e86b = () =>
    state.addHeapObject(Symbol.iterator);

  wbg.__wbg_length_22ac23eaec9d8053 = (arg0: number) =>
    (state.getObject(arg0) as unknown[]).length;

  wbg.__wbg_length_d45040a40c570362 = (arg0: number) =>
    (state.getObject(arg0) as Uint8Array).length;

  wbg.__wbg_new_1ba21ce319a06297 = () => state.addHeapObject({});

  wbg.__wbg_new_25f239778d6112b9 = () => state.addHeapObject([]);

  wbg.__wbg_new_6421f6084cc5bc5a = (arg0: number) =>
    state.addHeapObject(new Uint8Array(state.getObject(arg0) as ArrayBuffer));

  wbg.__wbg_next_138a17bbf04e926c = (arg0: number) =>
    state.addHeapObject((state.getObject(arg0) as Iterator<unknown>).next);

  wbg.__wbg_next_3cfe5c0fe2a4cc53 = function () {
    return state.handleError(function (arg0: number) {
      return state.addHeapObject(
        (state.getObject(arg0) as Iterator<unknown>).next()
      );
    }, Array.from(arguments));
  };

  wbg.__wbg_prototypesetcall_dfe9b766cdc1f1fd = (
    arg0: number,
    arg1: number,
    arg2: number
  ) => {
    Uint8Array.prototype.set.call(
      state.getArrayFromWasm(arg0, arg1),
      state.getObject(arg2) as Uint8Array
    );
  };

  wbg.__wbg_set_3f1d0b984ed272ed = (
    arg0: number,
    arg1: number,
    arg2: number
  ) => {
    (state.getObject(arg0) as Record<string, unknown>)[
      state.takeObject(arg1) as string
    ] = state.takeObject(arg2);
  };

  wbg.__wbg_set_7df433eea03a5c14 = (
    arg0: number,
    arg1: number,
    arg2: number
  ) => {
    (state.getObject(arg0) as unknown[])[arg1 >>> 0] = state.takeObject(arg2);
  };

  wbg.__wbg_value_57b7b035e117f7ee = (arg0: number) =>
    state.addHeapObject(
      (state.getObject(arg0) as IteratorResult<unknown>).value
    );

  wbg.__wbindgen_cast_2241b6af4c4b2941 = (arg0: number, arg1: number) =>
    state.addHeapObject(state.getStringFromWasm(arg0, arg1));

  wbg.__wbindgen_cast_4625c577ab2ec9ee = (arg0: bigint) =>
    state.addHeapObject(BigInt.asUintN(64, arg0));

  wbg.__wbindgen_cast_d6cd19b81560fd6e = (arg0: number) =>
    state.addHeapObject(arg0);

  wbg.__wbindgen_object_clone_ref = (arg0: number) =>
    state.addHeapObject(state.getObject(arg0));

  wbg.__wbindgen_object_drop_ref = (arg0: number) => {
    state.takeObject(arg0);
  };

  wbg.__wbindgen_object_is_undefined = (arg0: number) =>
    state.getObject(arg0) === undefined;

  return imports;
}
