# Sorex Loader

Self-contained JavaScript loader for `.sorex` files.

## Structure

```
loader/
├── index.ts      # Public API: loadSorex, loadSorexSync, SorexSearcher
├── parser.ts     # .sorex file parsing, CRC32 validation
├── searcher.ts   # SorexSearcher wrapper class
├── wasm-state.ts # WASM instance state (heap, memory, text encoding)
├── imports.ts    # wasm-bindgen import functions
├── build.ts      # Bundle script
└── tsconfig.json # TypeScript config
```

## Building

After modifying any TypeScript files, rebuild the bundled output:

```bash
cd src/build/loader
bun run build.ts
```

This generates `target/loader/sorex-loader.js` which is embedded in the Rust CLI via `include_str!`.

**Important:** You must run the build script before `cargo build`.

## Type Checking

```bash
cd src/build/loader
bunx tsc --noEmit
```

## How It Works

1. **Parser** extracts WASM bytes and index data from `.sorex` files
2. **WasmState** manages per-instance heap and memory views
3. **Imports** provides wasm-bindgen glue functions bound to each instance
4. **Searcher** wraps the WASM search API with JS types

Multiple WASM versions can coexist on the same page - each unique WASM binary gets its own isolated instance.
