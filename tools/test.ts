#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run
/**
 * Sorex Test Suite
 *
 * Combines WASM integrity tests and E2E search tests.
 *
 * Usage: deno task test
 */

import { join, dirname, fromFileUrl } from "https://deno.land/std@0.224.0/path/mod.ts";
import { existsSync } from "https://deno.land/std@0.224.0/fs/exists.ts";
import { walk } from "https://deno.land/std@0.224.0/fs/walk.ts";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const FIXTURES = join(ROOT, "data/e2e/fixtures");
const OUTPUT = join(ROOT, "data/e2e/output");
const SOREX_BIN = join(ROOT, "target/release/sorex");
const WASM_PATH = join(ROOT, "target/pkg/sorex_bg.wasm");

// =============================================================================
// TEST UTILITIES
// =============================================================================

function assert(condition: boolean, message: string) {
  if (!condition) {
    throw new Error(`Assertion failed: ${message}`);
  }
}

function assertEqual<T>(actual: T, expected: T, message: string) {
  if (actual !== expected) {
    throw new Error(`${message}: expected ${expected}, got ${actual}`);
  }
}

function log(icon: string, message: string) {
  console.log(`  ${icon} ${message}`);
}

// =============================================================================
// WASM INTEGRITY TESTS
// =============================================================================

async function testWasmIntegrity() {
  console.log("\n1. WASM File Integrity");

  if (!existsSync(WASM_PATH)) {
    console.log("  ⚠ WASM file not found, skipping integrity tests");
    console.log("    Run: wasm-pack build --target web --release");
    return;
  }

  const wasmBytes = await Deno.readFile(WASM_PATH);
  const sizeKB = (wasmBytes.length / 1024).toFixed(1);
  log("✓", `WASM file size: ${sizeKB}KB (expected ~200-350KB)`);

  if (wasmBytes.length < 100000) {
    throw new Error("WASM file is too small! Likely not properly compiled.");
  }

  // Check magic bytes
  const magic = Array.from(wasmBytes.slice(0, 4))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");

  if (magic !== "0061736d") {
    throw new Error(`Invalid WASM magic bytes: ${magic} (expected 0061736d)`);
  }
  log("✓", `Valid WASM magic bytes: ${magic}`);

  // Check for expected exports
  const wasmStr = new TextDecoder("latin1").decode(wasmBytes);
  const exports = [
    "__wbindgen_",
    "sorexsearcher_new",
    "sorexincrementalloader_new",
  ];

  let foundExports = 0;
  for (const exp of exports) {
    if (wasmStr.includes(exp)) {
      foundExports++;
    }
  }

  if (foundExports < exports.length) {
    throw new Error(`Only found ${foundExports}/${exports.length} expected exports`);
  }
  log("✓", `Found ${foundExports}/${exports.length} expected wasm-bindgen exports`);
}

// =============================================================================
// E2E SEARCH TESTS
// =============================================================================

async function runCommand(cmd: string[]): Promise<{ success: boolean; output: string }> {
  const command = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    cwd: ROOT,
    stdout: "piped",
    stderr: "piped",
  });

  const { success, stdout, stderr } = await command.output();
  const output = new TextDecoder().decode(stdout) + new TextDecoder().decode(stderr);
  return { success, output };
}

async function setup() {
  console.log("\n2. E2E Setup");

  // Build CLI
  log("…", "Building sorex CLI...");
  const { success } = await runCommand(["cargo", "build", "--release"]);
  if (!success) {
    throw new Error("Failed to build sorex CLI");
  }
  log("✓", "CLI built");

  // Clean and create output directory
  if (existsSync(OUTPUT)) {
    await Deno.remove(OUTPUT, { recursive: true });
  }
  await Deno.mkdir(OUTPUT, { recursive: true });
  log("✓", "Output directory ready");
}

async function buildIndex(): Promise<string> {
  console.log("\n3. Build Index");

  log("…", "Building search index...");
  const { success, output } = await runCommand([
    SOREX_BIN,
    "index",
    "--input",
    FIXTURES,
    "--output",
    OUTPUT,
  ]);

  if (!success) {
    console.error(output);
    throw new Error("Failed to build index");
  }

  // Find generated .sorex file
  let indexPath = "";
  for await (const entry of walk(OUTPUT, { maxDepth: 1 })) {
    if (entry.isFile && entry.name.endsWith(".sorex")) {
      indexPath = entry.path;
      break;
    }
  }

  if (!indexPath) {
    throw new Error("No .sorex file generated");
  }

  log("✓", `Generated: ${indexPath}`);
  return indexPath;
}

async function testCLISearch(indexPath: string) {
  console.log("\n4. CLI Search Tests");

  // Test: Search for "rust"
  const { success: s1, output: o1 } = await runCommand([
    SOREX_BIN,
    "search",
    indexPath,
    "rust",
    "--limit",
    "10",
  ]);
  assert(s1, "CLI search should succeed");
  assert(o1.toLowerCase().includes("rust"), 'Should find results for "rust"');
  log("✓", 'Search "rust": found results');

  // Test: Search for "typescript"
  const { success: s2, output: o2 } = await runCommand([
    SOREX_BIN,
    "search",
    indexPath,
    "typescript",
    "--limit",
    "10",
  ]);
  assert(s2, "CLI search should succeed");
  assert(o2.toLowerCase().includes("typescript"), 'Should find results for "typescript"');
  log("✓", 'Search "typescript": found results');

  // Test: Fuzzy search (typo)
  const { success: s3, output: o3 } = await runCommand([
    SOREX_BIN,
    "search",
    indexPath,
    "javascrip", // Missing 't'
    "--limit",
    "10",
  ]);
  assert(s3, "CLI fuzzy search should succeed");
  log("✓", 'Fuzzy search "javascrip": completed');

  // Test: Non-existent term
  const { success: s4, output: o4 } = await runCommand([
    SOREX_BIN,
    "search",
    indexPath,
    "xyznonexistent",
    "--limit",
    "10",
  ]);
  assert(s4, "CLI search for non-existent term should not crash");
  log("✓", 'Search "xyznonexistent": no crash');
}

async function testDemoGeneration() {
  console.log("\n5. Demo Generation");

  // Clean output
  if (existsSync(OUTPUT)) {
    await Deno.remove(OUTPUT, { recursive: true });
  }
  await Deno.mkdir(OUTPUT, { recursive: true });

  // Build with --demo flag
  const { success, output } = await runCommand([
    SOREX_BIN,
    "index",
    "--input",
    FIXTURES,
    "--output",
    OUTPUT,
    "--demo",
  ]);

  if (!success) {
    console.error(output);
    throw new Error("Failed to build index with --demo flag");
  }

  const demoPath = join(OUTPUT, "demo.html");
  assert(existsSync(demoPath), "demo.html should be generated with --demo flag");

  const demoContent = await Deno.readTextFile(demoPath);
  assert(demoContent.includes("loadSorex"), "demo.html should use loadSorex");
  assert(demoContent.includes("sorex.js"), "demo.html should import sorex.js");
  log("✓", "demo.html generated and valid");
}

async function testInspect(indexPath: string) {
  console.log("\n6. Inspect Command");

  const { success, output } = await runCommand([SOREX_BIN, "inspect", indexPath]);

  assert(success, "Inspect should succeed");
  assert(output.includes("version") || output.includes("Version"), "Should show version");
  log("✓", "Inspect command works");
}

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  console.log("=".repeat(60));
  console.log("Sorex Test Suite");
  console.log("=".repeat(60));

  try {
    await testWasmIntegrity();
    await setup();
    const indexPath = await buildIndex();
    await testCLISearch(indexPath);
    await testInspect(indexPath);
    await testDemoGeneration();

    console.log("\n" + "=".repeat(60));
    console.log("All tests passed!");
    console.log("=".repeat(60));
  } catch (err) {
    console.error("\n" + "=".repeat(60));
    console.error("TEST FAILED");
    console.error("=".repeat(60));
    console.error(err);
    Deno.exit(1);
  }
}

main();
