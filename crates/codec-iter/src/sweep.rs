use anyhow::Result;

use crate::eval::{self, Codec, EvalResult};
use crate::source::SourceImage;

pub struct SweepResult {
    pub configs: Vec<(String, EvalResult)>,
}

/// Run eval across multiple codec configurations.
///
/// Each entry in `codecs` is a (name, Codec) pair. The name is used for display.
pub fn run_sweep(
    images: &[SourceImage],
    codecs: &[Codec],
    quality_levels: &[u8],
    use_gpu: bool,
) -> Result<SweepResult> {
    let mut configs = Vec::new();

    for codec in codecs {
        eprintln!("  Evaluating {}...", codec.summary);
        let result = eval::run_eval(images, codec, quality_levels, use_gpu)?;
        configs.push((codec.summary.clone(), result));
    }

    Ok(SweepResult { configs })
}

pub fn print_sweep_results(result: &SweepResult, n_images: usize, n_qualities: usize) {
    println!(
        "codec-iter sweep -- ({n_images} images, {n_qualities} qualities, {} configs)\n",
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
