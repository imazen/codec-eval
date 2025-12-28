//! Extract image characteristics for encoder prediction
//!
//! Computes various image statistics that may predict which encoder
//! will perform better.

use clap::Parser;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "image-heuristics")]
#[command(about = "Extract image characteristics for encoder prediction")]
struct Args {
    /// Directory containing images
    corpus_dir: PathBuf,

    /// Output CSV file
    #[arg(short, long, default_value = "image_heuristics.csv")]
    output: PathBuf,
}

#[derive(Debug, Clone)]
struct ImageHeuristics {
    image: String,
    width: usize,
    height: usize,
    pixels: usize,

    // Luminance statistics
    mean_luminance: f32,
    luminance_variance: f32,
    luminance_std: f32,

    // Edge/gradient statistics
    edge_strength_mean: f32,
    edge_strength_max: f32,
    edge_density: f32, // % of pixels with strong edges

    // Block statistics (8x8 blocks)
    flat_block_pct: f32,     // variance < 100
    low_var_block_pct: f32,  // variance < 500
    mid_var_block_pct: f32,  // variance 500-2000
    high_var_block_pct: f32, // variance 2000-5000
    detail_block_pct: f32,   // variance > 5000
    block_variance_mean: f32,
    block_variance_std: f32,

    // Color statistics
    color_variance: f32,  // variance across R,G,B channels
    saturation_mean: f32, // mean saturation
    saturation_std: f32,  // saturation variation

    // Frequency domain approximation (DCT-like features)
    high_freq_energy: f32, // estimate of high frequency content
    low_freq_energy: f32,  // estimate of low frequency content
    freq_ratio: f32,       // high/low ratio

    // Texture measures
    local_contrast_mean: f32, // mean of local contrast
    local_contrast_std: f32,  // variation in local contrast

    // Spatial complexity
    horizontal_complexity: f32,
    vertical_complexity: f32,
    diagonal_complexity: f32,
}

fn load_image(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgb8();
    let width = rgb.width() as usize;
    let height = rgb.height() as usize;
    Some((rgb.into_raw(), width, height))
}

fn compute_heuristics(
    rgb: &[u8],
    width: usize,
    height: usize,
    image_name: &str,
) -> ImageHeuristics {
    let pixels = width * height;

    // Convert to grayscale for luminance analysis
    let gray: Vec<f32> = rgb
        .chunks(3)
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect();

    // Luminance statistics
    let mean_luminance: f32 = gray.iter().sum::<f32>() / pixels as f32;
    let luminance_variance: f32 = gray
        .iter()
        .map(|&v| (v - mean_luminance).powi(2))
        .sum::<f32>()
        / pixels as f32;
    let luminance_std = luminance_variance.sqrt();

    // Edge detection (Sobel-like)
    let mut edge_strengths = Vec::with_capacity((width - 2) * (height - 2));
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            let gx = gray[idx + 1] - gray[idx - 1];
            let gy = gray[idx + width] - gray[idx - width];
            let strength = (gx * gx + gy * gy).sqrt();
            edge_strengths.push(strength);
        }
    }

    let edge_strength_mean =
        edge_strengths.iter().sum::<f32>() / edge_strengths.len().max(1) as f32;
    let edge_strength_max = edge_strengths.iter().cloned().fold(0.0f32, |a, b| a.max(b));
    let edge_density = edge_strengths.iter().filter(|&&e| e > 30.0).count() as f32
        / edge_strengths.len().max(1) as f32;

    // Block variance analysis (8x8 blocks)
    let blocks_x = width / 8;
    let blocks_y = height / 8;
    let mut block_variances = Vec::with_capacity(blocks_x * blocks_y);

    for by in 0..blocks_y {
        for bx in 0..blocks_x {
            let mut block_pixels = Vec::with_capacity(64);
            for dy in 0..8 {
                for dx in 0..8 {
                    let idx = (by * 8 + dy) * width + (bx * 8 + dx);
                    block_pixels.push(gray[idx]);
                }
            }
            let block_mean: f32 = block_pixels.iter().sum::<f32>() / 64.0;
            let block_var: f32 = block_pixels
                .iter()
                .map(|&v| (v - block_mean).powi(2))
                .sum::<f32>()
                / 64.0;
            block_variances.push(block_var);
        }
    }

    let num_blocks = block_variances.len().max(1) as f32;
    let flat_block_pct =
        100.0 * block_variances.iter().filter(|&&v| v < 100.0).count() as f32 / num_blocks;
    let low_var_block_pct =
        100.0 * block_variances.iter().filter(|&&v| v < 500.0).count() as f32 / num_blocks;
    let mid_var_block_pct = 100.0
        * block_variances
            .iter()
            .filter(|&&v| v >= 500.0 && v < 2000.0)
            .count() as f32
        / num_blocks;
    let high_var_block_pct = 100.0
        * block_variances
            .iter()
            .filter(|&&v| v >= 2000.0 && v < 5000.0)
            .count() as f32
        / num_blocks;
    let detail_block_pct =
        100.0 * block_variances.iter().filter(|&&v| v >= 5000.0).count() as f32 / num_blocks;

    let block_variance_mean = block_variances.iter().sum::<f32>() / num_blocks;
    let block_variance_std = (block_variances
        .iter()
        .map(|&v| (v - block_variance_mean).powi(2))
        .sum::<f32>()
        / num_blocks)
        .sqrt();

    // Color statistics
    let r_mean: f32 = rgb.chunks(3).map(|p| p[0] as f32).sum::<f32>() / pixels as f32;
    let g_mean: f32 = rgb.chunks(3).map(|p| p[1] as f32).sum::<f32>() / pixels as f32;
    let b_mean: f32 = rgb.chunks(3).map(|p| p[2] as f32).sum::<f32>() / pixels as f32;

    let r_var: f32 = rgb
        .chunks(3)
        .map(|p| (p[0] as f32 - r_mean).powi(2))
        .sum::<f32>()
        / pixels as f32;
    let g_var: f32 = rgb
        .chunks(3)
        .map(|p| (p[1] as f32 - g_mean).powi(2))
        .sum::<f32>()
        / pixels as f32;
    let b_var: f32 = rgb
        .chunks(3)
        .map(|p| (p[2] as f32 - b_mean).powi(2))
        .sum::<f32>()
        / pixels as f32;
    let color_variance = (r_var + g_var + b_var) / 3.0;

    // Saturation
    let saturations: Vec<f32> = rgb
        .chunks(3)
        .map(|p| {
            let max = p[0].max(p[1]).max(p[2]) as f32;
            let min = p[0].min(p[1]).min(p[2]) as f32;
            if max > 0.0 { (max - min) / max } else { 0.0 }
        })
        .collect();
    let saturation_mean = saturations.iter().sum::<f32>() / pixels as f32;
    let saturation_std = (saturations
        .iter()
        .map(|&s| (s - saturation_mean).powi(2))
        .sum::<f32>()
        / pixels as f32)
        .sqrt();

    // Frequency domain approximation (difference of adjacent pixels)
    let mut low_freq = 0.0f32;
    let mut high_freq = 0.0f32;
    for y in 0..height {
        for x in 0..width - 1 {
            let idx = y * width + x;
            let diff = (gray[idx + 1] - gray[idx]).abs();
            if diff < 10.0 {
                low_freq += 1.0;
            } else if diff > 30.0 {
                high_freq += 1.0;
            }
        }
    }
    let total_transitions = ((width - 1) * height) as f32;
    let low_freq_energy = low_freq / total_transitions;
    let high_freq_energy = high_freq / total_transitions;
    let freq_ratio = if low_freq_energy > 0.0 {
        high_freq_energy / low_freq_energy
    } else {
        high_freq_energy
    };

    // Local contrast (3x3 neighborhoods)
    let mut local_contrasts = Vec::new();
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            let mut min_val = gray[idx];
            let mut max_val = gray[idx];
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nidx = ((y as i32 + dy) as usize) * width + (x as i32 + dx) as usize;
                    min_val = min_val.min(gray[nidx]);
                    max_val = max_val.max(gray[nidx]);
                }
            }
            local_contrasts.push(max_val - min_val);
        }
    }
    let local_contrast_mean =
        local_contrasts.iter().sum::<f32>() / local_contrasts.len().max(1) as f32;
    let local_contrast_std = (local_contrasts
        .iter()
        .map(|&c| (c - local_contrast_mean).powi(2))
        .sum::<f32>()
        / local_contrasts.len().max(1) as f32)
        .sqrt();

    // Directional complexity
    let mut h_complexity = 0.0f32;
    let mut v_complexity = 0.0f32;
    let mut d_complexity = 0.0f32;

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            h_complexity += (gray[idx + 1] - gray[idx - 1]).abs();
            v_complexity += (gray[idx + width] - gray[idx - width]).abs();
            d_complexity += (gray[idx + width + 1] - gray[idx - width - 1]).abs();
        }
    }
    let n = ((width - 2) * (height - 2)) as f32;
    let horizontal_complexity = h_complexity / n;
    let vertical_complexity = v_complexity / n;
    let diagonal_complexity = d_complexity / n;

    ImageHeuristics {
        image: image_name.to_string(),
        width,
        height,
        pixels,
        mean_luminance,
        luminance_variance,
        luminance_std,
        edge_strength_mean,
        edge_strength_max,
        edge_density,
        flat_block_pct,
        low_var_block_pct,
        mid_var_block_pct,
        high_var_block_pct,
        detail_block_pct,
        block_variance_mean,
        block_variance_std,
        color_variance,
        saturation_mean,
        saturation_std,
        high_freq_energy,
        low_freq_energy,
        freq_ratio,
        local_contrast_mean,
        local_contrast_std,
        horizontal_complexity,
        vertical_complexity,
        diagonal_complexity,
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
            }
        }
    }
    images.sort();
    images
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    println!("=== Image Heuristics Extractor ===\n");
    println!("Corpus: {}", args.corpus_dir.display());
    println!("Output: {}\n", args.output.display());

    let images = find_images(&args.corpus_dir);
    println!("Found {} images\n", images.len());

    let mut results = Vec::new();

    for (i, path) in images.iter().enumerate() {
        eprint!(
            "\rProcessing {}/{}: {}",
            i + 1,
            images.len(),
            path.file_name().unwrap_or_default().to_string_lossy()
        );

        if let Some((rgb, width, height)) = load_image(path) {
            let image_name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let h = compute_heuristics(&rgb, width, height, &image_name);
            results.push(h);
        }
    }
    eprintln!("\rProcessed {} images", results.len());

    // Write CSV
    let mut file = std::fs::File::create(&args.output)?;
    writeln!(
        file,
        "image,width,height,pixels,\
         mean_luminance,luminance_variance,luminance_std,\
         edge_strength_mean,edge_strength_max,edge_density,\
         flat_block_pct,low_var_block_pct,mid_var_block_pct,high_var_block_pct,detail_block_pct,\
         block_variance_mean,block_variance_std,\
         color_variance,saturation_mean,saturation_std,\
         high_freq_energy,low_freq_energy,freq_ratio,\
         local_contrast_mean,local_contrast_std,\
         horizontal_complexity,vertical_complexity,diagonal_complexity"
    )?;

    for h in &results {
        writeln!(
            file,
            "{},{},{},{},{:.2},{:.2},{:.2},{:.2},{:.2},{:.4},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.2},{:.4},{:.4},{:.4},{:.4},{:.4},{:.2},{:.2},{:.2},{:.2},{:.2}",
            h.image,
            h.width,
            h.height,
            h.pixels,
            h.mean_luminance,
            h.luminance_variance,
            h.luminance_std,
            h.edge_strength_mean,
            h.edge_strength_max,
            h.edge_density,
            h.flat_block_pct,
            h.low_var_block_pct,
            h.mid_var_block_pct,
            h.high_var_block_pct,
            h.detail_block_pct,
            h.block_variance_mean,
            h.block_variance_std,
            h.color_variance,
            h.saturation_mean,
            h.saturation_std,
            h.high_freq_energy,
            h.low_freq_energy,
            h.freq_ratio,
            h.local_contrast_mean,
            h.local_contrast_std,
            h.horizontal_complexity,
            h.vertical_complexity,
            h.diagonal_complexity
        )?;
    }

    println!(
        "\nWrote {} results to {}",
        results.len(),
        args.output.display()
    );

    Ok(())
}
