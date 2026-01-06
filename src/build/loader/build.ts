#!/usr/bin/env bun
/**
 * Build script for sieve-loader.js
 *
 * Bundles TypeScript modules into a single self-contained JS file with external source map.
 * Output: target/loader/sieve-loader.js (included by Rust via include_str!)
 *         target/loader/sieve-loader.js.map (for debugging)
 *
 * Usage: cd src/build/loader && bun run build.ts
 */

import { build } from "bun";
import { mkdir } from "fs/promises";

// Output to target/loader/ (idiomatic Rust build artifact location)
const outdir = "../../../target/loader";
await mkdir(outdir, { recursive: true });

const result = await build({
  entrypoints: ["./index.ts"],
  outdir,
  naming: "sieve-loader.js",
  target: "browser",
  format: "esm",
  minify: false, // Keep readable for debugging
  sourcemap: "external", // External .map file for debugging
});

if (!result.success) {
  console.error("Build failed:");
  for (const log of result.logs) {
    console.error(log);
  }
  process.exit(1);
}

// Read the output and add header comment
const output = await Bun.file(`${outdir}/sieve-loader.js`).text();
const header = `/**
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

`;

await Bun.write(`${outdir}/sieve-loader.js`, header + output);
console.log("Built target/loader/sieve-loader.js");
console.log("Built target/loader/sieve-loader.js.map");
