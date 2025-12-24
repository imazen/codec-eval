//! Statistics command.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use codec_eval::import::ExternalResult;
use codec_eval::stats::Summary;

pub fn run(
    input: PathBuf,
    by_codec: bool,
    by_image: bool,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Loading results from: {}", input.display());
    }

    let results = load_results(&input)?;

    println!("Total results: {}", results.len());
    println!();

    // Overall stats
    print_overall_stats(&results);

    if by_codec {
        println!();
        print_by_codec(&results);
    }

    if by_image {
        println!();
        print_by_image(&results);
    }

    Ok(())
}

fn print_overall_stats(results: &[ExternalResult]) {
    println!("Overall Statistics:");
    println!("{:-<60}", "");

    // File sizes
    let sizes: Vec<f64> = results.iter()
        .filter_map(|r| r.file_size.map(|s| s as f64))
        .collect();
    if let Some(summary) = Summary::compute(&sizes) {
        println!("File size (bytes):");
        println!("  Mean: {:.0}, Median: {:.0}", summary.mean, summary.median);
        println!("  Min: {:.0}, Max: {:.0}", summary.min, summary.max);
        println!("  StdDev: {:.0}", summary.std_dev);
    }

    // DSSIM
    let dssim: Vec<f64> = results.iter()
        .filter_map(|r| r.dssim)
        .collect();
    if let Some(summary) = Summary::compute(&dssim) {
        println!("DSSIM:");
        println!("  Mean: {:.6}, Median: {:.6}", summary.mean, summary.median);
        println!("  Min: {:.6}, Max: {:.6}", summary.min, summary.max);
    }

    // SSIMULACRA2
    let ssim2: Vec<f64> = results.iter()
        .filter_map(|r| r.ssimulacra2)
        .collect();
    if let Some(summary) = Summary::compute(&ssim2) {
        println!("SSIMULACRA2:");
        println!("  Mean: {:.2}, Median: {:.2}", summary.mean, summary.median);
        println!("  Min: {:.2}, Max: {:.2}", summary.min, summary.max);
    }

    // PSNR
    let psnr: Vec<f64> = results.iter()
        .filter_map(|r| r.psnr)
        .collect();
    if let Some(summary) = Summary::compute(&psnr) {
        println!("PSNR:");
        println!("  Mean: {:.2}, Median: {:.2}", summary.mean, summary.median);
        println!("  Min: {:.2}, Max: {:.2}", summary.min, summary.max);
    }
}

fn print_by_codec(results: &[ExternalResult]) {
    println!("Statistics by Codec:");
    println!("{:-<60}", "");

    let mut by_codec: HashMap<&str, Vec<&ExternalResult>> = HashMap::new();
    for r in results {
        by_codec.entry(&r.codec).or_default().push(r);
    }

    let mut sorted: Vec<_> = by_codec.into_iter().collect();
    sorted.sort_by_key(|(codec, _)| *codec);

    println!("{:<15} {:>8} {:>12} {:>10} {:>10}",
        "Codec", "Results", "Avg Size", "Avg DSSIM", "Avg PSNR");
    println!("{:-<60}", "");

    for (codec, codec_results) in sorted {
        let sizes: Vec<f64> = codec_results.iter()
            .filter_map(|r| r.file_size.map(|s| s as f64))
            .collect();
        let avg_size = sizes.iter().sum::<f64>() / sizes.len().max(1) as f64;

        let dssim: Vec<f64> = codec_results.iter()
            .filter_map(|r| r.dssim)
            .collect();
        let avg_dssim = if dssim.is_empty() {
            "-".to_string()
        } else {
            format!("{:.6}", dssim.iter().sum::<f64>() / dssim.len() as f64)
        };

        let psnr: Vec<f64> = codec_results.iter()
            .filter_map(|r| r.psnr)
            .collect();
        let avg_psnr = if psnr.is_empty() {
            "-".to_string()
        } else {
            format!("{:.2}", psnr.iter().sum::<f64>() / psnr.len() as f64)
        };

        println!("{:<15} {:>8} {:>12.0} {:>10} {:>10}",
            codec,
            codec_results.len(),
            avg_size,
            avg_dssim,
            avg_psnr
        );
    }
}

fn print_by_image(results: &[ExternalResult]) {
    println!("Statistics by Image:");
    println!("{:-<60}", "");

    let mut by_image: HashMap<&str, Vec<&ExternalResult>> = HashMap::new();
    for r in results {
        by_image.entry(&r.image_name).or_default().push(r);
    }

    println!("{:<30} {:>8} {:>12}",
        "Image", "Results", "Codecs");
    println!("{:-<60}", "");

    let mut sorted: Vec<_> = by_image.into_iter().collect();
    sorted.sort_by_key(|(img, _)| *img);

    for (image, image_results) in sorted.iter().take(20) {
        let mut codecs: Vec<&str> = image_results.iter()
            .map(|r| r.codec.as_str())
            .collect();
        codecs.sort();
        codecs.dedup();

        let name = if image.len() > 28 {
            format!("...{}", &image[image.len()-25..])
        } else {
            (*image).to_string()
        };

        println!("{:<30} {:>8} {:>12}",
            name,
            image_results.len(),
            codecs.len()
        );
    }

    if sorted.len() > 20 {
        println!("... and {} more images", sorted.len() - 20);
    }
}

fn load_results(path: &PathBuf) -> Result<Vec<ExternalResult>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    // Try JSON first
    if let Ok(results) = serde_json::from_str::<Vec<ExternalResult>>(&content) {
        return Ok(results);
    }

    // Try CSV
    let importer = codec_eval::import::CsvImporter::auto_detect();
    importer.import(path)
        .with_context(|| format!("Failed to parse {} as JSON or CSV", path.display()))
}
