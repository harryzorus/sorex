/**
 * Playwright Global Setup
 *
 * Ensures test fixtures are built before E2E tests run.
 * This guarantees the index and sorex.js are always up-to-date.
 */

import { execSync } from "node:child_process";
import { existsSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");

/** Source files that affect the test output */
const SOURCES = [
  "tools/loader.ts",
  "tools/build.ts",
  "data/e2e/fixtures/manifest.json",
  "data/e2e/fixtures/0.json",
  "data/e2e/fixtures/1.json",
  "data/e2e/fixtures/2.json",
  "target/pkg/sorex.js",
  "target/pkg/sorex_bg.wasm",
];

/** Output files that should be rebuilt if stale */
const OUTPUTS = [
  "target/e2e/output/index.sorex",
  "target/e2e/output/sorex.js",
  "target/e2e/output/demo.html",
];

function getModTime(path: string): number {
  try {
    return statSync(path).mtimeMs;
  } catch {
    return 0;
  }
}

function needsRebuild(): boolean {
  // Check if outputs exist
  for (const output of OUTPUTS) {
    const fullPath = join(ROOT, output);
    if (!existsSync(fullPath)) {
      console.log(`  [global-setup] Missing: ${output}`);
      return true;
    }
  }

  // Check if any source is newer than oldest output
  const oldestOutput = Math.min(
    ...OUTPUTS.map((p) => getModTime(join(ROOT, p)))
  );
  for (const source of SOURCES) {
    const sourceTime = getModTime(join(ROOT, source));
    if (sourceTime > oldestOutput) {
      console.log(`  [global-setup] Source newer than output: ${source}`);
      return true;
    }
  }

  return false;
}

export default async function globalSetup() {
  console.log("[global-setup] Checking if fixtures need rebuild...");

  if (!needsRebuild()) {
    console.log("[global-setup] Fixtures up-to-date, skipping rebuild");
    return;
  }

  console.log("[global-setup] Rebuilding fixtures...");

  // Step 1: Build the loader (in case loader.ts changed)
  console.log("[global-setup] Building loader...");
  execSync("deno task build", {
    cwd: join(ROOT, "tools"),
    stdio: "inherit",
  });

  // Step 2: Build the release binary (picks up new loader)
  console.log("[global-setup] Building release binary...");
  execSync("cargo build --release --quiet", {
    cwd: ROOT,
    stdio: "inherit",
  });

  // Step 3: Build test index
  console.log("[global-setup] Building test index...");
  execSync(
    "./target/release/sorex index --input data/e2e/fixtures --output target/e2e/output --demo",
    {
      cwd: ROOT,
      stdio: "inherit",
    }
  );

  console.log("[global-setup] Fixtures rebuilt successfully");
}
