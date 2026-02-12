#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use zencodecs::config::jpeg::ChromaSubsampling;

mod baseline;
mod config;
mod eval;
mod source;
mod sweep;

#[derive(Parser)]
#[command(name = "codec-iter")]
#[command(about = "Fast codec iteration tool for encoder development")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Evaluate a codec configuration against stored baseline
    Eval {
        #[arg(long, default_value = "jpeg")]
        format: String,

        /// Corpus directory containing source images
        #[arg(long, default_value = "~/work/codec-corpus/CID22/CID22-512/training")]
        corpus: PathBuf,

        /// Number of images to evaluate (3=tiny, 5=small, 15=medium)
        #[arg(long, default_value_t = 3)]
        limit: usize,

        /// Quality preset
        #[arg(long, default_value = "quick")]
        quality: QualityPreset,

        /// Chroma subsampling: 420, 444, 422
        #[arg(long)]
        subsampling: Option<String>,

        /// Use XYB color space instead of YCbCr
        #[arg(long)]
        xyb: bool,

        /// Directory for baseline files
        #[arg(long, default_value = "./baselines")]
        baselines_dir: PathBuf,

        /// Force save current results as baseline
        #[arg(long)]
        save_baseline: bool,
    },

    /// Sweep over configuration matrix
    Sweep {
        #[arg(long, default_value = "jpeg")]
        format: String,

        #[arg(long, default_value = "~/work/codec-corpus/CID22/CID22-512/training")]
        corpus: PathBuf,

        #[arg(long, default_value_t = 3)]
        limit: usize,

        #[arg(long, default_value = "quick")]
        quality: QualityPreset,

        /// Comma-separated subsampling modes: 420,444
        #[arg(long)]
        subsampling: Option<String>,

        /// Comma-separated XYB modes: on,off
        #[arg(long)]
        xyb: Option<String>,
    },

    /// Manage baselines
    Baseline {
        #[command(subcommand)]
        action: BaselineAction,
    },
}

#[derive(Subcommand)]
enum BaselineAction {
    /// Save current results as baseline
    Save {
        #[arg(long, default_value = "jpeg")]
        format: String,

        #[arg(long, default_value = "~/work/codec-corpus/CID22/CID22-512/training")]
        corpus: PathBuf,

        #[arg(long, default_value_t = 3)]
        limit: usize,

        #[arg(long, default_value = "quick")]
        quality: QualityPreset,

        #[arg(long)]
        subsampling: Option<String>,

        #[arg(long)]
        xyb: bool,

        #[arg(long, default_value = "./baselines")]
        baselines_dir: PathBuf,
    },

    /// Show stored baseline
    Show {
        #[arg(long, default_value = "jpeg")]
        format: String,

        #[arg(long, default_value = "./baselines")]
        baselines_dir: PathBuf,
    },
}

#[derive(Clone, Debug, ValueEnum)]
enum QualityPreset {
    Quick,
    Standard,
    Dense,
}

impl std::fmt::Display for QualityPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Quick => write!(f, "quick"),
            Self::Standard => write!(f, "standard"),
            Self::Dense => write!(f, "dense"),
        }
    }
}

fn quality_levels(preset: &QualityPreset) -> Vec<u8> {
    match preset {
        QualityPreset::Quick => vec![75, 85, 95],
        QualityPreset::Standard => vec![50, 60, 70, 75, 80, 85, 90, 95],
        QualityPreset::Dense => (50..=98).step_by(2).map(|q| q as u8).collect(),
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(format!("{home}/{rest}"));
        }
    }
    path.to_path_buf()
}

fn build_jpeg_config(subsampling: Option<&str>, xyb: bool) -> config::JpegConfig {
    let sub = subsampling
        .and_then(config::parse_subsampling)
        .unwrap_or(ChromaSubsampling::Quarter);

    config::JpegConfig {
        subsampling: sub,
        xyb,
        ..config::JpegConfig::default()
    }
}

fn print_eval_results(
    result: &eval::EvalResult,
    baseline_opt: Option<&baseline::Baseline>,
    n_images: usize,
    n_qualities: usize,
) {
    println!(
        "codec-iter -- jpeg/{} ({n_images} images, {n_qualities} qualities, {}ms)\n",
        result.config_summary, result.total_ms
    );

    if let Some(bl) = baseline_opt {
        let rows = baseline::compare_with_baseline(&result.points, bl);

        println!(
            "  {:>3}  {:>8}  {:>7}  {:>8}  {:>8}  {:>7}",
            "Q", "BPP", "SSIM2", "d BPP", "d SSIM2", "Pareto"
        );
        println!("  {}", "-".repeat(52));

        for row in &rows {
            println!(
                "  {:>3}  {:>8.3}  {:>7.1}  {:>+8.3}  {:>+8.2}  {:>+7.2}",
                row.quality, row.bpp, row.ssim2, row.delta_bpp, row.delta_ssim2, row.pareto
            );
        }

        let avg_pareto = if rows.is_empty() {
            0.0
        } else {
            rows.iter().map(|r| r.pareto).sum::<f64>() / rows.len() as f64
        };
        let direction = if avg_pareto > 0.01 {
            "BETTER"
        } else if avg_pareto < -0.01 {
            "WORSE"
        } else {
            "SAME"
        };

        println!(
            "\n  Overall: {:+.2} avg Pareto distance ({direction})",
            avg_pareto
        );
        println!(
            "  Baseline: {} ({})",
            bl.created_at.format("%Y-%m-%d"),
            bl.config_summary
        );
    } else {
        // No baseline, just show raw results
        println!(
            "  {:>3}  {:>8}  {:>7}  {:>8}  {:>6}",
            "Q", "BPP", "SSIM2", "Bytes", "Enc ms"
        );
        println!("  {}", "-".repeat(42));

        // Aggregate by quality
        let mut by_q: std::collections::BTreeMap<u8, (Vec<f64>, Vec<f64>, Vec<usize>, Vec<u64>)> =
            Default::default();
        for p in &result.points {
            let entry = by_q.entry(p.quality).or_default();
            entry.0.push(p.bpp);
            entry.1.push(p.ssim2);
            entry.2.push(p.size_bytes);
            entry.3.push(p.encode_ms);
        }

        for (q, (bpps, ssims, sizes, times)) in &by_q {
            let n = bpps.len() as f64;
            let avg_bpp = bpps.iter().sum::<f64>() / n;
            let avg_ssim2 = ssims.iter().sum::<f64>() / n;
            let avg_size = sizes.iter().sum::<usize>() / sizes.len();
            let avg_time = times.iter().sum::<u64>() / times.len() as u64;
            println!(
                "  {:>3}  {:>8.3}  {:>7.1}  {:>8}  {:>6}",
                q, avg_bpp, avg_ssim2, avg_size, avg_time
            );
        }

        println!("\n  (no baseline found, showing raw results)");
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Eval {
            format,
            corpus,
            limit,
            quality,
            subsampling,
            xyb,
            baselines_dir,
            save_baseline: force_save,
        } => {
            let corpus = expand_tilde(&corpus);
            let jpeg_config = build_jpeg_config(subsampling.as_deref(), xyb);
            let quality_levels = quality_levels(&quality);

            eprintln!(
                "Loading {limit} images from {}...",
                corpus.display()
            );
            let images = source::load_sources(&corpus, limit)?;
            eprintln!("Loaded {} images", images.len());

            eprintln!(
                "Evaluating {} ({} quality levels)...",
                config::config_summary(&jpeg_config),
                quality_levels.len()
            );
            let result = eval::run_eval(&images, &jpeg_config, &quality_levels)?;

            // Load or auto-create baseline
            let bl = baseline::load_baseline(&baselines_dir, &format)?;

            if bl.is_none() || force_save {
                let new_baseline = baseline::Baseline {
                    format: format.clone(),
                    config_summary: result.config_summary.clone(),
                    corpus_path: corpus.to_string_lossy().to_string(),
                    created_at: Utc::now(),
                    points: result.points.clone(),
                };
                baseline::save_baseline(&baselines_dir, &new_baseline)?;

                if bl.is_none() {
                    eprintln!("Auto-saved as baseline (first run)");
                }
            }

            println!();
            print_eval_results(&result, bl.as_ref(), images.len(), quality_levels.len());
        }

        Commands::Sweep {
            format: _,
            corpus,
            limit,
            quality,
            subsampling,
            xyb,
        } => {
            let corpus = expand_tilde(&corpus);
            let quality_levels = quality_levels(&quality);

            let subsamplings: Vec<ChromaSubsampling> = subsampling
                .as_deref()
                .unwrap_or("420,444")
                .split(',')
                .filter_map(|s| config::parse_subsampling(s.trim()))
                .collect();

            let xyb_modes: Vec<bool> = xyb
                .as_deref()
                .unwrap_or("off")
                .split(',')
                .map(|s| matches!(s.trim(), "on" | "true" | "yes" | "1"))
                .collect();

            eprintln!(
                "Loading {limit} images from {}...",
                corpus.display()
            );
            let images = source::load_sources(&corpus, limit)?;
            eprintln!("Loaded {} images", images.len());

            let sweep_config = sweep::SweepConfig {
                subsamplings,
                xyb_modes,
            };

            eprintln!(
                "Sweeping {} configs...",
                sweep_config.subsamplings.len() * sweep_config.xyb_modes.len()
            );
            let result = sweep::run_sweep(&images, &sweep_config, &quality_levels)?;

            println!();
            sweep::print_sweep_results(&result, images.len(), quality_levels.len());
        }

        Commands::Baseline { action } => match action {
            BaselineAction::Save {
                format,
                corpus,
                limit,
                quality,
                subsampling,
                xyb,
                baselines_dir,
            } => {
                let corpus = expand_tilde(&corpus);
                let jpeg_config = build_jpeg_config(subsampling.as_deref(), xyb);
                let quality_levels = quality_levels(&quality);

                eprintln!(
                    "Loading {limit} images from {}...",
                    corpus.display()
                );
                let images = source::load_sources(&corpus, limit)?;

                eprintln!("Evaluating for baseline...");
                let result = eval::run_eval(&images, &jpeg_config, &quality_levels)?;

                let bl = baseline::Baseline {
                    format,
                    config_summary: result.config_summary.clone(),
                    corpus_path: corpus.to_string_lossy().to_string(),
                    created_at: Utc::now(),
                    points: result.points,
                };
                baseline::save_baseline(&baselines_dir, &bl)?;
            }

            BaselineAction::Show {
                format,
                baselines_dir,
            } => {
                match baseline::load_baseline(&baselines_dir, &format)? {
                    Some(bl) => {
                        println!("Baseline: {}", bl.config_summary);
                        println!("Created:  {}", bl.created_at.format("%Y-%m-%d %H:%M:%S"));
                        println!("Corpus:   {}", bl.corpus_path);
                        println!("Points:   {}", bl.points.len());
                        println!();

                        // Show aggregated by quality
                        let mut by_q: std::collections::BTreeMap<u8, (Vec<f64>, Vec<f64>)> =
                            Default::default();
                        for p in &bl.points {
                            let entry = by_q.entry(p.quality).or_default();
                            entry.0.push(p.bpp);
                            entry.1.push(p.ssim2);
                        }

                        println!("  {:>3}  {:>8}  {:>7}", "Q", "BPP", "SSIM2");
                        println!("  {}", "-".repeat(22));
                        for (q, (bpps, ssims)) in &by_q {
                            let n = bpps.len() as f64;
                            println!(
                                "  {:>3}  {:>8.3}  {:>7.1}",
                                q,
                                bpps.iter().sum::<f64>() / n,
                                ssims.iter().sum::<f64>() / n
                            );
                        }
                    }
                    None => {
                        println!(
                            "No baseline found for '{format}' in {}",
                            baselines_dir.display()
                        );
                    }
                }
            }
        },
    }

    Ok(())
}
