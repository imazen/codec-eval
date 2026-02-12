#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand, ValueEnum};
use zencodecs::config::jpeg::ChromaSubsampling;

#[cfg(feature = "avif")]
mod avif_config;
mod baseline;
mod config;
mod eval;
#[cfg(feature = "gpu")]
mod gpu;
mod source;
mod sweep;

use eval::Codec;

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
        /// Codec format: jpeg or avif
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

        /// Chroma subsampling: 420, 444, 422 (JPEG only)
        #[arg(long)]
        subsampling: Option<String>,

        /// Use XYB color space instead of YCbCr (JPEG only)
        #[arg(long)]
        xyb: bool,

        /// AVIF preset: baseline, qm, qm-rdotx, qm-cdef-rdotx
        #[arg(long, default_value = "qm")]
        avif_preset: String,

        /// AVIF encoder speed (1=slowest/best, 10=fastest)
        #[arg(long)]
        avif_speed: Option<u8>,

        /// Force 8-bit depth for AVIF (default: auto/10-bit)
        #[arg(long)]
        avif_8bit: bool,

        /// Use GPU-accelerated SSIM2 (requires CUDA, ~4x faster)
        #[arg(long)]
        gpu: bool,

        /// Directory for baseline files
        #[arg(long, default_value = "./baselines")]
        baselines_dir: PathBuf,

        /// Force save current results as baseline
        #[arg(long)]
        save_baseline: bool,
    },

    /// Sweep over configuration matrix
    Sweep {
        /// Codec format: jpeg or avif
        #[arg(long, default_value = "jpeg")]
        format: String,

        #[arg(long, default_value = "~/work/codec-corpus/CID22/CID22-512/training")]
        corpus: PathBuf,

        #[arg(long, default_value_t = 3)]
        limit: usize,

        #[arg(long, default_value = "quick")]
        quality: QualityPreset,

        /// Comma-separated subsampling modes: 420,444 (JPEG only)
        #[arg(long)]
        subsampling: Option<String>,

        /// Comma-separated XYB modes: on,off (JPEG only)
        #[arg(long)]
        xyb: Option<String>,

        /// Comma-separated AVIF presets: qm,qm-rdotx (AVIF only)
        #[arg(long)]
        avif_presets: Option<String>,

        /// AVIF encoder speed (1=slowest/best, 10=fastest)
        #[arg(long)]
        avif_speed: Option<u8>,

        /// Force 8-bit depth for AVIF (default: auto/10-bit)
        #[arg(long)]
        avif_8bit: bool,

        /// Use GPU-accelerated SSIM2 (requires CUDA, ~4x faster)
        #[arg(long)]
        gpu: bool,
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

        /// AVIF preset (for avif format)
        #[arg(long, default_value = "qm")]
        avif_preset: String,

        /// AVIF encoder speed
        #[arg(long)]
        avif_speed: Option<u8>,

        /// Force 8-bit depth for AVIF
        #[arg(long)]
        avif_8bit: bool,

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

fn build_jpeg_codec(jpeg_config: config::JpegConfig) -> Codec {
    let summary = config::config_summary(&jpeg_config);
    Codec {
        encode: Box::new(move |img, quality| {
            use zencodecs::{EncodeRequest, ImageFormat};
            let codec_config = config::build_codec_config(&jpeg_config, quality);
            let encoded = EncodeRequest::new(ImageFormat::Jpeg)
                .with_codec_config(&codec_config)
                .encode_rgb8(img)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(encoded.bytes().to_vec())
        }),
        decode: Box::new(|data| {
            use zencodecs::DecodeRequest;
            let decoded = DecodeRequest::new(data)
                .decode()
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Ok(decoded.into_rgb8())
        }),
        summary,
    }
}

fn print_eval_results(
    result: &eval::EvalResult,
    baseline_opt: Option<&baseline::Baseline>,
    format_name: &str,
    n_images: usize,
    n_qualities: usize,
) {
    println!(
        "codec-iter -- {format_name}/{} ({n_images} images, {n_qualities} qualities, {}ms)\n",
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

#[allow(unused_variables)] // AVIF args only used with avif feature
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
            avif_preset,
            avif_speed,
            avif_8bit,
            gpu,
            baselines_dir,
            save_baseline: force_save,
        } => {
            let corpus = expand_tilde(&corpus);
            let quality_levels = quality_levels(&quality);

            eprintln!("Loading {limit} images from {}...", corpus.display());
            let images = source::load_sources(&corpus, limit)?;
            eprintln!("Loaded {} images", images.len());

            let (codec, baseline_key) = match format.as_str() {
                "jpeg" => {
                    let jpeg_config = build_jpeg_config(subsampling.as_deref(), xyb);
                    let codec = build_jpeg_codec(jpeg_config);
                    (codec, "jpeg".to_string())
                }
                #[cfg(feature = "avif")]
                "avif" => {
                    let mut avif_cfg = avif_config::AvifConfig::from_preset(&avif_preset)?;
                    if let Some(speed) = avif_speed {
                        avif_cfg.speed = speed;
                    }
                    if avif_8bit {
                        avif_cfg.bit_depth_8 = true;
                    }
                    let key = format!("avif-{avif_preset}");
                    let codec = avif_config::build_avif_codec(&avif_cfg);
                    (codec, key)
                }
                #[cfg(not(feature = "avif"))]
                "avif" => {
                    anyhow::bail!(
                        "AVIF support requires the 'avif' feature. Build with: cargo build --features avif"
                    );
                }
                other => anyhow::bail!("Unknown format: {other}. Supported: jpeg, avif"),
            };

            eprintln!(
                "Evaluating {} ({} quality levels)...",
                codec.summary,
                quality_levels.len()
            );
            let result = eval::run_eval(&images, &codec, &quality_levels, gpu)?;

            // Load or auto-create baseline
            let bl = baseline::load_baseline(&baselines_dir, &baseline_key)?;

            if bl.is_none() || force_save {
                let new_baseline = baseline::Baseline {
                    format: baseline_key.clone(),
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
            print_eval_results(
                &result,
                bl.as_ref(),
                &format,
                images.len(),
                quality_levels.len(),
            );
        }

        Commands::Sweep {
            format,
            corpus,
            limit,
            quality,
            subsampling,
            xyb,
            avif_presets,
            avif_speed,
            avif_8bit,
            gpu,
        } => {
            let corpus = expand_tilde(&corpus);
            let quality_levels = quality_levels(&quality);

            eprintln!("Loading {limit} images from {}...", corpus.display());
            let images = source::load_sources(&corpus, limit)?;
            eprintln!("Loaded {} images", images.len());

            let codecs: Vec<Codec> = match format.as_str() {
                "jpeg" => {
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

                    let mut codecs = Vec::new();
                    for &sub in &subsamplings {
                        for &xyb_mode in &xyb_modes {
                            let cfg = config::JpegConfig {
                                subsampling: sub,
                                xyb: xyb_mode,
                                ..config::JpegConfig::default()
                            };
                            codecs.push(build_jpeg_codec(cfg));
                        }
                    }
                    codecs
                }
                #[cfg(feature = "avif")]
                "avif" => {
                    let preset_names: Vec<&str> = avif_presets
                        .as_deref()
                        .unwrap_or("baseline,qm,qm-rdotx")
                        .split(',')
                        .map(|s| s.trim())
                        .collect();

                    let mut codecs = Vec::new();
                    for preset_name in preset_names {
                        let mut cfg = avif_config::AvifConfig::from_preset(preset_name)?;
                        if let Some(speed) = avif_speed {
                            cfg.speed = speed;
                        }
                        if avif_8bit {
                            cfg.bit_depth_8 = true;
                        }
                        codecs.push(avif_config::build_avif_codec(&cfg));
                    }
                    codecs
                }
                #[cfg(not(feature = "avif"))]
                "avif" => {
                    anyhow::bail!(
                        "AVIF support requires the 'avif' feature. Build with: cargo build --features avif"
                    );
                }
                other => anyhow::bail!("Unknown format: {other}. Supported: jpeg, avif"),
            };

            eprintln!("Sweeping {} configs...", codecs.len());
            let result = sweep::run_sweep(&images, &codecs, &quality_levels, gpu)?;

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
                avif_preset,
                avif_speed,
                avif_8bit,
                baselines_dir,
            } => {
                let corpus = expand_tilde(&corpus);
                let quality_levels = quality_levels(&quality);

                eprintln!("Loading {limit} images from {}...", corpus.display());
                let images = source::load_sources(&corpus, limit)?;

                let (codec, baseline_key) = match format.as_str() {
                    "jpeg" => {
                        let jpeg_config = build_jpeg_config(subsampling.as_deref(), xyb);
                        let codec = build_jpeg_codec(jpeg_config);
                        (codec, "jpeg".to_string())
                    }
                    #[cfg(feature = "avif")]
                    "avif" => {
                        let mut avif_cfg = avif_config::AvifConfig::from_preset(&avif_preset)?;
                        if let Some(speed) = avif_speed {
                            avif_cfg.speed = speed;
                        }
                        if avif_8bit {
                            avif_cfg.bit_depth_8 = true;
                        }
                        let key = format!("avif-{avif_preset}");
                        let codec = avif_config::build_avif_codec(&avif_cfg);
                        (codec, key)
                    }
                    #[cfg(not(feature = "avif"))]
                    "avif" => {
                        anyhow::bail!(
                            "AVIF support requires the 'avif' feature. Build with: cargo build --features avif"
                        );
                    }
                    other => anyhow::bail!("Unknown format: {other}. Supported: jpeg, avif"),
                };

                eprintln!("Evaluating for baseline...");
                let result = eval::run_eval(&images, &codec, &quality_levels, false)?;

                let bl = baseline::Baseline {
                    format: baseline_key,
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
