/**
 * End-to-end test for sieve search
 *
 * Tests the full pipeline:
 * 1. Build index with `sieve index`
 * 2. Load with emitted sieve-loader.js
 * 3. Run search queries
 * 4. Verify results
 */

import { execSync } from 'child_process';
import { existsSync, readFileSync, rmSync, mkdirSync } from 'fs';
import { join } from 'path';

const ROOT = join(import.meta.dir, '..');
const FIXTURES = join(import.meta.dir, 'fixtures');
const OUTPUT = join(import.meta.dir, 'output');
const SIEVE_BIN = join(ROOT, 'target/release/sieve');

// Test utilities
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

// Setup: build CLI and create output directory
function setup() {
  console.log('Building sieve CLI...');
  execSync('cargo build --release', { cwd: ROOT, stdio: 'inherit' });

  if (existsSync(OUTPUT)) {
    rmSync(OUTPUT, { recursive: true });
  }
  mkdirSync(OUTPUT, { recursive: true });
}

// Build index from fixtures
function buildIndex(): string {
  console.log('\nBuilding search index...');
  execSync(`${SIEVE_BIN} index --input ${FIXTURES} --output ${OUTPUT}`, {
    cwd: ROOT,
    stdio: 'inherit',
  });

  // Find the generated .sieve file
  const files = Bun.file(OUTPUT).name;
  const sieveFiles = Array.from(
    new Bun.Glob('*.sieve').scanSync({ cwd: OUTPUT })
  );
  assert(sieveFiles.length > 0, 'No .sieve file generated');

  const indexPath = join(OUTPUT, sieveFiles[0]);
  console.log(`  Generated: ${indexPath}`);
  return indexPath;
}

// Load and test search
async function testSearch(indexPath: string) {
  console.log('\nTesting search functionality...');

  // Read the loader and index
  const loaderPath = join(OUTPUT, 'sieve-loader.js');
  assert(existsSync(loaderPath), 'sieve-loader.js not found');

  // Import the loader dynamically
  const loader = await import(loaderPath);
  assert(typeof loader.loadSieveSync === 'function', 'loadSieveSync not exported');
  assert(typeof loader.SieveSearcher === 'function', 'SieveSearcher not exported');

  // Load the index
  const indexBuffer = readFileSync(indexPath);
  const searcher = loader.loadSieveSync(indexBuffer.buffer);

  // Test: Document count
  assertEqual(searcher.doc_count(), 3, 'Document count');
  console.log('  ✓ Document count: 3');

  // Test: Has vocabulary
  assert(searcher.has_vocabulary(), 'Vocabulary should be loaded');
  console.log('  ✓ Vocabulary loaded');

  // Test: Has docs
  assert(searcher.has_docs(), 'Docs should be loaded');
  console.log('  ✓ Docs loaded');

  // Test: Search for "rust"
  const rustResults = searcher.search('rust', 10);
  assert(rustResults.length > 0, 'Should find results for "rust"');
  assert(
    rustResults.some((r: any) => r.title.toLowerCase().includes('rust')),
    'Should find Rust document'
  );
  console.log(`  ✓ Search "rust": ${rustResults.length} results`);

  // Test: Search for "typescript"
  const tsResults = searcher.search('typescript', 10);
  assert(tsResults.length > 0, 'Should find results for "typescript"');
  console.log(`  ✓ Search "typescript": ${tsResults.length} results`);

  // Test: Search for "webassembly"
  const wasmResults = searcher.search('webassembly', 10);
  assert(wasmResults.length > 0, 'Should find results for "webassembly"');
  console.log(`  ✓ Search "webassembly": ${wasmResults.length} results`);

  // Test: Search for "performance" (appears in wasm doc)
  const perfResults = searcher.search('performance', 10);
  assert(perfResults.length > 0, 'Should find results for "performance"');
  console.log(`  ✓ Search "performance": ${perfResults.length} results`);

  // Test: Search for non-existent term
  const noResults = searcher.search('xyznonexistent', 10);
  assertEqual(noResults.length, 0, 'Non-existent term results');
  console.log('  ✓ Search "xyznonexistent": 0 results');

  // Test: Fuzzy search (typo in "javascript")
  const fuzzyResults = searcher.search('javascrip', 10); // Missing 't'
  assert(fuzzyResults.length > 0, 'Fuzzy search should find results');
  console.log(`  ✓ Fuzzy search "javascrip": ${fuzzyResults.length} results`);

  // Test: Result structure
  const result = rustResults[0];
  assert('href' in result, 'Result should have href');
  assert('title' in result, 'Result should have title');
  assert('excerpt' in result, 'Result should have excerpt');
  console.log('  ✓ Result structure validated');

  // Free the searcher
  searcher.free();
  console.log('  ✓ Searcher freed');
}

// Verify demo.html is generated with --demo flag
async function testDemoGeneration() {
  console.log('\nTesting demo generation...');

  // Clean output
  if (existsSync(OUTPUT)) {
    rmSync(OUTPUT, { recursive: true });
  }
  mkdirSync(OUTPUT, { recursive: true });

  // Build with --demo flag
  execSync(`${SIEVE_BIN} index --input ${FIXTURES} --output ${OUTPUT} --demo`, {
    cwd: ROOT,
    stdio: 'inherit',
  });

  const demoPath = join(OUTPUT, 'demo.html');
  assert(existsSync(demoPath), 'demo.html should be generated with --demo flag');

  const demoContent = readFileSync(demoPath, 'utf-8');
  assert(demoContent.includes('loadSieve'), 'demo.html should use loadSieve');
  assert(demoContent.includes('sieve-loader.js'), 'demo.html should import sieve-loader.js');
  console.log('  ✓ demo.html generated and valid');
}

// Main
async function main() {
  console.log('='.repeat(60));
  console.log('Sieve E2E Test Suite');
  console.log('='.repeat(60));

  try {
    setup();
    const indexPath = buildIndex();
    await testSearch(indexPath);
    await testDemoGeneration();

    console.log('\n' + '='.repeat(60));
    console.log('All tests passed!');
    console.log('='.repeat(60));
  } catch (err) {
    console.error('\n' + '='.repeat(60));
    console.error('TEST FAILED');
    console.error('='.repeat(60));
    console.error(err);
    process.exit(1);
  }
}

main();
