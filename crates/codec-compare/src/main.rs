//! Multi-codec image comparison CLI.
//!
//! Compares image codecs across formats (JPEG, WebP, AVIF) with
//! statistical analysis and Pareto front visualization.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use codec_eval::corpus::Corpus;
use codec_eval::eval::{CorpusReport, ImageData};
use codec_eval::metrics::MetricConfig;
use codec_eval::viewing::ViewingCondition;
use image::GenericImageView;

use codec_compare::encoders::STANDARD_QUALITY_LEVELS;
use codec_compare::registry::{CodecRegistry, CompareConfig, FormatSelection};
use codec_compare::report::{Metric, ReportGenerator};

#[derive(Parser)]
#[command(name = "codec-compare")]
#[command(about = "Multi-codec image comparison with Pareto analysis")]
#[command(version)]
struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run comparison on a corpus of images
    Run {
        /// Path to image corpus directory
        #[arg(short, long)]
        corpus: PathBuf,

        /// Output directory for reports
        #[arg(short, long, default_value = "./reports")]
        output: PathBuf,

        /// Formats to compare (comma-separated: jpeg,webp,avif)
        #[arg(short, long, default_value = "all")]
        formats: String,

        /// Quality levels to test (comma-separated)
        #[arg(short, long)]
        quality: Option<String>,

        /// AVIF encoder speed (0-10, lower = slower/better)
        #[arg(long, default_value = "6")]
        avif_speed: u8,

        /// Primary metric for analysis
        #[arg(long, default_value = "ssimulacra2")]
        metric: MetricArg,

        /// Viewing condition preset
        #[arg(long, default_value = "desktop")]
        viewing: ViewingArg,

        /// Maximum number of images to process
        #[arg(long)]
        limit: Option<usize>,

        /// Use XYB color space for metrics (recommended for jpegli)
        #[arg(long)]
        xyb: bool,
    },

    /// Run comparison on a single image
    Single {
        /// Path to input image
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory for reports
        #[arg(short, long, default_value = "./reports")]
        output: PathBuf,

        /// Formats to compare
        #[arg(short, long, default_value = "all")]
        formats: String,

        /// Quality levels to test
        #[arg(short, long)]
        quality: Option<String>,

        /// AVIF encoder speed
        #[arg(long, default_value = "6")]
        avif_speed: u8,

        /// Use XYB color space for metrics
        #[arg(long)]
        xyb: bool,
    },

    /// List available codecs
    List,

    /// Generate report from existing JSON results
    Report {
        /// Path to corpus report JSON
        #[arg(short, long)]
        input: PathBuf,

        /// Output directory for charts and stats
        #[arg(short, long, default_value = "./reports")]
        output: PathBuf,

        /// Primary metric for analysis
        #[arg(long, default_value = "ssimulacra2")]
        metric: MetricArg,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum MetricArg {
    Ssimulacra2,
    Dssim,
    Butteraugli,
    Psnr,
}

impl From<MetricArg> for Metric {
    fn from(arg: MetricArg) -> Self {
        match arg {
            MetricArg::Ssimulacra2 => Metric::Ssimulacra2,
            MetricArg::Dssim => Metric::Dssim,
            MetricArg::Butteraugli => Metric::Butteraugli,
            MetricArg::Psnr => Metric::Psnr,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum ViewingArg {
    Desktop,
    Laptop,
    Smartphone,
}

impl From<ViewingArg> for ViewingCondition {
    fn from(arg: ViewingArg) -> Self {
        match arg {
            ViewingArg::Desktop => ViewingCondition::desktop(),
            ViewingArg::Laptop => ViewingCondition::laptop(),
            ViewingArg::Smartphone => ViewingCondition::smartphone(),
        }
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            corpus,
            output,
            formats,
            quality,
            avif_speed,
            metric,
            viewing,
            limit,
            xyb,
        } => {
            let format_selection = parse_formats(&formats)?;
            let quality_levels = parse_quality_levels(quality.as_deref())?;
            let metrics = if xyb {
                MetricConfig::perceptual_xyb()
            } else {
                MetricConfig::perceptual()
            };

            run_corpus(
                &corpus,
                &output,
                format_selection,
                &quality_levels,
                avif_speed,
                metric.into(),
                viewing.into(),
                metrics,
                limit,
                cli.verbose,
            )?;
        }

        Commands::Single {
            input,
            output,
            formats,
            quality,
            avif_speed,
            xyb,
        } => {
            let format_selection = parse_formats(&formats)?;
            let quality_levels = parse_quality_levels(quality.as_deref())?;
            let metrics = if xyb {
                MetricConfig::perceptual_xyb()
            } else {
                MetricConfig::perceptual()
            };

            run_single(
                &input,
                &output,
                format_selection,
                &quality_levels,
                avif_speed,
                metrics,
                cli.verbose,
            )?;
        }

        Commands::List => {
            list_codecs();
        }

        Commands::Report {
            input,
            output,
            metric,
        } => {
            generate_report(&input, &output, metric.into())?;
        }
    }

    Ok(())
}

fn parse_formats(s: &str) -> anyhow::Result<FormatSelection> {
    if s == "all" {
        return Ok(FormatSelection::all());
    }

    let mut selection = FormatSelection {
        jpeg: false,
        webp: false,
        avif: false,
    };

    for part in s.split(',') {
        match part.trim().to_lowercase().as_str() {
            "jpeg" | "jpg" => selection.jpeg = true,
            "webp" => selection.webp = true,
            "avif" => selection.avif = true,
            other => anyhow::bail!("Unknown format: {}. Use: jpeg, webp, avif", other),
        }
    }

    Ok(selection)
}

fn parse_quality_levels(s: Option<&str>) -> anyhow::Result<Vec<f64>> {
    match s {
        None => Ok(STANDARD_QUALITY_LEVELS.to_vec()),
        Some(s) => {
            let mut levels = Vec::new();
            for part in s.split(',') {
                let level: f64 = part.trim().parse()?;
                if !(0.0..=100.0).contains(&level) {
                    anyhow::bail!("Quality must be 0-100, got {}", level);
                }
                levels.push(level);
            }
            levels.sort_by(|a, b| a.partial_cmp(b).unwrap());
            Ok(levels)
        }
    }
}

fn run_corpus(
    corpus_path: &PathBuf,
    output: &PathBuf,
    formats: FormatSelection,
    quality_levels: &[f64],
    avif_speed: u8,
    metric: Metric,
    viewing: ViewingCondition,
    metrics: MetricConfig,
    limit: Option<usize>,
    verbose: bool,
) -> anyhow::Result<()> {
    println!("Discovering corpus at {}...", corpus_path.display());

    let corpus = Corpus::discover(corpus_path)?;
    let images = &corpus.images;

    let image_count = limit.unwrap_or(images.len()).min(images.len());
    println!(
        "Found {} images, processing {}...\n",
        images.len(),
        image_count
    );

    // Setup config
    let config = CompareConfig::new(output)
        .with_quality_levels(quality_levels.to_vec())
        .with_viewing(viewing)
        .with_metrics(metrics)
        .with_formats(formats)
        .with_avif_speed(avif_speed);

    // Create registry and register codecs
    let mut registry = CodecRegistry::new(config);
    registry.register_all();

    let registered = registry.registered_codecs();
    println!("Registered codecs: {}", registered.join(", "));
    println!("Quality levels: {:?}", quality_levels);
    println!();

    // Create corpus report
    let mut corpus_report = CorpusReport::new("codec-compare".to_string());

    // Process images
    for (i, corpus_image) in images.iter().take(image_count).enumerate() {
        let path = corpus_image.full_path(&corpus.root_path);
        let name = corpus_image.name();

        print!("[{}/{}] {}... ", i + 1, image_count, name);
        std::io::Write::flush(&mut std::io::stdout())?;

        // Load image
        let img = match image::open(&path) {
            Ok(img) => img,
            Err(e) => {
                println!("SKIP ({})", e);
                continue;
            }
        };

        let (width, height) = img.dimensions();
        let rgb = img.to_rgb8();
        let pixels: Vec<u8> = rgb.into_raw();

        let image_data = ImageData::RgbSlice {
            data: pixels,
            width: width as usize,
            height: height as usize,
        };

        // Evaluate
        match registry.evaluate_image(&name, image_data) {
            Ok(report) => {
                let result_count = report.results.len();
                println!("OK ({} results)", result_count);

                if verbose {
                    for r in &report.results {
                        let ssim = r
                            .metrics
                            .ssimulacra2
                            .map(|s| format!("{:.2}", s))
                            .unwrap_or("-".to_string());
                        println!(
                            "    {} q{}: {} bytes, SSIM2={}",
                            r.codec_id, r.quality as u32, r.file_size, ssim
                        );
                    }
                }

                registry.write_image_report(&report)?;
                corpus_report.images.push(report);
            }
            Err(e) => {
                println!("ERROR: {}", e);
            }
        }
    }

    // Write corpus report
    registry.write_corpus_report(&corpus_report)?;

    // Generate analysis
    println!("\nGenerating analysis...");
    let generator = ReportGenerator::new(output).with_metric(metric);
    let report = generator.generate(&corpus_report)?;

    // Print summary
    report.stats.print_summary();

    println!("\nReports written to {}", output.display());
    println!("  - pareto.svg: Rate-distortion Pareto chart");
    println!("  - pareto.json: Pareto front data");
    println!("  - stats.json: Comparison statistics");

    Ok(())
}

fn run_single(
    input: &PathBuf,
    output: &PathBuf,
    formats: FormatSelection,
    quality_levels: &[f64],
    avif_speed: u8,
    metrics: MetricConfig,
    verbose: bool,
) -> anyhow::Result<()> {
    println!("Processing {}...", input.display());

    // Load image
    let img = image::open(input)?;
    let (width, height) = img.dimensions();
    let rgb = img.to_rgb8();
    let pixels: Vec<u8> = rgb.into_raw();

    let image_data = ImageData::RgbSlice {
        data: pixels,
        width: width as usize,
        height: height as usize,
    };

    let name = input.file_stem().unwrap().to_string_lossy();

    // Setup config
    let config = CompareConfig::new(output)
        .with_quality_levels(quality_levels.to_vec())
        .with_viewing(ViewingCondition::desktop())
        .with_metrics(metrics)
        .with_formats(formats)
        .with_avif_speed(avif_speed);

    // Create registry and register codecs
    let mut registry = CodecRegistry::new(config);
    registry.register_all();

    let registered = registry.registered_codecs();
    println!("Registered codecs: {}", registered.join(", "));
    println!("Quality levels: {:?}\n", quality_levels);

    // Evaluate
    let report = registry.evaluate_image(&name, image_data)?;
    registry.write_image_report(&report)?;

    // Print results
    println!("\nResults for {}:", name);
    println!("{:-<90}", "");
    println!(
        "{:<15} {:>8} {:>10} {:>12} {:>12} {:>12} {:>12}",
        "Codec", "Quality", "Size", "BPP", "SSIM2", "DSSIM", "Time(ms)"
    );
    println!("{:-<90}", "");

    for result in &report.results {
        let ssim2 = result
            .metrics
            .ssimulacra2
            .map(|s| format!("{:.2}", s))
            .unwrap_or("-".to_string());
        let dssim = result
            .metrics
            .dssim
            .map(|d| format!("{:.6}", d))
            .unwrap_or("-".to_string());

        println!(
            "{:<15} {:>8.0} {:>10} {:>12.3} {:>12} {:>12} {:>12}",
            result.codec_id,
            result.quality,
            format_size(result.file_size),
            result.bits_per_pixel,
            ssim2,
            dssim,
            result.encode_time.as_millis()
        );
    }

    // Create minimal corpus report for chart generation
    let mut corpus_report = CorpusReport::new("codec-compare".to_string());
    corpus_report.images.push(report);

    // Generate chart
    let generator = ReportGenerator::new(output).with_metric(Metric::Ssimulacra2);
    generator.generate(&corpus_report)?;

    println!("\nReports written to {}", output.display());

    Ok(())
}

fn list_codecs() {
    println!("Available codecs:\n");

    println!("JPEG:");
    #[cfg(feature = "mozjpeg")]
    println!("  - mozjpeg: Mozilla's optimized JPEG encoder");
    #[cfg(not(feature = "mozjpeg"))]
    println!("  - mozjpeg: (not compiled - enable 'mozjpeg' feature)");

    #[cfg(feature = "jpegli")]
    println!("  - jpegli: Google's perceptually-optimized JPEG encoder");
    #[cfg(not(feature = "jpegli"))]
    println!("  - jpegli: (not compiled - enable 'jpegli' feature)");

    println!("\nWebP:");
    #[cfg(feature = "webp")]
    println!("  - webp: Google's WebP encoder (libwebp)");
    #[cfg(not(feature = "webp"))]
    println!("  - webp: (not compiled - enable 'webp' feature)");

    println!("\nAVIF:");
    #[cfg(feature = "avif")]
    {
        println!("  - avif-aom: AVIF with libaom (reference, best quality)");
        println!("  - avif-rav1e: AVIF with rav1e (Rust, balanced)");
        println!("  - avif-svt: AVIF with SVT-AV1 (Intel, fastest)");
    }
    #[cfg(not(feature = "avif"))]
    println!("  - avif-*: (not compiled - enable 'avif' feature)");

    println!("\nTo enable all codecs: cargo build --features all");
}

fn generate_report(input: &PathBuf, output: &PathBuf, metric: Metric) -> anyhow::Result<()> {
    println!("Loading corpus report from {}...", input.display());

    let json = std::fs::read_to_string(input)?;
    let corpus_report: CorpusReport = serde_json::from_str(&json)?;

    println!(
        "Found {} images with {} total results",
        corpus_report.images.len(),
        corpus_report
            .images
            .iter()
            .map(|i| i.results.len())
            .sum::<usize>()
    );

    let generator = ReportGenerator::new(output).with_metric(metric);
    let report = generator.generate(&corpus_report)?;

    report.stats.print_summary();

    println!("\nReports written to {}", output.display());

    Ok(())
}

fn format_size(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0)
    }
}
