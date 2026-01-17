#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run
/**
 * Sorex Benchmark Regression Tracking
 *
 * Tests all query types (exact, substring, fuzzy) across datasets.
 * Compares native CLI vs WASM (Deno) performance.
 *
 * Usage: deno task regression
 */

import { join, dirname, fromFileUrl } from "https://deno.land/std@0.224.0/path/mod.ts";
import { existsSync } from "https://deno.land/std@0.224.0/fs/exists.ts";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");

// =============================================================================
// CONFIGURATION
// =============================================================================

interface DatasetConfig {
  name: string;
  indexGlob: string;
  queries: {
    exact: string[];
    substring: string[];
    fuzzy: string[];
  };
}

const DATASETS: DatasetConfig[] = [
  {
    name: "cutlass",
    indexGlob: "target/datasets/cutlass/*.sorex",
    queries: {
      exact: ["gemm", "kernel", "tensor", "warp", "cuda"],
      substring: ["ker", "ten", "war", "gem", "mat"],
      fuzzy: ["kernal", "tensr", "wrp", "gemn", "matrx"],
    },
  },
  {
    name: "pytorch",
    indexGlob: "target/datasets/pytorch/*.sorex",
    queries: {
      exact: ["tensor", "module", "forward", "backward", "autograd"],
      substring: ["ten", "mod", "for", "back", "auto"],
      fuzzy: ["tensr", "modul", "forwrd", "backwrd", "autogrd"],
    },
  },
];

const WARMUP_ITERATIONS = 10;
const MEASURED_ITERATIONS = 20;

// =============================================================================
// TYPES
// =============================================================================

interface BenchmarkResult {
  query: string;
  tier: "exact" | "substring" | "fuzzy";
  nativeUs: number;
  wasmUs: number;
  speedup: number;
}

interface DatasetResults {
  dataset: string;
  indexPath: string;
  vocabSize: number;
  docCount: number;
  results: BenchmarkResult[];
}

// =============================================================================
// BENCHMARK FUNCTIONS
// =============================================================================

async function runNativeBenchmark(indexPath: string, query: string): Promise<number> {
  const command = new Deno.Command(join(ROOT, "target/release/sorex"), {
    args: ["search", indexPath, query, "--limit", "10"],
    cwd: ROOT,
    stdout: "piped",
    stderr: "piped",
  });

  const { stdout, stderr } = await command.output();
  const output = new TextDecoder().decode(stdout) + new TextDecoder().decode(stderr);

  const match = output.match(/Search total:\s*([\d.]+)\s*(us|ms)/);
  if (match) {
    const value = parseFloat(match[1]);
    const unit = match[2];
    return unit === "ms" ? value * 1000 : value;
  }
  return -1;
}

async function runWasmBenchmark(indexPath: string, query: string): Promise<number> {
  const loaderPath = join(ROOT, "target/loader/sorex.js");

  const script = `
import { loadSorexSync } from '${loaderPath}';

const WARMUP = ${WARMUP_ITERATIONS};
const MEASURED = ${MEASURED_ITERATIONS};

const buffer = await Deno.readFile('${indexPath}');
const searcher = loadSorexSync(buffer.buffer);

// Warmup
for (let i = 0; i < WARMUP; i++) {
  searcher.searchSync('${query}', 10);
}

// Measure
const start = performance.now();
for (let i = 0; i < MEASURED; i++) {
  searcher.searchSync('${query}', 10);
}
const elapsed = (performance.now() - start) / MEASURED;

console.log(JSON.stringify({ timeUs: elapsed * 1000 }));
`;

  const scriptPath = "/tmp/sorex-bench.ts";
  await Deno.writeTextFile(scriptPath, script);

  try {
    const command = new Deno.Command("deno", {
      args: ["run", "--allow-read", scriptPath],
      cwd: ROOT,
      stdout: "piped",
      stderr: "piped",
    });

    const { success, stdout, stderr } = await command.output();

    if (!success) {
      console.error("Deno stderr:", new TextDecoder().decode(stderr));
      return -1;
    }

    const output = new TextDecoder().decode(stdout).trim();
    const parsed = JSON.parse(output);
    return parsed.timeUs;
  } finally {
    try {
      await Deno.remove(scriptPath);
    } catch {
      // Ignore cleanup errors
    }
  }
}

function findIndexPath(datasetDir: string): string | null {
  try {
    for (const entry of Deno.readDirSync(datasetDir)) {
      if (entry.isFile && entry.name.endsWith(".sorex")) {
        return join(datasetDir, entry.name);
      }
    }
  } catch {
    // Directory doesn't exist
  }
  return null;
}

async function getIndexStats(indexPath: string): Promise<{ vocabSize: number; docCount: number }> {
  const command = new Deno.Command(join(ROOT, "target/release/sorex"), {
    args: ["inspect", indexPath],
    cwd: ROOT,
    stdout: "piped",
    stderr: "piped",
  });

  const { stdout, stderr } = await command.output();
  const output = new TextDecoder().decode(stdout) + new TextDecoder().decode(stderr);

  const vocabMatch = output.match(/Vocabulary size:\s*([\d,]+)/);
  const docMatch = output.match(/Documents:\s*(\d+)/);

  return {
    vocabSize: vocabMatch ? parseInt(vocabMatch[1].replace(/,/g, "")) : 0,
    docCount: docMatch ? parseInt(docMatch[1]) : 0,
  };
}

async function benchmarkDataset(config: DatasetConfig): Promise<DatasetResults | null> {
  const datasetDir = join(ROOT, "target/datasets", config.name);
  const indexPath = findIndexPath(datasetDir);

  if (!indexPath) {
    console.error(`Index not found for ${config.name}`);
    return null;
  }

  console.log(`\n${"=".repeat(60)}`);
  console.log(`Dataset: ${config.name.toUpperCase()}`);
  console.log(`Index: ${indexPath}`);
  console.log("=".repeat(60));

  const { vocabSize, docCount } = await getIndexStats(indexPath);
  console.log(`Vocabulary: ${vocabSize.toLocaleString()} terms`);
  console.log(`Documents: ${docCount}`);

  const results: BenchmarkResult[] = [];

  for (const tier of ["exact", "substring", "fuzzy"] as const) {
    console.log(`\n--- ${tier.toUpperCase()} MATCHES ---`);
    console.log(
      `${"Query".padEnd(15)} ${"Native (us)".padStart(12)} ${"WASM (us)".padStart(12)} ${"Speedup".padStart(10)}`
    );
    console.log("-".repeat(51));

    for (const query of config.queries[tier]) {
      // Run native benchmark (average of multiple runs for stability)
      const nativeTimes: number[] = [];
      for (let i = 0; i < 5; i++) {
        nativeTimes.push(await runNativeBenchmark(indexPath, query));
      }
      const nativeUs = nativeTimes.sort((a, b) => a - b)[2]; // Median

      // Run WASM benchmark
      const wasmUs = await runWasmBenchmark(indexPath, query);

      const speedup = nativeUs / wasmUs;

      results.push({ query, tier, nativeUs, wasmUs, speedup });

      console.log(
        `${query.padEnd(15)} ${nativeUs.toFixed(1).padStart(12)} ${wasmUs.toFixed(1).padStart(12)} ${speedup.toFixed(1).padStart(9)}x`
      );
    }
  }

  return {
    dataset: config.name,
    indexPath,
    vocabSize,
    docCount,
    results,
  };
}

function printSummary(allResults: DatasetResults[]) {
  console.log(`\n${"=".repeat(70)}`);
  console.log("REGRESSION SUMMARY");
  console.log("=".repeat(70));

  const regressions: { dataset: string; query: string; tier: string; speedup: number }[] = [];

  for (const dataset of allResults) {
    console.log(`\n${dataset.dataset.toUpperCase()}:`);

    for (const tier of ["exact", "substring", "fuzzy"] as const) {
      const tierResults = dataset.results.filter((r) => r.tier === tier);
      const avgSpeedup = tierResults.reduce((sum, r) => sum + r.speedup, 0) / tierResults.length;
      const minSpeedup = Math.min(...tierResults.map((r) => r.speedup));
      const maxSpeedup = Math.max(...tierResults.map((r) => r.speedup));

      const status = minSpeedup >= 1.0 ? "OK" : "WARN";
      console.log(
        `  ${tier.padEnd(12)} avg: ${avgSpeedup.toFixed(1)}x  range: ${minSpeedup.toFixed(1)}x - ${maxSpeedup.toFixed(1)}x ${status}`
      );

      for (const r of tierResults) {
        if (r.speedup < 1.0) {
          regressions.push({
            dataset: dataset.dataset,
            query: r.query,
            tier: r.tier,
            speedup: r.speedup,
          });
        }
      }
    }
  }

  if (regressions.length > 0) {
    console.log("\nREGRESSIONS DETECTED:");
    for (const r of regressions) {
      console.log(
        `  ${r.dataset}/${r.tier}: "${r.query}" - WASM is ${(1 / r.speedup).toFixed(1)}x SLOWER than native`
      );
    }
    Deno.exit(1);
  } else {
    console.log("\nNo regressions - WASM is faster or equal across all queries");
  }
}

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  console.log("Sorex Benchmark Regression Tracking");
  console.log(`Warmup: ${WARMUP_ITERATIONS} iterations, Measured: ${MEASURED_ITERATIONS} iterations`);
  console.log(`Date: ${new Date().toISOString()}`);

  // Ensure release build exists
  const binaryPath = join(ROOT, "target/release/sorex");
  if (!existsSync(binaryPath)) {
    console.log("Building release binary...");
    const command = new Deno.Command("cargo", {
      args: ["build", "--release"],
      cwd: ROOT,
      stdout: "inherit",
      stderr: "inherit",
    });
    await command.output();
  }

  // Ensure loader exists
  const loaderPath = join(ROOT, "target/loader/sorex.js");
  if (!existsSync(loaderPath)) {
    console.error("Error: ./target/loader/sorex.js not found");
    console.error("Run: cargo build --release");
    Deno.exit(1);
  }

  const allResults: DatasetResults[] = [];

  for (const dataset of DATASETS) {
    const results = await benchmarkDataset(dataset);
    if (results) {
      allResults.push(results);
    }
  }

  printSummary(allResults);

  // Output JSON for CI integration
  const jsonPath = join(ROOT, "target", "benchmark-results.json");
  const jsonOutput = {
    timestamp: new Date().toISOString(),
    warmupIterations: WARMUP_ITERATIONS,
    measuredIterations: MEASURED_ITERATIONS,
    datasets: allResults,
  };

  await Deno.writeTextFile(jsonPath, JSON.stringify(jsonOutput, null, 2));
  console.log(`\nResults saved to: ${jsonPath}`);
}

main().catch(console.error);
