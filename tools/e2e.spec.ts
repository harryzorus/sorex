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

import { test, expect } from "@playwright/test";

// Helper to wait for search to be ready
async function waitForReady(page: import("@playwright/test").Page) {
  const results = page.locator("#results");
  await expect(results).toContainText("Ready! Type to search.", {
    timeout: 15000,
  });
}

// Helper to perform search and wait for results
async function searchFor(
  page: import("@playwright/test").Page,
  query: string
) {
  const input = page.locator("#search");
  await input.fill(query);
  // Wait for debounce (100ms) + processing time
  await page.waitForTimeout(300);
}

test.describe("Sorex Search", () => {
  test.beforeEach(async ({ page }) => {
    // Capture console errors for debugging
    page.on("pageerror", (err) => console.error("Page error:", err.message));
    page.on("console", (msg) => {
      if (msg.type() === "error") {
        console.error("Console error:", msg.text());
      }
    });

    await page.goto("/demo.html");
  });

  test("initializes and becomes ready", async ({ page }) => {
    // Wait for search to initialize
    await waitForReady(page);

    // Input should be enabled
    const input = page.locator("#search");
    await expect(input).toBeEnabled();

    // Stats should show doc/term counts
    const stats = page.locator("#stats");
    await expect(stats).toContainText("docs");
  });

  test("exact match finds correct result", async ({ page }) => {
    await waitForReady(page);

    // Search for "Rust" - should find "Getting Started with Rust"
    await searchFor(page, "Rust");

    const results = page.locator("#results");
    await expect(results).toContainText("Getting Started with Rust", {
      timeout: 5000,
    });
  });

  test("prefix match finds results", async ({ page }) => {
    await waitForReady(page);

    // Search for "Type" - should find "TypeScript for JavaScript Developers"
    await searchFor(page, "Type");

    const results = page.locator("#results");
    await expect(results).toContainText("TypeScript", { timeout: 5000 });
  });

  test("fuzzy match handles typos", async ({ page }) => {
    await waitForReady(page);

    // Search for "Ruts" (typo for Rust) - should still find Rust doc
    await searchFor(page, "Ruts");

    const results = page.locator("#results");
    await expect(results).toContainText("Rust", { timeout: 5000 });
  });

  test("no results shows appropriate message", async ({ page }) => {
    await waitForReady(page);

    // Search for nonsense - should show no results
    await searchFor(page, "xyznonexistent");

    const results = page.locator("#results");
    await expect(results).toContainText("No results", { timeout: 5000 });
  });

  test("clearing input returns to ready state", async ({ page }) => {
    await waitForReady(page);

    // Type something
    await searchFor(page, "Rust");
    const results = page.locator("#results");
    await expect(results).toContainText("Getting Started", { timeout: 5000 });

    // Clear input
    await searchFor(page, "");
    await expect(results).toContainText("Type to search", { timeout: 5000 });
  });

  test("search completes within 100ms", async ({ page }) => {
    await waitForReady(page);

    // Perform search
    await searchFor(page, "Rust");

    // Wait for stats to show timing
    const stats = page.locator("#stats");
    await expect(stats).toContainText("ms", { timeout: 5000 });

    // Extract timing from stats text (format: "X results in Y.Zms")
    const statsText = await stats.textContent();
    const match = statsText?.match(/in\s+(\d+\.?\d*)\s*ms/);
    expect(match).toBeTruthy();

    const timeMs = parseFloat(match![1]);
    expect(timeMs).toBeLessThan(100);
  });

  test("results are clickable links", async ({ page }) => {
    await waitForReady(page);

    await searchFor(page, "Rust");
    const results = page.locator("#results");
    await expect(results).toContainText("Getting Started", { timeout: 5000 });

    // Result should be a link
    const link = results.locator("a").first();
    await expect(link).toHaveAttribute("href", /\/docs\/getting-started/);
  });
});
