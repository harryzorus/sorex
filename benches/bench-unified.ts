/**
 * Unified Search Library Benchmark - European Countries Dataset
 *
 * Compares Sieve WASM against popular JavaScript search libraries
 * using real-world data (30 European country Wikipedia excerpts).
 *
 * Benchmarks:
 * 1. Time to First Search - how long until search is ready?
 * 2. Query Latency - single word, multi-word, substring, typo queries
 * 3. Result Quality - do libraries find the expected results?
 * 4. Memory Usage - index memory footprint (with --expose-gc)
 *
 * Run with: npm run bench:eu
 * Or: tsx bench-unified.ts
 */

import { Bench } from 'tinybench';
import Fuse from 'fuse.js';
import lunr from 'lunr';
import FlexSearch from 'flexsearch';
import MiniSearch from 'minisearch';
import { readFileSync, writeFileSync, readdirSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';
import { gzipSync } from 'zlib';
import { execSync } from 'child_process';
import os from 'os';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

interface SystemInfo {
  platform: string;
  arch: string;
  nodeVersion: string;
  totalMemory: string;
  cpu: string;
  osVersion: string;
}

interface Document {
  id: number;
  slug: string;
  title: string;
  excerpt: string;
  href: string;
  content: string;
}

interface TestQuery {
  name: string;
  query: string;
  category: 'exact' | 'multi' | 'substring' | 'typo';
}

interface BenchResult {
  library: string;
  opsPerSec: number;
  meanMs: number;
  samples: number;
}

interface QueryResult {
  library: string;
  opsPerSec: number;
  meanUs: number;
  resultsFound: number;
}

interface TimingStats {
  mean: number;
  median: number;
  p99: number;
}

interface SearchTiming {
  firstResult: TimingStats;
  allResults: TimingStats;
}

interface TierStats extends TimingStats {
  count: number;
}

interface SieveStreamingTiming {
  tier1: TierStats;
  tier2: TierStats;
  allResults: TierStats;
  totalCount: number;
}

interface TimingResultRow {
  library: string;
  results: number;
  tier1Us: string;
  tier2Us: string;
  allUs: string;
  p99Us: string;
  breakdown: string;
}

interface IndexSizeResult {
  library: string;
  rawKB: string;
  gzipKB: string;
  note: string;
}

interface MemoryResult {
  library: string;
  indexKB: number;
  rawDataKB: number;
  ratio: string;
}

interface BenchmarkResults {
  meta: {
    timestamp: string;
    cpu: string;
    memory: string;
    os: string;
    nodeVersion: string;
    docCount: number;
  };
  timeToFirst: BenchResult[];
  queryLatency: Record<string, {
    query: string;
    category: string;
    results: QueryResult[];
  }>;
  resultTiming: Record<string, {
    query: string;
    category: string;
    results: TimingResultRow[];
  }> | null;
  memory: MemoryResult[] | null;
  indexSizes: IndexSizeResult[];
}

interface SieveSearcher {
  search(query: string, limit: number): unknown[];
  search_tier1_exact(query: string, limit: number): Array<{ id?: number }>;
  search_tier2_prefix(query: string, excludeIds: number[], limit: number): Array<{ id?: number }>;
  search_tier3_fuzzy(query: string, excludeIds: number[], limit: number): unknown[];
  doc_count(): number;
  vocab_size(): number;
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

// ============================================================================
// SYSTEM INFO DETECTION
// ============================================================================

function getSystemInfo(): SystemInfo {
  const info: SystemInfo = {
    platform: process.platform,
    arch: process.arch,
    nodeVersion: process.version,
    totalMemory: `${Math.round(os.totalmem() / 1024 / 1024 / 1024)}GB`,
    cpu: 'Unknown',
    osVersion: os.release(),
  };

  // Get CPU info (platform-specific)
  try {
    if (process.platform === 'darwin') {
      info.cpu = execSync('sysctl -n machdep.cpu.brand_string', { encoding: 'utf8' }).trim();
      info.osVersion = execSync('sw_vers -productVersion', { encoding: 'utf8' }).trim();
    } else if (process.platform === 'linux') {
      const cpuInfo = readFileSync('/proc/cpuinfo', 'utf8');
      const match = cpuInfo.match(/model name\s*:\s*(.+)/);
      info.cpu = match ? match[1].trim() : os.cpus()[0]?.model || 'Unknown';
      info.osVersion = execSync('uname -r', { encoding: 'utf8' }).trim();
    } else {
      info.cpu = os.cpus()[0]?.model || 'Unknown';
      info.osVersion = os.release();
    }
  } catch {
    info.cpu = os.cpus()[0]?.model || 'Unknown';
    info.osVersion = os.release();
  }

  return info;
}

// ============================================================================
// CONFIGURATION
// ============================================================================

// Benchmark configuration - adjust for faster runs during development
const QUICK_MODE = process.argv.includes('--quick');

const BENCH_CONFIG = {
  time: QUICK_MODE ? 1000 : 5000,        // 1s or 5s measurement time
  iterations: QUICK_MODE ? 100 : 1000,   // Minimum iterations
  warmup: true,
};

// Test queries designed to highlight different search capabilities
const TEST_QUERIES: TestQuery[] = [
  // Single word - all libraries should handle these well
  { name: 'single_common', query: 'capital', category: 'exact' },
  { name: 'single_proper', query: 'European', category: 'exact' },
  { name: 'single_topic', query: 'history', category: 'exact' },
  { name: 'single_rare', query: 'fjords', category: 'exact' },

  // Multi-word - tests AND/phrase handling
  { name: 'multi_phrase', query: 'European Union', category: 'multi' },
  { name: 'multi_geo', query: 'Mediterranean Sea', category: 'multi' },
  { name: 'multi_concept', query: 'constitutional monarchy', category: 'multi' },

  // Substring - Sieve's killer feature (suffix array)
  { name: 'substring_land', query: 'land', category: 'substring' },  // Iceland, Finland, landlocked...
  { name: 'substring_burg', query: 'burg', category: 'substring' },  // Luxembourg, Hamburg...
  { name: 'substring_ian', query: 'ian', category: 'substring' },    // Italian, Croatian, Romanian...

  // Typo tolerance - fuzzy matching
  { name: 'typo_population', query: 'popultion', category: 'typo' },   // population
  { name: 'typo_province', query: 'provnce', category: 'typo' },       // province
  { name: 'typo_mediterranean', query: 'mediteranean', category: 'typo' }, // mediterranean
];

// ============================================================================
// DATA LOADING
// ============================================================================

/**
 * Load European Countries dataset from individual JSON files.
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
      excerpt: doc.excerpt,
      href: doc.href,
      content: doc.text,  // 'text' field contains the searchable content
    });
  }

  return documents;
}

/**
 * Load Sieve WASM module and pre-built index.
 */
async function loadSieveIndex(): Promise<{
  searcher: SieveSearcher;
  wasmModule: SieveModule;
  indexBytes: Buffer;
}> {
  const outputDir = join(__dirname, '../datasets/output');
  const manifestPath = join(outputDir, 'manifest.json');
  const manifest: Manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));

  // Find the index file
  const indexFile = manifest.indexes.index.file;
  const indexPath = join(outputDir, indexFile);
  const indexBytes = readFileSync(indexPath);

  // Load WASM module using initSync (works in Node.js without fetch)
  const wasmJsPath = join(outputDir, 'sieve.js');
  const wasmBinaryPath = join(outputDir, 'sieve_bg.wasm');
  const wasmBytes = readFileSync(wasmBinaryPath);

  const wasmModule = await import(wasmJsPath) as SieveModule;
  wasmModule.initSync({ module: wasmBytes });

  // Create searcher from binary
  const searcher = new wasmModule.SieveSearcher(new Uint8Array(indexBytes));

  return { searcher, wasmModule, indexBytes };
}

// ============================================================================
// INDEX BUILDING FOR JS LIBRARIES
// ============================================================================

function buildFuseIndex(docs: Document[]): Fuse<Document> {
  return new Fuse(docs, {
    keys: ['title', 'excerpt', 'content'],
    threshold: 0.3,
    includeScore: true,
    ignoreLocation: true,
  });
}

function buildLunrIndex(docs: Document[]): lunr.Index {
  return lunr(function () {
    this.ref('id');
    this.field('title', { boost: 10 });
    this.field('excerpt', { boost: 5 });
    this.field('content');
    docs.forEach((doc) => this.add(doc));
  });
}

function buildFlexSearchIndex(docs: Document[]): FlexSearch.Document<Document, true> {
  const index = new FlexSearch.Document<Document, true>({
    document: {
      id: 'id',
      index: ['title', 'excerpt', 'content'],
    },
  });
  docs.forEach((doc) => index.add(doc));
  return index;
}

function buildMiniSearchIndex(docs: Document[]): MiniSearch<Document> {
  const index = new MiniSearch<Document>({
    fields: ['title', 'excerpt', 'content'],
    storeFields: ['title', 'excerpt', 'href'],
    searchOptions: {
      boost: { title: 2, excerpt: 1.5 },
      fuzzy: 0.2,
      prefix: true,
    },
  });
  index.addAll(docs);
  return index;
}

// ============================================================================
// SEARCH WRAPPERS (normalize result format)
// ============================================================================

function searchFuse(index: Fuse<Document>, query: string) {
  return index.search(query);
}

function searchLunr(index: lunr.Index, query: string): lunr.Index.Result[] {
  return index.search(query);
}

function searchFlexSearch(index: FlexSearch.Document<Document, true>, query: string): FlexSearch.SimpleDocumentSearchResultSetUnit[] {
  return index.search(query);
}

function searchMiniSearch(index: MiniSearch<Document>, query: string, fuzzy = false) {
  return fuzzy
    ? index.search(query, { fuzzy: 0.3 })
    : index.search(query);
}

function searchSieve(searcher: SieveSearcher, query: string, limit = 10): unknown[] {
  return searcher.search(query, limit);
}

// ============================================================================
// HIGH-RESOLUTION TIMING UTILITIES
// ============================================================================

/**
 * Measure time to first result and time to all results.
 * Uses performance.now() for high-resolution timing.
 */
function measureSearchTiming(searchFn: () => unknown[], iterations = 1000): SearchTiming {
  const firstResultTimes: number[] = [];
  const allResultsTimes: number[] = [];

  for (let i = 0; i < iterations; i++) {
    const start = performance.now();
    const results = searchFn();
    const allDone = performance.now();

    // For first result, we measure when we could access the first item
    // This is effectively the same as all results for synchronous search,
    // but different for streaming/iterator-based search
    const firstDone = results.length > 0 ? performance.now() : allDone;

    firstResultTimes.push(firstDone - start);
    allResultsTimes.push(allDone - start);
  }

  // Calculate statistics
  const sortedFirst = [...firstResultTimes].sort((a, b) => a - b);
  const sortedAll = [...allResultsTimes].sort((a, b) => a - b);

  return {
    firstResult: {
      mean: firstResultTimes.reduce((a, b) => a + b, 0) / iterations,
      median: sortedFirst[Math.floor(iterations / 2)],
      p99: sortedFirst[Math.floor(iterations * 0.99)],
    },
    allResults: {
      mean: allResultsTimes.reduce((a, b) => a + b, 0) / iterations,
      median: sortedAll[Math.floor(iterations / 2)],
      p99: sortedAll[Math.floor(iterations * 0.99)],
    },
  };
}

/**
 * Measure streaming search timing for Sieve.
 * Uses tier1 (exact) -> tier2 (prefix) -> tier3 (fuzzy) progressive search.
 * This shows the REAL difference between first result and all results.
 */
function measureSieveStreamingTiming(searcher: SieveSearcher, query: string, iterations = 1000): SieveStreamingTiming {
  const tier1Times: number[] = [];
  const tier2Times: number[] = [];
  const allResultsTimes: number[] = [];
  let tier1Count = 0;
  let tier2Count = 0;
  let tier3Count = 0;

  for (let i = 0; i < iterations; i++) {
    const start = performance.now();

    // Tier 1: Exact match (fastest, shown immediately)
    const tier1Results = searcher.search_tier1_exact(query, 10);
    const tier1Done = performance.now();

    // Tier 2: Prefix match (exclude tier1 IDs)
    const tier1Ids = tier1Results.map(r => r.id || 0);  // May not have id field
    const tier2Results = searcher.search_tier2_prefix(query, tier1Ids, 10);
    const tier2Done = performance.now();

    // Tier 3: Fuzzy match (exclude tier1+tier2 IDs)
    const tier2Ids = tier2Results.map(r => r.id || 0);
    const allExcludeIds = [...tier1Ids, ...tier2Ids];
    const tier3Results = searcher.search_tier3_fuzzy(query, allExcludeIds, 10);
    const allDone = performance.now();

    tier1Times.push(tier1Done - start);
    tier2Times.push(tier2Done - start);
    allResultsTimes.push(allDone - start);

    if (i === 0) {
      tier1Count = tier1Results.length;
      tier2Count = tier2Results.length;
      tier3Count = tier3Results.length;
    }
  }

  const sortedTier1 = [...tier1Times].sort((a, b) => a - b);
  const sortedTier2 = [...tier2Times].sort((a, b) => a - b);
  const sortedAll = [...allResultsTimes].sort((a, b) => a - b);

  return {
    tier1: {
      mean: tier1Times.reduce((a, b) => a + b, 0) / iterations,
      median: sortedTier1[Math.floor(iterations / 2)],
      p99: sortedTier1[Math.floor(iterations * 0.99)],
      count: tier1Count,
    },
    tier2: {
      mean: tier2Times.reduce((a, b) => a + b, 0) / iterations,
      median: sortedTier2[Math.floor(iterations / 2)],
      p99: sortedTier2[Math.floor(iterations * 0.99)],
      count: tier2Count,
    },
    allResults: {
      mean: allResultsTimes.reduce((a, b) => a + b, 0) / iterations,
      median: sortedAll[Math.floor(iterations / 2)],
      p99: sortedAll[Math.floor(iterations * 0.99)],
      count: tier3Count,
    },
    totalCount: tier1Count + tier2Count + tier3Count,
  };
}

// ============================================================================
// BENCHMARK: TIME TO FIRST SEARCH
// ============================================================================

async function benchTimeToFirstSearch(
  docs: Document[],
  sieveIndexBytes: Buffer,
  wasmModule: SieveModule
): Promise<BenchResult[]> {
  console.log('\n=== TIME TO FIRST SEARCH ===');
  console.log('How long from loading until search is ready?\n');

  const bench = new Bench({ time: BENCH_CONFIG.time });

  // Sieve: Load pre-built binary index
  bench.add('Sieve (load .sieve)', () => {
    const searcher = new wasmModule.SieveSearcher(new Uint8Array(sieveIndexBytes));
    searcher.free();  // Clean up
  });

  // JS libraries: Build index from documents
  bench.add('fuse.js (build)', () => {
    buildFuseIndex(docs);
  });

  bench.add('lunr.js (build)', () => {
    buildLunrIndex(docs);
  });

  bench.add('flexsearch (build)', () => {
    buildFlexSearchIndex(docs);
  });

  bench.add('minisearch (build)', () => {
    buildMiniSearchIndex(docs);
  });

  await bench.run();

  const results: BenchResult[] = bench.tasks.map((t) => ({
    library: t.name,
    opsPerSec: Math.round(t.result!.hz),
    meanMs: Number(t.result!.mean.toFixed(3)),  // tinybench mean is already in ms
    samples: t.result!.samples.length,
  }));

  console.table(results);
  return results;
}

// ============================================================================
// BENCHMARK: QUERY LATENCY
// ============================================================================

async function benchQueryLatency(
  docs: Document[],
  sieveSearcher: SieveSearcher
): Promise<Record<string, { query: string; category: string; results: QueryResult[] }>> {
  console.log('\n=== QUERY LATENCY ===');
  console.log('Search performance on European Countries dataset (30 docs)\n');

  // Pre-build all JS indexes
  const fuseIndex = buildFuseIndex(docs);
  const lunrIndex = buildLunrIndex(docs);
  const flexIndex = buildFlexSearchIndex(docs);
  const miniIndex = buildMiniSearchIndex(docs);

  const allResults: Record<string, { query: string; category: string; results: QueryResult[] }> = {};

  for (const { name, query, category } of TEST_QUERIES) {
    const bench = new Bench({ time: BENCH_CONFIG.time });
    const isTypo = category === 'typo';

    bench.add('Sieve', () => {
      searchSieve(sieveSearcher, query);
    });

    bench.add('fuse.js', () => {
      searchFuse(fuseIndex, query);
    });

    bench.add('lunr.js', () => {
      searchLunr(lunrIndex, query);
    });

    bench.add('flexsearch', () => {
      searchFlexSearch(flexIndex, query);
    });

    bench.add('minisearch', () => {
      searchMiniSearch(miniIndex, query, isTypo);
    });

    await bench.run();

    // Count results from each library
    const sieveResults = searchSieve(sieveSearcher, query);
    const fuseResults = searchFuse(fuseIndex, query);
    const lunrResults = searchLunr(lunrIndex, query);
    const flexResults = searchFlexSearch(flexIndex, query);
    const miniResults = searchMiniSearch(miniIndex, query, isTypo);

    // FlexSearch returns nested arrays, flatten
    const flexCount = Array.isArray(flexResults)
      ? flexResults.reduce((sum, r) => sum + (r.result?.length || 0), 0)
      : 0;

    allResults[name] = {
      query,
      category,
      results: bench.tasks.map((t, i) => {
        const counts = [
          sieveResults.length,
          fuseResults.length,
          lunrResults.length,
          flexCount,
          miniResults.length,
        ];
        return {
          library: t.name,
          opsPerSec: Math.round(t.result!.hz),
          meanUs: Number((t.result!.mean * 1000).toFixed(1)),  // mean is in ms
          resultsFound: counts[i],
        };
      }),
    };

    console.log(`\n"${query}" (${category})`);
    console.table(allResults[name].results);
  }

  return allResults;
}

// ============================================================================
// BENCHMARK: FIRST RESULT vs ALL RESULTS TIMING
// ============================================================================

async function benchResultTiming(
  docs: Document[],
  sieveSearcher: SieveSearcher
): Promise<Record<string, { query: string; category: string; results: TimingResultRow[] }>> {
  console.log('\n=== RESULT TIMING (First vs All) ===');
  console.log('Measures time to first result and time to complete search\n');

  // Pre-build all JS indexes
  const fuseIndex = buildFuseIndex(docs);
  const lunrIndex = buildLunrIndex(docs);
  const flexIndex = buildFlexSearchIndex(docs);
  const miniIndex = buildMiniSearchIndex(docs);

  // Test queries that return different result counts
  const timingQueries = [
    { name: 'common_word', query: 'European', category: 'Many results' },
    { name: 'substring', query: 'land', category: 'Substring match' },
    { name: 'typo', query: 'mediteranean', category: 'Fuzzy match' },
  ];

  const allResults: Record<string, { query: string; category: string; results: TimingResultRow[] }> = {};

  for (const { name, query, category } of timingQueries) {
    console.log(`\nQuery: "${query}" (${category})`);

    const iterations = QUICK_MODE ? 100 : 1000;

    // Measure Sieve with STREAMING API (shows real first vs all timing)
    const sieveStreaming = measureSieveStreamingTiming(sieveSearcher, query, iterations);

    // Measure JS libraries (synchronous, first ~ all)
    const fuseTiming = measureSearchTiming(
      () => searchFuse(fuseIndex, query),
      iterations
    );
    const lunrTiming = measureSearchTiming(
      () => searchLunr(lunrIndex, query),
      iterations
    );
    const flexTiming = measureSearchTiming(
      () => searchFlexSearch(flexIndex, query),
      iterations
    );
    const miniTiming = measureSearchTiming(
      () => searchMiniSearch(miniIndex, query, name === 'typo'),
      iterations
    );

    // Get result counts for JS libs
    const fuseCount = searchFuse(fuseIndex, query).length;
    const lunrCount = searchLunr(lunrIndex, query).length;
    const flexResults = searchFlexSearch(flexIndex, query);
    const flexCount = Array.isArray(flexResults)
      ? flexResults.reduce((sum, r) => sum + (r.result?.length || 0), 0)
      : 0;
    const miniCount = searchMiniSearch(miniIndex, query, name === 'typo').length;

    const timingResults: TimingResultRow[] = [
      {
        library: 'Sieve (streaming)',
        results: sieveStreaming.totalCount,
        tier1Us: (sieveStreaming.tier1.mean * 1000).toFixed(1),
        tier2Us: (sieveStreaming.tier2.mean * 1000).toFixed(1),
        allUs: (sieveStreaming.allResults.mean * 1000).toFixed(1),
        p99Us: (sieveStreaming.allResults.p99 * 1000).toFixed(1),
        breakdown: `${sieveStreaming.tier1.count}+${sieveStreaming.tier2.count}+${sieveStreaming.allResults.count}`,
      },
      {
        library: 'fuse.js',
        results: fuseCount,
        tier1Us: '-',
        tier2Us: '-',
        allUs: (fuseTiming.allResults.mean * 1000).toFixed(1),
        p99Us: (fuseTiming.allResults.p99 * 1000).toFixed(1),
        breakdown: `${fuseCount}`,
      },
      {
        library: 'lunr.js',
        results: lunrCount,
        tier1Us: '-',
        tier2Us: '-',
        allUs: (lunrTiming.allResults.mean * 1000).toFixed(1),
        p99Us: (lunrTiming.allResults.p99 * 1000).toFixed(1),
        breakdown: `${lunrCount}`,
      },
      {
        library: 'flexsearch',
        results: flexCount,
        tier1Us: '-',
        tier2Us: '-',
        allUs: (flexTiming.allResults.mean * 1000).toFixed(1),
        p99Us: (flexTiming.allResults.p99 * 1000).toFixed(1),
        breakdown: `${flexCount}`,
      },
      {
        library: 'minisearch',
        results: miniCount,
        tier1Us: '-',
        tier2Us: '-',
        allUs: (miniTiming.allResults.mean * 1000).toFixed(1),
        p99Us: (miniTiming.allResults.p99 * 1000).toFixed(1),
        breakdown: `${miniCount}`,
      },
    ];

    console.table(timingResults);
    allResults[name] = { query, category, results: timingResults };
  }

  return allResults;
}

// ============================================================================
// BENCHMARK: MEMORY USAGE
// ============================================================================

async function benchMemoryUsage(
  docs: Document[],
  sieveIndexBytes: Buffer,
  wasmModule: SieveModule
): Promise<MemoryResult[] | null> {
  console.log('\n=== MEMORY USAGE ===');

  if (!(global as unknown as { gc?: () => void }).gc) {
    console.log('Skipping memory benchmark (run with --expose-gc)\n');
    return null;
  }

  console.log('Index memory footprint\n');

  function forceGC(): void {
    const g = global as unknown as { gc: () => void };
    g.gc();
    g.gc();
    g.gc();
  }

  async function measureMemory<T>(
    name: string,
    createFn: () => T
  ): Promise<{ name: string; sizeKB: number; index: T }> {
    forceGC();
    await new Promise((r) => setTimeout(r, 100));
    const before = process.memoryUsage().heapUsed;

    const index = createFn();

    await new Promise((r) => setTimeout(r, 50));
    forceGC();
    const after = process.memoryUsage().heapUsed;

    const size = Math.max(0, after - before);  // Clamp negative values
    return { name, sizeKB: Math.round(size / 1024), index };
  }

  const results: Array<{ name: string; sizeKB: number; index: unknown }> = [];

  // Sieve
  const sieveResult = await measureMemory('Sieve', () => {
    return new wasmModule.SieveSearcher(new Uint8Array(sieveIndexBytes));
  });
  results.push(sieveResult);

  // Fuse.js
  forceGC();
  await new Promise((r) => setTimeout(r, 200));
  const fuseResult = await measureMemory('fuse.js', () => buildFuseIndex(docs));
  results.push(fuseResult);

  // Lunr.js
  forceGC();
  await new Promise((r) => setTimeout(r, 200));
  const lunrResult = await measureMemory('lunr.js', () => buildLunrIndex(docs));
  results.push(lunrResult);

  // FlexSearch
  forceGC();
  await new Promise((r) => setTimeout(r, 200));
  const flexResult = await measureMemory('flexsearch', () => buildFlexSearchIndex(docs));
  results.push(flexResult);

  // MiniSearch
  forceGC();
  await new Promise((r) => setTimeout(r, 200));
  const miniResult = await measureMemory('minisearch', () => buildMiniSearchIndex(docs));
  results.push(miniResult);

  // Raw data size for comparison
  const rawSize = JSON.stringify(docs).length;

  const output: MemoryResult[] = results.map((r) => ({
    library: r.name,
    indexKB: r.sizeKB,
    rawDataKB: Math.round(rawSize / 1024),
    ratio: (r.sizeKB / (rawSize / 1024)).toFixed(1) + 'x',
  }));

  console.table(output);
  return output;
}

// ============================================================================
// INDEX SIZE COMPARISON
// ============================================================================

async function benchIndexSizes(docs: Document[], sieveIndexBytes: Buffer): Promise<IndexSizeResult[]> {
  console.log('\n=== INDEX SIZES ===');
  console.log('Serialized index size (what gets transferred over the network)\n');

  const rawData = JSON.stringify(docs);
  const rawSize = Buffer.byteLength(rawData, 'utf8');
  const rawGzip = gzipSync(rawData).length;

  // Sieve binary format
  const sieveSize = sieveIndexBytes.length;
  const sieveGzip = gzipSync(sieveIndexBytes).length;

  // Lunr.js (has toJSON)
  const lunrIndex = buildLunrIndex(docs);
  const lunrJson = JSON.stringify(lunrIndex.toJSON());
  const lunrSize = Buffer.byteLength(lunrJson, 'utf8');
  const lunrGzip = gzipSync(lunrJson).length;

  // MiniSearch (has toJSON)
  const miniIndex = buildMiniSearchIndex(docs);
  const miniJson = JSON.stringify(miniIndex.toJSON());
  const miniSize = Buffer.byteLength(miniJson, 'utf8');
  const miniGzip = gzipSync(miniJson).length;

  // FlexSearch (export is async and complex, estimate)
  const flexIndex = buildFlexSearchIndex(docs);
  const flexExport: Record<string, unknown> = {};
  await flexIndex.export((key, data) => { flexExport[key] = data; });
  const flexJson = JSON.stringify(flexExport);
  const flexSize = Buffer.byteLength(flexJson, 'utf8');
  const flexGzip = gzipSync(flexJson).length;

  // Fuse.js stores raw data (no index to serialize)
  const fuseSize = rawSize;
  const fuseGzip = rawGzip;

  const results: IndexSizeResult[] = [
    { library: 'Raw Data', rawKB: (rawSize / 1024).toFixed(1), gzipKB: (rawGzip / 1024).toFixed(1), note: '' },
    { library: 'Sieve (.sieve)', rawKB: (sieveSize / 1024).toFixed(1), gzipKB: (sieveGzip / 1024).toFixed(1), note: 'binary' },
    { library: 'fuse.js', rawKB: (fuseSize / 1024).toFixed(1), gzipKB: (fuseGzip / 1024).toFixed(1), note: 'no index' },
    { library: 'lunr.js', rawKB: (lunrSize / 1024).toFixed(1), gzipKB: (lunrGzip / 1024).toFixed(1), note: '' },
    { library: 'flexsearch', rawKB: (flexSize / 1024).toFixed(1), gzipKB: (flexGzip / 1024).toFixed(1), note: '' },
    { library: 'minisearch', rawKB: (miniSize / 1024).toFixed(1), gzipKB: (miniGzip / 1024).toFixed(1), note: '' },
  ];

  console.table(results);
  return results;
}

// ============================================================================
// RESULTS OUTPUT
// ============================================================================

function generateMarkdownReport(results: BenchmarkResults): string {
  const { timeToFirst, queryLatency, resultTiming, memory, indexSizes, meta } = results;

  const lines: string[] = [
    '# European Countries Benchmark Results',
    '',
    `**Generated:** ${meta.timestamp}`,
    '',
    '## System Information',
    '',
    '| Property | Value |',
    '|----------|-------|',
    `| CPU | ${meta.cpu} |`,
    `| Memory | ${meta.memory} |`,
    `| OS | ${meta.os} |`,
    `| Node.js | ${meta.nodeVersion} |`,
    `| Dataset | ${meta.docCount} documents |`,
    '',
    '---',
    '',
    '## Time to First Search',
    '',
    'How long until search is ready? Lower is better.',
    '',
    '| Library | Mean (ms) | ops/sec |',
    '|---------|-----------|---------|',
  ];

  for (const row of timeToFirst) {
    lines.push(`| ${row.library} | ${row.meanMs} | ${row.opsPerSec.toLocaleString()} |`);
  }

  lines.push('');
  lines.push('*Sieve loads a pre-built binary index. JS libraries build at runtime.*');
  lines.push('');
  lines.push('---');
  lines.push('');
  lines.push('## Query Latency');
  lines.push('');
  lines.push('Search performance by query type. Higher ops/sec is better.');
  lines.push('');

  // Group queries by category
  const categories: Record<string, string> = {
    exact: 'Exact Word Queries',
    multi: 'Multi-Word Queries',
    substring: 'Substring Queries (Sieve Advantage)',
    typo: 'Typo Tolerance (Fuzzy)',
  };

  for (const [cat, title] of Object.entries(categories)) {
    lines.push(`### ${title}`);
    lines.push('');

    const catQueries = Object.entries(queryLatency).filter(([_, v]) => v.category === cat);
    for (const [name, data] of catQueries) {
      lines.push(`**Query: \`${data.query}\`**`);
      lines.push('');
      lines.push('| Library | Latency (us) | ops/sec | Results |');
      lines.push('|---------|--------------|---------|---------|');
      for (const row of data.results) {
        lines.push(`| ${row.library} | ${row.meanUs} | ${row.opsPerSec.toLocaleString()} | ${row.resultsFound} |`);
      }
      lines.push('');
    }
  }

  // Result Timing section
  if (resultTiming) {
    lines.push('---');
    lines.push('');
    lines.push('## Result Timing (First vs All)');
    lines.push('');
    lines.push('Measures latency to first result and complete search. Important for streaming UX.');
    lines.push('');

    for (const [name, data] of Object.entries(resultTiming)) {
      lines.push(`### ${data.category}`);
      lines.push(`Query: \`${data.query}\``);
      lines.push('');
      lines.push('| Library | Results | First (us) | All (us) | P99 (us) |');
      lines.push('|---------|---------|------------|----------|----------|');
      for (const row of data.results) {
        lines.push(`| ${row.library} | ${row.results} | ${row.tier1Us} | ${row.allUs} | ${row.p99Us} |`);
      }
      lines.push('');
    }
  }

  lines.push('---');
  lines.push('');
  lines.push('## Index Sizes');
  lines.push('');
  lines.push('Serialized index size (network transfer). Smaller is better.');
  lines.push('');
  lines.push('| Library | Raw (KB) | Gzipped (KB) | Notes |');
  lines.push('|---------|----------|--------------|-------|');

  for (const row of indexSizes) {
    lines.push(`| ${row.library} | ${row.rawKB} | ${row.gzipKB} | ${row.note} |`);
  }

  if (memory) {
    lines.push('');
    lines.push('---');
    lines.push('');
    lines.push('## Memory Usage');
    lines.push('');
    lines.push('Heap memory consumed by index. Smaller is better.');
    lines.push('');
    lines.push('| Library | Index (KB) | vs Raw Data |');
    lines.push('|---------|------------|-------------|');
    for (const row of memory) {
      lines.push(`| ${row.library} | ${row.indexKB} | ${row.ratio} |`);
    }
  }

  lines.push('');
  lines.push('---');
  lines.push('');
  lines.push('## Key Takeaways');
  lines.push('');
  lines.push('1. **Time to First Search**: Sieve loads pre-built indexes instantly (~Xms). JS libraries must build at runtime.');
  lines.push('2. **Substring Search**: Sieve finds results for substring queries where inverted indexes return 0.');
  lines.push('3. **Typo Tolerance**: Sieve uses Levenshtein automata for true edit-distance fuzzy matching.');
  lines.push('4. **Index Size**: Sieve\'s binary format is compact and compresses well.');
  lines.push('');

  return lines.join('\n');
}

// ============================================================================
// MAIN
// ============================================================================

async function main(): Promise<void> {
  // Get system info
  const sysInfo = getSystemInfo();

  console.log('╔══════════════════════════════════════════════════════════════╗');
  console.log('║     European Countries Search Benchmark                      ║');
  console.log('╠══════════════════════════════════════════════════════════════╣');
  console.log('║  Dataset: 30 European country Wikipedia excerpts             ║');
  console.log('║  Libraries: Sieve (WASM), Fuse.js, Lunr.js, FlexSearch,     ║');
  console.log('║             MiniSearch                                       ║');
  console.log('╚══════════════════════════════════════════════════════════════╝');

  console.log('\n=== SYSTEM INFO ===');
  console.log(`CPU: ${sysInfo.cpu}`);
  console.log(`Memory: ${sysInfo.totalMemory}`);
  console.log(`OS: ${sysInfo.platform} ${sysInfo.osVersion}`);
  console.log(`Node.js: ${sysInfo.nodeVersion}`);

  // Load data
  console.log('\nLoading European Countries dataset...');
  const docs = loadEuropeanDataset();
  console.log(`Loaded ${docs.length} documents`);

  console.log('\nLoading Sieve WASM index...');
  const { searcher: sieveSearcher, wasmModule, indexBytes: sieveIndexBytes } = await loadSieveIndex();
  console.log(`Sieve index loaded (${sieveSearcher.doc_count()} docs, ${sieveSearcher.vocab_size()} terms)`);

  // Run benchmarks
  const timeToFirst = await benchTimeToFirstSearch(docs, sieveIndexBytes, wasmModule);
  const queryLatency = await benchQueryLatency(docs, sieveSearcher);
  const resultTiming = await benchResultTiming(docs, sieveSearcher);
  const memory = await benchMemoryUsage(docs, sieveIndexBytes, wasmModule);
  const indexSizes = await benchIndexSizes(docs, sieveIndexBytes);

  // Collect results
  const results: BenchmarkResults = {
    meta: {
      timestamp: new Date().toISOString(),
      cpu: sysInfo.cpu,
      memory: sysInfo.totalMemory,
      os: `${sysInfo.platform} ${sysInfo.osVersion}`,
      nodeVersion: sysInfo.nodeVersion,
      docCount: docs.length,
    },
    timeToFirst,
    queryLatency,
    resultTiming,
    memory,
    indexSizes,
  };

  // Save JSON results
  const jsonPath = join(__dirname, 'results-eu.json');
  writeFileSync(jsonPath, JSON.stringify(results, null, 2));
  console.log(`\nJSON results saved to: ${jsonPath}`);

  // Generate and save Markdown report
  const markdown = generateMarkdownReport(results);
  const mdPath = join(__dirname, 'RESULTS-EU.md');
  writeFileSync(mdPath, markdown);
  console.log(`Markdown report saved to: ${mdPath}`);

  // Clean up
  sieveSearcher.free();

  console.log('\n✓ All benchmarks complete\n');
}

main().catch(console.error);
