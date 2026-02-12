use anyhow::Result;
use zencodecs::config::jpeg::ChromaSubsampling;

use crate::config::JpegConfig;
use crate::eval::{self, EvalResult};
use crate::source::SourceImage;

pub struct SweepConfig {
    pub subsamplings: Vec<ChromaSubsampling>,
    pub xyb_modes: Vec<bool>,
}

pub struct SweepResult {
    pub configs: Vec<(String, EvalResult)>,
}

pub fn run_sweep(
    images: &[SourceImage],
    sweep: &SweepConfig,
    quality_levels: &[u8],
) -> Result<SweepResult> {
    let mut configs = Vec::new();

    for &subsampling in &sweep.subsamplings {
        for &xyb in &sweep.xyb_modes {
            let config = JpegConfig {
                subsampling,
                xyb,
                ..JpegConfig::default()
            };

            let summary = crate::config::config_summary(&config);
            eprintln!("  Evaluating {summary}...");
            let result = eval::run_eval(images, &config, quality_levels)?;
            configs.push((summary, result));
        }
    }

    Ok(SweepResult { configs })
}

pub fn print_sweep_results(result: &SweepResult, n_images: usize, n_qualities: usize) {
    println!(
        "codec-iter sweep -- jpeg ({n_images} images, {n_qualities} qualities, {} configs)\n",
        result.configs.len()
    );
    println!(
        "  {:<40} {:>8} {:>10} {:>8}",
        "Config", "Avg BPP", "Avg SSIM2", "Time"
    );
    println!("  {}", "-".repeat(70));

    // Compute averages for each config
    let mut rows: Vec<(String, f64, f64, u64)> = result
        .configs
        .iter()
        .map(|(name, r)| {
            let n = r.points.len() as f64;
            let avg_bpp = r.points.iter().map(|p| p.bpp).sum::<f64>() / n;
            let avg_ssim2 = r.points.iter().map(|p| p.ssim2).sum::<f64>() / n;
            (name.clone(), avg_bpp, avg_ssim2, r.total_ms)
        })
        .collect();

    // Sort by avg SSIM2 descending (best first)
    rows.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));

    for (i, (name, avg_bpp, avg_ssim2, ms)) in rows.iter().enumerate() {
        let marker = if i == 0 { " *" } else { "" };
        println!(
            "  {:<40} {:>8.3} {:>10.1} {:>6}ms{}",
            name, avg_bpp, avg_ssim2, ms, marker
        );
    }

    println!("\n  * = best avg SSIM2");
}
