#!/usr/bin/env -S deno run --allow-read --allow-net
/**
 * Test server with COOP/COEP headers for SharedArrayBuffer
 *
 * Required for testing multi-threaded WASM in browsers.
 *
 * Usage:
 *   deno task serve                           # Default: ./data/pkg-test on port 8888
 *   deno run -A serve.ts --dir ./output --port 3000
 */

import { parseArgs } from "jsr:@std/cli/parse-args";

const args = parseArgs(Deno.args, {
  string: ["dir", "port"],
  default: { dir: "./data/pkg-test", port: "8888" },
});

const PORT = parseInt(args.port);
const SERVE_DIR = args.dir;

const MIME_TYPES: Record<string, string> = {
  ".html": "text/html",
  ".js": "application/javascript",
  ".wasm": "application/wasm",
  ".json": "application/json",
  ".css": "text/css",
};

function getMimeType(path: string): string {
  const ext = path.substring(path.lastIndexOf("."));
  return MIME_TYPES[ext] ?? "application/octet-stream";
}

async function handler(req: Request): Promise<Response> {
  const url = new URL(req.url);
  let path = url.pathname;

  console.log("Request:", path);

  // Handle test page
  if (path === "/test-threading.html" || path === "/test") {
    path = "/test-threading.html";
  }

  // Redirect root/snippets to sorex.js
  if (path === "/" || path === "/snippets" || path === "/snippets/") {
    console.log("Redirecting to sorex.js");
    path = "/sorex.js";
  }

  const filePath = SERVE_DIR + path;

  try {
    const file = await Deno.readFile(filePath);
    return new Response(file, {
      headers: {
        "Content-Type": getMimeType(path),
        // COOP/COEP headers for SharedArrayBuffer
        "Cross-Origin-Opener-Policy": "same-origin",
        "Cross-Origin-Embedder-Policy": "require-corp",
        "Cross-Origin-Resource-Policy": "cross-origin",
      },
    });
  } catch {
    return new Response("Not found", { status: 404 });
  }
}

console.log(`Server running at http://localhost:${PORT}/`);
console.log(`Test page: http://localhost:${PORT}/test-threading.html`);

Deno.serve({ port: PORT }, handler);
