//! Optimized brute-force quality sweep for SSIM2 correlation analysis.
//!
//! Uses fast-ssim2 with Ssimulacra2Reference for batch comparisons,
//! direct u8 input, and real-time ETA display.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use codec_eval::corpus::Corpus;
use codec_eval::eval::ImageData;
use fast_ssim2::Ssimulacra2Reference;
use image::GenericImageView;
use imgref::Img;

use codec_compare::encoders::{self, CodecImpl};

#[derive(Parser)]
#[command(name = "brute-force-sweep")]
#[command(about = "Run all codecs at fine quality levels for SSIM2 correlation (optimized)")]
struct Args {
    /// Paths to image corpus directories (can specify multiple)
    #[arg(short, long, required = true)]
    corpus: Vec<PathBuf>,

    /// Output CSV file
    #[arg(short, long, default_value = "brute_force_results.csv")]
    output: PathBuf,

    /// Minimum quality level (0-100)
    #[arg(long, default_value = "0")]
    min_quality: u8,

    /// Maximum quality level (0-100)
    #[arg(long, default_value = "100")]
    max_quality: u8,

    /// Quality step size
    #[arg(long, default_value = "2")]
    step: u8,

    /// Maximum number of images to process per corpus
    #[arg(long)]
    limit: Option<usize>,

    /// Include zenjpeg codec
    #[arg(long)]
    zenjpeg: bool,

    /// Include WebP codec
    #[arg(long)]
    webp: bool,

    /// Include AVIF codec
    #[arg(long)]
    avif: bool,

    /// AVIF encoder speed (1-10, lower = slower/better)
    #[arg(long, default_value = "6")]
    avif_speed: u8,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn format_duration(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else {
        format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Generate quality levels
    let quality_levels: Vec<f64> = (args.min_quality..=args.max_quality)
        .step_by(args.step as usize)
        .map(|q| q as f64)
        .collect();

    println!(
        "Quality levels: {} points from {} to {} (step {})",
        quality_levels.len(),
        args.min_quality,
        args.max_quality,
        args.step
    );

    // Discover all corpuses
    let mut all_images: Vec<(PathBuf, String)> = Vec::new();
    for corpus_path in &args.corpus {
        println!("Discovering corpus at {}...", corpus_path.display());
        let corpus = Corpus::discover(corpus_path)?;
        let limit = args
            .limit
            .unwrap_or(corpus.images.len())
            .min(corpus.images.len());
        for img in corpus.images.iter().take(limit) {
            all_images.push((img.full_path(&corpus.root_path), img.name().to_string()));
        }
        println!("  Found {} images, taking {}", corpus.images.len(), limit);
    }
    let image_count = all_images.len();
    println!("Total images across all corpuses: {}\n", image_count);

    // Build list of codecs
    let mut codecs: Vec<Box<dyn CodecImpl>> = Vec::new();

    // JPEG codecs - all variants
    for codec in encoders::jpeg::MozJpegCodec::all_variants() {
        if codec.is_available() {
            codecs.push(Box::new(codec));
        }
    }
    for codec in encoders::jpeg::JpegliCodec::all_variants() {
        if codec.is_available() {
            codecs.push(Box::new(codec));
        }
    }

    // Zenjpeg
    if args.zenjpeg {
        for codec in encoders::zenjpeg::ZenjpegCodec::all_variants() {
            if codec.is_available() {
                codecs.push(Box::new(codec));
            }
        }
    }

    // WebP
    if args.webp {
        let webp = encoders::webp::WebPCodec::new();
        if webp.is_available() {
            codecs.push(Box::new(webp));
        }
    }

    // AVIF
    if args.avif {
        for codec in encoders::avif::AvifCodec::all() {
            let codec = codec.with_speed(args.avif_speed);
            if codec.is_available() {
                codecs.push(Box::new(codec));
            }
        }
    }

    let codec_ids: Vec<_> = codecs.iter().map(|c| c.id()).collect();
    println!("Registered codecs: {}", codec_ids.join(", "));

    let total_encodes = image_count * codecs.len() * quality_levels.len();
    println!(
        "Total encodes: {} images x {} codecs x {} qualities = {}",
        image_count,
        codecs.len(),
        quality_levels.len(),
        total_encodes
    );
    println!();

    // Prepare CSV output
    let file = File::create(&args.output)?;
    let mut writer = BufWriter::new(file);
    writeln!(
        writer,
        "image,codec,quality,width,height,file_size,bpp,encode_ms,ssimulacra2"
    )?;

    // Process images with ETA
    let start_time = Instant::now();
    let mut total_results = 0;
    let mut encodes_done = 0;

    for (img_idx, (path, name)) in all_images.iter().enumerate() {
        // Load image
        let img = match image::open(&path) {
            Ok(img) => img,
            Err(e) => {
                println!("[{}/{}] {} SKIP ({})", img_idx + 1, image_count, name, e);
                continue;
            }
        };

        let (width, height) = img.dimensions();
        let rgb = img.to_rgb8();
        let pixels: Vec<u8> = rgb.clone().into_raw();

        // Create fast-ssim2 reference once per image (using [u8; 3] array format)
        let rgb_arr: Vec<[u8; 3]> = pixels.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        let ref_img = Img::new(rgb_arr.as_slice(), width as usize, height as usize);
        let ssim_ref = Ssimulacra2Reference::new(ref_img)
            .map_err(|e| anyhow::anyhow!("Failed to create SSIM2 reference: {}", e))?;

        let image_data = ImageData::RgbSlice {
            data: pixels,
            width: width as usize,
            height: height as usize,
        };

        let img_start = Instant::now();
        let mut img_results = 0;

        // Encode with each codec at each quality
        for codec in &codecs {
            let encode_fn = codec.encode_fn();
            let decode_fn = codec.decode_fn();

            for &quality in &quality_levels {
                let request = codec_eval::eval::session::EncodeRequest {
                    quality,
                    params: HashMap::new(),
                };

                let encode_start = Instant::now();
                let encoded = match encode_fn(&image_data, &request) {
                    Ok(data) => data,
                    Err(e) => {
                        if args.verbose {
                            eprintln!("  {} q{}: encode error: {}", codec.id(), quality as u32, e);
                        }
                        encodes_done += 1;
                        continue;
                    }
                };
                let encode_ms = encode_start.elapsed().as_millis();

                // Decode
                let decoded = match decode_fn(&encoded) {
                    Ok(data) => data,
                    Err(e) => {
                        if args.verbose {
                            eprintln!("  {} q{}: decode error: {}", codec.id(), quality as u32, e);
                        }
                        encodes_done += 1;
                        continue;
                    }
                };

                // Calculate SSIM2 using precomputed reference
                let decoded_rgb = decoded.to_rgb8_vec();
                let decoded_arr: Vec<[u8; 3]> = decoded_rgb
                    .chunks_exact(3)
                    .map(|c| [c[0], c[1], c[2]])
                    .collect();
                let decoded_img = Img::new(decoded_arr.as_slice(), width as usize, height as usize);

                let ssim2 = match ssim_ref.compare(decoded_img) {
                    Ok(score) => score,
                    Err(e) => {
                        if args.verbose {
                            eprintln!("  {} q{}: ssim2 error: {}", codec.id(), quality as u32, e);
                        }
                        encodes_done += 1;
                        continue;
                    }
                };

                let file_size = encoded.len();
                let bpp = (file_size * 8) as f64 / (width * height) as f64;

                writeln!(
                    writer,
                    "{},{},{},{},{},{},{:.6},{},{:.4}",
                    name,
                    codec.id(),
                    quality as u32,
                    width,
                    height,
                    file_size,
                    bpp,
                    encode_ms,
                    ssim2,
                )?;

                img_results += 1;
                encodes_done += 1;
            }
        }

        total_results += img_results;

        // Calculate ETA
        let elapsed = start_time.elapsed();
        let rate = encodes_done as f64 / elapsed.as_secs_f64();
        let remaining = total_encodes.saturating_sub(encodes_done);
        let eta = if rate > 0.0 {
            Duration::from_secs_f64(remaining as f64 / rate)
        } else {
            Duration::from_secs(0)
        };

        println!(
            "[{}/{}] {} ({} results in {:.1}s) | {}/{} encodes | ETA: {}",
            img_idx + 1,
            image_count,
            name,
            img_results,
            img_start.elapsed().as_secs_f64(),
            encodes_done,
            total_encodes,
            format_duration(eta),
        );
    }

    writer.flush()?;

    let elapsed = start_time.elapsed();
    println!("\nCompleted in {}", format_duration(elapsed));
    println!("Total results: {}", total_results);
    println!(
        "Throughput: {:.1} encodes/sec",
        total_results as f64 / elapsed.as_secs_f64()
    );
    println!("Output: {}", args.output.display());

    Ok(())
}
