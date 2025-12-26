//! Report generation with Pareto charts and statistics.

use std::collections::HashMap;
use std::fs;

use codec_eval::eval::CorpusReport;
use codec_eval::stats::chart::{ChartConfig, ChartPoint, ChartSeries, generate_svg};
use codec_eval::stats::{ParetoFront, RDPoint, bd_rate};

use crate::Result;
use crate::encoders::codec_color;

/// Generate a comprehensive comparison report.
pub struct ReportGenerator {
    /// Primary metric for Pareto analysis.
    pub primary_metric: Metric,
    /// Output directory.
    pub output_dir: std::path::PathBuf,
}

/// Which metric to use for analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    Ssimulacra2,
    Dssim,
    Butteraugli,
    Psnr,
}

impl Metric {
    /// Get metric name for display.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Ssimulacra2 => "SSIMULACRA2",
            Self::Dssim => "DSSIM",
            Self::Butteraugli => "Butteraugli",
            Self::Psnr => "PSNR",
        }
    }

    /// Whether lower values are better for this metric.
    pub fn lower_is_better(&self) -> bool {
        matches!(self, Self::Dssim | Self::Butteraugli)
    }

    /// Get Y-axis label.
    pub fn y_label(&self) -> &'static str {
        match self {
            Self::Ssimulacra2 => "SSIMULACRA2 Score",
            Self::Dssim => "DSSIM (lower is better)",
            Self::Butteraugli => "Butteraugli Distance",
            Self::Psnr => "PSNR (dB)",
        }
    }
}

impl Default for ReportGenerator {
    fn default() -> Self {
        Self {
            primary_metric: Metric::Ssimulacra2,
            output_dir: std::path::PathBuf::from("./reports"),
        }
    }
}

impl ReportGenerator {
    /// Create a new report generator.
    pub fn new(output_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            ..Default::default()
        }
    }

    /// Set the primary metric.
    pub fn with_metric(mut self, metric: Metric) -> Self {
        self.primary_metric = metric;
        self
    }

    /// Generate all reports from a corpus report.
    pub fn generate(&self, corpus: &CorpusReport) -> Result<GeneratedReport> {
        fs::create_dir_all(&self.output_dir)?;

        // Extract RD points
        let rd_points = self.extract_rd_points(corpus);

        // Compute Pareto front
        let pareto = ParetoFront::compute(&rd_points);

        // Generate charts
        let chart_path = self.output_dir.join("pareto.svg");
        let svg = self.generate_pareto_chart(&rd_points)?;
        fs::write(&chart_path, &svg)?;

        // Generate per-format charts
        self.generate_format_charts(&rd_points)?;

        // Compute statistics
        let stats = self.compute_statistics(corpus, &pareto);

        // Write stats JSON
        let stats_path = self.output_dir.join("stats.json");
        let stats_json = serde_json::to_string_pretty(&stats)?;
        fs::write(&stats_path, stats_json)?;

        // Write Pareto JSON
        let pareto_path = self.output_dir.join("pareto.json");
        let pareto_json = serde_json::to_string_pretty(&pareto)?;
        fs::write(&pareto_path, pareto_json)?;

        Ok(GeneratedReport {
            pareto_chart_path: chart_path,
            stats,
            pareto,
        })
    }

    /// Extract RD points from corpus report.
    fn extract_rd_points(&self, corpus: &CorpusReport) -> Vec<RDPoint> {
        let mut points = Vec::new();

        for image in &corpus.images {
            for result in &image.results {
                let quality = match self.primary_metric {
                    Metric::Ssimulacra2 => result.metrics.ssimulacra2,
                    Metric::Dssim => result.metrics.dssim.map(|d| -d), // negate for "higher is better"
                    Metric::Butteraugli => result.metrics.butteraugli.map(|b| -b),
                    Metric::Psnr => result.metrics.psnr,
                };

                if let Some(q) = quality {
                    points.push(RDPoint {
                        codec: result.codec_id.clone(),
                        quality_setting: result.quality,
                        bpp: result.bits_per_pixel,
                        quality: q,
                        encode_time_ms: Some(result.encode_time.as_millis() as f64),
                        image: Some(image.name.clone()),
                    });
                }
            }
        }

        points
    }

    /// Generate the main Pareto chart.
    fn generate_pareto_chart(&self, points: &[RDPoint]) -> Result<String> {
        let mut by_codec: HashMap<&str, Vec<&RDPoint>> = HashMap::new();
        for p in points {
            by_codec.entry(&p.codec).or_default().push(p);
        }

        let mut series = Vec::new();
        for (codec, pts) in &by_codec {
            // Average points at each quality level
            let mut by_quality: HashMap<u32, Vec<&RDPoint>> = HashMap::new();
            for p in pts {
                by_quality
                    .entry(p.quality_setting as u32)
                    .or_default()
                    .push(*p);
            }

            let mut chart_points: Vec<ChartPoint> = by_quality
                .iter()
                .map(|(q, pts): (&u32, &Vec<&RDPoint>)| {
                    let avg_bpp = pts.iter().map(|p| p.bpp).sum::<f64>() / pts.len() as f64;
                    let avg_quality = pts.iter().map(|p| p.quality).sum::<f64>() / pts.len() as f64;
                    ChartPoint {
                        x: avg_bpp,
                        y: if self.primary_metric.lower_is_better() {
                            -avg_quality // un-negate for display
                        } else {
                            avg_quality
                        },
                        label: Some(format!("q{}", q)),
                    }
                })
                .collect();

            chart_points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

            series.push(ChartSeries {
                name: (*codec).to_string(),
                color: codec_color(codec).to_string(),
                points: chart_points,
            });
        }

        let config = ChartConfig::new(format!("{} vs Bits per Pixel", self.primary_metric.name()))
            .with_x_label("Bits per Pixel (BPP) \u{2192}")
            .with_y_label(format!("\u{2190} {}", self.primary_metric.y_label()))
            .with_lower_is_better(self.primary_metric.lower_is_better())
            .with_dimensions(800, 500);

        Ok(generate_svg(&series, &config))
    }

    /// Generate per-format comparison charts.
    fn generate_format_charts(&self, points: &[RDPoint]) -> Result<()> {
        // Group by format
        let mut by_format: HashMap<&str, Vec<&RDPoint>> = HashMap::new();
        for p in points {
            let format = if p.codec.starts_with("avif") {
                "avif"
            } else if p.codec.contains("jpeg") || p.codec == "mozjpeg" || p.codec == "jpegli" {
                "jpeg"
            } else if p.codec == "webp" {
                "webp"
            } else {
                "other"
            };
            by_format.entry(format).or_default().push(p);
        }

        for (format, pts) in &by_format {
            let pts: &Vec<&RDPoint> = pts;
            if pts.is_empty() {
                continue;
            }

            let mut by_codec: HashMap<&str, Vec<&RDPoint>> = HashMap::new();
            for p in pts {
                by_codec.entry(&p.codec).or_default().push(*p);
            }

            let mut series = Vec::new();
            for (codec, codec_pts) in &by_codec {
                let mut by_quality: HashMap<u32, Vec<&RDPoint>> = HashMap::new();
                for p in codec_pts {
                    by_quality
                        .entry(p.quality_setting as u32)
                        .or_default()
                        .push(*p);
                }

                let mut chart_points: Vec<ChartPoint> = by_quality
                    .iter()
                    .map(|(q, qpts): (&u32, &Vec<&RDPoint>)| {
                        let avg_bpp = qpts.iter().map(|p| p.bpp).sum::<f64>() / qpts.len() as f64;
                        let avg_quality =
                            qpts.iter().map(|p| p.quality).sum::<f64>() / qpts.len() as f64;
                        ChartPoint {
                            x: avg_bpp,
                            y: if self.primary_metric.lower_is_better() {
                                -avg_quality
                            } else {
                                avg_quality
                            },
                            label: Some(format!("q{}", q)),
                        }
                    })
                    .collect();

                chart_points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

                series.push(ChartSeries {
                    name: (*codec).to_string(),
                    color: codec_color(codec).to_string(),
                    points: chart_points,
                });
            }

            let format_upper = (*format).to_uppercase();
            let config = ChartConfig::new(format!(
                "{} Codecs: {} vs BPP",
                format_upper,
                self.primary_metric.name()
            ))
            .with_x_label("Bits per Pixel (BPP) \u{2192}")
            .with_y_label(format!("\u{2190} {}", self.primary_metric.y_label()))
            .with_lower_is_better(self.primary_metric.lower_is_better())
            .with_dimensions(700, 450);

            let svg = generate_svg(&series, &config);
            let path = self.output_dir.join(format!("pareto_{}.svg", format));
            fs::write(path, svg)?;
        }

        Ok(())
    }

    /// Compute summary statistics.
    fn compute_statistics(&self, corpus: &CorpusReport, pareto: &ParetoFront) -> ComparisonStats {
        let mut codec_stats: HashMap<String, CodecStats> = HashMap::new();

        // Aggregate by codec
        for image in &corpus.images {
            for result in &image.results {
                let entry = codec_stats
                    .entry(result.codec_id.clone())
                    .or_insert_with(|| CodecStats {
                        codec_id: result.codec_id.clone(),
                        codec_version: result.codec_version.clone(),
                        format: result
                            .codec_id
                            .split('-')
                            .last()
                            .unwrap_or("unknown")
                            .to_string(),
                        sample_count: 0,
                        bpp_values: Vec::new(),
                        quality_values: Vec::new(),
                        encode_times_ms: Vec::new(),
                        bd_rate_vs_baseline: None,
                    });

                entry.sample_count += 1;
                entry.bpp_values.push(result.bits_per_pixel);

                let q = match self.primary_metric {
                    Metric::Ssimulacra2 => result.metrics.ssimulacra2,
                    Metric::Dssim => result.metrics.dssim,
                    Metric::Butteraugli => result.metrics.butteraugli,
                    Metric::Psnr => result.metrics.psnr,
                };
                if let Some(q) = q {
                    entry.quality_values.push(q);
                }

                entry
                    .encode_times_ms
                    .push(result.encode_time.as_millis() as f64);
            }
        }

        // Compute BD-Rate vs baseline (first codec alphabetically)
        let baseline_id = codec_stats.keys().min().cloned();
        if let Some(ref baseline) = baseline_id {
            let baseline_points: Vec<_> = pareto
                .points
                .iter()
                .filter(|p| &p.codec == baseline)
                .collect();

            for (codec_id, stats) in codec_stats.iter_mut() {
                if codec_id == baseline {
                    stats.bd_rate_vs_baseline = Some(0.0);
                    continue;
                }

                let codec_points: Vec<_> = pareto
                    .points
                    .iter()
                    .filter(|p| &p.codec == codec_id)
                    .collect();

                if baseline_points.len() >= 2 && codec_points.len() >= 2 {
                    let baseline_rd: Vec<(f64, f64)> =
                        baseline_points.iter().map(|p| (p.quality, p.bpp)).collect();
                    let codec_rd: Vec<(f64, f64)> =
                        codec_points.iter().map(|p| (p.quality, p.bpp)).collect();

                    if let Some(rate) = bd_rate(&baseline_rd, &codec_rd) {
                        stats.bd_rate_vs_baseline = Some(rate);
                    }
                }
            }
        }

        // Compute summaries
        let mut stats_vec: Vec<CodecStats> = codec_stats.into_values().collect();
        stats_vec.sort_by(|a, b| a.codec_id.cmp(&b.codec_id));

        ComparisonStats {
            image_count: corpus.images.len(),
            metric: self.primary_metric.name().to_string(),
            baseline_codec: baseline_id,
            codecs: stats_vec,
            pareto_front_size: pareto.points.len(),
        }
    }
}

/// Generated report output.
pub struct GeneratedReport {
    /// Path to the Pareto chart SVG.
    pub pareto_chart_path: std::path::PathBuf,
    /// Comparison statistics.
    pub stats: ComparisonStats,
    /// The computed Pareto front.
    pub pareto: ParetoFront,
}

/// Overall comparison statistics.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ComparisonStats {
    /// Number of images in the corpus.
    pub image_count: usize,
    /// Metric used for analysis.
    pub metric: String,
    /// Baseline codec for BD-Rate calculation.
    pub baseline_codec: Option<String>,
    /// Per-codec statistics.
    pub codecs: Vec<CodecStats>,
    /// Number of points on the Pareto front.
    pub pareto_front_size: usize,
}

/// Statistics for a single codec.
#[derive(Debug, Clone, serde::Serialize)]
pub struct CodecStats {
    /// Codec identifier.
    pub codec_id: String,
    /// Codec version.
    pub codec_version: String,
    /// Output format.
    pub format: String,
    /// Number of samples.
    pub sample_count: usize,
    /// BPP values for summary.
    #[serde(skip)]
    pub bpp_values: Vec<f64>,
    /// Quality values for summary.
    #[serde(skip)]
    pub quality_values: Vec<f64>,
    /// Encode times in ms.
    #[serde(skip)]
    pub encode_times_ms: Vec<f64>,
    /// BD-Rate vs baseline (negative = better).
    pub bd_rate_vs_baseline: Option<f64>,
}

impl ComparisonStats {
    /// Print a summary table to stdout.
    pub fn print_summary(&self) {
        println!("\n{:=<80}", "");
        println!("CODEC COMPARISON SUMMARY");
        println!("{:=<80}", "");
        println!(
            "Images: {}  |  Metric: {}  |  Pareto points: {}",
            self.image_count, self.metric, self.pareto_front_size
        );

        if let Some(ref baseline) = self.baseline_codec {
            println!("BD-Rate baseline: {}", baseline);
        }

        println!("\n{:-<80}", "");
        println!(
            "{:<15} {:>10} {:>12} {:>12} {:>15}",
            "Codec", "Samples", "Avg BPP", "Avg Quality", "BD-Rate (%)"
        );
        println!("{:-<80}", "");

        for codec in &self.codecs {
            let avg_bpp = if codec.bpp_values.is_empty() {
                0.0
            } else {
                codec.bpp_values.iter().sum::<f64>() / codec.bpp_values.len() as f64
            };

            let avg_quality = if codec.quality_values.is_empty() {
                0.0
            } else {
                codec.quality_values.iter().sum::<f64>() / codec.quality_values.len() as f64
            };

            let bd_rate_str = codec
                .bd_rate_vs_baseline
                .map(|r| format!("{:+.1}", r))
                .unwrap_or_else(|| "-".to_string());

            println!(
                "{:<15} {:>10} {:>12.3} {:>12.2} {:>15}",
                codec.codec_id, codec.sample_count, avg_bpp, avg_quality, bd_rate_str
            );
        }

        println!("{:-<80}", "");
        println!("\nBD-Rate: negative = better compression than baseline");
    }
}
