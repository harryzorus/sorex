// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! Deno runtime for executing embedded WASM search.
//!
//! Why execute WASM through Deno when we have native Rust? Parity testing. The
//! WASM module is what browsers actually run, and it's easy to introduce bugs
//! that only manifest in the JS boundary code. This module lets us run the same
//! searches through both paths and compare results.
//!
//! The implementation is more complex than it should be because V8's WASM support
//! has quirks. We need polyfills for `TextEncoder`, `TextDecoder`, and other Web
//! APIs that deno_core doesn't provide. And we need to warm up TurboFan before
//! benchmarking, otherwise the first few searches run at baseline compiler speed.
//!
//! # Usage
//!
//! ```rust,ignore
//! use sorex::deno_runtime::{DenoRuntime, WasmSearchResult};
//!
//! // Create runtime (initializes Deno once)
//! let runtime = DenoRuntime::new()?;
//!
//! // Load .sorex file bytes and JS loader
//! let sorex_bytes = std::fs::read("index.sorex")?;
//! let loader_js = include_str!("path/to/sorex.js");
//!
//! // Execute search through WASM
//! let results = runtime.search(&sorex_bytes, loader_js, "query", 10)?;
//! ```

use serde::{Deserialize, Serialize};

/// Search result from WASM execution.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmSearchResult {
    pub href: String,
    pub title: String,
    pub excerpt: String,
    pub section_id: Option<String>,
    pub tier: u8,
    pub match_type: u8,
    pub score: f64,
    pub matched_term: Option<String>,
}

/// Search result with per-tier timing breakdown.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmTierTimingResult {
    pub results: Vec<WasmSearchResult>,
    pub t1_count: usize,
    pub t2_count: usize,
    pub t3_count: usize,
    pub t1_time_us: f64,
    pub t2_time_us: f64,
    pub t3_time_us: f64,
}

/// Context passed to the user's scoring function for each (term, doc, match) tuple.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringContext {
    /// The vocabulary term being indexed
    pub term: String,
    /// Document metadata
    pub doc: ScoringDocContext,
    /// Match location within the document
    #[serde(rename = "match")]
    pub match_info: ScoringMatchContext,
}

/// Document metadata for scoring context.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringDocContext {
    pub id: usize,
    pub title: String,
    pub excerpt: String,
    pub href: String,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub category: Option<String>,
    pub author: Option<String>,
    pub tags: Vec<String>,
}

/// Match location within the document.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScoringMatchContext {
    pub field_type: String,
    pub heading_level: u8,
    pub section_id: Option<String>,
    pub offset: usize,
    pub text_length: usize,
}

#[cfg(feature = "deno-runtime")]
mod deno_impl {
    use super::WasmSearchResult;
    use deno_core::{v8, JsRuntime, RuntimeOptions};
    use std::sync::Once;

    static DENO_INIT: Once = Once::new();

    /// Polyfills for Web APIs.
    ///
    /// deno_core provides raw V8 access but doesn't include browser APIs
    /// like TextEncoder/TextDecoder. We provide minimal implementations here.
    const WEB_API_POLYFILLS: &str = r#"
// TextEncoder polyfill (UTF-8 only)
if (typeof TextEncoder === 'undefined') {
    class TextEncoder {
        encode(str) {
            const utf8 = [];
            for (let i = 0; i < str.length; i++) {
                let charcode = str.charCodeAt(i);
                if (charcode < 0x80) {
                    utf8.push(charcode);
                } else if (charcode < 0x800) {
                    utf8.push(0xc0 | (charcode >> 6), 0x80 | (charcode & 0x3f));
                } else if (charcode < 0xd800 || charcode >= 0xe000) {
                    utf8.push(0xe0 | (charcode >> 12), 0x80 | ((charcode >> 6) & 0x3f), 0x80 | (charcode & 0x3f));
                } else {
                    // surrogate pair
                    i++;
                    charcode = 0x10000 + (((charcode & 0x3ff) << 10) | (str.charCodeAt(i) & 0x3ff));
                    utf8.push(0xf0 | (charcode >> 18), 0x80 | ((charcode >> 12) & 0x3f), 0x80 | ((charcode >> 6) & 0x3f), 0x80 | (charcode & 0x3f));
                }
            }
            return new Uint8Array(utf8);
        }
        encodeInto(str, dest) {
            const encoded = this.encode(str);
            const len = Math.min(encoded.length, dest.length);
            dest.set(encoded.subarray(0, len));
            return { read: str.length, written: len };
        }
    }
    globalThis.TextEncoder = TextEncoder;
}

// TextDecoder polyfill (UTF-8 only)
if (typeof TextDecoder === 'undefined') {
    class TextDecoder {
        constructor(encoding = 'utf-8', options = {}) {
            this.encoding = encoding;
            this.fatal = options.fatal || false;
            this.ignoreBOM = options.ignoreBOM || false;
        }
        decode(input) {
            if (!input) return '';
            const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
            let result = '';
            let i = 0;
            while (i < bytes.length) {
                let c = bytes[i++];
                if (c < 0x80) {
                    result += String.fromCharCode(c);
                } else if ((c & 0xe0) === 0xc0) {
                    result += String.fromCharCode(((c & 0x1f) << 6) | (bytes[i++] & 0x3f));
                } else if ((c & 0xf0) === 0xe0) {
                    result += String.fromCharCode(((c & 0x0f) << 12) | ((bytes[i++] & 0x3f) << 6) | (bytes[i++] & 0x3f));
                } else if ((c & 0xf8) === 0xf0) {
                    const codePoint = ((c & 0x07) << 18) | ((bytes[i++] & 0x3f) << 12) | ((bytes[i++] & 0x3f) << 6) | (bytes[i++] & 0x3f);
                    // Convert to surrogate pair
                    const surrogate = codePoint - 0x10000;
                    result += String.fromCharCode(0xd800 + (surrogate >> 10), 0xdc00 + (surrogate & 0x3ff));
                }
            }
            return result;
        }
    }
    globalThis.TextDecoder = TextDecoder;
}

// Performance API stub (for timing marks)
if (typeof performance === 'undefined') {
    globalThis.performance = {
        mark: function() {},
        measure: function() {},
        now: function() { return Date.now(); }
    };
}

// FinalizationRegistry stub (not critical for testing)
if (typeof FinalizationRegistry === 'undefined') {
    globalThis.FinalizationRegistry = class {
        register() {}
        unregister() {}
    };
}

// Base64 decode function for receiving binary data from Rust
globalThis.__base64ToBytes__ = function(base64) {
    const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/';
    const lookup = new Uint8Array(256);
    for (let i = 0; i < chars.length; i++) {
        lookup[chars.charCodeAt(i)] = i;
    }

    let bufferLength = base64.length * 0.75;
    if (base64[base64.length - 1] === '=') bufferLength--;
    if (base64[base64.length - 2] === '=') bufferLength--;

    const bytes = new Uint8Array(bufferLength);
    let p = 0;

    for (let i = 0; i < base64.length; i += 4) {
        const encoded1 = lookup[base64.charCodeAt(i)];
        const encoded2 = lookup[base64.charCodeAt(i + 1)];
        const encoded3 = lookup[base64.charCodeAt(i + 2)];
        const encoded4 = lookup[base64.charCodeAt(i + 3)];

        bytes[p++] = (encoded1 << 2) | (encoded2 >> 4);
        if (p < bufferLength) bytes[p++] = ((encoded2 & 15) << 4) | (encoded3 >> 2);
        if (p < bufferLength) bytes[p++] = ((encoded3 & 3) << 6) | (encoded4 & 63);
    }

    return bytes;
};
"#;

    /// Base64 encode bytes for passing to JavaScript.
    fn bytes_to_base64(bytes: &[u8]) -> String {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::with_capacity(bytes.len().div_ceil(3) * 4);

        for chunk in bytes.chunks(3) {
            let b0 = chunk[0] as usize;
            let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
            let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

            result.push(CHARS[b0 >> 2] as char);
            result.push(CHARS[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

            if chunk.len() > 1 {
                result.push(CHARS[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
            } else {
                result.push('=');
            }

            if chunk.len() > 2 {
                result.push(CHARS[b2 & 0x3f] as char);
            } else {
                result.push('=');
            }
        }

        result
    }

    /// Initialize Deno with V8 flags optimized for WASM performance.
    ///
    /// V8's WASM compilation tiers:
    /// - Liftoff: Baseline compiler, fast compilation, moderate execution speed
    /// - TurboFan: Optimizing compiler, slower compilation, fastest execution
    ///
    /// We configure V8 to use aggressive tier-up settings for benchmarking accuracy.
    fn init_deno() {
        DENO_INIT.call_once(|| {
            // Set V8 flags for aggressive TurboFan compilation
            // --wasm-dynamic-tiering-budget=10: Lower threshold for tier-up (default ~1000)
            // This forces TurboFan to kick in after ~10 function calls instead of ~1000
            deno_core::v8_set_flags(vec![
                "".to_string(), // argv[0] placeholder
                "--wasm-dynamic-tiering-budget=10".to_string(),
            ]);
        });
    }

    /// Number of iterations needed to trigger V8 TurboFan optimization for WASM.
    ///
    /// With --wasm-dynamic-tiering-budget=10, TurboFan triggers after ~10 calls.
    /// We use 10 iterations - just enough to ensure optimization kicks in.
    /// The benchmark loop will run many more iterations for statistical accuracy.
    pub const TURBOFAN_WARMUP_ITERATIONS: usize = 10;

    /// Persistent Deno search context with pre-initialized WASM.
    ///
    /// Use this for benchmarking to avoid WASM initialization overhead on each search.
    /// The WASM module is loaded once during construction and reused for all searches.
    ///
    /// # Example
    /// ```rust,ignore
    /// let ctx = DenoSearchContext::new(&sorex_bytes, &loader_js)?;
    /// let results1 = ctx.search("query1", 10)?;
    /// let results2 = ctx.search("query2", 10)?;  // Reuses initialized WASM
    /// ```
    pub struct DenoSearchContext {
        runtime: JsRuntime,
    }

    impl DenoSearchContext {
        /// Create a new search context with pre-initialized WASM.
        ///
        /// This loads the .sorex file, initializes WASM, and creates a SorexSearcher
        /// that will be reused for all subsequent searches.
        pub fn new(sorex_bytes: &[u8], loader_js: &str) -> Result<Self, String> {
            init_deno();

            let mut runtime = JsRuntime::new(RuntimeOptions::default());

            // Add polyfills
            runtime
                .execute_script("<polyfills>", WEB_API_POLYFILLS)
                .map_err(|e| format!("Failed to run polyfills: {}", e))?;

            // Load sorex.js
            let loader_code = strip_esm_exports(loader_js);
            runtime
                .execute_script("<sorex-loader>", loader_code)
                .map_err(|e| format!("Failed to run loader: {}", e))?;

            // Initialize WASM with base64-encoded sorex bytes
            // Use loadSorexSync which handles shared memory creation for WASM
            let base64_bytes = bytes_to_base64(sorex_bytes);
            let init_code = format!(
                r#"
                (function() {{
                    const sorexBytes = __base64ToBytes__("{base64}");
                    const sorexBuffer = sorexBytes.buffer;
                    // Use loadSorexSync which handles memory import for shared memory WASM
                    globalThis.__sorex_searcher__ = loadSorexSync(sorexBuffer);
                    return true;
                }})();
                "#,
                base64 = base64_bytes
            );
            runtime
                .execute_script("<init>", init_code)
                .map_err(|e| format!("Failed to initialize WASM: {}", e))?;

            Ok(Self { runtime })
        }

        /// Execute a search using the pre-initialized WASM searcher.
        ///
        /// This only runs the search - no WASM initialization overhead.
        pub fn search(
            &mut self,
            query: &str,
            limit: usize,
        ) -> Result<Vec<WasmSearchResult>, String> {
            let escaped_query = escape_js_string(query);
            let search_code = format!(
                r#"JSON.stringify(__sorex_searcher__.searchSync("{}", {}))"#,
                escaped_query, limit
            );

            let result = self
                .runtime
                .execute_script("<search>", search_code)
                .map_err(|e| format!("Search failed: {}", e))?;

            // Convert v8::Global to string using isolate and context
            let json_str = {
                let context = self.runtime.main_context();
                let isolate = self.runtime.v8_isolate();
                v8::scope!(scope, isolate);
                let context_local = v8::Local::new(scope, context);
                let scope = &mut v8::ContextScope::new(scope, context_local);
                let local = v8::Local::new(scope, result);
                local.to_rust_string_lossy(scope)
            };

            // Debug: print first 500 chars of JSON
            #[cfg(test)]
            eprintln!(
                "DEBUG JSON (first 500): {}",
                &json_str[..json_str.len().min(500)]
            );

            // Parse results
            let results: Vec<WasmSearchResult> = serde_json::from_str(&json_str).map_err(|e| {
                format!("Failed to parse search results: {} (json: {})", e, json_str)
            })?;

            Ok(results)
        }

        /// Execute a search with per-tier timing breakdown.
        ///
        /// Returns results along with T1/T2/T3 timing in microseconds.
        pub fn search_with_tier_timing(
            &mut self,
            query: &str,
            limit: usize,
        ) -> Result<super::WasmTierTimingResult, String> {
            let escaped_query = escape_js_string(query);
            let search_code = format!(
                r#"JSON.stringify(__sorex_searcher__.searchWithTierTiming("{}", {}))"#,
                escaped_query, limit
            );

            let result = self
                .runtime
                .execute_script("<search_tier_timing>", search_code)
                .map_err(|e| format!("Search with tier timing failed: {}", e))?;

            // Convert v8::Global to string using isolate and context
            let json_str = {
                let context = self.runtime.main_context();
                let isolate = self.runtime.v8_isolate();
                v8::scope!(scope, isolate);
                let context_local = v8::Local::new(scope, context);
                let scope = &mut v8::ContextScope::new(scope, context_local);
                let local = v8::Local::new(scope, result);
                local.to_rust_string_lossy(scope)
            };

            // Parse results
            let timing_result: super::WasmTierTimingResult = serde_json::from_str(&json_str)
                .map_err(|e| {
                    format!(
                        "Failed to parse tier timing results: {} (json: {})",
                        e, json_str
                    )
                })?;

            Ok(timing_result)
        }

        /// Warm up the WASM module to trigger V8 TurboFan optimization.
        ///
        /// V8 uses tiered compilation for WASM:
        /// 1. Liftoff (baseline): Fast compilation, moderate execution
        /// 2. TurboFan (optimizing): Slow compilation, fast execution
        ///
        /// TurboFan kicks in after a function is called enough times (~1000).
        /// This method runs the search multiple times to trigger tier-up.
        ///
        /// Call this before benchmarking to ensure consistent, optimized performance.
        pub fn warmup_turbofan(&mut self, query: &str, limit: usize) {
            for _ in 0..TURBOFAN_WARMUP_ITERATIONS {
                let _ = self.search(query, limit);
            }
        }
    }

    /// Deno runtime for executing WASM search.
    ///
    /// Reusable runtime instance - create once and use for multiple searches.
    pub struct DenoRuntime {
        _phantom: std::marker::PhantomData<()>,
    }

    impl DenoRuntime {
        /// Create a new Deno runtime instance.
        pub fn new() -> Result<Self, String> {
            init_deno();
            Ok(Self {
                _phantom: std::marker::PhantomData,
            })
        }

        /// Execute search using embedded WASM via Deno.
        ///
        /// This:
        /// 1. Parses the .sorex file to extract WASM
        /// 2. Instantiates the WASM module synchronously
        /// 3. Runs the search and returns results
        ///
        /// # Arguments
        /// * `sorex_bytes` - The complete .sorex file bytes
        /// * `loader_js` - The sorex.js content (bundled JS)
        /// * `query` - Search query string
        /// * `limit` - Maximum number of results
        pub fn search(
            &self,
            sorex_bytes: &[u8],
            loader_js: &str,
            query: &str,
            limit: usize,
        ) -> Result<Vec<WasmSearchResult>, String> {
            let mut runtime = JsRuntime::new(RuntimeOptions::default());

            // Add polyfills
            runtime
                .execute_script("<polyfills>", WEB_API_POLYFILLS)
                .map_err(|e| format!("Failed to run polyfills: {}", e))?;

            // Load sorex.js
            let loader_code = strip_esm_exports(loader_js);
            runtime
                .execute_script("<sorex-loader>", loader_code)
                .map_err(|e| format!("Failed to run loader: {}", e))?;

            // Execute search with base64-encoded sorex bytes
            let base64_bytes = bytes_to_base64(sorex_bytes);
            let escaped_query = escape_js_string(query);
            let search_code = format!(
                r#"
                (function() {{
                    try {{
                        const sorexBytes = __base64ToBytes__("{base64}");
                        const sorexBuffer = sorexBytes.buffer;

                        // Use loadSorexSync from sorex.js
                        if (typeof loadSorexSync === 'function') {{
                            const searcher = loadSorexSync(sorexBuffer);
                            const results = searcher.searchSync("{query}", {limit});
                            // Debug: log first result structure
                            if (results.length > 0) {{
                                console.log("DEBUG first result keys:", Object.keys(results[0]));
                                console.log("DEBUG first result:", JSON.stringify(results[0]));
                            }}
                            return JSON.stringify(results);
                        }}

                        // Fallback: use parseSorex + initSync directly
                        if (typeof parseSorex === 'function' && typeof initSync === 'function') {{
                            const parsed = parseSorex(sorexBuffer);
                            initSync(parsed.wasm);
                            const searcher = new SorexSearcher(parsed.index);
                            const results = searcher.searchSync("{query}", {limit});
                            return JSON.stringify(results);
                        }}

                        return JSON.stringify({{ error: "No compatible loader found" }});
                    }} catch (e) {{
                        return JSON.stringify({{ error: e.message || String(e), stack: e.stack }});
                    }}
                }})();
                "#,
                base64 = base64_bytes,
                query = escaped_query,
                limit = limit
            );

            let result = runtime
                .execute_script("<search>", search_code)
                .map_err(|e| format!("Search failed: {}", e))?;

            // Convert v8::Global to string using isolate and context
            let json_str = {
                let context = runtime.main_context();
                let isolate = runtime.v8_isolate();
                v8::scope!(scope, isolate);
                let context_local = v8::Local::new(scope, context);
                let scope = &mut v8::ContextScope::new(scope, context_local);
                let local = v8::Local::new(scope, result);
                local.to_rust_string_lossy(scope)
            };

            // Debug: print first 500 chars of JSON
            #[cfg(test)]
            eprintln!(
                "DEBUG DenoRuntime JSON (first 500): {}",
                &json_str[..json_str.len().min(500)]
            );

            // Check for error response
            if json_str.contains("\"error\"") {
                #[derive(serde::Deserialize)]
                struct ErrorResponse {
                    error: String,
                }
                if let Ok(err) = serde_json::from_str::<ErrorResponse>(&json_str) {
                    return Err(format!("WASM search error: {}", err.error));
                }
            }

            // Parse results
            let results: Vec<WasmSearchResult> = serde_json::from_str(&json_str).map_err(|e| {
                format!("Failed to parse search results: {} (json: {})", e, json_str)
            })?;

            Ok(results)
        }
    }

    /// Escape a string for safe inclusion in JavaScript.
    fn escape_js_string(s: &str) -> String {
        let mut result = String::with_capacity(s.len());
        for c in s.chars() {
            match c {
                '\\' => result.push_str("\\\\"),
                '"' => result.push_str("\\\""),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
                c if c.is_control() => {
                    result.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => result.push(c),
            }
        }
        result
    }

    /// Strip TypeScript interface/type declarations (they span multiple lines with braces).
    /// Only matches keywords at the start of a line (with optional whitespace).
    fn strip_typescript_declarations(code: &str) -> String {
        let mut final_result = String::new();
        let mut in_declaration = false;
        let mut brace_count = 0;

        for line in code.lines() {
            let trimmed = line.trim_start();

            // Check if this line starts a new declaration
            if !in_declaration
                && (trimmed.starts_with("export interface ")
                    || trimmed.starts_with("interface ")
                    || trimmed.starts_with("export type ")
                    || (trimmed.starts_with("type ") && !trimmed.contains("typeof")))
            {
                in_declaration = true;
                brace_count = 0;
                // Count braces on this line
                for c in line.chars() {
                    if c == '{' {
                        brace_count += 1;
                    } else if c == '}' {
                        brace_count -= 1;
                    }
                }
                // Check if declaration ended on the same line
                if brace_count <= 0 && line.contains('{') {
                    in_declaration = false;
                }
                final_result.push_str("// TS declaration removed\n");
                continue;
            }

            if in_declaration {
                // Count braces to find when we exit the declaration
                for c in line.chars() {
                    if c == '{' {
                        brace_count += 1;
                    } else if c == '}' {
                        brace_count -= 1;
                    }
                }
                if brace_count <= 0 {
                    in_declaration = false;
                }
                continue; // Skip this line entirely
            }

            final_result.push_str(line);
            final_result.push('\n');
        }

        final_result
    }

    /// Strip TypeScript type annotations from function parameters and return types.
    /// Converts: `function foo(x: number): string` -> `function foo(x)`
    fn strip_typescript_annotations(code: &str) -> String {
        let mut result = String::new();
        let mut chars = code.chars().peekable();

        while let Some(c) = chars.next() {
            if c == ':' {
                // Look ahead to see if this is a type annotation
                // Skip whitespace after colon
                let mut lookahead = String::new();
                while chars.peek() == Some(&' ') || chars.peek() == Some(&'\t') {
                    lookahead.push(chars.next().unwrap());
                }

                // Check if next char looks like a type (uppercase, or basic types)
                if let Some(&next) = chars.peek() {
                    // Check for common TypeScript type patterns
                    if next.is_ascii_uppercase()
                        || matches!(
                            chars.clone().take(6).collect::<String>().as_str(),
                            s if s.starts_with("number")
                                || s.starts_with("string")
                                || s.starts_with("boolea") // boolean
                                || s.starts_with("void")
                                || s.starts_with("null")
                                || s.starts_with("undefi") // undefined
                                || s.starts_with("any")
                                || s.starts_with("never")
                        )
                    {
                        // This looks like a type annotation, skip until we hit:
                        // - ) for parameter types
                        // - , for multiple parameters
                        // - { for function body
                        // - = for default values
                        // - ; for type aliases
                        let mut depth = 0;
                        while let Some(&nc) = chars.peek() {
                            if nc == '<' || nc == '(' || nc == '[' {
                                depth += 1;
                                chars.next();
                            } else if nc == '>' || nc == ')' || nc == ']' {
                                if depth > 0 {
                                    depth -= 1;
                                    chars.next();
                                } else {
                                    break;
                                }
                            } else if depth == 0
                                && (nc == ',' || nc == '{' || nc == '=' || nc == ';')
                            {
                                break;
                            } else {
                                chars.next();
                            }
                        }
                        continue;
                    }
                }

                // Not a type annotation, restore the colon and lookahead
                result.push(c);
                result.push_str(&lookahead);
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Strip ES module syntax from JavaScript for global evaluation.
    /// This is for pure JavaScript files (like sorex.js loader) - no TypeScript stripping.
    ///
    /// Handles:
    /// 1. `export { ... }` - removed entirely
    /// 2. `export default function name` -> `function name`
    /// 3. `export default` -> removed (keeps the value)
    /// 4. `export const/let/var` -> `const/let/var`
    /// 5. `import.meta.url` -> `undefined`
    fn strip_esm_exports(js: &str) -> String {
        let mut result = js.to_string();

        // Replace import.meta.url with undefined
        // Note: import.meta is a syntax error when parsed outside ES modules,
        // so we must replace it before the code is even parsed.
        result = result.replace("import.meta.url", "undefined");
        result = result.replace("import.meta", "undefined");

        // Strip `export default function` -> `function`
        result = result.replace("export default function ", "function ");

        // Strip `export default class` -> `class`
        result = result.replace("export default class ", "class ");

        // Strip `export default ` (for default exports of expressions)
        result = result.replace("export default ", "const __default_export__ = ");

        // Strip `export const/let/var` -> `const/let/var`
        result = result.replace("export const ", "const ");
        result = result.replace("export let ", "let ");
        result = result.replace("export var ", "var ");

        // Strip `export function` -> `function`
        result = result.replace("export function ", "function ");

        // Strip `export class` -> `class`
        result = result.replace("export class ", "class ");

        // Strip export block section (marked by "// ES Module exports" comment)
        if let Some(comment_start) = result.find("\n// ES Module exports") {
            result = result[..comment_start].to_string();
        }

        // Strip remaining `export { ... }` blocks (named exports at end of file)
        while let Some(export_start) = result.find("export {") {
            if let Some(export_end) = result[export_start..].find("};") {
                let before = &result[..export_start];
                let after = &result[export_start + export_end + 2..];
                result = format!("{}{}", before.trim_end(), after);
            } else {
                // Malformed export, just remove to end
                result = result[..export_start].to_string();
                break;
            }
        }

        result
    }

    /// Strip ES module syntax AND TypeScript from ranking function files.
    /// This is for TypeScript ranking files - includes full TS stripping.
    fn strip_typescript_for_ranking(ts: &str) -> String {
        let mut result = ts.to_string();

        // Replace import.meta.url with undefined
        result = result.replace("import.meta.url", "undefined");
        result = result.replace("import.meta", "undefined");

        // Strip `export default function` -> `function`
        result = result.replace("export default function ", "function ");

        // Strip `export default class` -> `class`
        result = result.replace("export default class ", "class ");

        // Strip `export default ` (for default exports of expressions)
        result = result.replace("export default ", "const __default_export__ = ");

        // TypeScript interfaces and types need special handling - comment them out entirely
        // They can span multiple lines with nested braces
        result = strip_typescript_declarations(&result);

        // Strip TypeScript type annotations from function signatures
        result = strip_typescript_annotations(&result);

        // Strip `export const/let/var` -> `const/let/var`
        result = result.replace("export const ", "const ");
        result = result.replace("export let ", "let ");
        result = result.replace("export var ", "var ");

        // Strip `export function` -> `function`
        result = result.replace("export function ", "function ");

        // Strip `export class` -> `class`
        result = result.replace("export class ", "class ");

        // Strip export block section (marked by "// ES Module exports" comment)
        if let Some(comment_start) = result.find("\n// ES Module exports") {
            result = result[..comment_start].to_string();
        }

        // Strip remaining `export { ... }` blocks (named exports at end of file)
        while let Some(export_start) = result.find("export {") {
            if let Some(export_end) = result[export_start..].find("};") {
                let before = &result[..export_start];
                let after = &result[export_start + export_end + 2..];
                result = format!("{}{}", before.trim_end(), after);
            } else {
                // Malformed export, just remove to end
                result = result[..export_start].to_string();
                break;
            }
        }

        result
    }

    /// Evaluator for user-defined ranking functions at index time.
    ///
    /// Loads a TypeScript/JavaScript ranking function and evaluates it for each
    /// (term, doc, match) tuple during index construction.
    ///
    /// # Example
    /// ```rust,ignore
    /// use sorex::deno_runtime::RankingEvaluator;
    ///
    /// let evaluator = RankingEvaluator::new("./ranking.ts")?;
    /// let score = evaluator.evaluate(&context)?;
    /// ```
    pub struct ScoringEvaluator {
        runtime: JsRuntime,
    }

    /// Default scoring function embedded in the binary.
    /// Mirrors the Lean-proven constants from src/scoring/core.rs.
    const DEFAULT_SCORING_CODE: &str = include_str!("../../tools/score.ts");

    impl ScoringEvaluator {
        /// Create a new scoring evaluator using the default scoring function.
        ///
        /// The default function mirrors the Lean-proven constants from
        /// src/scoring/core.rs, ensuring title > heading > content with
        /// position-based tie-breaking.
        pub fn from_default() -> Result<Self, String> {
            Self::from_code(DEFAULT_SCORING_CODE)
        }

        /// Create a new scoring evaluator from a TypeScript/JavaScript file.
        ///
        /// The file must export a default function with signature:
        /// `(ctx: ScoringContext) => number`
        pub fn from_file(path: &std::path::Path) -> Result<Self, String> {
            let code = std::fs::read_to_string(path)
                .map_err(|e| format!("Failed to read scoring function: {}", e))?;
            Self::from_code(&code)
        }

        /// Create a new scoring evaluator from TypeScript/JavaScript source code.
        pub fn from_code(code: &str) -> Result<Self, String> {
            init_deno();

            let mut runtime = JsRuntime::new(RuntimeOptions::default());

            // Add polyfills (needed for TextEncoder/TextDecoder if used in ranking function)
            runtime
                .execute_script("<polyfills>", WEB_API_POLYFILLS)
                .map_err(|e| format!("Failed to run polyfills: {}", e))?;

            // Strip ESM syntax AND TypeScript for ranking files
            let stripped = strip_typescript_for_ranking(code);

            // The ranking function must be the default export.
            // We extract it by looking for common patterns.
            let setup_code = format!(
                r#"
                // User's ranking code
                {code}

                // Extract the default export
                if (typeof score === 'function') {{
                    globalThis.__scoring_fn__ = score;
                }} else if (typeof defaultScore === 'function') {{
                    globalThis.__scoring_fn__ = defaultScore;
                }} else {{
                    // Try to find any exported function
                    throw new Error("Scoring file must export a 'score' or 'default' function");
                }}
                "#,
                code = stripped
            );

            runtime
                .execute_script("<ranking-setup>", setup_code)
                .map_err(|e| format!("Failed to load scoring function: {}", e))?;

            Ok(Self { runtime })
        }

        /// Evaluate the scoring function for a given context.
        ///
        /// Returns the integer score (higher = better ranking).
        pub fn evaluate(&mut self, ctx: &super::ScoringContext) -> Result<u32, String> {
            let ctx_json = serde_json::to_string(ctx)
                .map_err(|e| format!("Failed to serialize context: {}", e))?;

            let eval_code = format!(
                r#"
                (function() {{
                    const ctx = {ctx_json};
                    const score = __scoring_fn__(ctx);
                    // Ensure we return a valid integer score
                    if (typeof score !== 'number' || !Number.isFinite(score)) {{
                        throw new Error("Scoring function must return a number, got: " + typeof score);
                    }}
                    return Math.floor(Math.max(0, score));
                }})();
                "#,
                ctx_json = ctx_json
            );

            let result = self
                .runtime
                .execute_script("<ranking-eval>", eval_code)
                .map_err(|e| format!("Ranking evaluation failed: {}", e))?;

            // Convert v8::Global to number
            let score = {
                let context = self.runtime.main_context();
                let isolate = self.runtime.v8_isolate();
                v8::scope!(scope, isolate);
                let context_local = v8::Local::new(scope, context);
                let scope = &mut v8::ContextScope::new(scope, context_local);
                let local = v8::Local::new(scope, result);
                local.to_rust_string_lossy(scope)
            };

            score
                .parse::<u32>()
                .map_err(|e| format!("Invalid score value '{}': {}", score, e))
        }

        /// Evaluate the scoring function for multiple contexts in batch.
        ///
        /// More efficient than calling `evaluate` repeatedly because it avoids
        /// per-call overhead of serialization and JS context switching.
        pub fn evaluate_batch(
            &mut self,
            contexts: &[super::ScoringContext],
        ) -> Result<Vec<u32>, String> {
            if contexts.is_empty() {
                return Ok(Vec::new());
            }

            let contexts_json = serde_json::to_string(contexts)
                .map_err(|e| format!("Failed to serialize contexts: {}", e))?;

            let eval_code = format!(
                r#"
                (function() {{
                    const contexts = {contexts_json};
                    const scores = contexts.map(ctx => {{
                        const score = __scoring_fn__(ctx);
                        if (typeof score !== 'number' || !Number.isFinite(score)) {{
                            throw new Error("Scoring function must return a number");
                        }}
                        return Math.floor(Math.max(0, score));
                    }});
                    return JSON.stringify(scores);
                }})();
                "#,
                contexts_json = contexts_json
            );

            let result = self
                .runtime
                .execute_script("<ranking-batch>", eval_code)
                .map_err(|e| format!("Batch ranking evaluation failed: {}", e))?;

            // Convert v8::Global to JSON string
            let json_str = {
                let context = self.runtime.main_context();
                let isolate = self.runtime.v8_isolate();
                v8::scope!(scope, isolate);
                let context_local = v8::Local::new(scope, context);
                let scope = &mut v8::ContextScope::new(scope, context_local);
                let local = v8::Local::new(scope, result);
                local.to_rust_string_lossy(scope)
            };

            serde_json::from_str::<Vec<u32>>(&json_str)
                .map_err(|e| format!("Failed to parse batch scores: {} (json: {})", e, json_str))
        }

        /// Evaluate the scoring function with configurable chunk size.
        ///
        /// This allows experimenting with different batch sizes to find the optimal
        /// balance between JS call overhead and V8 JIT optimization.
        ///
        /// - `chunk_size = None` → All contexts in one batch (current behavior)
        /// - `chunk_size = Some(n)` → Process in chunks of n contexts
        pub fn evaluate_batch_chunked(
            &mut self,
            contexts: &[super::ScoringContext],
            chunk_size: Option<usize>,
        ) -> Result<Vec<u32>, String> {
            match chunk_size {
                None | Some(0) => self.evaluate_batch(contexts),
                Some(size) => {
                    let mut all_scores = Vec::with_capacity(contexts.len());
                    for chunk in contexts.chunks(size) {
                        let scores = self.evaluate_batch(chunk)?;
                        all_scores.extend(scores);
                    }
                    Ok(all_scores)
                }
            }
        }
    }
}

#[cfg(feature = "deno-runtime")]
pub use deno_impl::{DenoRuntime, DenoSearchContext, ScoringEvaluator, TURBOFAN_WARMUP_ITERATIONS};

/// Legacy function for backwards compatibility.
#[cfg(feature = "deno-runtime")]
pub fn search_with_deno(
    sorex_bytes: &[u8],
    loader_js: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<WasmSearchResult>, String> {
    let runtime = deno_impl::DenoRuntime::new()?;
    runtime.search(sorex_bytes, loader_js, query, limit)
}

/// Placeholder for non-deno builds.
#[cfg(not(feature = "deno-runtime"))]
pub fn search_with_deno(
    _sorex_bytes: &[u8],
    _loader_js: &str,
    _query: &str,
    _limit: usize,
) -> Result<Vec<WasmSearchResult>, String> {
    Err("Deno runtime not enabled. Build with --features deno-runtime".to_string())
}

/// Placeholder DenoRuntime for non-deno builds.
#[cfg(not(feature = "deno-runtime"))]
pub struct DenoRuntime;

#[cfg(not(feature = "deno-runtime"))]
impl DenoRuntime {
    pub fn new() -> Result<Self, String> {
        Err("Deno runtime not enabled. Build with --features deno-runtime".to_string())
    }

    pub fn search(
        &self,
        _sorex_bytes: &[u8],
        _loader_js: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<WasmSearchResult>, String> {
        Err("Deno runtime not enabled".to_string())
    }
}

/// Placeholder DenoSearchContext for non-deno builds.
#[cfg(not(feature = "deno-runtime"))]
pub struct DenoSearchContext;

#[cfg(not(feature = "deno-runtime"))]
impl DenoSearchContext {
    pub fn new(_sorex_bytes: &[u8], _loader_js: &str) -> Result<Self, String> {
        Err("Deno runtime not enabled. Build with --features deno-runtime".to_string())
    }

    pub fn search(&mut self, _query: &str, _limit: usize) -> Result<Vec<WasmSearchResult>, String> {
        Err("Deno runtime not enabled".to_string())
    }
}

/// Placeholder ScoringEvaluator for non-deno builds.
/// All methods return errors since Deno is required for scoring.
#[cfg(not(feature = "deno-runtime"))]
pub struct ScoringEvaluator;

#[cfg(not(feature = "deno-runtime"))]
impl ScoringEvaluator {
    pub fn from_default() -> Result<Self, String> {
        Err("Deno runtime not enabled. Build with --features deno-runtime".to_string())
    }

    pub fn from_file(_path: &std::path::Path) -> Result<Self, String> {
        Err("Deno runtime not enabled. Build with --features deno-runtime".to_string())
    }

    pub fn from_code(_code: &str) -> Result<Self, String> {
        Err("Deno runtime not enabled. Build with --features deno-runtime".to_string())
    }

    pub fn evaluate(&mut self, _ctx: &ScoringContext) -> Result<u32, String> {
        Err("Deno runtime not enabled".to_string())
    }

    pub fn evaluate_batch(&mut self, _contexts: &[ScoringContext]) -> Result<Vec<u32>, String> {
        Err("Deno runtime not enabled".to_string())
    }

    pub fn evaluate_batch_chunked(
        &mut self,
        _contexts: &[ScoringContext],
        _chunk_size: Option<usize>,
    ) -> Result<Vec<u32>, String> {
        Err("Deno runtime not enabled".to_string())
    }
}
