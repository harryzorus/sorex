// Copyright 2025-present Harīṣh Tummalachērla
// SPDX-License-Identifier: Apache-2.0

//! CLI definitions for the sorex command-line interface.
//!
//! Three subcommands: `index` to build indexes, `inspect` to examine `.sorex`
//! files, and `search` to query them. The search command can optionally run
//! through the WASM module via Deno for parity testing, and includes a
//! benchmarking mode that runs until statistical confidence is achieved.

pub mod display;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sorex",
    about = "Formally verified full-text search index builder",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build search index from directory of JSON files
    Index {
        /// Input directory containing manifest.json and document files
        #[arg(short, long)]
        input: String,

        /// Output directory for .sorex files
        #[arg(short, long)]
        output: String,

        /// Generate demo HTML page showing integration example
        #[arg(long)]
        demo: bool,

        /// Path to custom ranking function (TypeScript/JavaScript)
        ///
        /// The scoring function receives a ScoringContext for each (term, doc, match)
        /// tuple and returns an integer score (higher = better ranking).
        ///
        /// If not specified, uses the built-in default scoring based on field type
        /// (title=1000, heading=100, content=10) with position bonus.
        #[arg(long)]
        ranking: Option<String>,

        /// Batch size for ranking function evaluation (experimental)
        ///
        /// Controls how many contexts are evaluated in each JavaScript call.
        /// Default (0) evaluates all contexts for each term in one batch.
        /// Smaller values may improve V8 JIT optimization but increase call overhead.
        #[arg(long, default_value = "0")]
        ranking_batch_size: usize,
    },

    /// Inspect a .sorex file structure
    Inspect {
        /// Path to .sorex file
        file: String,
    },

    /// Search a .sorex file and display results
    Search {
        /// Path to .sorex file
        file: String,

        /// Search query
        query: String,

        /// Maximum number of results to return
        #[arg(short, long, default_value = "10")]
        limit: usize,

        /// Use embedded WASM via Deno runtime instead of native Rust
        #[arg(long)]
        wasm: bool,

        /// Run benchmark until target confidence interval is achieved
        #[arg(long)]
        bench: bool,

        /// Target confidence level for benchmark (default: 95%)
        #[arg(long, default_value = "95")]
        confidence: u8,
    },
}
