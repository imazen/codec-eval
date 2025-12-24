//! Corpus management commands.

use std::path::PathBuf;

use anyhow::{Context, Result};
use codec_eval::corpus::{Corpus, ImageCategory};

use crate::CorpusAction;

pub fn run(action: CorpusAction, verbose: bool) -> Result<()> {
    match action {
        CorpusAction::Discover { path, output, checksums } => {
            discover(&path, output.as_deref(), checksums, verbose)
        }
        CorpusAction::Info { path } => info(&path, verbose),
        CorpusAction::List { path, category, format, min_width, min_height } => {
            list(&path, category.as_deref(), format.as_deref(), min_width, min_height, verbose)
        }
    }
}

fn discover(path: &PathBuf, output: Option<&std::path::Path>, checksums: bool, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("Discovering images in: {}", path.display());
    }

    let mut corpus = Corpus::discover(path)
        .with_context(|| format!("Failed to discover images in {}", path.display()))?;

    if checksums {
        if verbose {
            eprintln!("Computing checksums...");
        }
        let count = corpus.compute_checksums()
            .context("Failed to compute checksums")?;
        if verbose {
            eprintln!("Computed {} checksums", count);
        }
    }

    let stats = corpus.stats();
    println!("Discovered {} images", stats.image_count);
    println!("  Total size: {} bytes", stats.total_bytes);
    println!("  Dimensions: {}x{} to {}x{}",
        stats.min_width, stats.min_height,
        stats.max_width, stats.max_height);

    if let Some(output_path) = output {
        corpus.save(output_path)
            .with_context(|| format!("Failed to save corpus to {}", output_path.display()))?;
        println!("Saved manifest to: {}", output_path.display());
    } else {
        // Print JSON to stdout
        let json = serde_json::to_string_pretty(&corpus)?;
        println!("{json}");
    }

    Ok(())
}

fn info(path: &PathBuf, _verbose: bool) -> Result<()> {
    let corpus = if path.is_dir() {
        Corpus::discover(path)
            .with_context(|| format!("Failed to discover images in {}", path.display()))?
    } else {
        Corpus::load(path)
            .with_context(|| format!("Failed to load corpus from {}", path.display()))?
    };

    let stats = corpus.stats();

    println!("Corpus: {}", corpus.name);
    println!("  Path: {}", corpus.root_path.display());
    println!("  Images: {}", stats.image_count);
    println!("  Total pixels: {}", stats.total_pixels);
    println!("  Total size: {} bytes ({:.2} MB)",
        stats.total_bytes,
        stats.total_bytes as f64 / 1_000_000.0);
    println!("  Dimensions: {}x{} to {}x{}",
        stats.min_width, stats.min_height,
        stats.max_width, stats.max_height);

    if !corpus.metadata.category_counts.is_empty() {
        println!("  Categories:");
        for (cat, count) in &corpus.metadata.category_counts {
            println!("    {}: {}", cat, count);
        }
    }

    // Count formats
    let mut formats: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for img in &corpus.images {
        *formats.entry(&img.format).or_default() += 1;
    }
    if !formats.is_empty() {
        println!("  Formats:");
        let mut sorted: Vec<_> = formats.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        for (format, count) in sorted {
            println!("    {}: {}", format, count);
        }
    }

    Ok(())
}

fn list(
    path: &PathBuf,
    category: Option<&str>,
    format: Option<&str>,
    min_width: Option<u32>,
    min_height: Option<u32>,
    _verbose: bool,
) -> Result<()> {
    let corpus = if path.is_dir() {
        Corpus::discover(path)
            .with_context(|| format!("Failed to discover images in {}", path.display()))?
    } else {
        Corpus::load(path)
            .with_context(|| format!("Failed to load corpus from {}", path.display()))?
    };

    let category_filter = category.and_then(|s| s.parse::<ImageCategory>().ok());

    for img in &corpus.images {
        // Apply filters
        if let Some(cat) = category_filter {
            if img.category != Some(cat) {
                continue;
            }
        }

        if let Some(fmt) = format {
            if !img.format.eq_ignore_ascii_case(fmt) {
                continue;
            }
        }

        if let Some(min_w) = min_width {
            if img.width < min_w {
                continue;
            }
        }

        if let Some(min_h) = min_height {
            if img.height < min_h {
                continue;
            }
        }

        println!("{}\t{}x{}\t{}\t{}",
            img.relative_path.display(),
            img.width,
            img.height,
            img.format,
            img.category.map_or("".to_string(), |c| c.to_string())
        );
    }

    Ok(())
}
