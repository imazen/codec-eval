//! Compute R-D knee calibration from corpus data.
//!
//! Sweeps a JPEG encoder across quality levels on CID22 and CLIC2025,
//! measures both SSIMULACRA2 and Butteraugli, then computes the corpus-
//! aggregate R-D curve and finds the 45° knee for each metric.

use anyhow::Result;
use butteraugli::{ButteraugliParams, compute_butteraugli};
use clap::Parser;
use codec_eval::stats::rd_knee::{CorpusAggregate, FixedFrame, RDCalibration};
use rayon::prelude::*;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "rd-calibrate")]
#[command(about = "Compute R-D knee calibration from corpus aggregate data")]
struct Args {
    /// Directory containing images (PNG)
    corpus_dir: PathBuf,

    /// Corpus name for labeling output
    #[arg(short = 'n', long, default_value = "CID22-training")]
    corpus_name: String,

    /// Codec name label
    #[arg(short, long, default_value = "mozjpeg-420-prog")]
    codec: String,

    /// Quality levels to sweep (start:step:end)
    #[arg(short, long, default_value = "10:2:98")]
    quality_range: String,

    /// Output CSV path (optional)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Max images to process (0 = all)
    #[arg(short, long, default_value = "0")]
    max_images: usize,
}

/// One encode measurement for one image at one quality level.
#[derive(Debug, Clone)]
struct Measurement {
    image: String,
    quality: u8,
    bpp: f64,
    ssimulacra2: f64,
    butteraugli: f64,
}

fn parse_range(s: &str) -> Vec<u8> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        3 => {
            let start: u8 = parts[0].parse().unwrap_or(10);
            let step: u8 = parts[1].parse().unwrap_or(2);
            let end: u8 = parts[2].parse().unwrap_or(98);
            (start..=end).step_by(step as usize).collect()
        }
        _ => vec![10, 20, 30, 40, 50, 60, 70, 75, 80, 85, 90, 95],
    }
}

fn find_png_images(dir: &Path) -> Vec<PathBuf> {
    let mut images = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext.to_string_lossy().to_lowercase() == "png" {
                        images.push(path);
                    }
                }
            } else if path.is_dir() {
                images.extend(find_png_images(&path));
            }
        }
    }
    images.sort();
    images
}

/// Load image as RGB8 bytes.
fn load_image(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgb8();
    let width = rgb.width() as usize;
    let height = rgb.height() as usize;
    Some((rgb.into_raw(), width, height))
}

/// Encode with mozjpeg, decode, measure both metrics.
#[cfg(feature = "mozjpeg")]
fn encode_and_measure(
    rgb: &[u8],
    width: usize,
    height: usize,
    quality: u8,
) -> Option<(f64, f64, f64)> {
    use mozjpeg::{ColorSpace, Compress};

    // Encode
    let mut comp = Compress::new(ColorSpace::JCS_RGB);
    comp.set_size(width, height);
    comp.set_quality(quality as f32);
    comp.set_chroma_sampling_pixel_sizes((2, 2), (2, 2)); // 4:2:0
    comp.set_optimize_coding(true);
    comp.set_progressive_mode();
    comp.set_optimize_scans(true);

    let mut comp = comp.start_compress(Vec::new()).ok()?;
    comp.write_scanlines(rgb).ok()?;
    let jpeg_bytes = comp.finish().ok()?;

    let bpp = (jpeg_bytes.len() as f64 * 8.0) / (width * height) as f64;

    // Decode
    let img = image::load_from_memory_with_format(&jpeg_bytes, image::ImageFormat::Jpeg).ok()?;
    let decoded = img.to_rgb8();
    let decoded_raw = decoded.as_raw();

    // SSIMULACRA2
    let source_img = imgref::ImgRef::new(
        bytemuck::cast_slice::<u8, [u8; 3]>(rgb),
        width,
        height,
    );
    let distorted_img = imgref::ImgRef::new(
        bytemuck::cast_slice::<u8, [u8; 3]>(decoded_raw),
        width,
        height,
    );
    let s2 = fast_ssim2::compute_ssimulacra2(source_img, distorted_img).ok()?;

    // Butteraugli
    let params = ButteraugliParams::default();
    let ba = compute_butteraugli(rgb, decoded_raw, width, height, &params)
        .ok()?
        .score;

    if s2.is_finite() && ba.is_finite() {
        Some((bpp, s2, ba))
    } else {
        None
    }
}

#[cfg(not(feature = "mozjpeg"))]
fn encode_and_measure(
    _rgb: &[u8],
    _width: usize,
    _height: usize,
    _quality: u8,
) -> Option<(f64, f64, f64)> {
    eprintln!("ERROR: mozjpeg feature not enabled");
    None
}

fn main() -> Result<()> {
    let args = Args::parse();
    let quality_levels = parse_range(&args.quality_range);

    println!("=== R-D Knee Calibration ===\n");
    println!("Corpus:    {}", args.corpus_name);
    println!("Codec:     {}", args.codec);
    println!("Qualities: {} levels ({}-{})",
        quality_levels.len(), quality_levels.first().unwrap_or(&0), quality_levels.last().unwrap_or(&0));
    println!("Directory: {}\n", args.corpus_dir.display());

    let mut images = find_png_images(&args.corpus_dir);
    if args.max_images > 0 && images.len() > args.max_images {
        images.truncate(args.max_images);
    }
    println!("Found {} images\n", images.len());

    if images.is_empty() {
        anyhow::bail!("No PNG images found in {}", args.corpus_dir.display());
    }

    // Process all images in parallel
    let all_measurements: Vec<Measurement> = images
        .par_iter()
        .enumerate()
        .flat_map(|(i, path)| {
            let image_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            if i % 10 == 0 {
                eprintln!("  Processing {}/{}: {}", i + 1, images.len(), image_name);
            }

            let Some((rgb, width, height)) = load_image(path) else {
                return Vec::new();
            };

            quality_levels
                .iter()
                .filter_map(|&q| {
                    let (bpp, s2, ba) = encode_and_measure(&rgb, width, height, q)?;
                    Some(Measurement {
                        image: image_name.clone(),
                        quality: q,
                        bpp,
                        ssimulacra2: s2,
                        butteraugli: ba,
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect();

    let image_count = images.len();
    println!("\nCollected {} measurements from {} images\n",
        all_measurements.len(), image_count);

    // Write CSV if requested
    if let Some(ref csv_path) = args.output {
        let mut wtr = csv::Writer::from_path(csv_path)?;
        wtr.write_record(["image", "quality", "bpp", "ssimulacra2", "butteraugli"])?;
        for m in &all_measurements {
            wtr.write_record(&[
                &m.image,
                &m.quality.to_string(),
                &format!("{:.6}", m.bpp),
                &format!("{:.4}", m.ssimulacra2),
                &format!("{:.4}", m.butteraugli),
            ])?;
        }
        wtr.flush()?;
        println!("Wrote CSV: {}\n", csv_path.display());
    }

    // Aggregate: group by quality level, take mean across images
    let mut by_quality: BTreeMap<u8, Vec<(f64, f64, f64)>> = BTreeMap::new();
    for m in &all_measurements {
        by_quality
            .entry(m.quality)
            .or_default()
            .push((m.bpp, m.ssimulacra2, m.butteraugli));
    }

    let mut curve: Vec<(f64, f64, f64)> = by_quality
        .iter()
        .map(|(_q, points)| {
            let n = points.len() as f64;
            let avg_bpp: f64 = points.iter().map(|(b, _, _)| b).sum::<f64>() / n;
            let avg_s2: f64 = points.iter().map(|(_, s, _)| s).sum::<f64>() / n;
            let avg_ba: f64 = points.iter().map(|(_, _, b)| b).sum::<f64>() / n;
            (avg_bpp, avg_s2, avg_ba)
        })
        .collect();

    // Sort by bpp (should already be, but ensure it)
    curve.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    println!("=== Corpus Aggregate R-D Curve ===\n");
    println!("{:>6} {:>8} {:>10} {:>10}", "Q", "bpp", "s2", "ba");
    println!("{}", "-".repeat(38));
    for ((_q, _points), (bpp, s2, ba)) in by_quality.iter().zip(curve.iter()) {
        println!("{:>6} {:>8.4} {:>10.2} {:>10.3}", _q, bpp, s2, ba);
    }

    // Compute knees
    let agg = CorpusAggregate {
        corpus: args.corpus_name.clone(),
        codec: args.codec.clone(),
        curve: curve.clone(),
        image_count,
    };

    let frame = FixedFrame::WEB;

    println!("\n=== Knee Detection (FixedFrame::WEB) ===\n");

    match agg.ssimulacra2_knee(&frame) {
        Some(knee) => {
            println!("SSIMULACRA2 knee:");
            println!("  bpp:         {:.4}", knee.bpp);
            println!("  s2:          {:.2}", knee.quality);
            println!("  fixed_angle: {:.1}°", knee.fixed_angle);
            println!("  bpp range:   [{:.4}, {:.4}]",
                knee.norm.bpp_range.min, knee.norm.bpp_range.max);
            println!("  s2 range:    [{:.2}, {:.2}]",
                knee.norm.quality_range.min, knee.norm.quality_range.max);
        }
        None => println!("SSIMULACRA2 knee: FAILED to compute"),
    }

    println!();

    match agg.butteraugli_knee(&frame) {
        Some(knee) => {
            println!("Butteraugli knee:");
            println!("  bpp:         {:.4}", knee.bpp);
            println!("  ba:          {:.3}", knee.quality);
            println!("  fixed_angle: {:.1}°", knee.fixed_angle);
            println!("  bpp range:   [{:.4}, {:.4}]",
                knee.norm.bpp_range.min, knee.norm.bpp_range.max);
            println!("  ba range:    [{:.3}, {:.3}]",
                knee.norm.quality_range.min, knee.norm.quality_range.max);
        }
        None => println!("Butteraugli knee: FAILED to compute"),
    }

    match agg.calibrate(&frame) {
        Some(cal) => {
            println!("\n=== Calibration Summary ===\n");
            let (lo, hi) = cal.disagreement_range();
            println!("Disagreement range: [{:.4}, {:.4}] bpp", lo, hi);
            println!("  s2 knee at {:.4} bpp (s2={:.2}, angle={:.1}°)",
                cal.ssimulacra2.bpp, cal.ssimulacra2.quality, cal.ssimulacra2.fixed_angle);
            println!("  ba knee at {:.4} bpp (ba={:.3}, angle={:.1}°)",
                cal.butteraugli.bpp, cal.butteraugli.quality, cal.butteraugli.fixed_angle);

            // Generate SVG plot
            let title = format!("{} — {}", args.codec, args.corpus_name);
            let svg = codec_eval::stats::plot_rd_svg(&curve, &cal, &title);

            let svg_dir = args.output.as_ref()
                .and_then(|p| p.parent())
                .unwrap_or(Path::new("."));
            let svg_path = svg_dir.join(format!("{}-{}-rd.svg",
                args.codec.replace(' ', "-"),
                args.corpus_name.replace(' ', "-")));
            std::fs::write(&svg_path, &svg)?;
            println!("\nWrote SVG: {}", svg_path.display());

            // Print Rust code for defaults
            println!("\n=== Rust Default Code ===\n");
            print_rust_default(&cal);
        }
        None => println!("\nCalibration: FAILED"),
    }

    Ok(())
}

fn print_rust_default(cal: &RDCalibration) {
    println!("let frame = FixedFrame::WEB;");
    println!("RDCalibration {{");
    println!("    frame,");
    println!("    ssimulacra2: RDKnee {{");
    println!("        bpp: {:.4},", cal.ssimulacra2.bpp);
    println!("        quality: {:.2},", cal.ssimulacra2.quality);
    println!("        fixed_angle: frame.s2_angle({:.4}, {:.2}),",
        cal.ssimulacra2.bpp, cal.ssimulacra2.quality);
    println!("        norm: NormalizationContext {{");
    println!("            bpp_range: AxisRange::new({:.4}, {:.4}),",
        cal.ssimulacra2.norm.bpp_range.min, cal.ssimulacra2.norm.bpp_range.max);
    println!("            quality_range: AxisRange::new({:.2}, {:.2}),",
        cal.ssimulacra2.norm.quality_range.min, cal.ssimulacra2.norm.quality_range.max);
    println!("            direction: QualityDirection::HigherIsBetter,");
    println!("        }},");
    println!("    }},");
    println!("    butteraugli: RDKnee {{");
    println!("        bpp: {:.4},", cal.butteraugli.bpp);
    println!("        quality: {:.3},", cal.butteraugli.quality);
    println!("        fixed_angle: frame.ba_angle({:.4}, {:.3}),",
        cal.butteraugli.bpp, cal.butteraugli.quality);
    println!("        norm: NormalizationContext {{");
    println!("            bpp_range: AxisRange::new({:.4}, {:.4}),",
        cal.butteraugli.norm.bpp_range.min, cal.butteraugli.norm.bpp_range.max);
    println!("            quality_range: AxisRange::new({:.3}, {:.3}),",
        cal.butteraugli.norm.quality_range.min, cal.butteraugli.norm.quality_range.max);
    println!("            direction: QualityDirection::LowerIsBetter,");
    println!("        }},");
    println!("    }},");
    println!("    corpus: \"{}\".into(),", cal.corpus);
    println!("    codec: \"{}\".into(),", cal.codec);
    println!("    image_count: {},", cal.image_count);
    println!("    computed_at: \"{}\".into(),", chrono::Utc::now().to_rfc3339());
    println!("}}");
}
