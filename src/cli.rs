use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sieve",
    about = "Formally verified full-text search index builder",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Build search index from directory of JSON files
    Build {
        /// Input directory containing manifest.json and document files
        #[arg(short, long)]
        input: String,

        /// Output directory for .sieve files and optionally WASM
        #[arg(short, long)]
        output: String,

        /// Specific indexes to build (comma-separated). Default: all in manifest
        #[arg(long, value_delimiter = ',')]
        indexes: Option<Vec<String>>,

        /// Emit WASM/JS/TypeScript files alongside indexes
        #[arg(long)]
        emit_wasm: bool,
    },

    /// Inspect a .sieve file structure
    Inspect {
        /// Path to .sieve file
        file: String,
    },
}
