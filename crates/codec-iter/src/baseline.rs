use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::eval::EvalPoint;

#[derive(Serialize, Deserialize)]
pub struct Baseline {
    pub format: String,
    pub config_summary: String,
    pub corpus_path: String,
    pub created_at: DateTime<Utc>,
    pub points: Vec<EvalPoint>,
}

pub fn baseline_path(baselines_dir: &Path, format: &str) -> PathBuf {
    baselines_dir.join(format!("{format}.json"))
}

pub fn load_baseline(baselines_dir: &Path, format: &str) -> Result<Option<Baseline>> {
    let path = baseline_path(baselines_dir, format);
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(&path)
        .with_context(|| format!("reading baseline from {}", path.display()))?;
    let baseline: Baseline =
        serde_json::from_str(&data).with_context(|| format!("parsing {}", path.display()))?;
    Ok(Some(baseline))
}

pub fn save_baseline(baselines_dir: &Path, baseline: &Baseline) -> Result<()> {
    fs::create_dir_all(baselines_dir)?;
    let path = baseline_path(baselines_dir, &baseline.format);
    let data = serde_json::to_string_pretty(baseline)?;
    fs::write(&path, data)?;
    eprintln!("Baseline saved to {}", path.display());
    Ok(())
}

pub struct ComparisonRow {
    pub quality: u8,
    pub bpp: f64,
    pub ssim2: f64,
    pub delta_bpp: f64,
    pub delta_ssim2: f64,
    pub pareto: f64,
}

pub fn compare_with_baseline(points: &[EvalPoint], baseline: &Baseline) -> Vec<ComparisonRow> {
    // Aggregate current results by quality (average across images)
    let current_by_q = aggregate_by_quality(points);
    let baseline_by_q = aggregate_by_quality(&baseline.points);

    let mut qualities: Vec<u8> = current_by_q.keys().copied().collect();
    qualities.sort_unstable();

    qualities
        .into_iter()
        .map(|q| {
            let (avg_bpp, avg_ssim2) = current_by_q[&q];

            let (delta_bpp, delta_ssim2) = baseline_by_q
                .get(&q)
                .map(|&(base_bpp, base_ssim2)| (avg_bpp - base_bpp, avg_ssim2 - base_ssim2))
                .unwrap_or((0.0, 0.0));

            // Pareto distance: positive = better (lower bpp + higher ssim2)
            // Scale factor 10.0 roughly balances BPP units vs SSIM2 units
            let pareto = delta_ssim2 - delta_bpp * 10.0;

            ComparisonRow {
                quality: q,
                bpp: avg_bpp,
                ssim2: avg_ssim2,
                delta_bpp,
                delta_ssim2,
                pareto,
            }
        })
        .collect()
}

fn aggregate_by_quality(points: &[EvalPoint]) -> HashMap<u8, (f64, f64)> {
    let mut acc: HashMap<u8, (Vec<f64>, Vec<f64>)> = HashMap::new();
    for p in points {
        let entry = acc.entry(p.quality).or_default();
        entry.0.push(p.bpp);
        entry.1.push(p.ssim2);
    }

    acc.into_iter()
        .map(|(q, (bpps, ssims))| {
            let n = bpps.len() as f64;
            let avg_bpp = bpps.iter().sum::<f64>() / n;
            let avg_ssim2 = ssims.iter().sum::<f64>() / n;
            (q, (avg_bpp, avg_ssim2))
        })
        .collect()
}
