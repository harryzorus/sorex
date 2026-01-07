/**
 * Crawls NVIDIA CUTLASS documentation from docs.nvidia.com
 *
 * Usage: npx tsx benches/crawl-cutlass-docs.ts
 *
 * Outputs:
 * - datasets/cutlass/*.json (per-document JSON files)
 * - datasets/cutlass/manifest.json (index manifest)
 */

import { writeFileSync, mkdirSync, existsSync, rmSync } from 'fs';
import { join } from 'path';
import { load as cheerioLoad } from 'cheerio';
import { createHash } from 'crypto';

const NVIDIA_DOCS_BASE = 'https://docs.nvidia.com/cutlass/latest';
const OUTPUT_DIR = 'datasets/cutlass';

interface FieldBoundary {
	docId: number;
	start: number;
	end: number;
	fieldType: 'title' | 'heading' | 'content';
	sectionId: string | null;
}

interface CutlassDoc {
	id: number;
	slug: string;
	title: string;
	excerpt: string;
	href: string;
	type: 'page';
	category: string;
	text: string;
	fieldBoundaries: FieldBoundary[];
}

// Content hash for deduplication
const contentHashes = new Set<string>();

function hashContent(text: string): string {
	return createHash('sha256').update(text).digest('hex').slice(0, 16);
}

function slugify(text: string): string {
	return text
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-|-$/g, '');
}

// ============================================================================
// docs.nvidia.com HTML Crawling
// ============================================================================

const visitedUrls = new Set<string>();
const pendingUrls: string[] = [];

async function fetchHtmlPage(url: string): Promise<string | null> {
	try {
		console.log(`  Fetching ${url}...`);
		const res = await fetch(url, {
			headers: {
				'User-Agent': 'Mozilla/5.0 (compatible; SieveCrawler/1.0)',
				Accept: 'text/html'
			}
		});

		if (!res.ok) {
			console.log(`    Warning: ${res.status} for ${url}`);
			return null;
		}

		return res.text();
	} catch (error) {
		console.log(`    Error fetching ${url}:`, error);
		return null;
	}
}

function extractHtmlTitle($: ReturnType<typeof cheerioLoad>): string {
	// Try article title first
	const $h1 = $('article h1').first();
	if ($h1.length) {
		const $clone = $h1.clone();
		$clone.find('.headerlink, a.headerlink').remove();
		return $clone.text().trim();
	}

	// Fall back to page title
	const pageTitle = $('title').text().trim();
	if (pageTitle) return pageTitle.replace(/ - NVIDIA CUTLASS.*$/, '').replace(/#$/, '').trim();

	return 'Untitled';
}

function extractHtmlSections(
	$: ReturnType<typeof cheerioLoad>
): Array<{ id: string; heading: string; content: string; level: number }> {
	const sections: Array<{ id: string; heading: string; content: string; level: number }> = [];

	// Find main content area
	const article = $('article, .main-content, main, .content').first();
	if (!article.length) return sections;

	// Extract headings with their content
	article.find('h1, h2, h3, h4, h5, h6').each((_, heading) => {
		const $heading = $(heading);

		// Get heading level (1-6)
		const tagName = heading.tagName.toLowerCase();
		const level = parseInt(tagName.charAt(1), 10);

		// Clean heading text by removing headerlink anchors
		const $clone = $heading.clone();
		$clone.find('.headerlink, a.headerlink').remove();
		const headingText = $clone.text().trim();

		// Get the anchor ID
		let id = $heading.attr('id');
		if (!id) {
			const prevSpan = $heading.prev('span[id]');
			if (prevSpan.length) {
				id = prevSpan.attr('id') || '';
			}
		}
		if (!id) {
			id = slugify(headingText);
		}

		// Get content until next heading
		let content = '';
		let next = $heading.next();
		while (next.length) {
			if (next.is('h1, h2, h3, h4, h5, h6')) {
				break;
			}
			const $nextClone = next.clone();
			$nextClone.find('pre, code.highlight').remove();
			content += $nextClone.text() + ' ';
			next = next.next();
		}

		sections.push({
			id,
			heading: headingText,
			content: content.trim(),
			level
		});
	});

	return sections;
}

function extractHtmlText($: ReturnType<typeof cheerioLoad>): string {
	const $clone = $.root().clone();
	const $doc = cheerioLoad($clone.html() || '');

	$doc('script, style, nav, footer, .sidebar, .toc, .headerlink, a.headerlink').remove();

	const article = $doc('article, .main-content, main, .content').first();
	const text = article.length ? article.text() : $doc('body').text();

	return text
		.replace(/\s+/g, ' ')
		.replace(/\s#\s/g, ' ')
		.replace(/#$/gm, '')
		.trim();
}

function extractHtmlLinks($: ReturnType<typeof cheerioLoad>, baseUrl: string): string[] {
	const links: string[] = [];

	$('a[href]').each((_, el) => {
		const href = $(el).attr('href');
		if (!href) return;

		let absoluteUrl: string;
		try {
			absoluteUrl = new URL(href, baseUrl).toString();
		} catch {
			return;
		}

		if (absoluteUrl.startsWith(NVIDIA_DOCS_BASE) && absoluteUrl.endsWith('.html')) {
			const cleanUrl = absoluteUrl.split('#')[0];
			if (!visitedUrls.has(cleanUrl)) {
				links.push(cleanUrl);
			}
		}
	});

	return links;
}

async function crawlNvidiaDocs(): Promise<CutlassDoc[]> {
	const docs: CutlassDoc[] = [];
	let docId = 0;

	pendingUrls.push(`${NVIDIA_DOCS_BASE}/index.html`);

	while (pendingUrls.length > 0) {
		const url = pendingUrls.shift()!;

		if (visitedUrls.has(url)) continue;
		visitedUrls.add(url);

		const html = await fetchHtmlPage(url);
		if (!html) continue;

		const $ = cheerioLoad(html);

		const links = extractHtmlLinks($, url);
		pendingUrls.push(...links);

		const title = extractHtmlTitle($);
		const text = extractHtmlText($);
		const sections = extractHtmlSections($);

		if (text.length < 100) {
			console.log(`    Skipping thin page: ${title}`);
			continue;
		}

		const hash = hashContent(text);
		if (contentHashes.has(hash)) {
			console.log(`    Skipping duplicate: ${title}`);
			continue;
		}
		contentHashes.add(hash);

		let category = 'CUTLASS';
		if (url.includes('/cute/') || url.includes('cute_')) category = 'CuTe';
		if (url.includes('/pythonDSL/') || url.includes('python')) category = 'Python DSL';
		if (url.includes('cutlass_3x') || url.includes('3x')) category = 'CUTLASS 3.x';
		if (url.includes('cutlass_2x') || url.includes('2x')) category = 'CUTLASS 2.x';

		// Skip legacy docs
		if (category === 'CUTLASS 2.x' || category === 'CUTLASS 3.x') {
			console.log(`    Skipping legacy: ${title} (${category})`);
			continue;
		}

		const slug = url
			.replace(NVIDIA_DOCS_BASE, '')
			.replace(/^\//, '')
			.replace(/\.html$/, '')
			.replace(/\//g, '-');

		const fieldBoundaries: FieldBoundary[] = [];
		let offset = 0;
		const currentDocId = docId;

		fieldBoundaries.push({
			docId: currentDocId,
			start: offset,
			end: offset + title.length,
			fieldType: 'title',
			sectionId: null
		});
		offset += title.length + 1;

		for (const section of sections) {
			fieldBoundaries.push({
				docId: currentDocId,
				start: offset,
				end: offset + section.heading.length,
				fieldType: 'heading',
				sectionId: section.id
			});
			offset += section.heading.length + 1;

			if (section.content) {
				fieldBoundaries.push({
					docId: currentDocId,
					start: offset,
					end: offset + section.content.length,
					fieldType: 'content',
					sectionId: section.id
				});
				offset += section.content.length + 1;
			}
		}

		const excerpt = text.slice(0, 200) + (text.length > 200 ? '...' : '');

		docs.push({
			id: docId++,
			slug,
			title,
			excerpt,
			href: url,
			type: 'page',
			category,
			text: `${title} ${text}`,
			fieldBoundaries
		});

		console.log(`    Added: ${title} (${category})`);

		await new Promise((r) => setTimeout(r, 200));
	}

	return docs;
}

// ============================================================================
// Main
// ============================================================================

async function main() {
	console.log('=== NVIDIA CUTLASS Documentation Crawler ===\n');

	if (existsSync(OUTPUT_DIR)) {
		rmSync(OUTPUT_DIR, { recursive: true });
	}
	mkdirSync(OUTPUT_DIR, { recursive: true });

	console.log('\nCrawling docs.nvidia.com...\n');

	const allDocs = await crawlNvidiaDocs();

	console.log(`\n  Crawled ${allDocs.length} pages from docs.nvidia.com`);

	console.log('\nWriting output files...\n');

	// Renumber IDs sequentially
	allDocs.forEach((doc, i) => {
		doc.id = i;
		for (const boundary of doc.fieldBoundaries) {
			boundary.docId = i;
		}
	});

	// Write per-document JSON files
	for (const doc of allDocs) {
		writeFileSync(join(OUTPUT_DIR, `${doc.id}.json`), JSON.stringify(doc, null, '\t'));
	}

	// Write manifest
	const manifest = {
		version: 1,
		documents: allDocs.map((_, i) => `${i}.json`),
		indexes: {
			all: { name: 'all', include: 'all' }
		}
	};
	writeFileSync(join(OUTPUT_DIR, 'manifest.json'), JSON.stringify(manifest, null, '\t'));

	// Write summary
	const summary = {
		totalDocuments: allDocs.length,
		categories: [...new Set(allDocs.map((d) => d.category))],
		timestamp: new Date().toISOString()
	};
	writeFileSync(join(OUTPUT_DIR, 'summary.json'), JSON.stringify(summary, null, '\t'));

	console.log(`\n=== Summary ===`);
	console.log(`Total documents: ${allDocs.length}`);
	console.log(`Categories: ${summary.categories.join(', ')}`);
	console.log(`\nOutput written to: ${OUTPUT_DIR}/`);
	console.log(`\nBuild index with: sieve build --input ${OUTPUT_DIR} --output ${OUTPUT_DIR}`);
}

main().catch(console.error);
