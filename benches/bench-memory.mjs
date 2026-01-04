#!/usr/bin/env node
/**
 * Memory footprint benchmark for search libraries
 * Run with: node --expose-gc bench-memory.mjs
 */

import Fuse from 'fuse.js';
import lunr from 'lunr';
import FlexSearch from 'flexsearch';
import MiniSearch from 'minisearch';

// Realistic word distributions for blog content
const COMMON_WORDS = ['the', 'be', 'to', 'of', 'and', 'a', 'in', 'that', 'have', 'I', 'it', 'for', 'not', 'on', 'with', 'he', 'as', 'you', 'do', 'at'];
const TECH_WORDS = ['rust', 'async', 'performance', 'api', 'server', 'database', 'cache', 'memory', 'thread', 'concurrent', 'algorithm', 'optimization', 'latency', 'throughput', 'scalable', 'distributed', 'microservice', 'container', 'kubernetes', 'docker'];
const VERBS = ['implement', 'build', 'create', 'optimize', 'deploy', 'scale', 'configure', 'debug', 'test', 'refactor', 'migrate', 'benchmark', 'profile', 'analyze', 'design'];

function generateWord() {
	const r = Math.random();
	if (r < 0.6) return COMMON_WORDS[Math.floor(Math.random() * COMMON_WORDS.length)];
	if (r < 0.85) return TECH_WORDS[Math.floor(Math.random() * TECH_WORDS.length)];
	return VERBS[Math.floor(Math.random() * VERBS.length)];
}

function generateText(wordCount) {
	return Array.from({ length: wordCount }, generateWord).join(' ');
}

function generateCorpus(postCount, wordsPerPost) {
	return Array.from({ length: postCount }, (_, i) => ({
		id: `post-${i}`,
		title: generateText(8),
		content: generateText(wordsPerPost),
		slug: `post-${i}`,
	}));
}

function forceGC() {
	if (global.gc) {
		global.gc();
		global.gc();
		global.gc();
	}
}

function getHeapUsed() {
	forceGC();
	return process.memoryUsage().heapUsed;
}

function formatBytes(bytes) {
	if (bytes < 1024) return `${bytes} B`;
	if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
	return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

async function measureMemory(name, createIndex) {
	// Warm up and stabilize heap
	forceGC();
	await new Promise(r => setTimeout(r, 100));

	const baseline = getHeapUsed();

	// Create index
	const index = createIndex();

	// Force GC and measure
	await new Promise(r => setTimeout(r, 50));
	const afterIndex = getHeapUsed();

	const indexSize = afterIndex - baseline;

	return { name, indexSize, baseline, afterIndex };
}

async function benchmarkSize(label, posts, wordsPerPost) {
	console.log(`\n${'='.repeat(60)}`);
	console.log(`${label}: ${posts} posts × ${wordsPerPost} words`);
	console.log('='.repeat(60));

	const corpus = generateCorpus(posts, wordsPerPost);
	const rawSize = JSON.stringify(corpus).length;
	console.log(`Raw corpus size: ${formatBytes(rawSize)}`);

	// Keep corpus reference alive
	const results = [];

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
		const ratio = (r.indexSize / rawSize).toFixed(1);
		const sign = r.indexSize >= 0 ? '' : '-';
		console.log(`| ${r.name.padEnd(11)} | ${formatBytes(Math.abs(r.indexSize)).padStart(10)} | ${sign}${ratio}× |`);
	}

	return { label, posts, wordsPerPost, rawSize, results };
}

async function main() {
	if (!global.gc) {
		console.error('ERROR: Run with --expose-gc flag:');
		console.error('  node --expose-gc bench-memory.mjs');
		process.exit(1);
	}

	console.log('Memory Footprint Benchmark');
	console.log(`Node.js ${process.version}`);
	console.log(`Platform: ${process.platform} ${process.arch}`);
	console.log('');

	const allResults = [];

	// Small blog
	allResults.push(await benchmarkSize('Small Blog', 20, 500));

	// Medium blog
	allResults.push(await benchmarkSize('Medium Blog', 100, 1000));

	// Large blog
	allResults.push(await benchmarkSize('Large Blog', 500, 1500));

	// Output JSON for chart generation
	console.log('\n\n--- JSON Results ---');
	console.log(JSON.stringify(allResults, null, 2));
}

main().catch(console.error);
