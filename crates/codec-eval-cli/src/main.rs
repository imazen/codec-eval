//! codec-eval CLI - Image codec comparison tool

use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod commands;

/// Image codec comparison and evaluation tool.
#[derive(Parser)]
#[command(name = "codec-eval")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Discover and manage image corpora
    Corpus {
        #[command(subcommand)]
        action: CorpusAction,
    },

    /// Sparse checkout for partial corpus downloads
    Sparse {
        #[command(subcommand)]
        action: SparseAction,
    },

    /// Import external benchmark results from CSV
    Import {
        /// Input CSV file
        #[arg(short, long)]
        input: PathBuf,

        /// Output JSON file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Image column name
        #[arg(long)]
        image_col: Option<String>,

        /// Codec column name
        #[arg(long)]
        codec_col: Option<String>,
    },

    /// Calculate Pareto front from benchmark results
    Pareto {
        /// Input JSON or CSV file with benchmark results
        #[arg(short, long)]
        input: PathBuf,

        /// Output file (JSON)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Quality metric to use (ssimulacra2, dssim, psnr)
        #[arg(long, default_value = "dssim")]
        metric: String,
    },

    /// Show statistics for benchmark results
    Stats {
        /// Input JSON or CSV file
        #[arg(short, long)]
        input: PathBuf,

        /// Group by codec
        #[arg(long)]
        by_codec: bool,

        /// Group by image
        #[arg(long)]
        by_image: bool,
    },
}

#[derive(Subcommand)]
enum CorpusAction {
    /// Discover images in a directory
    Discover {
        /// Directory to scan
        path: PathBuf,

        /// Output manifest file
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Compute checksums for deduplication
        #[arg(long)]
        checksums: bool,
    },

    /// Show corpus information
    Info {
        /// Corpus manifest file or directory
        path: PathBuf,
    },

    /// List images in a corpus
    List {
        /// Corpus manifest file or directory
        path: PathBuf,

        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Filter by format
        #[arg(long)]
        format: Option<String>,

        /// Minimum width
        #[arg(long)]
        min_width: Option<u32>,

        /// Minimum height
        #[arg(long)]
        min_height: Option<u32>,
    },
}

#[derive(Subcommand)]
pub enum SparseAction {
    /// Clone a repository with sparse checkout
    Clone {
        /// Repository URL
        url: String,

        /// Target directory
        target: PathBuf,

        /// Shallow clone depth (faster, less history)
        #[arg(long)]
        depth: Option<u32>,

        /// Initial paths to include
        #[arg(long)]
        include: Vec<String>,

        /// Include only specific formats (e.g., png, jpg)
        #[arg(long)]
        format: Vec<String>,

        /// Include only specific categories
        #[arg(long)]
        category: Vec<String>,
    },

    /// Initialize sparse checkout in existing repo
    Init {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Add paths/patterns to sparse checkout
    Add {
        /// Repository path
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Paths or patterns to add
        patterns: Vec<String>,

        /// Add by format extension
        #[arg(long)]
        format: Vec<String>,

        /// Add by category directory
        #[arg(long)]
        category: Vec<String>,
    },

    /// Set sparse checkout paths (replaces existing)
    Set {
        /// Repository path
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Paths or patterns to set
        patterns: Vec<String>,

        /// Set by format extension
        #[arg(long)]
        format: Vec<String>,

        /// Set by category directory
        #[arg(long)]
        category: Vec<String>,
    },

    /// Show sparse checkout status
    Status {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// List current sparse checkout patterns
    List {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Preview what files would match patterns
    Preview {
        /// Repository path
        #[arg(short, long, default_value = ".")]
        path: PathBuf,

        /// Patterns to preview
        patterns: Vec<String>,

        /// Preview by format
        #[arg(long)]
        format: Vec<String>,

        /// Preview by category
        #[arg(long)]
        category: Vec<String>,
    },

    /// Disable sparse checkout (get all files)
    Disable {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },

    /// Fetch and checkout sparse files
    Fetch {
        /// Repository path
        #[arg(default_value = ".")]
        path: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Corpus { action } => commands::corpus::run(action, cli.verbose),
        Commands::Sparse { action } => commands::sparse::run(action, cli.verbose),
        Commands::Import { input, output, image_col, codec_col } => {
            commands::import::run(input, output, image_col, codec_col, cli.verbose)
        }
        Commands::Pareto { input, output, metric } => {
            commands::pareto::run(input, output, &metric, cli.verbose)
        }
        Commands::Stats { input, by_codec, by_image } => {
            commands::stats::run(input, by_codec, by_image, cli.verbose)
        }
    }
}
