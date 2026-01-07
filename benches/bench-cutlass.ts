/**
 * Benchmarks search engines against NVIDIA CUTLASS documentation.
 *
 * Prerequisites:
 * 1. Run: npx tsx benches/crawl-cutlass-docs.ts
 * 2. Run: sieve build --input datasets/cutlass --output datasets/cutlass
 *
 * Usage: npx tsx benches/bench-cutlass.ts
 *
 * Outputs:
 * - benches/results-cutlass.json (raw results)
 * - benches/RESULTS-CUTLASS.md (formatted table)
 */

import { readFileSync, writeFileSync, existsSync, readdirSync } from 'fs';
import { join } from 'path';
import { execSync } from 'child_process';

const DATASET_DIR = 'datasets/cutlass';

interface Document {
	id: number;
	slug: string;
	title: string;
	excerpt: string;
	href: string;
	type: 'page';
	category: string;
	text: string;
}

interface BenchmarkResult {
	engine: string;
	query: string;
	timeUs: number;
	resultCount: number;
}

function loadDocuments(): Document[] {
	const manifestPath = join(DATASET_DIR, 'manifest.json');
	if (!existsSync(manifestPath)) {
		console.error(`Error: No manifest found at ${manifestPath}`);
		console.error('Run: npx tsx benches/crawl-cutlass-docs.ts');
		process.exit(1);
	}

	const manifest = JSON.parse(readFileSync(manifestPath, 'utf-8'));
	const docs: Document[] = [];

	for (const filename of manifest.documents) {
		const docPath = join(DATASET_DIR, filename);
		if (existsSync(docPath)) {
			docs.push(JSON.parse(readFileSync(docPath, 'utf-8')));
		}
	}

	return docs;
}

async function runBenchmarks(documents: Document[]): Promise<BenchmarkResult[]> {
	console.log('\n=== Running Benchmarks ===\n');

	const results: BenchmarkResult[] = [];

	// GPU/CUTLASS-relevant queries
	const queries = [
		{ q: 'gemm', desc: 'core operation' },
		{ q: 'warp', desc: 'GPU concept' },
		{ q: 'tensor', desc: 'common term' },
		{ q: 'blackwell', desc: 'latest arch' },
		{ q: 'mma', desc: 'matrix multiply-accumulate' },
		{ q: 'tma', desc: 'tensor memory access' },
		{ q: 'epilouge', desc: 'typo for epilogue' },
		{ q: 'syncronize', desc: 'typo for synchronize' }
	];

	// Prepare text corpus for JS-based engines
	const corpus = documents.map((d) => ({
		id: d.id,
		title: d.title,
		text: d.text,
		href: d.href
	}));

	const totalChars = corpus.reduce((a, d) => a + d.text.length, 0);
	console.log(`Corpus: ${documents.length} documents, ${(totalChars / 1024).toFixed(1)} KB\n`);

	// 1. fuse.js
	console.log('Testing fuse.js...');
	const Fuse = (await import('fuse.js')).default;
	const fuse = new Fuse(corpus, {
		keys: ['title', 'text'],
		threshold: 0.4,
		includeScore: true
	});

	for (const { q } of queries) {
		fuse.search(q); // warm up
		const times: number[] = [];
		let resultCount = 0;
		for (let i = 0; i < 10; i++) {
			const start = performance.now();
			const fuseResults = fuse.search(q);
			times.push((performance.now() - start) * 1000);
			resultCount = fuseResults.length;
		}
		const avgTime = times.reduce((a, b) => a + b, 0) / times.length;
		results.push({ engine: 'fuse.js', query: q, timeUs: avgTime, resultCount });
	}

	// 2. FlexSearch
	console.log('Testing FlexSearch...');
	const FlexSearch = (await import('flexsearch')).default;
	const flexIndex = new FlexSearch.Document({
		document: { id: 'id', index: ['title', 'text'] }
	});
	corpus.forEach((doc) => flexIndex.add(doc));

	for (const { q } of queries) {
		flexIndex.search(q); // warm up
		const times: number[] = [];
		let resultCount = 0;
		for (let i = 0; i < 10; i++) {
			const start = performance.now();
			const flexResults = flexIndex.search(q);
			times.push((performance.now() - start) * 1000);
			resultCount = flexResults.reduce((acc: number, r: { result: unknown[] }) => acc + r.result.length, 0);
		}
		const avgTime = times.reduce((a, b) => a + b, 0) / times.length;
		results.push({ engine: 'FlexSearch', query: q, timeUs: avgTime, resultCount });
	}

	// 3. lunr.js
	console.log('Testing lunr.js...');
	const lunr = (await import('lunr')).default;
	const lunrIndex = lunr(function () {
		this.field('title');
		this.field('text');
		this.ref('id');
		corpus.forEach((doc) => this.add(doc));
	});

	for (const { q } of queries) {
		try { lunrIndex.search(q); } catch { /* ignore */ }
		const times: number[] = [];
		let resultCount = 0;
		for (let i = 0; i < 10; i++) {
			const start = performance.now();
			let lunrResults: unknown[] = [];
			try {
				lunrResults = lunrIndex.search(q);
			} catch { /* lunr throws on special chars */ }
			times.push((performance.now() - start) * 1000);
			resultCount = lunrResults.length;
		}
		const avgTime = times.reduce((a, b) => a + b, 0) / times.length;
		results.push({ engine: 'lunr.js', query: q, timeUs: avgTime, resultCount });
	}

	// 4. MiniSearch
	console.log('Testing MiniSearch...');
	const MiniSearch = (await import('minisearch')).default;
	const miniSearch = new MiniSearch({
		fields: ['title', 'text'],
		storeFields: ['title', 'href']
	});
	miniSearch.addAll(corpus);

	for (const { q } of queries) {
		miniSearch.search(q); // warm up
		const times: number[] = [];
		let resultCount = 0;
		for (let i = 0; i < 10; i++) {
			const start = performance.now();
			const miniResults = miniSearch.search(q);
			times.push((performance.now() - start) * 1000);
			resultCount = miniResults.length;
		}
		const avgTime = times.reduce((a, b) => a + b, 0) / times.length;
		results.push({ engine: 'MiniSearch', query: q, timeUs: avgTime, resultCount });
	}

	// 5. Sieve (via CLI for now)
	console.log('Testing Sieve...');
	const sieveIndexPath = join(DATASET_DIR, 'index.sieve');

	// Check if index exists, if not try to build it
	const sieveFiles = readdirSync(DATASET_DIR).filter(f => f.endsWith('.sieve'));
	if (sieveFiles.length === 0) {
		console.log('  Building Sieve index...');
		try {
			execSync(`sieve build --input "${DATASET_DIR}" --output "${DATASET_DIR}"`, { stdio: 'pipe' });
			console.log('  Sieve index built successfully');
		} catch (e) {
			console.log('  Warning: Could not build Sieve index (sieve CLI not found)');
		}
	}

	// For now, add placeholder results (WASM benchmarks require browser/Node WASM runtime)
	for (const { q } of queries) {
		results.push({ engine: 'Sieve', query: q, timeUs: 0, resultCount: 0 });
	}

	return results;
}

function generateMarkdownReport(results: BenchmarkResult[], docCount: number): string {
	const lines: string[] = [
		'# CUTLASS Documentation Benchmark Results',
		'',
		`Dataset: ${docCount} NVIDIA CUTLASS documentation pages`,
		'',
		'## Query Latency (μs, lower is better)',
		'',
		'| Query | fuse.js | FlexSearch | lunr.js | MiniSearch |',
		'|-------|---------|------------|---------|------------|'
	];

	const queries = [...new Set(results.map((r) => r.query))];
	for (const query of queries) {
		const qResults = results.filter((r) => r.query === query);
		const row = [
			`\`${query}\``,
			...['fuse.js', 'FlexSearch', 'lunr.js', 'MiniSearch'].map((engine) => {
				const r = qResults.find((r) => r.engine === engine);
				return r ? `${r.timeUs.toFixed(0)}` : '-';
			})
		];
		lines.push(`| ${row.join(' | ')} |`);
	}

	lines.push('', '## Result Counts', '', '| Query | fuse.js | FlexSearch | lunr.js | MiniSearch |', '|-------|---------|------------|---------|------------|');

	for (const query of queries) {
		const qResults = results.filter((r) => r.query === query);
		const row = [
			`\`${query}\``,
			...['fuse.js', 'FlexSearch', 'lunr.js', 'MiniSearch'].map((engine) => {
				const r = qResults.find((r) => r.engine === engine);
				return r ? `${r.resultCount}` : '-';
			})
		];
		lines.push(`| ${row.join(' | ')} |`);
	}

	lines.push('', '## Key Findings', '', '- **Substring search**: "gemm" appears in many CUTLASS pages; word-based indexes vary in tokenization');
	lines.push('- **Typo tolerance**: "epilouge" (typo) tests fuzzy matching; most indexes require exact spelling');
	lines.push('- **GPU terminology**: "warp", "mma", "tma" test domain-specific vocabulary handling');
	lines.push('', `Generated: ${new Date().toISOString()}`);

	return lines.join('\n');
}

async function main() {
	console.log('=== NVIDIA CUTLASS Benchmark ===\n');

	const documents = loadDocuments();
	console.log(`Loaded ${documents.length} documents from ${DATASET_DIR}`);

	const results = await runBenchmarks(documents);

	// Print results table
	console.log('\n=== Results ===\n');
	const queries = [...new Set(results.map((r) => r.query))];
	for (const query of queries) {
		console.log(`Query: "${query}"`);
		const qResults = results
			.filter((r) => r.query === query && r.engine !== 'Sieve')
			.sort((a, b) => a.timeUs - b.timeUs);
		for (const r of qResults) {
			console.log(`  ${r.engine.padEnd(12)} ${r.timeUs.toFixed(0).padStart(8)}μs  (${r.resultCount} results)`);
		}
		console.log();
	}

	// Save raw results
	writeFileSync('benches/results-cutlass.json', JSON.stringify(results, null, 2));
	console.log('Saved: benches/results-cutlass.json');

	// Generate markdown report
	const report = generateMarkdownReport(results, documents.length);
	writeFileSync('benches/RESULTS-CUTLASS.md', report);
	console.log('Saved: benches/RESULTS-CUTLASS.md');
}

main().catch(console.error);
