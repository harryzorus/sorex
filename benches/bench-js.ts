/**
 * Browser-based search library benchmarks for blog-scale corpora
 *
 * Simulates realistic blog sizes:
 * - Small blog:  ~20 posts, ~500 words each  (personal blog)
 * - Medium blog: ~100 posts, ~1000 words each (active blogger)
 * - Large blog:  ~500 posts, ~1500 words each (publication)
 *
 * Libraries compared:
 * - Fuse.js: Fuzzy search with scoring
 * - Lunr.js: Full-text search with stemming (Lucene-like)
 * - FlexSearch: Fast full-text search with flexible configuration
 * - MiniSearch: Lightweight full-text search
 *
 * Run with: npm run bench
 */

import { Bench } from 'tinybench';
import Fuse from 'fuse.js';
import lunr from 'lunr';
import FlexSearch from 'flexsearch';
import MiniSearch from 'minisearch';
import { writeFileSync } from 'fs';
import { fileURLToPath } from 'url';
import { dirname, join } from 'path';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

interface BlogPost {
  id: number;
  title: string;
  excerpt: string;
  content: string;
  href: string;
  tags: string[];
}

interface BlogSizeConfig {
  posts: number;
  wordsPerPost: number;
  name: string;
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

interface FuzzyResult {
  library: string;
  opsPerSec: number;
  meanUs: number;
  foundResults: number;
}

interface MemoryMeasurement {
  library: string;
  indexKB: number;
  rawDataKB: number;
}

interface BenchmarkResults {
  timestamp: string;
  system: {
    node: string;
    platform: string;
    arch: string;
  };
  indexBuilding: Record<string, BenchResult[]>;
  searchQueries: Record<string, {
    query: string;
    description: string;
    category: string;
    results: QueryResult[];
  }>;
  fuzzySearch: Record<string, {
    typo: string;
    correct: string;
    editDistance: number;
    results: FuzzyResult[];
  }>;
  memoryUsage: Record<string, MemoryMeasurement[]>;
}

// ============================================================================
// BLOG CORPUS SIMULATION
// ============================================================================

// Realistic blog content vocabulary by category
const VOCABULARY = {
  technical: [
    'rust', 'programming', 'typescript', 'javascript', 'python', 'golang',
    'kubernetes', 'docker', 'serverless', 'microservices', 'api', 'database',
    'postgresql', 'redis', 'mongodb', 'graphql', 'rest', 'websocket',
    'authentication', 'authorization', 'encryption', 'security', 'performance',
    'optimization', 'caching', 'indexing', 'algorithm', 'data', 'structure',
    'binary', 'tree', 'hash', 'map', 'array', 'vector', 'queue', 'stack',
    'concurrency', 'parallelism', 'async', 'await', 'promise', 'future',
    'memory', 'allocation', 'garbage', 'collection', 'ownership', 'borrowing',
    'lifetime', 'trait', 'interface', 'generic', 'type', 'inference',
    'compiler', 'runtime', 'interpreter', 'virtual', 'machine', 'bytecode',
    'wasm', 'webassembly', 'browser', 'node', 'deno', 'bun', 'framework',
  ],
  general: [
    'the', 'a', 'an', 'is', 'are', 'was', 'were', 'be', 'been', 'being',
    'have', 'has', 'had', 'do', 'does', 'did', 'will', 'would', 'could',
    'should', 'may', 'might', 'must', 'shall', 'can', 'need', 'dare',
    'this', 'that', 'these', 'those', 'which', 'what', 'who', 'whom',
    'how', 'when', 'where', 'why', 'because', 'although', 'however',
    'therefore', 'moreover', 'furthermore', 'nevertheless', 'consequently',
    'application', 'system', 'solution', 'approach', 'method', 'technique',
    'implementation', 'development', 'engineering', 'architecture', 'design',
    'pattern', 'practice', 'principle', 'concept', 'idea', 'theory',
  ],
  transitions: [
    'first', 'second', 'third', 'finally', 'next', 'then', 'after',
    'before', 'during', 'while', 'meanwhile', 'subsequently', 'previously',
    'in conclusion', 'to summarize', 'for example', 'for instance',
    'in other words', 'that is', 'specifically', 'particularly',
  ],
};

// Blog sizes to benchmark (skip large for faster runs)
const BLOG_SIZES: Record<string, BlogSizeConfig> = {
  small: { posts: 20, wordsPerPost: 500, name: 'Small Blog (~20 posts)' },
  medium: { posts: 100, wordsPerPost: 1000, name: 'Medium Blog (~100 posts)' },
  // large: skipped - takes too long
};

// Benchmark configuration for tight confidence intervals
const BENCH_CONFIG = {
  time: 5000,        // 5s measurement time (vs default 500ms)
  iterations: 1000,  // Minimum iterations
  warmup: true,      // Always warm up
};

// Titles that look like real blog posts
const TITLE_TEMPLATES = [
  'How to Build a {tech} {thing}',
  'Understanding {concept} in {tech}',
  '{concept}: A Deep Dive',
  'Why I Switched from {tech} to {tech2}',
  'The Complete Guide to {concept}',
  '{number} Tips for Better {concept}',
  'Building {thing} with {tech}',
  '{tech} vs {tech2}: Which Should You Choose?',
  'My Journey Learning {tech}',
  'Optimizing {thing} Performance',
];

const TITLE_PARTS: Record<string, string[]> = {
  tech: ['Rust', 'TypeScript', 'Go', 'Python', 'React', 'Svelte', 'Docker', 'Kubernetes'],
  tech2: ['Node.js', 'Deno', 'Bun', 'Java', 'C++', 'Ruby', 'Elixir', 'Haskell'],
  concept: ['Async Programming', 'Type Safety', 'Memory Management', 'API Design', 'Testing', 'CI/CD', 'Microservices', 'Serverless'],
  thing: ['REST API', 'CLI Tool', 'Web App', 'Search Engine', 'Database', 'Cache Layer', 'Auth System', 'Real-time App'],
  number: ['5', '7', '10', '12', '15'],
};

function generateTitle(index: number): string {
  const template = TITLE_TEMPLATES[index % TITLE_TEMPLATES.length];
  return template
    .replace('{tech}', TITLE_PARTS.tech[index % TITLE_PARTS.tech.length])
    .replace('{tech2}', TITLE_PARTS.tech2[index % TITLE_PARTS.tech2.length])
    .replace('{concept}', TITLE_PARTS.concept[index % TITLE_PARTS.concept.length])
    .replace('{thing}', TITLE_PARTS.thing[index % TITLE_PARTS.thing.length])
    .replace('{number}', TITLE_PARTS.number[index % TITLE_PARTS.number.length]);
}

function generateContent(wordCount: number, seed: number): string {
  const words: string[] = [];
  const allWords = [...VOCABULARY.technical, ...VOCABULARY.general];

  for (let i = 0; i < wordCount; i++) {
    // Mix in transitions occasionally
    if (i > 0 && i % 50 === 0) {
      words.push(VOCABULARY.transitions[(seed + i) % VOCABULARY.transitions.length]);
    }
    words.push(allWords[(seed * 7 + i * 3) % allWords.length]);
  }

  return words.join(' ');
}

function generateBlogCorpus(size: string): BlogPost[] {
  const { posts, wordsPerPost } = BLOG_SIZES[size];
  return Array.from({ length: posts }, (_, i) => ({
    id: i,
    title: generateTitle(i),
    excerpt: generateContent(30, i),
    content: generateContent(wordsPerPost, i),
    href: `/posts/2024/${String(i % 12 + 1).padStart(2, '0')}/post-${i}`,
    tags: ['rust', 'programming', 'engineering'].slice(0, (i % 3) + 1),
  }));
}

// ============================================================================
// BENCHMARK RESULTS COLLECTION
// ============================================================================

const results: BenchmarkResults = {
  timestamp: new Date().toISOString(),
  system: {
    node: process.version,
    platform: process.platform,
    arch: process.arch,
  },
  indexBuilding: {},
  searchQueries: {},
  fuzzySearch: {},
  memoryUsage: {},
};

// ============================================================================
// INDEX BUILDING BENCHMARKS
// ============================================================================

async function benchIndexBuilding(): Promise<void> {
  console.log('\n=== INDEX BUILDING BENCHMARKS ===\n');

  for (const [sizeName, sizeConfig] of Object.entries(BLOG_SIZES)) {
    const docs = generateBlogCorpus(sizeName);
    const bench = new Bench({ time: BENCH_CONFIG.time });

    bench.add('fuse.js', () => {
      new Fuse(docs, {
        keys: ['title', 'excerpt', 'content'],
        threshold: 0.3,
      });
    });

    bench.add('lunr.js', () => {
      lunr(function () {
        this.field('title', { boost: 10 });
        this.field('excerpt', { boost: 5 });
        this.field('content');
        docs.forEach((doc) => this.add(doc));
      });
    });

    bench.add('flexsearch', () => {
      const index = new FlexSearch.Document({
        document: {
          id: 'id',
          index: ['title', 'excerpt', 'content'],
        },
      });
      docs.forEach((doc) => index.add(doc));
    });

    bench.add('minisearch', () => {
      const ms = new MiniSearch({
        fields: ['title', 'excerpt', 'content'],
        storeFields: ['title', 'excerpt', 'href'],
        searchOptions: {
          boost: { title: 2, excerpt: 1.5 },
        },
      });
      ms.addAll(docs);
    });

    await bench.run();

    results.indexBuilding[sizeName] = bench.tasks.map((t) => ({
      library: t.name,
      opsPerSec: Math.round(t.result!.hz),
      meanMs: Number(t.result!.mean.toFixed(3)),  // tinybench mean is already in ms
      samples: t.result!.samples.length,
    }));

    console.log(`\n${sizeConfig.name} (${sizeConfig.posts} posts x ${sizeConfig.wordsPerPost} words)`);
    console.table(results.indexBuilding[sizeName]);
  }
}

// ============================================================================
// SEARCH QUERY BENCHMARKS
// ============================================================================

async function benchSearchQueries(): Promise<void> {
  console.log('\n=== SEARCH QUERY BENCHMARKS ===\n');

  // Use medium blog for query benchmarks
  const docs = generateBlogCorpus('medium');

  // Pre-build all indexes
  const fuseIndex = new Fuse(docs, {
    keys: ['title', 'excerpt', 'content'],
    threshold: 0.3,
    includeScore: true,
  });

  const lunrIndex = lunr(function () {
    this.field('title', { boost: 10 });
    this.field('excerpt', { boost: 5 });
    this.field('content');
    docs.forEach((doc) => this.add(doc));
  });

  const flexIndex = new FlexSearch.Document({
    document: {
      id: 'id',
      index: ['title', 'excerpt', 'content'],
    },
  });
  docs.forEach((doc) => flexIndex.add(doc));

  const miniIndex = new MiniSearch({
    fields: ['title', 'excerpt', 'content'],
    storeFields: ['title', 'excerpt', 'href'],
    searchOptions: {
      boost: { title: 2, excerpt: 1.5 },
      fuzzy: 0.2,
    },
  });
  miniIndex.addAll(docs);

  // Realistic blog search queries
  const queries = [
    // WORD-BASED SEARCH (all libraries handle these)
    { name: 'single_term', query: 'rust', description: 'Single word (common)' },
    { name: 'multi_term', query: 'rust async programming', description: 'Multi-word query' },
    { name: 'prefix', query: 'perf', description: 'Word prefix' },

    // SUBSTRING SEARCH (suffix arrays excel, inverted indexes fail)
    { name: 'substring_mid', query: 'script', description: 'Substring: "script" in typescript/javascript' },
    { name: 'substring_suffix', query: 'netes', description: 'Substring: "netes" in kubernetes' },
    { name: 'substring_infix', query: 'chron', description: 'Substring: "chron" in asynchronous' },

    // TYPO TOLERANCE (fuzzy search capability)
    { name: 'typo_1char', query: 'ruts', description: 'Typo: "ruts" for rust (1 edit)' },
    { name: 'typo_swap', query: 'javasrcript', description: 'Typo: transposition in javascript' },
    { name: 'typo_missing', query: 'typscript', description: 'Typo: missing letter in typescript' },
  ];

  for (const { name, query, description } of queries) {
    const bench = new Bench({ time: BENCH_CONFIG.time });

    // For substring/typo queries, use fuzzy matching where available
    const isSubstring = name.startsWith('substring_');
    const isTypo = name.startsWith('typo_');

    bench.add('fuse.js', () => fuseIndex.search(query));
    bench.add('lunr.js', () => {
      // Lunr can do prefix with *, but NOT substring or typo
      const q = name === 'prefix' ? `${query}*` : query;
      lunrIndex.search(q);
    });
    bench.add('flexsearch', () => flexIndex.search(query));
    bench.add('minisearch', () => {
      // MiniSearch has fuzzy option for typos
      return isTypo
        ? miniIndex.search(query, { fuzzy: 0.3 })
        : miniIndex.search(query);
    });

    await bench.run();

    // Count actual results found
    const fuseResults = fuseIndex.search(query);
    const lunrQ = name === 'prefix' ? `${query}*` : query;
    const lunrResults = lunrIndex.search(lunrQ);
    const flexResults = flexIndex.search(query);
    const miniResults = isTypo
      ? miniIndex.search(query, { fuzzy: 0.3 })
      : miniIndex.search(query);

    results.searchQueries[name] = {
      query,
      description,
      category: isSubstring ? 'substring' : isTypo ? 'typo' : 'word',
      results: bench.tasks.map((t, i) => {
        const resultCounts = [fuseResults.length, lunrResults.length, flexResults.length, miniResults.length];
        return {
          library: t.name,
          opsPerSec: Math.round(t.result!.hz),
          meanUs: Number((t.result!.mean * 1000).toFixed(1)),  // mean is in ms
          resultsFound: resultCounts[i],
        };
      }),
    };

    console.log(`\n"${query}" - ${description}`);
    console.table(results.searchQueries[name].results);
  }
}

// ============================================================================
// FUZZY SEARCH BENCHMARKS
// ============================================================================

async function benchFuzzySearch(): Promise<void> {
  console.log('\n=== FUZZY SEARCH (TYPO TOLERANCE) ===\n');

  const docs = generateBlogCorpus('medium');

  const fuseIndex = new Fuse(docs, {
    keys: ['title', 'excerpt', 'content'],
    threshold: 0.4,
    distance: 100,
    includeScore: true,
  });

  const miniIndex = new MiniSearch({
    fields: ['title', 'excerpt', 'content'],
    storeFields: ['title'],
    searchOptions: {
      fuzzy: 0.3,
      prefix: true,
    },
  });
  miniIndex.addAll(docs);

  // Common typos users might make
  const typos = [
    { correct: 'rust', typo: 'ruts', editDistance: 1 },
    { correct: 'typescript', typo: 'typscript', editDistance: 1 },
    { correct: 'kubernetes', typo: 'kubernates', editDistance: 1 },
    { correct: 'authentication', typo: 'authentacation', editDistance: 1 },
    { correct: 'programming', typo: 'programing', editDistance: 1 },
  ];

  for (const { correct, typo, editDistance } of typos) {
    const bench = new Bench({ time: BENCH_CONFIG.time });

    bench.add('fuse.js', () => fuseIndex.search(typo));
    bench.add('minisearch', () => miniIndex.search(typo, { fuzzy: 0.3 }));

    await bench.run();

    const fuseResults = fuseIndex.search(typo);
    const miniResults = miniIndex.search(typo, { fuzzy: 0.3 });

    results.fuzzySearch[typo] = {
      typo,
      correct,
      editDistance,
      results: bench.tasks.map((t) => ({
        library: t.name,
        opsPerSec: Math.round(t.result!.hz),
        meanUs: Number((t.result!.mean * 1000).toFixed(1)),  // mean is in ms
        foundResults: t.name === 'fuse.js' ? fuseResults.length : miniResults.length,
      })),
    };

    console.log(`\n"${typo}" -> "${correct}" (edit distance: ${editDistance})`);
    console.table(results.fuzzySearch[typo].results);
  }
}

// ============================================================================
// MEMORY USAGE
// ============================================================================

async function benchMemoryUsage(): Promise<void> {
  console.log('\n=== MEMORY USAGE ===\n');

  // Use Bun.gc() if available, otherwise try global.gc
  const gc = typeof Bun !== 'undefined' ? Bun.gc : (global as unknown as { gc?: () => void }).gc;

  for (const [sizeName, sizeConfig] of Object.entries(BLOG_SIZES)) {
    const docs = generateBlogCorpus(sizeName);

    // Estimate raw data size
    const rawSize = JSON.stringify(docs).length;

    const measurements: MemoryMeasurement[] = [];

    // Fuse.js
    if (gc) gc(true);
    const fuseBefore = process.memoryUsage().heapUsed;
    const fuse = new Fuse(docs, { keys: ['title', 'excerpt', 'content'] });
    if (gc) gc(true);
    const fuseAfter = process.memoryUsage().heapUsed;
    measurements.push({
      library: 'fuse.js',
      indexKB: Math.round((fuseAfter - fuseBefore) / 1024),
      rawDataKB: Math.round(rawSize / 1024),
    });

    // Lunr.js
    if (gc) gc(true);
    const lunrBefore = process.memoryUsage().heapUsed;
    const lunrIdx = lunr(function () {
      this.field('title');
      this.field('excerpt');
      this.field('content');
      docs.forEach((doc) => this.add(doc));
    });
    if (gc) gc(true);
    const lunrAfter = process.memoryUsage().heapUsed;
    measurements.push({
      library: 'lunr.js',
      indexKB: Math.round((lunrAfter - lunrBefore) / 1024),
      rawDataKB: Math.round(rawSize / 1024),
    });

    // FlexSearch
    if (gc) gc(true);
    const flexBefore = process.memoryUsage().heapUsed;
    const flex = new FlexSearch.Document({
      document: { id: 'id', index: ['title', 'excerpt', 'content'] },
    });
    docs.forEach((doc) => flex.add(doc));
    if (gc) gc(true);
    const flexAfter = process.memoryUsage().heapUsed;
    measurements.push({
      library: 'flexsearch',
      indexKB: Math.round((flexAfter - flexBefore) / 1024),
      rawDataKB: Math.round(rawSize / 1024),
    });

    // MiniSearch
    if (gc) gc(true);
    const miniBefore = process.memoryUsage().heapUsed;
    const mini = new MiniSearch({ fields: ['title', 'excerpt', 'content'] });
    mini.addAll(docs);
    if (gc) gc(true);
    const miniAfter = process.memoryUsage().heapUsed;
    measurements.push({
      library: 'minisearch',
      indexKB: Math.round((miniAfter - miniBefore) / 1024),
      rawDataKB: Math.round(rawSize / 1024),
    });

    results.memoryUsage[sizeName] = measurements;

    console.log(`\n${sizeConfig.name}`);
    console.table(measurements);

    // Keep references alive
    void fuse, lunrIdx, flex, mini;
  }
}

// ============================================================================
// RESULTS OUTPUT
// ============================================================================

function outputResults(): void {
  const outputPath = join(__dirname, 'results-js.json');
  writeFileSync(outputPath, JSON.stringify(results, null, 2));
  console.log(`\nResults saved to: ${outputPath}`);

  // Generate markdown summary
  const md = generateMarkdownSummary();
  const mdPath = join(__dirname, 'RESULTS.md');
  writeFileSync(mdPath, md);
  console.log(`Markdown summary: ${mdPath}`);
}

function generateMarkdownSummary(): string {
  const lines: string[] = [
    '# Search Library Benchmark Results',
    '',
    `Generated: ${results.timestamp}`,
    `Platform: ${results.system.platform} ${results.system.arch}`,
    `Node: ${results.system.node}`,
    '',
    '## Index Building Time',
    '',
    'Lower is better. Shows time to build search index from scratch.',
    '',
  ];

  for (const [size, data] of Object.entries(results.indexBuilding)) {
    const config = BLOG_SIZES[size];
    lines.push(`### ${config.name}`);
    lines.push('');
    lines.push('| Library | ops/sec | Mean (ms) |');
    lines.push('|---------|---------|-----------|');
    for (const row of data) {
      lines.push(`| ${row.library} | ${row.opsPerSec} | ${row.meanMs} |`);
    }
    lines.push('');
  }

  lines.push('## Search Query Performance');
  lines.push('');
  lines.push('Higher ops/sec is better. Measured on medium blog (100 posts).');
  lines.push('');

  for (const [name, { query, description, results: data }] of Object.entries(results.searchQueries)) {
    lines.push(`### ${description}`);
    lines.push(`Query: \`${query}\``);
    lines.push('');
    lines.push('| Library | ops/sec | Mean (us) |');
    lines.push('|---------|---------|-----------|');
    for (const row of data) {
      lines.push(`| ${row.library} | ${row.opsPerSec} | ${row.meanUs} |`);
    }
    lines.push('');
  }

  lines.push('## Memory Usage');
  lines.push('');
  lines.push('Index memory consumption in KB.');
  lines.push('');

  for (const [size, data] of Object.entries(results.memoryUsage)) {
    const config = BLOG_SIZES[size];
    lines.push(`### ${config.name}`);
    lines.push('');
    lines.push('| Library | Index (KB) | Raw Data (KB) |');
    lines.push('|---------|------------|---------------|');
    for (const row of data) {
      lines.push(`| ${row.library} | ${row.indexKB} | ${row.rawDataKB} |`);
    }
    lines.push('');
  }

  return lines.join('\n');
}

// ============================================================================
// MAIN
// ============================================================================

async function main(): Promise<void> {
  console.log('╔══════════════════════════════════════════════════════════════╗');
  console.log('║        Blog Search Library Benchmarks                        ║');
  console.log('╠══════════════════════════════════════════════════════════════╣');
  console.log('║  Simulating realistic blog sizes:                            ║');
  console.log('║  - Small:  20 posts x 500 words   (personal blog)            ║');
  console.log('║  - Medium: 100 posts x 1000 words (active blogger)           ║');
  console.log('║  - Large:  500 posts x 1500 words (publication)              ║');
  console.log('╠══════════════════════════════════════════════════════════════╣');
  console.log('║  Libraries:                                                  ║');
  console.log('║  - Fuse.js:    Fuzzy search with scoring                     ║');
  console.log('║  - Lunr.js:    Full-text search (Lucene-like)                ║');
  console.log('║  - FlexSearch: Fast configurable search                      ║');
  console.log('║  - MiniSearch: Lightweight full-text search                  ║');
  console.log('╚══════════════════════════════════════════════════════════════╝');

  await benchIndexBuilding();
  await benchSearchQueries();
  await benchFuzzySearch();
  await benchMemoryUsage();

  outputResults();

  console.log('\n✓ All benchmarks complete\n');
}

main().catch(console.error);
