//! Find images where encoders behave very differently
//!
//! This tool scans a corpus of images and finds "outliers" where:
//! - mozjpeg significantly beats jpegli (or vice versa)
//! - Different quality levels cause unusual behavior
//! - Specific flags have outsized impact
//!
//! These outlier images help us understand what image characteristics
//! matter for encoder selection and parameter tuning.

use anyhow::{Context, Result};
use butteraugli::{ButteraugliParams, compute_butteraugli};
use clap::Parser;
use rayon::prelude::*;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "find-outliers")]
#[command(about = "Find images where encoders behave very differently")]
struct Args {
    /// Directory containing images to analyze
    #[arg(default_value = ".")]
    corpus_dir: PathBuf,

    /// Quality levels to test (comma-separated)
    #[arg(short, long, default_value = "50,70,85,95")]
    qualities: String,

    /// Minimum image dimension (skip smaller images)
    #[arg(long, default_value = "64")]
    min_size: usize,

    /// Number of top outliers to show
    #[arg(short = 'n', long, default_value = "10")]
    top_n: usize,

    /// Show detailed results for top N outliers
    #[arg(long, default_value = "5")]
    detailed: usize,

    /// Output format: text, json, csv
    #[arg(short, long, default_value = "text")]
    output: String,
}

/// Results for a single image across multiple encoders
#[derive(Debug, Clone)]
struct ImageResults {
    path: PathBuf,
    width: usize,
    height: usize,
    pixels: usize,

    // Results at each quality level: (q, bpp, butteraugli)
    mozjpeg: Vec<(u8, f64, f64)>,
    jpegli: Vec<(u8, f64, f64)>,

    // Computed metrics
    mozjpeg_advantage: f64, // positive = mozjpeg better
    jpegli_advantage: f64,  // positive = jpegli better
}

/// Encode image with mozjpeg at given quality, return (bpp, butteraugli)
#[cfg(feature = "mozjpeg")]
fn encode_mozjpeg(rgb: &[u8], width: usize, height: usize, quality: u8) -> Option<(f64, f64)> {
    use mozjpeg::{ColorSpace, Compress};

    let mut comp = Compress::new(ColorSpace::JCS_RGB);
    comp.set_size(width, height);
    comp.set_quality(quality as f32);
    comp.set_optimize_scans(true);

    let mut comp = comp.start_compress(Vec::new()).ok()?;
    comp.write_scanlines(rgb).ok()?;
    let result = comp.finish().ok()?;

    // Decode and compute butteraugli
    let img = image::load_from_memory_with_format(&result, image::ImageFormat::Jpeg).ok()?;
    let decoded_rgb = img.to_rgb8();
    let decoded = decoded_rgb.as_raw();

    let params = ButteraugliParams::default();
    let butter = compute_butteraugli(rgb, decoded, width, height, &params)
        .map(|r| r.score)
        .unwrap_or(f64::NAN);

    let bpp = (result.len() as f64 * 8.0) / (width * height) as f64;
    Some((bpp, butter))
}

#[cfg(not(feature = "mozjpeg"))]
fn encode_mozjpeg(_rgb: &[u8], _width: usize, _height: usize, _quality: u8) -> Option<(f64, f64)> {
    None
}

/// Encode image with jpegli at given quality, return (bpp, butteraugli)
#[cfg(feature = "jpegli")]
fn encode_jpegli(rgb: &[u8], width: usize, height: usize, quality: u8) -> Option<(f64, f64)> {
    use jpegli::encode::Encoder;
    use jpegli::quant::Quality;

    std::panic::catch_unwind(|| -> Option<(f64, f64)> {
        let encoder = Encoder::new()
            .width(width as u32)
            .height(height as u32)
            .quality(Quality::from_quality(quality as f32));

        let result = encoder.encode(rgb).ok()?;

        // Decode and compute butteraugli
        let img = image::load_from_memory_with_format(&result, image::ImageFormat::Jpeg).ok()?;
        let decoded_rgb = img.to_rgb8();
        let decoded = decoded_rgb.as_raw();

        let params = ButteraugliParams::default();
        let butter = compute_butteraugli(rgb, decoded, width, height, &params)
            .map(|r| r.score)
            .unwrap_or(f64::NAN);

        let bpp = (result.len() as f64 * 8.0) / (width * height) as f64;
        Some((bpp, butter))
    })
    .ok()
    .flatten()
}

#[cfg(not(feature = "jpegli"))]
fn encode_jpegli(_rgb: &[u8], _width: usize, _height: usize, _quality: u8) -> Option<(f64, f64)> {
    None
}

/// Load image and return (rgb_data, width, height)
fn load_image(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgb8();
    let width = rgb.width() as usize;
    let height = rgb.height() as usize;
    let data = rgb.into_raw();
    Some((data, width, height))
}

/// Compute advantage metric: how much better is encoder A vs B at similar quality?
/// Positive = A is better (smaller file at same quality)
fn compute_advantage(a_results: &[(u8, f64, f64)], b_results: &[(u8, f64, f64)]) -> f64 {
    // For each quality level, compute efficiency difference
    let mut total_advantage = 0.0;
    let mut count = 0;

    for &(q, a_bpp, a_butter) in a_results {
        // Find B result at same quality
        if let Some(&(_, b_bpp, b_butter)) = b_results.iter().find(|&&(bq, _, _)| bq == q) {
            if a_butter.is_finite() && b_butter.is_finite() && a_bpp > 0.0 && b_bpp > 0.0 {
                // Compute efficiency advantage (lower butteraugli/bpp is better)
                let a_eff = a_butter / a_bpp;
                let b_eff = b_butter / b_bpp;
                // Positive if A is more efficient (lower butteraugli per bit)
                total_advantage += (b_eff - a_eff) / b_eff.max(0.001);
                count += 1;
            }
        }
    }

    if count > 0 {
        total_advantage / count as f64
    } else {
        0.0
    }
}

fn analyze_image(path: &Path, qualities: &[u8], min_size: usize) -> Option<ImageResults> {
    let (rgb, width, height) = load_image(path)?;

    // Skip very small images
    if width < min_size || height < min_size {
        return None;
    }

    let mut mozjpeg_results = Vec::new();
    let mut jpegli_results = Vec::new();

    for &q in qualities {
        if let Some((bpp, butter)) = encode_mozjpeg(&rgb, width, height, q) {
            mozjpeg_results.push((q, bpp, butter));
        }
        if let Some((bpp, butter)) = encode_jpegli(&rgb, width, height, q) {
            jpegli_results.push((q, bpp, butter));
        }
    }

    if mozjpeg_results.is_empty() || jpegli_results.is_empty() {
        return None;
    }

    let mozjpeg_advantage = compute_advantage(&mozjpeg_results, &jpegli_results);
    let jpegli_advantage = compute_advantage(&jpegli_results, &mozjpeg_results);

    Some(ImageResults {
        path: path.to_path_buf(),
        width,
        height,
        pixels: width * height,
        mozjpeg: mozjpeg_results,
        jpegli: jpegli_results,
        mozjpeg_advantage,
        jpegli_advantage,
    })
}

fn find_images(dir: &Path) -> Vec<PathBuf> {
    let mut images = Vec::new();

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_lower = ext.to_string_lossy().to_lowercase();
                    if ext_lower == "png" || ext_lower == "jpg" || ext_lower == "jpeg" {
                        images.push(path);
                    }
                }
            } else if path.is_dir() {
                images.extend(find_images(&path));
            }
        }
    }

    images
}

fn print_text_report(results: &mut Vec<ImageResults>, args: &Args) {
    // Sort by mozjpeg advantage (most mozjpeg-favoring first)
    results.sort_by(|a, b| {
        b.mozjpeg_advantage
            .partial_cmp(&a.mozjpeg_advantage)
            .unwrap()
    });

    println!("\n=== Images where MOZJPEG wins (top {}) ===\n", args.top_n);
    println!(
        "{:>50} | {:>10} | {:>10} | {:>8}",
        "Image", "Advantage", "Size", "moz@85 bpp"
    );
    println!("{}", "-".repeat(90));

    for result in results.iter().take(args.top_n) {
        let moz_bpp = result
            .mozjpeg
            .iter()
            .find(|&&(q, _, _)| q == 85)
            .map(|&(_, bpp, _)| bpp)
            .unwrap_or(0.0);
        println!(
            "{:>50} | {:>+10.1}% | {:>10} | {:>8.3}",
            result
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            result.mozjpeg_advantage * 100.0,
            format!("{}x{}", result.width, result.height),
            moz_bpp
        );
    }

    // Sort by jpegli advantage (most jpegli-favoring first)
    results.sort_by(|a, b| b.jpegli_advantage.partial_cmp(&a.jpegli_advantage).unwrap());

    println!("\n=== Images where JPEGLI wins (top {}) ===\n", args.top_n);
    println!(
        "{:>50} | {:>10} | {:>10} | {:>8}",
        "Image", "Advantage", "Size", "jpegli@85 bpp"
    );
    println!("{}", "-".repeat(90));

    for result in results.iter().take(args.top_n) {
        let jpegli_bpp = result
            .jpegli
            .iter()
            .find(|&&(q, _, _)| q == 85)
            .map(|&(_, bpp, _)| bpp)
            .unwrap_or(0.0);
        println!(
            "{:>50} | {:>+10.1}% | {:>10} | {:>8.3}",
            result
                .path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy(),
            result.jpegli_advantage * 100.0,
            format!("{}x{}", result.width, result.height),
            jpegli_bpp
        );
    }

    // Summary statistics
    let moz_wins = results
        .iter()
        .filter(|r| r.mozjpeg_advantage > 0.05)
        .count();
    let jpegli_wins = results.iter().filter(|r| r.jpegli_advantage > 0.05).count();
    let ties = results.len() - moz_wins - jpegli_wins;

    println!("\n=== Summary ===\n");
    println!("Total images analyzed: {}", results.len());
    println!(
        "mozjpeg wins (>5% advantage): {} ({:.1}%)",
        moz_wins,
        100.0 * moz_wins as f64 / results.len() as f64
    );
    println!(
        "jpegli wins (>5% advantage): {} ({:.1}%)",
        jpegli_wins,
        100.0 * jpegli_wins as f64 / results.len() as f64
    );
    println!(
        "Ties (<5% difference): {} ({:.1}%)",
        ties,
        100.0 * ties as f64 / results.len() as f64
    );

    // Print detailed results for top outliers
    if args.detailed > 0 {
        println!("\n=== Detailed Results for Top Outliers ===\n");

        results.sort_by(|a, b| {
            let a_max = a.mozjpeg_advantage.abs().max(a.jpegli_advantage.abs());
            let b_max = b.mozjpeg_advantage.abs().max(b.jpegli_advantage.abs());
            b_max.partial_cmp(&a_max).unwrap()
        });

        for result in results.iter().take(args.detailed) {
            println!("Image: {}", result.path.display());
            println!(
                "  Size: {}x{} ({} pixels)",
                result.width, result.height, result.pixels
            );
            println!(
                "  mozjpeg advantage: {:+.1}%",
                result.mozjpeg_advantage * 100.0
            );
            println!(
                "  jpegli advantage: {:+.1}%",
                result.jpegli_advantage * 100.0
            );
            println!("  Results:");
            println!(
                "    {:>5} | {:>12} {:>12} | {:>12} {:>12}",
                "Q", "moz bpp", "moz butter", "jpegli bpp", "jpegli butter"
            );
            for &(q, moz_bpp, moz_butter) in &result.mozjpeg {
                if let Some(&(_, jpegli_bpp, jpegli_butter)) =
                    result.jpegli.iter().find(|&&(jq, _, _)| jq == q)
                {
                    println!(
                        "    {:>5} | {:>12.4} {:>12.4} | {:>12.4} {:>12.4}",
                        q, moz_bpp, moz_butter, jpegli_bpp, jpegli_butter
                    );
                }
            }
            println!();
        }
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Parse quality levels
    let qualities: Vec<u8> = args
        .qualities
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    if qualities.is_empty() {
        anyhow::bail!("No valid quality levels specified");
    }

    println!("=== Encoder Outlier Finder ===\n");
    println!("Scanning: {}\n", args.corpus_dir.display());
    println!("Quality levels: {:?}\n", qualities);

    let images = find_images(&args.corpus_dir);
    println!("Found {} images\n", images.len());

    if images.is_empty() {
        println!("No images found. Provide a directory path as argument.");
        return Ok(());
    }

    // Process images in parallel
    let results: Vec<ImageResults> = images
        .par_iter()
        .enumerate()
        .filter_map(|(i, path)| {
            eprint!(
                "\rProcessing {}/{}: {}...",
                i + 1,
                images.len(),
                path.file_name().unwrap_or_default().to_string_lossy()
            );
            analyze_image(path, &qualities, args.min_size)
        })
        .collect();

    eprintln!(
        "\rProcessed {} images successfully.          ",
        results.len()
    );

    if results.is_empty() {
        println!("No images could be analyzed.");
        return Ok(());
    }

    let mut results = results;

    match args.output.as_str() {
        "text" => print_text_report(&mut results, &args),
        "json" => {
            // Simple JSON output for scripting
            #[derive(serde::Serialize)]
            struct JsonResult {
                path: String,
                width: usize,
                height: usize,
                mozjpeg_advantage: f64,
                jpegli_advantage: f64,
            }

            let json_results: Vec<_> = results
                .iter()
                .map(|r| JsonResult {
                    path: r.path.display().to_string(),
                    width: r.width,
                    height: r.height,
                    mozjpeg_advantage: r.mozjpeg_advantage,
                    jpegli_advantage: r.jpegli_advantage,
                })
                .collect();

            println!("{}", serde_json::to_string_pretty(&json_results)?);
        }
        "csv" => {
            println!("path,width,height,mozjpeg_advantage,jpegli_advantage");
            for r in &results {
                println!(
                    "{},{},{},{:.4},{:.4}",
                    r.path.display(),
                    r.width,
                    r.height,
                    r.mozjpeg_advantage,
                    r.jpegli_advantage
                );
            }
        }
        _ => anyhow::bail!("Unknown output format: {}", args.output),
    }

    Ok(())
}
