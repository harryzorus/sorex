#!/usr/bin/env -S deno run --allow-read --allow-write --allow-net
/**
 * Documentation crawler for benchmark datasets
 *
 * Crawls CUTLASS or PyTorch documentation to generate test data.
 *
 * Usage:
 *   deno task crawl:cutlass
 *   deno task crawl:pytorch
 *   deno run --allow-all tools/crawl.ts --dataset cutlass
 */

import { parseArgs } from "https://deno.land/std@0.224.0/cli/parse_args.ts";
import { join, dirname, fromFileUrl } from "https://deno.land/std@0.224.0/path/mod.ts";
import { existsSync } from "https://deno.land/std@0.224.0/fs/exists.ts";
import { load as cheerioLoad } from "npm:cheerio@1.0.0";
import { createHash } from "node:crypto";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");

// =============================================================================
// CONFIGURATION
// =============================================================================

interface DatasetConfig {
  name: string;
  version: string;
  baseUrl: string;
  outputDir: string;
  maxPages: number;
  extractCategory: (url: string) => string;
  skipCategory?: (category: string) => boolean;
  titleSelector: string;
  contentSelector: string;
}

const DATASETS: Record<string, DatasetConfig> = {
  cutlass: {
    name: "NVIDIA CUTLASS",
    version: "4.3.4",
    baseUrl: "https://docs.nvidia.com/cutlass/4.3.4",
    outputDir: join(ROOT, "target/datasets/cutlass"),
    maxPages: 500,
    titleSelector: "article h1",
    contentSelector: "article, .main-content, main, .content",
    extractCategory: (url: string) => {
      if (url.includes("/cute/") || url.includes("cute_")) return "CuTe";
      if (url.includes("/pythonDSL/") || url.includes("python")) return "Python DSL";
      if (url.includes("cutlass_3x") || url.includes("3x")) return "CUTLASS 3.x";
      if (url.includes("cutlass_2x") || url.includes("2x")) return "CUTLASS 2.x";
      return "CUTLASS";
    },
    skipCategory: (category: string) => category === "CUTLASS 2.x" || category === "CUTLASS 3.x",
  },
  pytorch: {
    name: "PyTorch",
    version: "2.9",
    baseUrl: "https://pytorch.org/docs/2.9",
    outputDir: join(ROOT, "target/datasets/pytorch"),
    maxPages: 300,
    titleSelector: "article h1, .document h1, .body h1",
    contentSelector: "article, .document, .body, .content",
    extractCategory: (url: string) => {
      if (url.includes("/nn.")) return "Neural Networks";
      if (url.includes("/torch.")) return "Core";
      if (url.includes("/cuda")) return "CUDA";
      if (url.includes("/distributed")) return "Distributed";
      if (url.includes("/optim")) return "Optimization";
      if (url.includes("/autograd")) return "Autograd";
      return "PyTorch";
    },
  },
};

// =============================================================================
// TYPES
// =============================================================================

interface FieldBoundary {
  docId: number;
  start: number;
  end: number;
  fieldType: "title" | "heading" | "content";
  sectionId: string | null;
  headingLevel: number;
}

interface Doc {
  id: number;
  slug: string;
  title: string;
  excerpt: string;
  href: string;
  type: "page";
  category: string;
  text: string;
  fieldBoundaries: FieldBoundary[];
}

// =============================================================================
// UTILITIES
// =============================================================================

const contentHashes = new Set<string>();

function hashContent(text: string): string {
  return createHash("sha256").update(text).digest("hex").slice(0, 16);
}

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
}

// =============================================================================
// CRAWLER
// =============================================================================

const visitedUrls = new Set<string>();
const pendingUrls: string[] = [];

async function fetchPage(url: string): Promise<string | null> {
  try {
    console.log(`  Fetching ${url}...`);
    const res = await fetch(url, {
      headers: {
        "User-Agent": "Mozilla/5.0 (compatible; SorexCrawler/1.0)",
        Accept: "text/html",
      },
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

function extractTitle(
  $: ReturnType<typeof cheerioLoad>,
  config: DatasetConfig
): string {
  const $h1 = $(config.titleSelector).first();
  if ($h1.length) {
    const $clone = $h1.clone();
    $clone.find(".headerlink, a.headerlink, .viewcode-link").remove();
    return $clone.text().trim();
  }

  const pageTitle = $("title").text().trim();
  if (pageTitle) {
    return pageTitle
      .replace(/ - NVIDIA CUTLASS.*$/, "")
      .replace(/ — PyTorch.*$/, "")
      .replace(/#$/, "")
      .trim();
  }

  return "Untitled";
}

function extractSections(
  $: ReturnType<typeof cheerioLoad>,
  config: DatasetConfig
): Array<{ id: string; heading: string; content: string; level: number }> {
  const sections: Array<{ id: string; heading: string; content: string; level: number }> = [];

  const article = $(config.contentSelector).first();
  if (!article.length) return sections;

  article.find("h1, h2, h3, h4, h5, h6").each((_, heading) => {
    const $heading = $(heading);
    const tagName = heading.tagName.toLowerCase();
    const level = parseInt(tagName.charAt(1), 10);

    const $clone = $heading.clone();
    $clone.find(".headerlink, a.headerlink, .viewcode-link").remove();
    const headingText = $clone.text().trim();

    let id = $heading.attr("id");
    if (!id) {
      const prevSpan = $heading.prev("span[id]");
      if (prevSpan.length) {
        id = prevSpan.attr("id") || "";
      }
    }
    if (!id) {
      id = slugify(headingText);
    }

    let content = "";
    let next = $heading.next();
    while (next.length) {
      if (next.is("h1, h2, h3, h4, h5, h6")) break;
      const $nextClone = next.clone();
      $nextClone.find("pre, code.highlight").remove();
      content += $nextClone.text() + " ";
      next = next.next();
    }

    sections.push({ id, heading: headingText, content: content.trim(), level });
  });

  return sections;
}

function extractText($: ReturnType<typeof cheerioLoad>, config: DatasetConfig): string {
  const $clone = $.root().clone();
  const $doc = cheerioLoad($clone.html() || "");

  $doc("script, style, nav, footer, .sidebar, .toc, .headerlink, a.headerlink").remove();

  const article = $doc(config.contentSelector).first();
  const text = article.length ? article.text() : $doc("body").text();

  return text
    .replace(/\s+/g, " ")
    .replace(/\s#\s/g, " ")
    .replace(/#$/gm, "")
    .trim();
}

function extractLinks(
  $: ReturnType<typeof cheerioLoad>,
  baseUrl: string,
  config: DatasetConfig
): string[] {
  const links: string[] = [];

  $("a[href]").each((_, el) => {
    const href = $(el).attr("href");
    if (!href) return;

    let absoluteUrl: string;
    try {
      absoluteUrl = new URL(href, baseUrl).toString();
    } catch {
      return;
    }

    if (absoluteUrl.startsWith(config.baseUrl) && absoluteUrl.endsWith(".html")) {
      const cleanUrl = absoluteUrl.split("#")[0];
      if (!visitedUrls.has(cleanUrl)) {
        links.push(cleanUrl);
      }
    }
  });

  return links;
}

async function crawl(config: DatasetConfig): Promise<Doc[]> {
  const docs: Doc[] = [];
  let docId = 0;

  pendingUrls.push(`${config.baseUrl}/index.html`);

  while (pendingUrls.length > 0 && docs.length < config.maxPages) {
    const url = pendingUrls.shift()!;

    if (visitedUrls.has(url)) continue;
    visitedUrls.add(url);

    const html = await fetchPage(url);
    if (!html) continue;

    const $ = cheerioLoad(html);

    const links = extractLinks($, url, config);
    pendingUrls.push(...links);

    const title = extractTitle($, config);
    const text = extractText($, config);
    const sections = extractSections($, config);

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

    const category = config.extractCategory(url);
    if (config.skipCategory?.(category)) {
      console.log(`    Skipping legacy: ${title} (${category})`);
      continue;
    }

    const slug = url
      .replace(config.baseUrl, "")
      .replace(/^\//, "")
      .replace(/\.html$/, "")
      .replace(/\//g, "-");

    const fieldBoundaries: FieldBoundary[] = [];
    let offset = 0;
    const currentDocId = docId;

    fieldBoundaries.push({
      docId: currentDocId,
      start: offset,
      end: offset + title.length,
      fieldType: "title",
      sectionId: null,
      headingLevel: 0,
    });
    offset += title.length + 1;

    for (const section of sections) {
      fieldBoundaries.push({
        docId: currentDocId,
        start: offset,
        end: offset + section.heading.length,
        fieldType: "heading",
        sectionId: section.id,
        headingLevel: section.level,
      });
      offset += section.heading.length + 1;

      if (section.content) {
        fieldBoundaries.push({
          docId: currentDocId,
          start: offset,
          end: offset + section.content.length,
          fieldType: "content",
          sectionId: section.id,
          headingLevel: 5,
        });
        offset += section.content.length + 1;
      }
    }

    const excerpt = text.slice(0, 200) + (text.length > 200 ? "..." : "");

    docs.push({
      id: docId++,
      slug,
      title,
      excerpt,
      href: url,
      type: "page",
      category,
      text: `${title} ${text}`,
      fieldBoundaries,
    });

    console.log(`    Added: ${title} (${category})`);
    await new Promise((r) => setTimeout(r, 200));
  }

  return docs;
}

// =============================================================================
// MAIN
// =============================================================================

async function main() {
  const args = parseArgs(Deno.args, {
    string: ["dataset"],
    default: { dataset: "" },
  });

  const datasetName = args.dataset.toLowerCase();
  if (!datasetName || !DATASETS[datasetName]) {
    console.error("Usage: deno run --allow-all tools/crawl.ts --dataset <cutlass|pytorch>");
    console.error("Available datasets:", Object.keys(DATASETS).join(", "));
    Deno.exit(1);
  }

  const config = DATASETS[datasetName];
  console.log(`=== ${config.name} ${config.version} Documentation Crawler ===\n`);

  // Clean and create output directory
  if (existsSync(config.outputDir)) {
    await Deno.remove(config.outputDir, { recursive: true });
  }
  await Deno.mkdir(config.outputDir, { recursive: true });

  console.log(`\nCrawling ${config.baseUrl}...\n`);

  const docs = await crawl(config);
  console.log(`\n  Crawled ${docs.length} pages`);

  console.log("\nWriting output files...\n");

  // Renumber IDs sequentially
  docs.forEach((doc, i) => {
    doc.id = i;
    for (const boundary of doc.fieldBoundaries) {
      boundary.docId = i;
    }
  });

  // Write per-document JSON files
  for (const doc of docs) {
    await Deno.writeTextFile(
      join(config.outputDir, `${doc.id}.json`),
      JSON.stringify(doc, null, "\t")
    );
  }

  // Write manifest
  const manifest = {
    version: 1,
    documents: docs.map((_, i) => `${i}.json`),
    indexes: { all: { name: "all", include: "all" } },
  };
  await Deno.writeTextFile(
    join(config.outputDir, "manifest.json"),
    JSON.stringify(manifest, null, "\t")
  );

  // Write summary
  const summary = {
    version: config.version,
    totalDocuments: docs.length,
    categories: [...new Set(docs.map((d) => d.category))],
    timestamp: new Date().toISOString(),
  };
  await Deno.writeTextFile(
    join(config.outputDir, "summary.json"),
    JSON.stringify(summary, null, "\t")
  );

  console.log(`\n=== Summary ===`);
  console.log(`Total documents: ${docs.length}`);
  console.log(`Categories: ${summary.categories.join(", ")}`);
  console.log(`\nOutput written to: ${config.outputDir}/`);
  console.log(`\nBuild index with: sorex index --input ${config.outputDir} --output ${config.outputDir}`);
}

main();
