# Changelog

## [1.0.0] - 2026-01-14

A ground-up rewrite. v0.2.5 worked, but I wanted mathematical certainty that it was correct. v1.0 adds Lean 4 proofs, restructures everything into proper modules, and bumps the binary format to v12 for streaming optimization.

### What's New

**Lean 4 Proofs.** Eleven proof files covering binary search bounds, scoring invariants, tier exclusion, and section non-overlap. If `lake build` succeeds, the math is right. This isn't academic exercise; it's insurance against the bugs that only show up at 3am.

**Binary Format v12.** Reordered for streaming: WASM loads first so browsers can start `compileStreaming()` while the rest downloads. Dependency-ordered sections mean you can decode incrementally without random seeks.

**Modular Architecture.** Forty-five Rust files across eleven modules instead of a flat `src/` directory. Each module owns its invariants: `binary/` handles encoding, `search/` handles queries, `scoring/` handles ranking. Easier to test, easier to reason about.

**Tiered Search Rewrite.** Clean separation between T1 (exact), T2 (prefix), and T3 (fuzzy). Each tier explicitly excludes documents found in earlier tiers. No duplicates, no confusion about where a result came from.

**Heading Level Ranking.** h1 beats h2 beats h3 beats body text. Search results finally understand document hierarchy instead of treating everything as a flat bag of words.

**Property Tests.** Two hundred proptest cases across 45 test files encoding the Lean specs. Random queries, random limits, random documents. If there's an invariant violation hiding in the corner cases, proptest will find it.

**Fuzz Testing.** Eleven fuzz targets covering binary parsing, varint codec, Levenshtein matching, tier merging, and section boundaries. Run with `cargo +nightly fuzz run <target>`.

**Kani Model Checking.** Mathematical proofs that the varint parser cannot panic on any input. Five Kani proofs verify encode/decode roundtrips, empty input handling, and overlong varint rejection. Run with `cargo xtask kani`.

**Mutation Testing.** cargo-mutants systematically corrupts code to verify tests catch bugs. CI enforces 60% detection rate. Identified and closed five gaps in varint and suffix array encoding.

**Oracle-Based Differential Testing.** Simple reference implementations (O(n²) suffix array, linear scan binary search, Wagner-Fischer Levenshtein) compared against optimized code. If they disagree, the simple one is right.

**11-Step Verification.** `cargo xtask verify` runs Lean proofs, constant alignment, spec drift detection, invariant markers, clippy, release build, test fixtures, Rust tests, WASM parity, browser E2E, and mutation testing. One command, complete confidence.

**GitHub Actions CI.** Automated verification on every push: Lean proofs, Rust tests, WASM build, Kani model checking, and mutation testing.

**SIMD Optimizations.** portable_simd for the hot paths in WASM. The V8 CLI warm-up is noticeably faster.

### What Changed

- Binary format v7 → v12 (not backwards compatible; rebuild your indexes)
- Flat files → 11 semantic modules (`binary/`, `cli/`, `fuzzy/`, `index/`, `runtime/`, `scoring/`, `search/`, `util/`, `verify/`)
- Ad-hoc tests → 45 test files with property coverage
- Manual verification → automated 11-step pipeline
- Data files reorganized into `data/` directory

---

## [0.2.5] - 2026-01-07

First public release. The foundation that v1.0 rebuilt from scratch.

**Binary Format v7.** Embedded WASM runtime, Block PFOR postings compression, CRC32 integrity validation. Self-contained `.sorex` files that work in any browser.

**Three-Tier Search.** Exact match, prefix expansion, fuzzy matching via Levenshtein DFA. The core search strategy that v1.0 preserved and refined.

**TypeScript Loader.** `sorex.js` handles WASM initialization, thread pool setup, and the search API. Drop it in your page, point it at a `.sorex` file, done.

**Parallel Build.** Rayon-powered document loading. Index thousands of documents without waiting forever.

**CLI.** `sorex index` builds indexes with progress bars. `sorex inspect` shows what's inside a `.sorex` file.

[1.0.0]: https://github.com/harryzorus/sorex/compare/v0.2.5...v1.0.0
[0.2.5]: https://github.com/harryzorus/sorex/releases/tag/v0.2.5
