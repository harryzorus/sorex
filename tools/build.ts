#!/usr/bin/env -S deno run --allow-read --allow-write --allow-run --allow-env
/**
 * Build sorex.js from TypeScript loader + wasm-pack output
 *
 * This script:
 * 1. Compiles tools/loader.ts to JavaScript using Deno
 * 2. Reads wasm-bindgen output from target/pkg/sorex.js
 * 3. Reads wasm-bindgen-rayon workerHelpers.js snippet
 * 4. Combines: VERSION const + wasm-bindgen + startWorkers + loader
 * 5. Outputs a self-contained sorex.js
 *
 * Usage: deno task build
 */

import { join, dirname, fromFileUrl } from "https://deno.land/std@0.224.0/path/mod.ts";
import { existsSync } from "https://deno.land/std@0.224.0/fs/exists.ts";

const ROOT = join(dirname(fromFileUrl(import.meta.url)), "..");
const LOADER_TS = join(ROOT, "tools/loader.ts");
const PKG_JS = join(ROOT, "target/pkg/sorex.js");
const SNIPPETS_DIR = join(ROOT, "target/pkg/snippets");
const OUTPUT_DIR = join(ROOT, "target/loader");
const OUTPUT_FILE = join(OUTPUT_DIR, "sorex.js");
const HEADER_RS = join(ROOT, "src/binary/header.rs");

/**
 * Read VERSION from src/binary/header.rs (single source of truth).
 */
function readRustVersion(): number {
  const content = Deno.readTextFileSync(HEADER_RS);
  const match = content.match(/pub const VERSION:\s*u8\s*=\s*(\d+)/);
  if (!match) {
    throw new Error(`Failed to extract VERSION from ${HEADER_RS}`);
  }
  return parseInt(match[1], 10);
}

/**
 * Patch workerHelpers.js to fix browser import issue.
 */
function patchWorkerHelpers(): boolean {
  if (!existsSync(SNIPPETS_DIR)) {
    return false;
  }

  const dirs = [...Deno.readDirSync(SNIPPETS_DIR)].map((e) => e.name);
  const rayonDir = dirs.find((d) => d.startsWith("wasm-bindgen-rayon-"));
  if (!rayonDir) {
    return false;
  }

  const helperPath = join(SNIPPETS_DIR, rayonDir, "src/workerHelpers.js");
  if (!existsSync(helperPath)) {
    return false;
  }

  let content = Deno.readTextFileSync(helperPath);

  if (content.includes("../../../sorex.js")) {
    console.log(`  workerHelpers.js already patched`);
    return true;
  }

  const original = "const pkg = await import('../../..');";
  const patched =
    "// Fixed path: browsers can't import directories, need explicit file\n  const pkg = await import('../../../sorex.js');";

  if (content.includes(original)) {
    content = content.replace(original, patched);
    Deno.writeTextFileSync(helperPath, content);
    console.log(`  Patched workerHelpers.js for browser compatibility`);
    return true;
  }

  console.warn(`  Warning: Could not find import to patch in workerHelpers.js`);
  return false;
}

/**
 * Generate startWorkers function for thread pool initialization.
 */
function generateStartWorkers(): string {
  return `
// Worker helpers for wasm-bindgen-rayon
function waitForMsgType(target, type, timeoutMs = 30000) {
    return new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
            console.error('[sorex] Worker timeout waiting for:', type);
            reject(new Error('Worker initialization timeout'));
        }, timeoutMs);

        target.addEventListener('message', function onMsg(event) {
            const data = event.data;
            if (data?.type === 'wasm_bindgen_worker_error') {
                clearTimeout(timeout);
                target.removeEventListener('message', onMsg);
                reject(new Error(data.error));
                return;
            }
            if (data?.type !== type) return;
            clearTimeout(timeout);
            target.removeEventListener('message', onMsg);
            resolve(data);
        });
    });
}

let _loaderUrl = null;
let _workerBlobUrl = null;
let _pool = { workers: null, promise: null };

function getLoaderUrl() {
    if (_loaderUrl) return _loaderUrl;
    if (typeof import.meta !== 'undefined' && import.meta.url) {
        _loaderUrl = import.meta.url;
    }
    return _loaderUrl;
}

function getWorkerBlobUrl(loaderUrl) {
    if (_workerBlobUrl) return _workerBlobUrl;

    const workerCode = \`
self.onmessage = async function(e) {
    if (e.data?.type !== 'wasm_bindgen_worker_init') return;

    const { init, receiver } = e.data;
    const { loaderUrl, module_or_path: wasmModule, memory } = init;

    try {
        const pkg = await import(loaderUrl);
        await pkg.default({ module_or_path: wasmModule, memory });
        self.postMessage({ type: 'wasm_bindgen_worker_ready' });
        await new Promise(r => setTimeout(r, 0));
        pkg.wbg_rayon_start_worker(receiver);
    } catch (err) {
        self.postMessage({ type: 'wasm_bindgen_worker_error', error: err.message });
    }
};
\`;

    const blob = new Blob([workerCode], { type: 'application/javascript' });
    _workerBlobUrl = URL.createObjectURL(blob);
    return _workerBlobUrl;
}

async function startWorkers(module, memory, builder) {
    if (builder.numThreads() === 0) {
        throw new Error('num_threads must be > 0.');
    }

    if (_pool.promise) {
        await _pool.promise;
        return;
    }

    _pool.promise = (async () => {
        const loaderUrl = getLoaderUrl();
        if (!loaderUrl) {
            throw new Error('Cannot determine loader URL. import.meta.url not available.');
        }

        const workerUrl = getWorkerBlobUrl(loaderUrl);
        console.log('[sorex] Initializing thread pool with', builder.numThreads(), 'threads...');

        const receiver = builder.receiver();
        const workerInit = {
            type: 'wasm_bindgen_worker_init',
            init: { loaderUrl, module_or_path: module, memory },
            receiver: receiver
        };

        const numThreads = builder.numThreads();
        _pool.workers = await Promise.all(
            Array.from({ length: numThreads }, async () => {
                const worker = new Worker(workerUrl, { type: 'module' });
                worker.postMessage(workerInit);
                await waitForMsgType(worker, 'wasm_bindgen_worker_ready');
                return worker;
            })
        );

        console.log('[sorex] All', numThreads, 'workers ready');

        try {
            builder.build();
            console.log('[sorex] Thread pool ready - using parallel mode');
        } catch (e) {
            console.error('[sorex] builder.build() failed:', e);
            for (const worker of _pool.workers) {
                worker.terminate();
            }
            _pool.workers = null;
            _pool.promise = null;
            throw e;
        }
    })();

    await _pool.promise;
}
`;
}

/**
 * Strip ES module syntax from wasm-bindgen output.
 */
function stripModuleSyntax(source: string): string {
  let code = source;

  code = code.replace(/export class /g, "class ");
  code = code.replace(/export function /g, "function ");
  code = code.replace(/export async function /g, "async function ");
  code = code.replace(/export const /g, "const ");
  code = code.replace(/export let /g, "let ");
  code = code.replace(/\nexport\s*\{[^}]*\}\s*;?/g, "");
  code = code.replace(/\nexport\s+default\s+[^;]+;/g, "");
  code = code.replace(/^import\s+.*?;?\n/gm, "");

  code = code.replace(
    /function __wbg_get_imports\(\) \{/,
    `function __wbg_get_imports(memory) {`
  );
  code = code.replace(/imports\.wbg = \{\};/, `imports.wbg = memory ? { memory } : {};`);

  code = code.replace(
    /function initSync\(module\) \{\s*if \(wasm !== undefined\) return wasm;/,
    `function initSync(module) {
    if (wasm !== undefined) return wasm;
    let memory;`
  );

  code = code.replace(
    /if \(typeof module !== 'undefined'\) \{[\s\S]*?if \(Object\.getPrototypeOf\(module\) === Object\.prototype\)[\s\S]*?console\.warn\([^)]+\)[\s\S]*?\}\s*\}/,
    `if (typeof module !== 'undefined' && typeof module === 'object' && module !== null) {
        const opts = module;
        if (opts.module !== undefined) {
            module = opts.module;
            memory = opts.memory;
        }
    }`
  );

  code = code.replace(
    /const imports = __wbg_get_imports\(\);(\s*if \(!\(module instanceof WebAssembly\.Module\)\))/,
    `const imports = __wbg_get_imports(memory);$1`
  );

  return code;
}

/**
 * Compile loader.ts to JavaScript using Deno bundler.
 */
async function compileLoader(): Promise<string> {
  console.log("  Compiling loader.ts...");

  const command = new Deno.Command("deno", {
    args: ["bundle", "--quiet", LOADER_TS],
    stdout: "piped",
    stderr: "piped",
  });

  const { success, stdout, stderr } = await command.output();

  if (!success) {
    const errText = new TextDecoder().decode(stderr);
    throw new Error(`Failed to compile loader.ts: ${errText}`);
  }

  let code = new TextDecoder().decode(stdout);

  // Remove the declare statements and @EXPORTS@ marker
  code = code.replace(/declare\s+(const|let|var|class|function)[^;{]+[;{][^}]*}?/g, "");
  code = code.replace(/\/\/\s*@EXPORTS@/, "");

  // Strip any export statements from the bundled output
  code = code.replace(/export\s*\{[^}]*\}\s*;?/g, "");

  return code;
}

async function main() {
  console.log("Building sorex.js...");

  if (!existsSync(PKG_JS)) {
    throw new Error(`wasm-pack output not found: ${PKG_JS}\nRun: wasm-pack build --target web --release`);
  }

  const SOREX_VERSION = readRustVersion();
  console.log(`  Version: ${SOREX_VERSION}`);

  patchWorkerHelpers();

  const pkgJs = Deno.readTextFileSync(PKG_JS);
  const strippedBindings = stripModuleSyntax(pkgJs);
  const startWorkersCode = generateStartWorkers();
  const loaderCode = await compileLoader();

  // Combine everything
  const combined = `/**
 * Sorex Loader - Self-contained search loader
 *
 * AUTO-GENERATED FILE - Do not edit directly!
 * Source: tools/loader.ts + target/pkg/sorex.js
 * Generator: tools/build.ts
 *
 * Usage:
 *   import { loadSorex, SorexSearcher } from './sorex.js';
 *   const searcher = await loadSorex('./index.sorex');
 *   searcher.search('query', 10, { onFinish: (results) => console.log(results) });
 */

// Version constant (from src/binary/header.rs)
const VERSION = ${SOREX_VERSION};

// =============================================================================
// wasm-bindgen output
// =============================================================================

${strippedBindings}

// =============================================================================
// Thread pool workers
// =============================================================================

${startWorkersCode}

// =============================================================================
// Loader (compiled from tools/loader.ts)
// =============================================================================

${loaderCode}

// =============================================================================
// ES Module exports
// =============================================================================

export { loadSorex, loadSorexSync, SorexSearcherWrapper as SorexSearcher };
export { wbg_rayon_start_worker, __wbg_init as default };
`;

  // Write output
  Deno.mkdirSync(OUTPUT_DIR, { recursive: true });
  Deno.writeTextFileSync(OUTPUT_FILE, combined);
  console.log(`  Generated ${OUTPUT_FILE}`);

  // Copy to datasets
  const datasets = ["cutlass", "pytorch"];
  for (const dataset of datasets) {
    const destPath = join(ROOT, "target/datasets", dataset, "sorex.js");
    try {
      Deno.writeTextFileSync(destPath, combined);
      console.log(`  Updated ${destPath}`);
    } catch {
      // Dataset may not exist
    }
  }

  console.log("Done!");
}

main();
