//! ContextCompress CLI

use anyhow::Result;
use clap::{Parser, Subcommand};
use context_compress_core::{AbstractiveCompressor, CompressionStrategy, HybridCompressor};
use std::io::{self, Read, Write};
use std::path::PathBuf;
use tracing::info;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

#[derive(Parser)]
#[command(name = "cc")]
#[command(author = "Jayvee Tolibas")]
#[command(version = "0.1.0")]
#[command(about = "Compress LLM prompts and context to reduce token usage")]
struct Cli {
    #[arg(short = 's', long, value_enum, default_value = "hybrid")]
    strategy: Strategy,
    #[arg(short = 'r', long, default_value = "0.5")]
    ratio: f64,
    #[arg(short = 'i', long)]
    input: Option<PathBuf>,
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,
    #[arg(short = 'v', long)]
    verbose: bool,
    #[arg(long)]
    audit: bool,
    #[arg(short = 'l', long, default_value = "info")]
    log_level: String,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Compress { text: Option<String> },
    Count { text: Option<String>, #[arg(short = 'm', long, default_value = "gpt-4")] model: String },
    Cache,
    Init,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Strategy {
    Extractive,
    Abstractive,
    Hybrid,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_level));
    tracing_subscriber::registry().with(fmt::layer()).with(filter).init();
    info!("ContextCompress v{}", env!("CARGO_PKG_VERSION"));

    if let Some(command) = cli.command {
        return handle_command(command, &cli).await;
    }
    compress(&cli).await
}

async fn handle_command(command: Commands, cli: &Cli) -> Result<()> {
    match command {
        Commands::Compress { text } => {
            // Clone all needed fields FIRST before any moves
            let strategy = cli.strategy.clone();
            let ratio = cli.ratio;
            let output = cli.output.clone();
            let verbose = cli.verbose;
            let audit = cli.audit;
            let log_level = cli.log_level.clone();
            let input_arg = cli.input.clone();
            
            // Now read input (this won't cause partial move)
            let input = if let Some(t) = text {
                t
            } else {
                read_input(&input_arg)?
            };
            
            // Write input to temp file
            use tempfile::NamedTempFile;
            use std::io::Write;
            let mut temp_file = NamedTempFile::new()?;
            temp_file.write_all(input.as_bytes())?;
            
            // Create new Cli for compress
            let cli_for_compress = Cli {
                strategy,
                ratio,
                input: Some(temp_file.path().to_path_buf()),
                output,
                verbose,
                audit,
                log_level,
                command: None,
            };
            
            compress(&cli_for_compress).await
        }
        Commands::Count { text, model } => {
            use context_compress_core::TokenCounter;
            let input = if let Some(t) = text { t } else { read_input(&cli.input)? };
            let counter = TokenCounter::new(&model);
            let count = counter.count(&input)?;
            println!("Token count ({}): {}", model, count);
            Ok(())
        }
        Commands::Cache => {
            use context_compress_core::SemanticCache;
            let config = context_compress_core::cache::CacheConfig::default();
            let cache = SemanticCache::new(config)?;
            let stats = cache.stats()?;
            println!("Cache Statistics:");
            println!("  Entries: {}", stats.entry_count);
            println!("  Size: {:.2} MB", stats.total_size_bytes as f64 / 1024.0 / 1024.0);
            Ok(())
        }
        Commands::Init => {
            use context_compress_core::Config;
            let config = Config::default();
            let path = Config::default_path();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            config.save(&path)?;
            println!("Config file created at: {}", path.display());
            Ok(())
        }
    }
}

async fn compress(cli: &Cli) -> Result<()> {
    let input = read_input(&cli.input)?;
    if input.is_empty() {
        anyhow::bail!("No input provided");
    }

    let compressor = match cli.strategy {
        Strategy::Extractive => {
            HybridCompressor::new(context_compress_core::hybrid::HybridConfig {
                strategy: CompressionStrategy::Extractive,
                ..Default::default()
            })
        }
        Strategy::Abstractive => {
            HybridCompressor::new(context_compress_core::hybrid::HybridConfig {
                strategy: CompressionStrategy::Abstractive,
                ..Default::default()
            }).with_abstractive(AbstractiveCompressor::default())
        }
        Strategy::Hybrid => {
            HybridCompressor::default().with_abstractive(AbstractiveCompressor::default())
        }
    };

    let result = compressor.compress(&input).await?;

    if let Some(output_path) = &cli.output {
        std::fs::write(output_path, &result.text)?;
        eprintln!("Compressed output written to: {}", output_path.display());
    } else {
        io::stdout().write_all(result.text.as_bytes())?;
        println!();
    }

    if cli.verbose {
        eprintln!("\nCompression Statistics:");
        eprintln!("  Original tokens: {}", result.original_tokens);
        eprintln!("  Compressed tokens: {}", result.compressed_tokens);
        eprintln!("  Reduction: {} tokens ({:.1}%)", result.token_reduction(), result.reduction_percentage());
        eprintln!("  Compression ratio: {:.2}", result.compression_ratio);
    }

    Ok(())
}

fn read_input(path: &Option<PathBuf>) -> Result<String> {
    if let Some(path) = path {
        Ok(std::fs::read_to_string(path)?)
    } else {
        let mut input = String::new();
        io::stdin().read_to_string(&mut input)?;
        Ok(input)
    }
}
