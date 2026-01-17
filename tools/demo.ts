#!/usr/bin/env -S deno run --allow-all
/**
 * Build demo search index for a dataset.
 *
 * Crawls documentation (if needed) and builds a .sorex search index.
 * Self-contained with no external project dependencies.
 *
 * Usage:
 *   deno task demo:cutlass
 *   deno task demo:pytorch
 */

import { join, dirname, fromFileUrl } from "https://deno.land/std@0.224.0/path/mod.ts";
import { existsSync } from "https://deno.land/std@0.224.0/fs/exists.ts";
import { parseArgs } from "https://deno.land/std@0.224.0/cli/parse_args.ts";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const TOOLS_DIR = join(ROOT, "tools");

async function main() {
  const args = parseArgs(Deno.args, {
    string: ["dataset"],
    default: { dataset: "cutlass" },
  });

  const dataset = args.dataset.toLowerCase();
  const dataDir = join(ROOT, `target/datasets/${dataset}`);
  const manifestPath = join(dataDir, "manifest.json");

  console.log(`=== Building ${dataset} demo index ===\n`);

  // Step 1: Crawl if manifest doesn't exist
  if (!existsSync(manifestPath)) {
    console.log(`Crawling ${dataset} documentation...`);
    const crawl = new Deno.Command("deno", {
      args: ["task", `crawl:${dataset}`],
      cwd: TOOLS_DIR,
      stdin: "inherit",
      stdout: "inherit",
      stderr: "inherit",
    });
    const result = await crawl.output();
    if (!result.success) {
      console.error("Crawl failed");
      Deno.exit(1);
    }
  } else {
    console.log(`${dataset} data exists, skipping crawl`);
  }

  // Step 2: Build index with sorex CLI
  console.log(`\nBuilding search index...`);
  const sorex = new Deno.Command("sorex", {
    args: ["index", "--input", dataDir, "--output", dataDir],
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  });
  const indexResult = await sorex.output();
  if (!indexResult.success) {
    console.error("Index build failed");
    Deno.exit(1);
  }

  console.log(`\nDemo index built: ${dataDir}/`);
}

main();
