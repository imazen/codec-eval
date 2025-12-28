//! Rate-Distortion comparison at matched file sizes
//!
//! This compares encoders at the SAME BPP to see which achieves better quality.
//! This is a more fair comparison than same-Q comparisons.

use anyhow::Result;
use butteraugli::{ButteraugliParams, compute_butteraugli};
use clap::Parser;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "rd-compare")]
#[command(about = "Compare encoders at matched file sizes (rate-distortion)")]
struct Args {
    /// Directory containing images to analyze
    #[arg(default_value = ".")]
    corpus_dir: PathBuf,

    /// Target bits-per-pixel values to compare at
    #[arg(short, long, default_value = "0.5,1.0,1.5,2.0,3.0")]
    bpp_targets: String,

    /// Minimum image dimension
    #[arg(long, default_value = "64")]
    min_size: usize,

    /// Number of results to show
    #[arg(short = 'n', long, default_value = "10")]
    top_n: usize,
}

#[derive(Debug, Clone)]
struct RDPoint {
    quality: u8,
    bpp: f64,
    butteraugli: f64,
}

#[derive(Debug, Clone)]
struct ImageRD {
    path: PathBuf,
    width: usize,
    height: usize,
    mozjpeg_curve: Vec<RDPoint>,
    jpegli_curve: Vec<RDPoint>,
}

/// Encode with mozjpeg and return (bpp, butteraugli)
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

    let img = image::load_from_memory_with_format(&result, image::ImageFormat::Jpeg).ok()?;
    let decoded = img.to_rgb8();
    let decoded_raw = decoded.as_raw();

    let params = ButteraugliParams::default();
    let butter = compute_butteraugli(rgb, decoded_raw, width, height, &params)
        .map(|r| r.score)
        .unwrap_or(f64::NAN);

    let bpp = (result.len() as f64 * 8.0) / (width * height) as f64;
    Some((bpp, butter))
}

#[cfg(not(feature = "mozjpeg"))]
fn encode_mozjpeg(_: &[u8], _: usize, _: usize, _: u8) -> Option<(f64, f64)> {
    None
}

/// Encode with jpegli and return (bpp, butteraugli)
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

        let img = image::load_from_memory_with_format(&result, image::ImageFormat::Jpeg).ok()?;
        let decoded = img.to_rgb8();
        let decoded_raw = decoded.as_raw();

        let params = ButteraugliParams::default();
        let butter = compute_butteraugli(rgb, decoded_raw, width, height, &params)
            .map(|r| r.score)
            .unwrap_or(f64::NAN);

        let bpp = (result.len() as f64 * 8.0) / (width * height) as f64;
        Some((bpp, butter))
    })
    .ok()
    .flatten()
}

#[cfg(not(feature = "jpegli"))]
fn encode_jpegli(_: &[u8], _: usize, _: usize, _: u8) -> Option<(f64, f64)> {
    None
}

/// Load image
fn load_image(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgb8();
    let width = rgb.width() as usize;
    let height = rgb.height() as usize;
    Some((rgb.into_raw(), width, height))
}

/// Build full RD curve for an encoder
fn build_rd_curve<F>(rgb: &[u8], width: usize, height: usize, encode_fn: F) -> Vec<RDPoint>
where
    F: Fn(&[u8], usize, usize, u8) -> Option<(f64, f64)>,
{
    let mut points = Vec::new();
    // Sample quality levels densely
    for q in (20..=98).step_by(2) {
        if let Some((bpp, butter)) = encode_fn(rgb, width, height, q) {
            if butter.is_finite() {
                points.push(RDPoint {
                    quality: q,
                    bpp,
                    butteraugli: butter,
                });
            }
        }
    }
    points
}

/// Interpolate butteraugli at a target bpp
fn interpolate_at_bpp(curve: &[RDPoint], target_bpp: f64) -> Option<f64> {
    if curve.len() < 2 {
        return None;
    }

    // Find bracketing points
    let mut below: Option<&RDPoint> = None;
    let mut above: Option<&RDPoint> = None;

    for point in curve {
        if point.bpp <= target_bpp {
            if below.is_none() || point.bpp > below.unwrap().bpp {
                below = Some(point);
            }
        }
        if point.bpp >= target_bpp {
            if above.is_none() || point.bpp < above.unwrap().bpp {
                above = Some(point);
            }
        }
    }

    match (below, above) {
        (Some(b), Some(a)) if (a.bpp - b.bpp).abs() > 0.001 => {
            // Linear interpolation
            let t = (target_bpp - b.bpp) / (a.bpp - b.bpp);
            Some(b.butteraugli + t * (a.butteraugli - b.butteraugli))
        }
        (Some(b), Some(_)) => Some(b.butteraugli),
        (Some(b), None) => Some(b.butteraugli), // Extrapolate from below
        (None, Some(a)) => Some(a.butteraugli), // Extrapolate from above
        _ => None,
    }
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

fn main() -> Result<()> {
    let args = Args::parse();

    let bpp_targets: Vec<f64> = args
        .bpp_targets
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    println!("=== Rate-Distortion Comparison ===\n");
    println!("Comparing at matched file sizes (bpp): {:?}\n", bpp_targets);
    println!("Scanning: {}\n", args.corpus_dir.display());

    let images = find_images(&args.corpus_dir);
    println!("Found {} images\n", images.len());

    if images.is_empty() {
        println!("No images found.");
        return Ok(());
    }

    // Limit to first N images for speed
    let images: Vec<_> = images.into_iter().take(args.top_n).collect();

    // For each target bpp, track which encoder wins
    let mut moz_wins_at_bpp: Vec<(f64, usize)> = bpp_targets.iter().map(|&b| (b, 0)).collect();
    let mut jpegli_wins_at_bpp: Vec<(f64, usize)> = bpp_targets.iter().map(|&b| (b, 0)).collect();
    let mut total_images = 0;

    println!("Building RD curves (this takes a while)...\n");

    for (i, path) in images.iter().enumerate() {
        eprint!(
            "\rProcessing {}/{}: {}",
            i + 1,
            images.len(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );

        let Some((rgb, width, height)) = load_image(path) else {
            continue;
        };

        if width < args.min_size || height < args.min_size {
            continue;
        }

        let moz_curve = build_rd_curve(&rgb, width, height, encode_mozjpeg);
        let jpegli_curve = build_rd_curve(&rgb, width, height, encode_jpegli);

        if moz_curve.is_empty() || jpegli_curve.is_empty() {
            continue;
        }

        total_images += 1;

        // Compare at each target bpp
        for (idx, &target_bpp) in bpp_targets.iter().enumerate() {
            let moz_butter = interpolate_at_bpp(&moz_curve, target_bpp);
            let jpegli_butter = interpolate_at_bpp(&jpegli_curve, target_bpp);

            if let (Some(m), Some(j)) = (moz_butter, jpegli_butter) {
                if m < j {
                    moz_wins_at_bpp[idx].1 += 1;
                } else {
                    jpegli_wins_at_bpp[idx].1 += 1;
                }
            }
        }

        // Print detailed comparison for this image
        println!(
            "\n\nImage: {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        );
        println!("  Size: {}x{}", width, height);
        println!(
            "  {:>8} | {:>12} {:>12} | {:>12}",
            "BPP", "mozjpeg", "jpegli", "Winner"
        );
        println!("  {}", "-".repeat(55));

        for &target_bpp in &bpp_targets {
            let moz_butter = interpolate_at_bpp(&moz_curve, target_bpp);
            let jpegli_butter = interpolate_at_bpp(&jpegli_curve, target_bpp);

            match (moz_butter, jpegli_butter) {
                (Some(m), Some(j)) => {
                    let winner = if m < j { "mozjpeg" } else { "jpegli" };
                    let diff = ((m - j) / j * 100.0).abs();
                    println!(
                        "  {:>8.2} | {:>12.4} {:>12.4} | {} ({:.1}%)",
                        target_bpp, m, j, winner, diff
                    );
                }
                _ => {
                    println!("  {:>8.2} | {:>12} {:>12} |", target_bpp, "N/A", "N/A");
                }
            }
        }
    }

    eprintln!();

    // Summary
    println!("\n=== Summary (at matched file sizes) ===\n");
    println!("Total images analyzed: {}\n", total_images);
    println!(
        "{:>8} | {:>12} | {:>12}",
        "BPP", "mozjpeg wins", "jpegli wins"
    );
    println!("{}", "-".repeat(40));

    for (idx, &target_bpp) in bpp_targets.iter().enumerate() {
        let moz = moz_wins_at_bpp[idx].1;
        let jpegli = jpegli_wins_at_bpp[idx].1;
        println!(
            "{:>8.2} | {:>12} | {:>12}",
            target_bpp,
            format!("{} ({:.0}%)", moz, 100.0 * moz as f64 / total_images as f64),
            format!(
                "{} ({:.0}%)",
                jpegli,
                100.0 * jpegli as f64 / total_images as f64
            )
        );
    }

    Ok(())
}
