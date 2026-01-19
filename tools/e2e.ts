/**
 * Sorex Browser E2E Tests
 *
 * Tests the full browser experience: WASM loading, search initialization,
 * and query execution via the demo.html page.
 *
 * Test data (data/e2e/fixtures/):
 *   - Doc 0: "Getting Started with Rust"
 *   - Doc 1: "TypeScript for JavaScript Developers"
 *   - Doc 2: "WebAssembly Introduction"
 */

import { test, expect } from '@playwright/test';

// Helper to wait for search to be ready
async function waitForReady(page: import('@playwright/test').Page) {
	const results = page.locator('#results');
	await expect(results).toContainText('Ready! Type to search.', {
		timeout: 15000
	});
}

// Helper to perform search and wait for results
async function searchFor(page: import('@playwright/test').Page, query: string) {
	const input = page.locator('#search');
	await input.fill(query);
	// Wait for debounce (100ms) + processing time
	await page.waitForTimeout(300);
}

test.describe('Sorex Search', () => {
	test.beforeEach(async ({ page }) => {
		// Capture console errors for debugging
		page.on('pageerror', (err) => console.error('Page error:', err.message));
		page.on('console', (msg) => {
			if (msg.type() === 'error') {
				console.error('Console error:', msg.text());
			}
		});

		await page.goto('/demo.html');
	});

	// Note: Non-shared memory fallback is not supported because the WASM binary
	// is compiled with wasm-bindgen-rayon which requires SharedArrayBuffer for atomics.
	// Environments without SharedArrayBuffer (no COOP/COEP headers) cannot run Sorex.

	test('initializes and becomes ready', async ({ page }) => {
		// Wait for search to initialize
		await waitForReady(page);

		// Input should be enabled
		const input = page.locator('#search');
		await expect(input).toBeEnabled();

		// Stats should show doc/term counts
		const stats = page.locator('#stats');
		await expect(stats).toContainText('docs');
	});

	test('exact match finds correct result', async ({ page }) => {
		await waitForReady(page);

		// Search for "Rust" - should find "Getting Started with Rust"
		await searchFor(page, 'Rust');

		const results = page.locator('#results');
		await expect(results).toContainText('Getting Started with Rust', {
			timeout: 5000
		});
	});

	test('prefix match finds results', async ({ page }) => {
		await waitForReady(page);

		// Search for "Type" - should find "TypeScript for JavaScript Developers"
		await searchFor(page, 'Type');

		const results = page.locator('#results');
		await expect(results).toContainText('TypeScript', { timeout: 5000 });
	});

	test('fuzzy match handles typos', async ({ page }) => {
		await waitForReady(page);

		// Search for "Ruts" (typo for Rust) - should still find Rust doc
		await searchFor(page, 'Ruts');

		const results = page.locator('#results');
		await expect(results).toContainText('Rust', { timeout: 5000 });
	});

	test('no results shows appropriate message', async ({ page }) => {
		await waitForReady(page);

		// Search for nonsense - should show no results
		await searchFor(page, 'xyznonexistent');

		const results = page.locator('#results');
		await expect(results).toContainText('No results', { timeout: 5000 });
	});

	test('clearing input returns to ready state', async ({ page }) => {
		await waitForReady(page);

		// Type something
		await searchFor(page, 'Rust');
		const results = page.locator('#results');
		await expect(results).toContainText('Getting Started', { timeout: 5000 });

		// Clear input
		await searchFor(page, '');
		await expect(results).toContainText('Type to search', { timeout: 5000 });
	});

	test('search completes within 100ms', async ({ page }) => {
		await waitForReady(page);

		// Perform search
		await searchFor(page, 'Rust');

		// Wait for stats to show timing
		const stats = page.locator('#stats');
		await expect(stats).toContainText('ms', { timeout: 5000 });

		// Extract timing from stats text (format: "X results in Y.Zms")
		const statsText = await stats.textContent();
		const match = statsText?.match(/in\s+(\d+\.?\d*)\s*ms/);
		expect(match).toBeTruthy();

		const timeMs = parseFloat(match![1]);
		expect(timeMs).toBeLessThan(100);
	});

	test('results are clickable links', async ({ page }) => {
		await waitForReady(page);

		await searchFor(page, 'Rust');
		const results = page.locator('#results');
		await expect(results).toContainText('Getting Started', { timeout: 5000 });

		// Result should be a link
		const link = results.locator('a').first();
		await expect(link).toHaveAttribute('href', /\/docs\/getting-started/);
	});

	test('results include score', async ({ page }) => {
		await waitForReady(page);

		await searchFor(page, 'Rust');
		const results = page.locator('#results');
		await expect(results).toContainText('Getting Started', { timeout: 5000 });

		// Result should display a numeric score (e.g., "1005" or "750.5")
		const firstResult = results.locator('.result').first();
		const scoreEl = firstResult.locator('.result-score');
		const scoreText = await scoreEl.textContent();

		// Score should be a number (not "—" or empty)
		expect(scoreText).toBeTruthy();
		expect(scoreText).not.toBe('—');
		const score = parseFloat(scoreText!);
		expect(score).toBeGreaterThan(0);
	});

	test('results include match type badge', async ({ page }) => {
		await waitForReady(page);

		await searchFor(page, 'Rust');
		const results = page.locator('#results');
		await expect(results).toContainText('Getting Started', { timeout: 5000 });

		// Result should have a match type badge (Title, H2, H3, H4, or Content)
		const firstResult = results.locator('.result').first();
		const matchTypeEl = firstResult.locator('.result-match-type');
		const matchType = await matchTypeEl.textContent();

		expect(['Title', 'H2', 'H3', 'H4', 'Content']).toContain(matchType);
	});

	test('results include matched term', async ({ page }) => {
		await waitForReady(page);

		await searchFor(page, 'Rust');
		const results = page.locator('#results');
		await expect(results).toContainText('Getting Started', { timeout: 5000 });

		// Result should display the matched vocabulary term
		const firstResult = results.locator('.result').first();
		const matchedTermEl = firstResult.locator('.result-matched-term');
		const matchedTerm = await matchedTermEl.textContent();

		// For exact match "Rust", the matched term should be "rust" (lowercase)
		expect(matchedTerm).toBeTruthy();
		expect(matchedTerm!.toLowerCase()).toBe('rust');
	});

	test('fuzzy match has non-zero score', async ({ page }) => {
		await waitForReady(page);

		// Search for "Ruts" (typo for Rust) - should find Rust doc with non-zero score
		await searchFor(page, 'Ruts');

		const results = page.locator('#results');
		await expect(results).toContainText('Rust', { timeout: 5000 });

		// Fuzzy match should have a non-zero score (with T3 penalty applied)
		const firstResult = results.locator('.result').first();
		const scoreEl = firstResult.locator('.result-score');
		const scoreText = await scoreEl.textContent();

		expect(scoreText).toBeTruthy();
		const score = parseFloat(scoreText!);
		expect(score).toBeGreaterThan(0);
	});

	test('prefix match shows expanded term', async ({ page }) => {
		await waitForReady(page);

		// Search for "Type" (prefix of "typescript")
		await searchFor(page, 'Type');

		const results = page.locator('#results');
		await expect(results).toContainText('TypeScript', { timeout: 5000 });

		// The matched term should contain the prefix or be the expanded term
		const firstResult = results.locator('.result').first();
		const matchedTermEl = firstResult.locator('.result-matched-term');
		const matchedTerm = await matchedTermEl.textContent();

		expect(matchedTerm).toBeTruthy();
		// Matched term should start with "type" (the prefix we searched for)
		expect(matchedTerm!.toLowerCase()).toMatch(/^type/);
	});
});
