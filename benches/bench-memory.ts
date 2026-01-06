#!/usr/bin/env bun
/**
 * Memory footprint benchmark for search libraries
 *
 * Includes Sieve WASM alongside JS libraries.
 *
 * Run with: npm run bench:memory
 * Or: bun run bench-memory.ts
 */

import Fuse from 'fuse.js';
import lunr from 'lunr';
import FlexSearch from 'flexsearch';
import MiniSearch from 'minisearch';
import { readFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

interface Document {
  id: number | string;
  slug: string;
  title: string;
  content: string;
}

interface SieveSearcher {
  search(query: string, limit: number): unknown[];
  free(): void;
}

interface SieveModule {
  initSync(options: { module: Buffer }): void;
  SieveSearcher: new (bytes: Uint8Array) => SieveSearcher;
}

interface Manifest {
  documents: string[];
  indexes: {
    index: {
      file: string;
    };
  };
}

interface MemoryResult {
  name: string;
  indexSize: number;
  baseline: number;
  afterIndex: number;
  index: unknown;
}

interface BenchmarkResult {
  label: string;
  docCount?: number;
  posts?: number;
  wordsPerPost?: number;
  rawSize: number;
  results: MemoryResult[];
}

// ============================================================================
// SIEVE WASM LOADING
// ============================================================================

let sieveModule: SieveModule | null = null;
let sieveIndexBytes: Buffer | null = null;

async function loadSieveWasm(): Promise<void> {
  const outputDir = join(__dirname, '../datasets/output');
  const pkgDir = join(__dirname, '../pkg');

  // Find the .sieve file by globbing
  const { globSync } = await import('glob');
  const sieveFiles = globSync(join(outputDir, '*.sieve'));
  if (sieveFiles.length === 0) {
    throw new Error(`No .sieve files found in ${outputDir}. Run: sieve index --input datasets --output datasets/output`);
  }
  const indexPath = sieveFiles[0];
  sieveIndexBytes = readFileSync(indexPath);

  // Load WASM module from pkg/ directory (built by wasm-pack)
  const wasmJsPath = join(pkgDir, 'sieve.js');
  const wasmBinaryPath = join(pkgDir, 'sieve_bg.wasm');
  const wasmBytes = readFileSync(wasmBinaryPath);

  sieveModule = await import(wasmJsPath) as SieveModule;
  sieveModule.initSync({ module: wasmBytes });
}

// ============================================================================
// CORPUS GENERATION
// ============================================================================

const COMMON_WORDS = ['the', 'be', 'to', 'of', 'and', 'a', 'in', 'that', 'have', 'I', 'it', 'for', 'not', 'on', 'with', 'he', 'as', 'you', 'do', 'at'];
const TECH_WORDS = ['rust', 'async', 'performance', 'api', 'server', 'database', 'cache', 'memory', 'thread', 'concurrent', 'algorithm', 'optimization', 'latency', 'throughput', 'scalable', 'distributed', 'microservice', 'container', 'kubernetes', 'docker'];
const VERBS = ['implement', 'build', 'create', 'optimize', 'deploy', 'scale', 'configure', 'debug', 'test', 'refactor', 'migrate', 'benchmark', 'profile', 'analyze', 'design'];

function generateWord(): string {
  const r = Math.random();
  if (r < 0.6) return COMMON_WORDS[Math.floor(Math.random() * COMMON_WORDS.length)];
  if (r < 0.85) return TECH_WORDS[Math.floor(Math.random() * TECH_WORDS.length)];
  return VERBS[Math.floor(Math.random() * VERBS.length)];
}

function generateText(wordCount: number): string {
  return Array.from({ length: wordCount }, generateWord).join(' ');
}

function generateCorpus(postCount: number, wordsPerPost: number): Document[] {
  return Array.from({ length: postCount }, (_, i) => ({
    id: `post-${i}`,
    title: generateText(8),
    content: generateText(wordsPerPost),
    slug: `post-${i}`,
  }));
}

/**
 * Load European Countries dataset for Sieve comparison.
 */
function loadEuropeanDataset(): Document[] {
  const datasetsDir = join(__dirname, '../datasets');
  const manifestPath = join(datasetsDir, 'manifest.json');
  const manifest: Manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));

  const documents: Document[] = [];
  for (const filename of manifest.documents) {
    const filePath = join(datasetsDir, filename);
    const doc = JSON.parse(readFileSync(filePath, 'utf8'));
    documents.push({
      id: doc.id,
      slug: doc.slug,
      title: doc.title,
      content: doc.text,
    });
  }
  return documents;
}

// ============================================================================
// MEMORY MEASUREMENT
// ============================================================================

function forceGC(): void {
  // Use Bun.gc() if available
  if (typeof Bun !== 'undefined') {
    Bun.gc(true);
    Bun.gc(true);
    Bun.gc(true);
  } else if ((global as unknown as { gc?: () => void }).gc) {
    const g = global as unknown as { gc: () => void };
    g.gc();
    g.gc();
    g.gc();
  }
}

function getHeapUsed(): number {
  forceGC();
  return process.memoryUsage().heapUsed;
}

function formatBytes(bytes: number): string {
  const abs = Math.abs(bytes);
  const sign = bytes < 0 ? '-' : '';
  if (abs < 1024) return `${sign}${abs} B`;
  if (abs < 1024 * 1024) return `${sign}${(abs / 1024).toFixed(1)} KB`;
  return `${sign}${(abs / (1024 * 1024)).toFixed(2)} MB`;
}

async function measureMemory<T>(name: string, createIndex: () => T): Promise<MemoryResult> {
  // Multiple rounds to stabilize heap
  forceGC();
  await new Promise(r => setTimeout(r, 100));
  forceGC();
  await new Promise(r => setTimeout(r, 100));

  const baseline = getHeapUsed();

  // Create index
  const index = createIndex();

  // Force GC multiple times and wait for stabilization
  await new Promise(r => setTimeout(r, 100));
  forceGC();
  await new Promise(r => setTimeout(r, 100));
  forceGC();

  const afterIndex = getHeapUsed();

  // Clamp negative values to 0 with warning
  let indexSize = afterIndex - baseline;
  if (indexSize < 0) {
    console.warn(`  Warning: Negative heap delta for ${name} (GC timing issue)`);
    indexSize = 0;
  }

  return { name, indexSize, baseline, afterIndex, index };
}

// ============================================================================
// BENCHMARKS
// ============================================================================

async function benchmarkSyntheticCorpus(
  label: string,
  posts: number,
  wordsPerPost: number
): Promise<BenchmarkResult> {
  console.log(`\n${'='.repeat(60)}`);
  console.log(`${label}: ${posts} posts x ${wordsPerPost} words`);
  console.log('='.repeat(60));

  const corpus = generateCorpus(posts, wordsPerPost);
  const rawSize = JSON.stringify(corpus).length;
  console.log(`Raw corpus size: ${formatBytes(rawSize)}`);

  const results: MemoryResult[] = [];

  // Fuse.js
  const fuseResult = await measureMemory('fuse.js', () => {
    return new Fuse(corpus, {
      keys: ['title', 'content'],
      includeScore: true,
      threshold: 0.3,
    });
  });
  results.push(fuseResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // Lunr.js
  const lunrResult = await measureMemory('lunr.js', () => {
    return lunr(function() {
      this.ref('id');
      this.field('title', { boost: 10 });
      this.field('content');
      corpus.forEach(doc => this.add(doc));
    });
  });
  results.push(lunrResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // FlexSearch
  const flexResult = await measureMemory('flexsearch', () => {
    const index = new FlexSearch.Document({
      document: {
        id: 'id',
        index: ['title', 'content'],
      },
      tokenize: 'forward',
      cache: true,
    });
    corpus.forEach(doc => index.add(doc));
    return index;
  });
  results.push(flexResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // MiniSearch
  const miniResult = await measureMemory('minisearch', () => {
    const index = new MiniSearch({
      fields: ['title', 'content'],
      storeFields: ['title', 'slug'],
      searchOptions: {
        boost: { title: 2 },
        fuzzy: 0.2,
        prefix: true,
      },
    });
    index.addAll(corpus);
    return index;
  });
  results.push(miniResult);

  // Print results
  console.log('\n| Library | Index Size | vs Raw Data |');
  console.log('|---------|------------|-------------|');
  for (const r of results) {
    const ratio = rawSize > 0 ? (r.indexSize / rawSize).toFixed(1) : 'N/A';
    console.log(`| ${r.name.padEnd(11)} | ${formatBytes(r.indexSize).padStart(10)} | ${ratio}x |`);
  }

  return { label, posts, wordsPerPost, rawSize, results };
}

async function benchmarkEuropeanDataset(): Promise<BenchmarkResult> {
  console.log(`\n${'='.repeat(60)}`);
  console.log('European Countries Dataset (30 documents)');
  console.log('='.repeat(60));

  const corpus = loadEuropeanDataset();
  const rawSize = JSON.stringify(corpus).length;
  console.log(`Raw corpus size: ${formatBytes(rawSize)}`);

  const results: MemoryResult[] = [];

  // Sieve WASM
  const sieveResult = await measureMemory('Sieve', () => {
    return new sieveModule!.SieveSearcher(new Uint8Array(sieveIndexBytes!));
  });
  results.push(sieveResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // Fuse.js
  const fuseResult = await measureMemory('fuse.js', () => {
    return new Fuse(corpus, {
      keys: ['title', 'content'],
      includeScore: true,
      threshold: 0.3,
    });
  });
  results.push(fuseResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // Lunr.js
  const lunrResult = await measureMemory('lunr.js', () => {
    return lunr(function() {
      this.ref('id');
      this.field('title', { boost: 10 });
      this.field('content');
      corpus.forEach(doc => this.add(doc));
    });
  });
  results.push(lunrResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // FlexSearch
  const flexResult = await measureMemory('flexsearch', () => {
    const index = new FlexSearch.Document({
      document: {
        id: 'id',
        index: ['title', 'content'],
      },
      tokenize: 'forward',
      cache: true,
    });
    corpus.forEach(doc => index.add(doc));
    return index;
  });
  results.push(flexResult);
  forceGC();
  await new Promise(r => setTimeout(r, 200));

  // MiniSearch
  const miniResult = await measureMemory('minisearch', () => {
    const index = new MiniSearch({
      fields: ['title', 'content'],
      storeFields: ['title', 'slug'],
      searchOptions: {
        boost: { title: 2 },
        fuzzy: 0.2,
        prefix: true,
      },
    });
    index.addAll(corpus);
    return index;
  });
  results.push(miniResult);

  // Print results
  console.log('\n| Library | Index Size | vs Raw Data |');
  console.log('|---------|------------|-------------|');
  for (const r of results) {
    const ratio = rawSize > 0 ? (r.indexSize / rawSize).toFixed(1) : 'N/A';
    console.log(`| ${r.name.padEnd(11)} | ${formatBytes(r.indexSize).padStart(10)} | ${ratio}x |`);
  }

  return { label: 'European Countries', docCount: 30, rawSize, results };
}

// ============================================================================
// MAIN
// ============================================================================

async function main(): Promise<void> {
  // Check if GC is available
  const hasGC = typeof Bun !== 'undefined' || (global as unknown as { gc?: () => void }).gc;
  if (!hasGC) {
    console.warn('WARNING: GC not available. Memory measurements may be inaccurate.');
    console.warn('  For Node.js: run with --expose-gc flag');
    console.warn('  For Bun: Bun.gc() is available by default\n');
  }

  console.log('Memory Footprint Benchmark');
  console.log(`Runtime: ${typeof Bun !== 'undefined' ? `Bun ${Bun.version}` : `Node.js ${process.version}`}`);
  console.log(`Platform: ${process.platform} ${process.arch}`);
  console.log('');

  // Load Sieve WASM module
  console.log('Loading Sieve WASM...');
  await loadSieveWasm();
  console.log('Sieve WASM loaded');

  const allResults: BenchmarkResult[] = [];

  // European Countries (with Sieve)
  allResults.push(await benchmarkEuropeanDataset());

  // Synthetic corpora (without Sieve - no pre-built index)
  allResults.push(await benchmarkSyntheticCorpus('Small Blog', 20, 500));
  allResults.push(await benchmarkSyntheticCorpus('Medium Blog', 100, 1000));

  // Output JSON for chart generation
  console.log('\n\n--- JSON Results ---');
  console.log(JSON.stringify(allResults, null, 2));
}

main().catch(console.error);
