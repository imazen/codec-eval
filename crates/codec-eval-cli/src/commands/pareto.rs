//! Pareto front calculation command.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use codec_eval::import::ExternalResult;
use codec_eval::stats::{ParetoFront, RDPoint};

pub fn run(
    input: PathBuf,
    output: Option<PathBuf>,
    metric: &str,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Loading results from: {}", input.display());
    }

    // Load results (support both JSON and CSV)
    let results = load_results(&input)?;

    if verbose {
        eprintln!("Loaded {} results", results.len());
    }

    // Convert to RDPoints
    let points: Vec<RDPoint> = results
        .iter()
        .filter_map(|r| {
            let bpp = r.bits_per_pixel.or_else(|| {
                // Estimate from file size if we have dimensions
                r.file_size.map(|s| s as f64 * 8.0 / 1_000_000.0) // rough estimate
            })?;

            let quality = match metric.to_lowercase().as_str() {
                "dssim" => r.dssim.map(|d| -d), // Negate so higher is better
                "ssimulacra2" | "ssim2" => r.ssimulacra2,
                "psnr" => r.psnr,
                "butteraugli" | "ba" => r.butteraugli.map(|b| -b), // Negate
                _ => return None,
            }?;

            Some(RDPoint {
                codec: r.codec.clone(),
                quality_setting: r.quality_setting.unwrap_or(0.0),
                bpp,
                quality,
                encode_time_ms: r.encode_time_ms,
                image: Some(r.image_name.clone()),
            })
        })
        .collect();

    if points.is_empty() {
        bail!("No valid points found for metric '{}'", metric);
    }

    if verbose {
        eprintln!("Converted {} points with metric '{}'", points.len(), metric);
    }

    // Compute overall Pareto front
    let front = ParetoFront::compute(&points);

    println!("Pareto Front ({} points from {} total)", front.len(), points.len());
    println!();

    // Show codecs on the front
    let codecs = front.codecs();
    println!("Codecs on front: {}", codecs.join(", "));
    println!();

    // Show points
    println!("{:<15} {:>8} {:>10} {:>10} {:>12}",
        "Codec", "Quality", "BPP", metric.to_uppercase(), "Encode(ms)");
    println!("{:-<60}", "");

    for point in &front.points {
        let quality_str = match metric.to_lowercase().as_str() {
            "dssim" | "butteraugli" => format!("{:.6}", -point.quality),
            _ => format!("{:.2}", point.quality),
        };

        println!("{:<15} {:>8.1} {:>10.4} {:>10} {:>12}",
            point.codec,
            point.quality_setting,
            point.bpp,
            quality_str,
            point.encode_time_ms.map_or("-".to_string(), |t| format!("{:.0}", t))
        );
    }

    // Compute per-codec fronts
    println!();
    println!("Per-codec Pareto fronts:");
    let per_codec = ParetoFront::per_codec(&points);
    let mut sorted: Vec<_> = per_codec.iter().collect();
    sorted.sort_by_key(|(codec, _)| codec.as_str());

    for (codec, codec_front) in sorted {
        println!("  {}: {} points", codec, codec_front.len());
    }

    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&front)?;
        std::fs::write(&output_path, json)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        println!();
        println!("Saved to: {}", output_path.display());
    }

    Ok(())
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
