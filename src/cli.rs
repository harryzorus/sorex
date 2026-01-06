use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "sieve",
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

        /// Output directory for .sieve files
        #[arg(short, long)]
        output: String,

        /// Generate demo HTML page showing integration example
        #[arg(long)]
        demo: bool,
    },

    /// Inspect a .sieve file structure
    Inspect {
        /// Path to .sieve file
        file: String,
    },
}
