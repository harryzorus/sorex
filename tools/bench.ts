#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run --allow-net
/**
 * Sorex Full Benchmark Suite
 *
 * Single entry point for comprehensive benchmarking:
 * 1. Idempotent dataset crawling (skip if fresh)
 * 2. Compile Sorex (binary + WASM + loader) as needed
 * 3. Reindex datasets every time
 * 4. Run benchmarks with proper warmup and confidence intervals
 * 5. Compare against competing libraries
 * 6. Update documentation
 *
 * Usage:
 *   deno task bench                          # Full suite
 *   deno task bench:cutlass                  # Single dataset
 *   deno task bench:quick                    # Quick mode (fewer iterations)
 */

import { parseArgs } from "https://deno.land/std@0.224.0/cli/parse_args.ts";
import { join, dirname, fromFileUrl } from "https://deno.land/std@0.224.0/path/mod.ts";
import { existsSync } from "https://deno.land/std@0.224.0/fs/exists.ts";
import Fuse from "npm:fuse.js@7.0.0";
import lunr from "npm:lunr@2.3.9";
import FlexSearch from "npm:flexsearch@0.7.43";
import MiniSearch from "npm:minisearch@7.1.0";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const DATA_DIR = join(ROOT, "target/datasets");
const RESULTS_DIR = join(ROOT, "target/bench-results");
const COMPARISONS_DIR = join(ROOT, "docs/comparisons");

// =============================================================================
// CONFIGURATION
// =============================================================================

interface DatasetConfig {
  name: string;
  displayName: string;
  dir: string;
  maxAgeHours: number;
  queries: {
    exact: string[];
    prefix: string[];
    fuzzy: string[];
  };
}

const DATASETS: Record<string, DatasetConfig> = {
  cutlass: {
    name: "cutlass",
    displayName: "NVIDIA CUTLASS",
    dir: join(DATA_DIR, "cutlass"),
    maxAgeHours: 24 * 7,
    queries: {
      exact: ["gemm", "kernel", "tensor", "warp", "cuda", "epilogue"],
      prefix: ["ker", "ten", "war", "gem", "mat", "epi", "sync"],
      fuzzy: ["kernal", "tensr", "wrp", "gemn", "matrx", "epilouge", "syncronize"],
    },
  },
  pytorch: {
    name: "pytorch",
    displayName: "PyTorch",
    dir: join(DATA_DIR, "pytorch"),
    maxAgeHours: 24 * 7,
    queries: {
      exact: ["tensor", "module", "forward", "backward", "autograd", "optim"],
      prefix: ["ten", "mod", "for", "back", "auto", "opt"],
      fuzzy: ["tensro", "modul", "forwrd", "backwrd", "autogrd", "optimzer"],
    },
  },
};

interface BenchConfig {
  warmupIterations: number;
  measuredIterations: number;
  confidenceLevel: number;
  minIterations: number;
  maxIterations: number;
  targetCIPercent: number;
}

const QUICK_CONFIG: BenchConfig = {
  warmupIterations: 5,
  measuredIterations: 20,
  confidenceLevel: 0.95,
  minIterations: 10,
  maxIterations: 100,
  targetCIPercent: 10,
};

const FULL_CONFIG: BenchConfig = {
  warmupIterations: 10,
  measuredIterations: 20,
  confidenceLevel: 0.99,
  minIterations: 20,
  maxIterations: 500,
  targetCIPercent: 5,
};

// =============================================================================
// TYPES
// =============================================================================

interface Document {
  id: number;
  slug: string;
  title: string;
  excerpt: string;
  href: string;
  text: string;
}

interface BenchResult {
  query: string;
  tier: "exact" | "prefix" | "fuzzy";
  library: string;
  mean: number;
  stddev: number;
  ci95Low: number;
  ci95High: number;
  p99: number;
  resultCount: number;
  t1End?: number;
  t2End?: number;
  t3End?: number;
  t1Duration?: number;
  t2Duration?: number;
  t3Duration?: number;
  t1Results?: number;
  t2Results?: number;
  t3Results?: number;
}

interface LibraryBenchResult {
  library: string;
  query: string;
  tier: "exact" | "prefix" | "fuzzy";
  meanUs: number;
  stddev: number;
  ci95Low: number;
  ci95High: number;
  p99Us: number;
  resultCount: number;
}

// =============================================================================
// UTILITIES
// =============================================================================

function log(msg: string, level: "info" | "success" | "warn" | "error" = "info"): void {
  const icons = { info: "  ", success: "  ", warn: "  ", error: "  " };
  console.log(`${icons[level]}${msg}`);
}

async function runCommand(cmd: string[], options: { cwd?: string; silent?: boolean } = {}): Promise<boolean> {
  const command = new Deno.Command(cmd[0], {
    args: cmd.slice(1),
    cwd: options.cwd ?? ROOT,
    stdout: options.silent ? "piped" : "inherit",
    stderr: options.silent ? "piped" : "inherit",
  });

  const { success } = await command.output();
  return success;
}

function getNewestMtime(dir: string, pattern = /\.(rs|ts)$/): number {
  if (!existsSync(dir)) return 0;

  let newest = 0;
  try {
    for (const entry of Deno.readDirSync(dir)) {
      const fullPath = join(dir, entry.name);
      if (entry.isDirectory && !entry.name.startsWith(".") && entry.name !== "target") {
        newest = Math.max(newest, getNewestMtime(fullPath, pattern));
      } else if (entry.isFile && pattern.test(entry.name)) {
        try {
          const stat = Deno.statSync(fullPath);
          newest = Math.max(newest, stat.mtime?.getTime() ?? 0);
        } catch {
          // Ignore stat errors
        }
      }
    }
  } catch {
    // Directory read error
  }

  return newest;
}

// =============================================================================
// STEP 1: IDEMPOTENT DATASET CRAWLING
// =============================================================================

function shouldRecrawl(datasetDir: string, maxAgeHours: number): boolean {
  const manifestPath = join(datasetDir, "manifest.json");
  if (!existsSync(manifestPath)) return true;

  try {
    const stats = Deno.statSync(manifestPath);
    const ageHours = (Date.now() - (stats.mtime?.getTime() ?? 0)) / (1000 * 60 * 60);
    return ageHours > maxAgeHours;
  } catch {
    return true;
  }
}

async function ensureDataset(config: DatasetConfig, forceRecrawl: boolean): Promise<boolean> {
  console.log(`\nDataset: ${config.displayName}`);

  if (!forceRecrawl && !shouldRecrawl(config.dir, config.maxAgeHours)) {
    log("Fresh (skipping crawl)", "success");
    return true;
  }

  log(`Crawling ${config.displayName}...`, "info");

  const crawlScript = join(ROOT, "tools/crawl.ts");
  if (!existsSync(crawlScript)) {
    log(`Crawl script not found: ${crawlScript}`, "error");
    return false;
  }

  const success = await runCommand([
    "deno", "run", "--allow-read", "--allow-write", "--allow-net",
    crawlScript, "--dataset", config.name
  ]);

  if (success) {
    log("Crawl complete", "success");
  } else {
    log("Crawl failed", "error");
  }

  return success;
}

// =============================================================================
// STEP 2: COMPILE SOREX
// =============================================================================

async function compileSorex(): Promise<boolean> {
  console.log("\nCompiling Sorex...");

  const srcDir = join(ROOT, "src");
  const binaryPath = join(ROOT, "target/release/sorex");
  const wasmPath = join(ROOT, "target/wasm32-unknown-unknown/release/sorex.wasm");

  const srcModified = getNewestMtime(srcDir);
  const binaryMtime = existsSync(binaryPath)
    ? (Deno.statSync(binaryPath).mtime?.getTime() ?? 0)
    : 0;
  const wasmMtime = existsSync(wasmPath)
    ? (Deno.statSync(wasmPath).mtime?.getTime() ?? 0)
    : 0;

  // Build release binary if needed
  if (srcModified > binaryMtime) {
    log("Building release binary...", "info");
    const success = await runCommand(["cargo", "build", "--release"]);
    if (!success) {
      log("Binary build failed", "error");
      return false;
    }
    log("Binary built", "success");
  } else {
    log("Binary up to date", "success");
  }

  // Build WASM if needed
  if (srcModified > wasmMtime) {
    log("Building WASM...", "info");
    const success = await runCommand(
      ["wasm-pack", "build", "--target", "web", "--release", "--", "--features", "wasm"],
      { silent: true }
    );
    if (!success) {
      log("WASM build failed", "error");
      return false;
    }
    log("WASM built", "success");
  } else {
    log("WASM up to date", "success");
  }

  return true;
}

// =============================================================================
// STEP 3: INDEX DATASETS
// =============================================================================

async function indexDataset(config: DatasetConfig): Promise<string | null> {
  console.log(`\nIndexing ${config.displayName}...`);

  const success = await runCommand([
    join(ROOT, "target/release/sorex"),
    "index", "--input", config.dir, "--output", config.dir
  ]);

  if (!success) {
    log("Indexing failed", "error");
    return null;
  }

  // Find generated index file
  let indexPath = "";
  for (const entry of Deno.readDirSync(config.dir)) {
    if (entry.isFile && entry.name.endsWith(".sorex")) {
      indexPath = join(config.dir, entry.name);
      break;
    }
  }

  if (!indexPath) {
    log("No .sorex file generated", "error");
    return null;
  }

  log(`Created ${indexPath}`, "success");
  return indexPath;
}

// =============================================================================
// STEP 4: STATISTICS
// =============================================================================

function calculateStats(
  samples: number[],
  confidenceLevel: number
): { mean: number; stddev: number; ciLow: number; ciHigh: number; p99: number } {
  const n = samples.length;
  if (n === 0) return { mean: 0, stddev: 0, ciLow: 0, ciHigh: 0, p99: 0 };

  const mean = samples.reduce((a, b) => a + b, 0) / n;
  const variance = samples.reduce((sum, x) => sum + (x - mean) ** 2, 0) / (n - 1);
  const stddev = Math.sqrt(variance);

  const tCritical = confidenceLevel >= 0.99 ? 2.576 : confidenceLevel >= 0.95 ? 1.96 : 1.645;
  const marginOfError = tCritical * (stddev / Math.sqrt(n));

  const sorted = [...samples].sort((a, b) => a - b);
  const p99 = sorted[Math.floor(n * 0.99)] ?? sorted[n - 1];

  return {
    mean,
    stddev,
    ciLow: mean - marginOfError,
    ciHigh: mean + marginOfError,
    p99,
  };
}

function measureAdaptive(
  runFn: () => void,
  config: BenchConfig
): { samples: number[]; iterations: number; converged: boolean } {
  const samples: number[] = [];

  // Warmup
  for (let i = 0; i < config.warmupIterations; i++) {
    runFn();
  }

  let converged = false;
  const batchSize = config.measuredIterations;

  while (samples.length < config.maxIterations) {
    for (let i = 0; i < batchSize && samples.length < config.maxIterations; i++) {
      const start = performance.now();
      runFn();
      const end = performance.now();
      samples.push((end - start) * 1000);
    }

    if (samples.length < config.minIterations) continue;

    const stats = calculateStats(samples, config.confidenceLevel);
    const ciWidth = stats.ciHigh - stats.ciLow;
    const ciPercent = (ciWidth / 2 / stats.mean) * 100;

    if (ciPercent <= config.targetCIPercent) {
      converged = true;
      break;
    }
  }

  return { samples, iterations: samples.length, converged };
}

// =============================================================================
// STEP 5: SOREX BENCHMARK
// =============================================================================

interface SorexTierTiming {
  t1Time: number;
  t1Results: number;
  t2Time: number;
  t2Results: number;
  t3Time: number;
  t3Results: number;
  totalTime: number;
  totalResults: number;
}

function parseSorexOutput(output: string): SorexTierTiming | null {
  const t1Match = output.match(/T1 Exact:\s*([\d.]+)\s*(us|ms)\s*\((\d+)\s*results?\)/);
  const t2Match = output.match(/T2 Prefix:\s*([\d.]+)\s*(us|ms)\s*\((\d+)\s*results?\)/);
  const t3Match = output.match(/T3 Fuzzy:\s*([\d.]+)\s*(us|ms)\s*\((\d+)\s*results?\)/);
  const totalMatch = output.match(/Search total:\s*([\d.]+)\s*(us|ms)/);
  const resultsMatch = output.match(/RESULTS\s*\((\d+)\)/i);

  if (!totalMatch) return null;

  const parseTime = (match: RegExpMatchArray | null): number => {
    if (!match) return 0;
    const value = parseFloat(match[1]);
    const unit = match[2];
    return unit === "ms" ? value * 1000 : value;
  };

  const parseResults = (match: RegExpMatchArray | null): number => {
    if (!match) return 0;
    return parseInt(match[3]);
  };

  return {
    t1Time: parseTime(t1Match),
    t1Results: parseResults(t1Match),
    t2Time: parseTime(t2Match),
    t2Results: parseResults(t2Match),
    t3Time: parseTime(t3Match),
    t3Results: parseResults(t3Match),
    totalTime: parseTime(totalMatch),
    totalResults: resultsMatch ? parseInt(resultsMatch[1]) : 0,
  };
}

interface SorexBenchResult {
  mean: number;
  stddev: number;
  ciLow: number;
  ciHigh: number;
  p99: number;
  resultCount: number;
  iterations: number;
  converged: boolean;
  t1End: number;
  t2End: number;
  t3End: number;
  t1Duration: number;
  t2Duration: number;
  t3Duration: number;
  t1Results: number;
  t2Results: number;
  t3Results: number;
}

async function runSorexBenchmark(indexPath: string, query: string, config: BenchConfig): Promise<SorexBenchResult> {
  const totalSamples: number[] = [];
  const t1DurationSamples: number[] = [];
  const t2DurationSamples: number[] = [];
  const t3DurationSamples: number[] = [];
  const t1EndSamples: number[] = [];
  const t2EndSamples: number[] = [];
  const t3EndSamples: number[] = [];

  let t1Results = 0;
  let t2Results = 0;
  let t3Results = 0;
  let totalResults = 0;

  const runSearch = async (): Promise<string> => {
    const command = new Deno.Command(join(ROOT, "target/release/sorex"), {
      args: ["search", indexPath, query, "--limit", "1000"],
      cwd: ROOT,
      stdout: "piped",
      stderr: "piped",
    });
    const { stdout, stderr } = await command.output();
    return new TextDecoder().decode(stdout) + new TextDecoder().decode(stderr);
  };

  // Warmup
  for (let i = 0; i < config.warmupIterations; i++) {
    await runSearch();
  }

  let converged = false;
  const batchSize = config.measuredIterations;

  while (totalSamples.length < config.maxIterations) {
    for (let i = 0; i < batchSize && totalSamples.length < config.maxIterations; i++) {
      const output = await runSearch();
      const timing = parseSorexOutput(output);

      if (timing) {
        totalSamples.push(timing.totalTime);
        t1DurationSamples.push(timing.t1Time);
        t2DurationSamples.push(timing.t2Time);
        t3DurationSamples.push(timing.t3Time);

        const t1End = timing.t1Time;
        const t2End = timing.t1Time + timing.t2Time;
        const t3End = timing.totalTime;
        t1EndSamples.push(t1End);
        t2EndSamples.push(t2End);
        t3EndSamples.push(t3End);

        if (totalSamples.length === 1) {
          t1Results = timing.t1Results;
          t2Results = timing.t2Results;
          t3Results = timing.t3Results;
          totalResults = timing.totalResults;
        }
      }
    }

    if (totalSamples.length < config.minIterations) continue;

    const stats = calculateStats(totalSamples, config.confidenceLevel);
    const ciWidth = stats.ciHigh - stats.ciLow;
    const ciPercent = (ciWidth / 2 / stats.mean) * 100;

    if (ciPercent <= config.targetCIPercent) {
      converged = true;
      break;
    }
  }

  const stats = calculateStats(totalSamples, config.confidenceLevel);
  const t1DurationStats = calculateStats(t1DurationSamples, config.confidenceLevel);
  const t2DurationStats = calculateStats(t2DurationSamples, config.confidenceLevel);
  const t3DurationStats = calculateStats(t3DurationSamples, config.confidenceLevel);
  const t1EndStats = calculateStats(t1EndSamples, config.confidenceLevel);
  const t2EndStats = calculateStats(t2EndSamples, config.confidenceLevel);
  const t3EndStats = calculateStats(t3EndSamples, config.confidenceLevel);

  return {
    ...stats,
    resultCount: totalResults,
    iterations: totalSamples.length,
    converged,
    t1End: t1EndStats.mean,
    t2End: t2EndStats.mean,
    t3End: t3EndStats.mean,
    t1Duration: t1DurationStats.mean,
    t2Duration: t2DurationStats.mean,
    t3Duration: t3DurationStats.mean,
    t1Results,
    t2Results,
    t3Results,
  };
}

async function benchmarkDataset(
  config: DatasetConfig,
  indexPath: string,
  benchConfig: BenchConfig
): Promise<BenchResult[]> {
  console.log(`\nBenchmarking ${config.displayName}...`);
  console.log(`   Adaptive: min=${benchConfig.minIterations}, max=${benchConfig.maxIterations}, target CI=${benchConfig.targetCIPercent}%`);

  const results: BenchResult[] = [];

  for (const tier of ["exact", "prefix", "fuzzy"] as const) {
    console.log(`\n  ${tier.toUpperCase()} queries:`);
    console.log(
      `  ${"Query".padEnd(12)} ${"T1 End".padStart(10)} ${"T2 End".padStart(10)} ${"T3 End".padStart(10)} ${"N".padStart(5)} ${"Results".padStart(12)}`
    );
    console.log(`  ${"-".repeat(60)}`);

    for (const query of config.queries[tier]) {
      const stats = await runSorexBenchmark(indexPath, query, benchConfig);

      const nStr = stats.converged ? stats.iterations.toString() : `${stats.iterations}!`;
      const resultsStr = `${stats.t1Results}+${stats.t2Results}+${stats.t3Results}`;
      console.log(
        `  ${query.padEnd(12)} ${stats.t1End.toFixed(1).padStart(10)} ${stats.t2End.toFixed(1).padStart(10)} ${stats.t3End.toFixed(1).padStart(10)} ${nStr.padStart(5)} ${resultsStr.padStart(12)}`
      );

      results.push({
        query,
        tier,
        library: "sorex",
        mean: stats.mean,
        stddev: stats.stddev,
        ci95Low: stats.ciLow,
        ci95High: stats.ciHigh,
        p99: stats.p99,
        resultCount: stats.resultCount,
        t1End: stats.t1End,
        t2End: stats.t2End,
        t3End: stats.t3End,
        t1Duration: stats.t1Duration,
        t2Duration: stats.t2Duration,
        t3Duration: stats.t3Duration,
        t1Results: stats.t1Results,
        t2Results: stats.t2Results,
        t3Results: stats.t3Results,
      });
    }
  }

  return results;
}

// =============================================================================
// STEP 6: LIBRARY COMPARISON
// =============================================================================

function loadDocuments(datasetDir: string): Document[] {
  const manifestPath = join(datasetDir, "manifest.json");
  if (!existsSync(manifestPath)) return [];

  const manifest = JSON.parse(Deno.readTextFileSync(manifestPath));
  const documents: Document[] = [];

  for (const filename of manifest.documents) {
    const filePath = join(datasetDir, filename);
    if (existsSync(filePath)) {
      const doc = JSON.parse(Deno.readTextFileSync(filePath));
      documents.push({
        id: doc.id,
        slug: doc.slug,
        title: doc.title,
        excerpt: doc.excerpt,
        href: doc.href,
        text: doc.text,
      });
    }
  }

  return documents;
}

function buildFuseIndex(docs: Document[]): Fuse<Document> {
  return new Fuse(docs, {
    keys: ["title", "excerpt", "text"],
    threshold: 0.3,
    includeScore: true,
    ignoreLocation: true,
  });
}

function buildLunrIndex(docs: Document[]): lunr.Index {
  return lunr(function (this: lunr.Builder) {
    this.ref("id");
    this.field("title", { boost: 10 });
    this.field("excerpt", { boost: 5 });
    this.field("text");
    docs.forEach((doc) => this.add({ ...doc, content: doc.text }));
  });
}

function buildFlexSearchIndex(docs: Document[]): FlexSearch.Document<Document, true> {
  const index = new FlexSearch.Document<Document, true>({
    document: {
      id: "id",
      index: ["title", "excerpt", "text"],
    },
  });
  docs.forEach((doc) => index.add(doc));
  return index;
}

function buildMiniSearchIndex(docs: Document[]): MiniSearch<Document> {
  const index = new MiniSearch<Document>({
    fields: ["title", "excerpt", "text"],
    storeFields: ["title", "excerpt", "href"],
    searchOptions: {
      boost: { title: 2, excerpt: 1.5 },
      fuzzy: 0.2,
      prefix: true,
    },
  });
  index.addAll(docs);
  return index;
}

function measureLibrarySearch(
  searchFn: () => unknown[],
  config: BenchConfig
): { samples: number[]; resultCount: number; iterations: number; converged: boolean } {
  const firstResult = searchFn();
  const resultCount = Array.isArray(firstResult) ? firstResult.length : 0;

  const { samples, iterations, converged } = measureAdaptive(searchFn, config);

  return { samples, resultCount, iterations, converged };
}

async function benchmarkLibrariesOnDataset(
  config: DatasetConfig,
  benchConfig: BenchConfig
): Promise<LibraryBenchResult[]> {
  console.log(`\nLibrary Comparison: ${config.displayName}`);

  const docs = loadDocuments(config.dir);
  if (docs.length === 0) {
    log(`No documents found in ${config.dir}`, "warn");
    return [];
  }

  log(`Loaded ${docs.length} documents`, "info");
  log("Building JS library indexes...", "info");

  const fuseIndex = buildFuseIndex(docs);
  const lunrIndex = buildLunrIndex(docs);
  const flexIndex = buildFlexSearchIndex(docs);
  const miniIndex = buildMiniSearchIndex(docs);

  log("Indexes built", "success");

  const results: LibraryBenchResult[] = [];

  for (const tier of ["exact", "prefix", "fuzzy"] as const) {
    console.log(`\n  ${tier.toUpperCase()} queries:`);
    console.log(
      `  ${"Query".padEnd(12)} ${"Library".padEnd(12)} ${"Mean (us)".padStart(10)} ${"CI".padStart(8)} ${"P99".padStart(8)} ${"N".padStart(5)} ${"Results".padStart(8)}`
    );
    console.log(`  ${"-".repeat(70)}`);

    for (const query of config.queries[tier]) {
      const isTypo = tier === "fuzzy";

      const libraries = [
        { name: "FlexSearch", search: () => flexIndex.search(query).flatMap((r) => r.result ?? []) },
        { name: "MiniSearch", search: () => miniIndex.search(query, isTypo ? { fuzzy: 0.3 } : {}) },
        { name: "lunr.js", search: () => lunrIndex.search(query) },
        { name: "fuse.js", search: () => fuseIndex.search(query) },
      ];

      for (const lib of libraries) {
        const { samples, resultCount, iterations, converged } = measureLibrarySearch(lib.search, benchConfig);
        const stats = calculateStats(samples, benchConfig.confidenceLevel);

        const ciWidth = (stats.ciHigh - stats.ciLow) / 2;
        const nStr = converged ? iterations.toString() : `${iterations}!`;
        console.log(
          `  ${query.padEnd(12)} ${lib.name.padEnd(12)} ${stats.mean.toFixed(1).padStart(10)} ${`+${ciWidth.toFixed(1)}`.padStart(8)} ${stats.p99.toFixed(1).padStart(8)} ${nStr.padStart(5)} ${resultCount.toString().padStart(8)}`
        );

        results.push({
          library: lib.name,
          query,
          tier,
          meanUs: stats.mean,
          stddev: stats.stddev,
          ci95Low: stats.ciLow,
          ci95High: stats.ciHigh,
          p99Us: stats.p99,
          resultCount,
        });
      }
    }
  }

  return results;
}

// =============================================================================
// STEP 7: GENERATE REPORTS
// =============================================================================

function formatBytes(bytes: number): string {
  const abs = Math.abs(bytes);
  const sign = bytes < 0 ? "-" : "";
  if (abs < 1024) return `${sign}${abs} B`;
  if (abs < 1024 * 1024) return `${sign}${(abs / 1024).toFixed(1)} KB`;
  return `${sign}${(abs / (1024 * 1024)).toFixed(2)} MB`;
}

function generateLibraryComparisonMarkdown(
  datasetName: string,
  sorexResults: BenchResult[],
  libraryResults: LibraryBenchResult[]
): string {
  const config = DATASETS[datasetName];
  const lines: string[] = [];

  lines.push(`# ${config.displayName} Search Library Comparison`);
  lines.push("");
  lines.push(`**Date:** ${new Date().toISOString()}`);
  lines.push(`**Platform:** ${Deno.build.os}, ${Deno.build.arch}`);
  lines.push("");

  lines.push("## Search Latency");
  lines.push("");
  lines.push("**Key Difference - Progressive vs Batch Results:**");
  lines.push("");
  lines.push("- **Sorex**: Returns results progressively as each tier completes");
  lines.push("- **Other libraries**: Return all results at once");
  lines.push("");

  for (const tier of ["exact", "prefix", "fuzzy"] as const) {
    lines.push(`## ${tier.charAt(0).toUpperCase() + tier.slice(1)} Queries`);
    lines.push("");
    lines.push("| Query | Library | T1 End (us) | T2 End (us) | T3 End (us) | Results |");
    lines.push("|-------|---------|-------------|-------------|-------------|---------|");

    const tierSorex = sorexResults.filter((r) => r.tier === tier);
    const tierLibs = libraryResults.filter((r) => r.tier === tier);

    for (const query of config.queries[tier]) {
      const sorex = tierSorex.find((r) => r.query === query);
      if (sorex) {
        const t1End = sorex.t1End?.toFixed(1) ?? "-";
        const t2End = sorex.t2End?.toFixed(1) ?? "-";
        const t3End = sorex.t3End?.toFixed(1) ?? "-";
        const resultsStr = `${sorex.t1Results ?? 0}+${sorex.t2Results ?? 0}+${sorex.t3Results ?? 0}`;
        lines.push(`| ${query} | **Sorex** | ${t1End} | ${t2End} | ${t3End} | ${resultsStr} |`);
      }

      const queryLibs = tierLibs.filter((r) => r.query === query);
      for (const lib of queryLibs) {
        lines.push(`| ${query} | ${lib.library} | - | - | ${lib.meanUs.toFixed(1)} | ${lib.resultCount} |`);
      }
    }
    lines.push("");
  }

  return lines.join("\n");
}

// =============================================================================
// MAIN
// =============================================================================

async function main(): Promise<void> {
  const args = parseArgs(Deno.args, {
    string: ["dataset"],
    boolean: ["skip-crawl", "skip-index", "skip-compare", "quick", "force-crawl"],
    default: {
      "skip-crawl": false,
      "skip-index": false,
      "skip-compare": false,
      "quick": false,
      "force-crawl": false,
    },
  });

  const benchConfig = args.quick ? QUICK_CONFIG : FULL_CONFIG;
  const datasetKeys = args.dataset ? [args.dataset] : Object.keys(DATASETS);

  console.log("Sorex Benchmark Suite");
  console.log(`   Mode: ${args.quick ? "Quick" : "Full"}`);
  console.log(`   Datasets: ${datasetKeys.join(", ")}`);
  console.log(`   Warmup: ${benchConfig.warmupIterations}, Measured: ${benchConfig.measuredIterations}`);
  console.log(`   Confidence: ${benchConfig.confidenceLevel * 100}%`);

  // Step 1: Ensure datasets
  if (!args["skip-crawl"]) {
    for (const key of datasetKeys) {
      const config = DATASETS[key];
      if (!config) {
        log(`Unknown dataset: ${key}`, "error");
        continue;
      }
      await ensureDataset(config, args["force-crawl"]);
    }
  }

  // Step 2: Compile Sorex
  if (!(await compileSorex())) {
    log("Failed to compile Sorex", "error");
    Deno.exit(1);
  }

  // Step 3 & 4: Index + Benchmark each dataset
  const sorexResultsByDataset: Record<string, BenchResult[]> = {};

  for (const key of datasetKeys) {
    const config = DATASETS[key];
    if (!config) continue;

    console.log(`\n${"=".repeat(60)}`);
    console.log(`DATASET: ${config.displayName}`);
    console.log("=".repeat(60));

    if (!existsSync(config.dir)) {
      log(`Dataset directory not found: ${config.dir}`, "error");
      continue;
    }

    let indexPath: string | null = null;
    if (args["skip-index"]) {
      for (const entry of Deno.readDirSync(config.dir)) {
        if (entry.isFile && entry.name.endsWith(".sorex")) {
          indexPath = join(config.dir, entry.name);
          log(`Using existing index: ${entry.name}`, "info");
          break;
        }
      }
    } else {
      indexPath = await indexDataset(config);
    }

    if (!indexPath) {
      log(`No index available for ${config.displayName}`, "error");
      continue;
    }

    const results = await benchmarkDataset(config, indexPath, benchConfig);
    sorexResultsByDataset[key] = results;

    // Save per-dataset results
    await Deno.mkdir(RESULTS_DIR, { recursive: true });
    const jsonPath = join(RESULTS_DIR, `${key}.json`);
    await Deno.writeTextFile(
      jsonPath,
      JSON.stringify({ timestamp: new Date().toISOString(), config: benchConfig, results }, null, 2)
    );
    log(`Saved ${jsonPath}`, "success");
  }

  // Step 5: Run library comparison
  if (!args["skip-compare"]) {
    for (const key of datasetKeys) {
      const config = DATASETS[key];
      if (!config) continue;

      const libraryResults = await benchmarkLibrariesOnDataset(config, benchConfig);

      if (libraryResults.length > 0 && sorexResultsByDataset[key]) {
        const comparisonMd = generateLibraryComparisonMarkdown(key, sorexResultsByDataset[key], libraryResults);
        await Deno.mkdir(COMPARISONS_DIR, { recursive: true });
        const comparisonPath = join(COMPARISONS_DIR, `${key}.md`);
        await Deno.writeTextFile(comparisonPath, comparisonMd);
        log(`Saved ${comparisonPath}`, "success");
      }
    }
  }

  console.log("\nBenchmark suite complete!");
}

main().catch(console.error);
