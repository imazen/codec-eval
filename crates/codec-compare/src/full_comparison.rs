//! Full codec comparison with multiple quality metrics
//!
//! Encodes images with both mozjpeg and jpegli across quality levels,
//! computes butteraugli, dssim, and ssimulacra2 scores, outputs CSV.

use anyhow::Result;
use clap::Parser;
use codec_eval::metrics::{dssim, ssimulacra2};
use codec_eval::viewing::ViewingCondition;
use rayon::prelude::*;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Parser)]
#[command(name = "full-comparison")]
#[command(about = "Full codec comparison with multiple quality metrics")]
struct Args {
    /// Directory containing images
    corpus_dir: PathBuf,

    /// Output CSV file
    #[arg(short, long, default_value = "comparison_results.csv")]
    output: PathBuf,

    /// Quality levels (start,end,step)
    #[arg(short, long, default_value = "25,90,5")]
    quality_range: String,

    /// Maximum images to process (0 = all)
    #[arg(short, long, default_value = "0")]
    max_images: usize,
}

#[derive(Debug, Clone)]
struct EncodeResult {
    image: String,
    encoder: String,
    quality: u8,
    width: usize,
    height: usize,
    file_size: usize,
    bpp: f64,
    butteraugli: f64,
    dssim: f64,
    ssimulacra2: f64,
}

/// Load image as RGB
fn load_image(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgb8();
    let width = rgb.width() as usize;
    let height = rgb.height() as usize;
    Some((rgb.into_raw(), width, height))
}

/// Encode with mozjpeg
#[cfg(feature = "mozjpeg")]
fn encode_mozjpeg(rgb: &[u8], width: usize, height: usize, quality: u8) -> Option<Vec<u8>> {
    use mozjpeg::{ColorSpace, Compress};

    let mut comp = Compress::new(ColorSpace::JCS_RGB);
    comp.set_size(width, height);
    comp.set_quality(quality as f32);
    comp.set_optimize_scans(true);

    let mut comp = comp.start_compress(Vec::new()).ok()?;
    comp.write_scanlines(rgb).ok()?;
    comp.finish().ok()
}

#[cfg(not(feature = "mozjpeg"))]
fn encode_mozjpeg(_: &[u8], _: usize, _: usize, _: u8) -> Option<Vec<u8>> {
    None
}

/// Encode with jpegli (YCbCr mode)
#[cfg(feature = "jpegli")]
fn encode_jpegli(rgb: &[u8], width: usize, height: usize, quality: u8) -> Option<Vec<u8>> {
    use jpegli::encoder::{ChromaSubsampling, EncoderConfig, PixelLayout, Unstoppable};

    std::panic::catch_unwind(|| -> Option<Vec<u8>> {
        let config = EncoderConfig::ycbcr(quality, ChromaSubsampling::Quarter)
            .progressive(true)
            .optimize_huffman(true);

        let mut encoder = config
            .encode_from_bytes(width as u32, height as u32, PixelLayout::Rgb8Srgb)
            .ok()?;
        encoder.push_packed(rgb, Unstoppable).ok()?;
        encoder.finish().ok()
    })
    .ok()
    .flatten()
}

#[cfg(not(feature = "jpegli"))]
fn encode_jpegli(_: &[u8], _: usize, _: usize, _: u8) -> Option<Vec<u8>> {
    None
}

/// Encode with jpegli in XYB color space mode
#[cfg(feature = "jpegli")]
fn encode_jpegli_xyb(rgb: &[u8], width: usize, height: usize, quality: u8) -> Option<Vec<u8>> {
    use jpegli::encoder::{EncoderConfig, PixelLayout, Unstoppable, XybSubsampling};

    std::panic::catch_unwind(|| -> Option<Vec<u8>> {
        let config = EncoderConfig::xyb(quality, XybSubsampling::BQuarter)
            .progressive(true)
            .optimize_huffman(true);

        let mut encoder = config
            .encode_from_bytes(width as u32, height as u32, PixelLayout::Rgb8Srgb)
            .ok()?;
        encoder.push_packed(rgb, Unstoppable).ok()?;
        encoder.finish().ok()
    })
    .ok()
    .flatten()
}

#[cfg(not(feature = "jpegli"))]
fn encode_jpegli_xyb(_: &[u8], _: usize, _: usize, _: u8) -> Option<Vec<u8>> {
    None
}

/// Decode JPEG to RGB (standard decoder, no ICC handling)
fn decode_jpeg(data: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(data, image::ImageFormat::Jpeg).ok()?;
    Some(img.to_rgb8().into_raw())
}

/// Decode JPEG with ICC profile handling (required for XYB mode)
/// Note: XYB decoding with ICC requires the codec-eval icc feature
#[cfg(feature = "jpegli")]
fn decode_jpeg_with_icc(data: &[u8]) -> Option<Vec<u8>> {
    // The jpegli decoder is in prerelease; use standard decoder for now
    // XYB JPEGs require ICC profile handling which is available in codec-eval
    decode_jpeg(data)
}

#[cfg(not(feature = "jpegli"))]
fn decode_jpeg_with_icc(data: &[u8]) -> Option<Vec<u8>> {
    decode_jpeg(data)
}

/// Compute all quality metrics
fn compute_metrics(
    original: &[u8],
    decoded: &[u8],
    width: usize,
    height: usize,
) -> (f64, f64, f64) {
    // Butteraugli
    let butteraugli = {
        use butteraugli::{ButteraugliParams, compute_butteraugli};
        let params = ButteraugliParams::default();
        compute_butteraugli(original, decoded, width, height, &params)
            .map(|r| r.score)
            .unwrap_or(f64::NAN)
    };

    // DSSIM
    let dssim_val = {
        let ref_img = dssim::rgb8_to_dssim_image(original, width, height);
        let test_img = dssim::rgb8_to_dssim_image(decoded, width, height);
        dssim::calculate_dssim(&ref_img, &test_img, &ViewingCondition::desktop())
            .unwrap_or(f64::NAN)
    };

    // SSIMULACRA2
    let ssimulacra2_val =
        ssimulacra2::calculate_ssimulacra2(original, decoded, width, height).unwrap_or(f64::NAN);

    (butteraugli, dssim_val, ssimulacra2_val)
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
            }
        }
    }
    images.sort();
    images
}

fn process_image(path: &Path, qualities: &[u8]) -> Vec<EncodeResult> {
    let mut results = Vec::new();

    let Some((rgb, width, height)) = load_image(path) else {
        return results;
    };

    let image_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    for &quality in qualities {
        // mozjpeg
        if let Some(encoded) = encode_mozjpeg(&rgb, width, height, quality) {
            if let Some(decoded) = decode_jpeg(&encoded) {
                let (butteraugli, dssim, ssimulacra2) =
                    compute_metrics(&rgb, &decoded, width, height);
                let bpp = (encoded.len() as f64 * 8.0) / (width * height) as f64;
                results.push(EncodeResult {
                    image: image_name.clone(),
                    encoder: "mozjpeg".to_string(),
                    quality,
                    width,
                    height,
                    file_size: encoded.len(),
                    bpp,
                    butteraugli,
                    dssim,
                    ssimulacra2,
                });
            }
        }

        // jpegli (YCbCr mode)
        if let Some(encoded) = encode_jpegli(&rgb, width, height, quality) {
            if let Some(decoded) = decode_jpeg(&encoded) {
                let (butteraugli, dssim, ssimulacra2) =
                    compute_metrics(&rgb, &decoded, width, height);
                let bpp = (encoded.len() as f64 * 8.0) / (width * height) as f64;
                results.push(EncodeResult {
                    image: image_name.clone(),
                    encoder: "jpegli".to_string(),
                    quality,
                    width,
                    height,
                    file_size: encoded.len(),
                    bpp,
                    butteraugli,
                    dssim,
                    ssimulacra2,
                });
            }
        }

        // jpegli-xyb (XYB color space mode)
        if let Some(encoded) = encode_jpegli_xyb(&rgb, width, height, quality) {
            // XYB JPEGs require ICC-aware decoding
            if let Some(decoded) = decode_jpeg_with_icc(&encoded) {
                let (butteraugli, dssim, ssimulacra2) =
                    compute_metrics(&rgb, &decoded, width, height);
                let bpp = (encoded.len() as f64 * 8.0) / (width * height) as f64;
                results.push(EncodeResult {
                    image: image_name.clone(),
                    encoder: "jpegli-xyb".to_string(),
                    quality,
                    width,
                    height,
                    file_size: encoded.len(),
                    bpp,
                    butteraugli,
                    dssim,
                    ssimulacra2,
                });
            }
        }
    }

    results
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Parse quality range
    let parts: Vec<u8> = args
        .quality_range
        .split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();

    let (start, end, step) = match parts.as_slice() {
        [s, e, st] => (*s, *e, *st),
        _ => (25, 90, 5),
    };

    let qualities: Vec<u8> = (start..=end).step_by(step as usize).collect();

    println!("=== Full Codec Comparison ===\n");
    println!("Corpus: {}", args.corpus_dir.display());
    println!("Quality levels: {:?}", qualities);
    println!("Output: {}\n", args.output.display());

    let mut images = find_images(&args.corpus_dir);
    if args.max_images > 0 && images.len() > args.max_images {
        images.truncate(args.max_images);
    }

    println!("Found {} images", images.len());
    println!(
        "Total encodes: {} (images) x {} (qualities) x 3 (encoders) = {}\n",
        images.len(),
        qualities.len(),
        images.len() * qualities.len() * 3
    );

    // Process images in parallel
    let progress = Mutex::new(0usize);
    let total = images.len();

    let all_results: Vec<EncodeResult> = images
        .par_iter()
        .flat_map(|path| {
            let results = process_image(path, &qualities);
            let mut p = progress.lock().unwrap();
            *p += 1;
            eprint!("\rProcessed {}/{} images...", *p, total);
            results
        })
        .collect();

    eprintln!(
        "\rProcessed {} images, {} encode results",
        total,
        all_results.len()
    );

    // Write CSV
    let mut file = std::fs::File::create(&args.output)?;
    writeln!(
        file,
        "image,encoder,quality,width,height,file_size,bpp,butteraugli,dssim,ssimulacra2"
    )?;

    for r in &all_results {
        writeln!(
            file,
            "{},{},{},{},{},{},{:.6},{:.6},{:.8},{:.4}",
            r.image,
            r.encoder,
            r.quality,
            r.width,
            r.height,
            r.file_size,
            r.bpp,
            r.butteraugli,
            r.dssim,
            r.ssimulacra2
        )?;
    }

    println!(
        "\nWrote {} results to {}",
        all_results.len(),
        args.output.display()
    );

    Ok(())
}
