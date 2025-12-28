//! Analyze image characteristics to understand encoder preferences

use std::path::{Path, PathBuf};

/// Get path to the codec-corpus directory.
///
/// # Panics
/// Panics if directory cannot be found.
fn get_corpus_dir() -> PathBuf {
    // Check environment variable first
    if let Ok(dir) = std::env::var("CODEC_CORPUS_DIR") {
        let path = PathBuf::from(dir);
        if path.exists() {
            return path;
        }
    }

    // Check relative to manifest dir
    if let Ok(manifest) = std::env::var("CARGO_MANIFEST_DIR") {
        let candidates = [
            PathBuf::from(&manifest).join("../../codec-corpus"),
            PathBuf::from(&manifest).join("../../../codec-corpus"),
            PathBuf::from(&manifest).join("codec-corpus"),
        ];
        for path in candidates {
            if path.exists() {
                return path;
            }
        }
    }

    panic!(
        "codec-corpus directory not found.\n\
         Set CODEC_CORPUS_DIR environment variable or clone:\n\
         git clone --depth 1 https://github.com/imazen/codec-corpus ../codec-corpus"
    );
}

fn load_image(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let img = image::open(path).ok()?;
    let rgb = img.to_rgb8();
    let width = rgb.width() as usize;
    let height = rgb.height() as usize;
    Some((rgb.into_raw(), width, height))
}

/// Compute image statistics
fn analyze(rgb: &[u8], width: usize, height: usize) -> ImageStats {
    let pixels = width * height;

    // Convert to grayscale for analysis
    let gray: Vec<f32> = rgb
        .chunks(3)
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
        .collect();

    // Global variance
    let mean: f32 = gray.iter().sum::<f32>() / pixels as f32;
    let variance: f32 = gray.iter().map(|&v| (v - mean).powi(2)).sum::<f32>() / pixels as f32;

    // Edge strength (Sobel-like)
    let mut edge_sum = 0.0f32;
    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = y * width + x;
            let gx = gray[idx + 1] - gray[idx - 1];
            let gy = gray[idx + width] - gray[idx - width];
            edge_sum += (gx * gx + gy * gy).sqrt();
        }
    }
    let edge_strength = edge_sum / ((width - 2) * (height - 2)) as f32;

    // Block variance (8x8 blocks)
    let mut block_variances = Vec::new();
    for by in 0..(height / 8) {
        for bx in 0..(width / 8) {
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

    // Flat block percentage (variance < 100)
    let flat_blocks = block_variances.iter().filter(|&&v| v < 100.0).count();
    let flat_percentage = 100.0 * flat_blocks as f32 / block_variances.len() as f32;

    // High detail block percentage (variance > 1000)
    let detail_blocks = block_variances.iter().filter(|&&v| v > 1000.0).count();
    let detail_percentage = 100.0 * detail_blocks as f32 / block_variances.len() as f32;

    // Variance of block variances (texture uniformity)
    let mean_block_var: f32 = block_variances.iter().sum::<f32>() / block_variances.len() as f32;
    let var_of_var: f32 = block_variances
        .iter()
        .map(|&v| (v - mean_block_var).powi(2))
        .sum::<f32>()
        / block_variances.len() as f32;

    ImageStats {
        mean_luminance: mean,
        global_variance: variance,
        edge_strength,
        flat_block_pct: flat_percentage,
        detail_block_pct: detail_percentage,
        texture_uniformity: var_of_var.sqrt(),
    }
}

#[derive(Debug)]
struct ImageStats {
    mean_luminance: f32,
    global_variance: f32,
    edge_strength: f32,
    flat_block_pct: f32,
    detail_block_pct: f32,
    texture_uniformity: f32,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    let images: Vec<std::path::PathBuf> = if args.len() > 1 {
        args[1..]
            .iter()
            .map(|s| std::path::PathBuf::from(s))
            .collect()
    } else {
        // Default Kodak outliers - find relative to manifest or via CODEC_CORPUS_DIR
        let corpus_dir = get_corpus_dir();
        vec!["22.png", "20.png", "10.png", "23.png", "12.png", "8.png"]
            .into_iter()
            .map(|name| corpus_dir.join("kodak").join(name))
            .collect()
    };

    println!(
        "{:>12} | {:>8} | {:>8} | {:>8} | {:>8} | {:>8} | {:>8}",
        "Image", "Mean", "Variance", "Edges", "Flat%", "Detail%", "TexUnif"
    );
    println!("{}", "-".repeat(85));

    for path in images {
        if let Some((rgb, width, height)) = load_image(&path) {
            let stats = analyze(&rgb, width, height);
            println!(
                "{:>12} | {:>8.1} | {:>8.1} | {:>8.2} | {:>8.1} | {:>8.1} | {:>8.1}",
                path.file_name().unwrap_or_default().to_string_lossy(),
                stats.mean_luminance,
                stats.global_variance,
                stats.edge_strength,
                stats.flat_block_pct,
                stats.detail_block_pct,
                stats.texture_uniformity
            );
        }
    }
}
