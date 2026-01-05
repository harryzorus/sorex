/**
 * Index size comparison benchmark
 * Compares index sizes across search libraries
 */

import Fuse from 'fuse.js';
import lunr from 'lunr';
import FlexSearch from 'flexsearch';
import MiniSearch from 'minisearch';
import { gzipSync } from 'zlib';

// ============================================================================
// TYPE DEFINITIONS
// ============================================================================

interface Document {
  id: number;
  title: string;
  content: string;
}

interface SizeResult {
  raw: number;
  gzipped: number;
}

interface SizeConfig {
  name: string;
  posts: number;
  words: number;
}

// Technical vocabulary for realistic blog content
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
    'application', 'system', 'solution', 'approach', 'method', 'technique',
    'implementation', 'development', 'engineering', 'architecture', 'design',
    'pattern', 'practice', 'principle', 'concept', 'idea', 'theory',
  ],
};

function generateCorpus(numPosts: number, wordsPerPost: number): Document[] {
  const allWords = [...VOCABULARY.technical, ...VOCABULARY.general];
  const posts: Document[] = [];

  for (let i = 0; i < numPosts; i++) {
    const words: string[] = [];
    for (let j = 0; j < wordsPerPost; j++) {
      words.push(allWords[Math.floor(Math.random() * allWords.length)]);
    }
    const title = `Post ${i + 1}: ${VOCABULARY.technical[i % VOCABULARY.technical.length]}`;
    posts.push({
      id: i,
      title,
      content: words.join(' '),
    });
  }
  return posts;
}

function measureSize(obj: unknown): SizeResult {
  const json = JSON.stringify(obj);
  return {
    raw: Buffer.byteLength(json, 'utf8'),
    gzipped: gzipSync(json).length,
  };
}

async function benchmark(): Promise<void> {
  console.log('=== Index Size Comparison ===\n');

  const sizes: SizeConfig[] = [
    { name: 'Small (20 posts)', posts: 20, words: 500 },
    { name: 'Medium (100 posts)', posts: 100, words: 1000 },
  ];

  for (const size of sizes) {
    console.log(`\n${size.name}:`);
    console.log('-'.repeat(60));

    const corpus = generateCorpus(size.posts, size.words);
    const rawData = measureSize(corpus);

    // Fuse.js - no index built, stores raw docs
    const fuseIndex = new Fuse(corpus, { keys: ['title', 'content'] });
    const fuseSize = measureSize((fuseIndex as unknown as { _docs: Document[] })._docs);

    // Lunr.js
    const lunrIndex = lunr(function () {
      this.ref('id');
      this.field('title');
      this.field('content');
      corpus.forEach(doc => this.add(doc));
    });
    const lunrSize = measureSize(lunrIndex.toJSON());

    // FlexSearch
    const flexIndex = new FlexSearch.Document({
      document: { id: 'id', index: ['title', 'content'] }
    });
    corpus.forEach(doc => flexIndex.add(doc));
    const flexExport: Record<string, unknown> = {};
    await flexIndex.export((key, data) => { flexExport[key] = data; });
    const flexSize = measureSize(flexExport);

    // MiniSearch
    const miniIndex = new MiniSearch({
      fields: ['title', 'content'],
      storeFields: ['title'],
    });
    miniIndex.addAll(corpus);
    const miniSize = measureSize(miniIndex.toJSON());

    const results = [
      { library: 'Raw Data', raw: rawData.raw, gzip: rawData.gzipped, note: '' },
      { library: 'Fuse.js', raw: fuseSize.raw, gzip: fuseSize.gzipped, note: '(no index)' },
      { library: 'Lunr.js', raw: lunrSize.raw, gzip: lunrSize.gzipped, note: '' },
      { library: 'FlexSearch', raw: flexSize.raw, gzip: flexSize.gzipped, note: '' },
      { library: 'MiniSearch', raw: miniSize.raw, gzip: miniSize.gzipped, note: '' },
    ];

    console.log('| Library | Raw Size | Gzipped | Ratio |');
    console.log('|---------|----------|---------|-------|');
    for (const r of results) {
      const ratio = (r.raw / rawData.raw).toFixed(1);
      const note = r.note || '';
      console.log(`| ${r.library.padEnd(12)} | ${(r.raw / 1024).toFixed(1).padStart(6)} KB | ${(r.gzip / 1024).toFixed(1).padStart(5)} KB | ${ratio}x ${note} |`);
    }
  }

  console.log('\n=== WASM Bundle Sizes ===\n');
  console.log('| Component | Raw | Gzipped |');
  console.log('|-----------|-----|---------|');
  console.log('| sieve_bg.wasm | 310 KB | 146 KB |');
  console.log('| sieve.js | 32 KB | 6.6 KB |');
  console.log('| **Total** | **342 KB** | **153 KB** |');

  console.log('\n=== Comparison with JS Libraries ===\n');
  console.log('| Library | Bundle Size (gzip) |');
  console.log('|---------|-------------------|');
  console.log('| Sieve (WASM) | 153 KB |');
  console.log('| Fuse.js | ~24 KB |');
  console.log('| Lunr.js | ~8 KB |');
  console.log('| FlexSearch | ~6 KB |');
  console.log('| MiniSearch | ~8 KB |');
}

benchmark().catch(console.error);
