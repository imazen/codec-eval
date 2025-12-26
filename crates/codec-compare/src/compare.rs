//! Self-comparison API for codec developers.
//!
//! This module provides an API for codec implementations to compare
//! themselves against all other registered codecs, with options to
//! customize the comparison scope and output.
//!
//! # Example
//!
//! ```rust,ignore
//! use codec_compare::compare::{CompareAgainstAll, CompareOptions};
//!
//! // Your codec implements EncodeFn/DecodeFn
//! let my_codec = MyCodec::new();
//!
//! // Compare against all others with defaults
//! let results = CompareAgainstAll::new("my-codec", "1.0.0")
//!     .with_encode(my_codec.encode_fn())
//!     .with_decode(my_codec.decode_fn())
//!     .on_corpus("./test_images")
//!     .run()?;
//!
//! // Get BD-Rate comparison
//! for (other_codec, bd_rate) in results.bd_rates() {
//!     println!("{}: {:.1}% vs my-codec", other_codec, bd_rate);
//! }
//!
//! // Export charts
//! results.write_charts("./output")?;
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use codec_eval::corpus::Corpus;
use codec_eval::eval::{CorpusReport, EvalConfig, EvalSession, ImageData};
use codec_eval::metrics::MetricConfig;
use codec_eval::stats::{ParetoFront, RDPoint, bd_rate};
use codec_eval::viewing::ViewingCondition;
use image::GenericImageView;

use crate::encoders::{self, CodecImpl, STANDARD_QUALITY_LEVELS};
use crate::registry::FormatSelection;
use crate::report::{Metric, ReportGenerator};
use crate::{CompareError, Result};

/// Options for self-comparison.
#[derive(Debug, Clone)]
pub struct CompareOptions {
    /// Quality levels to test.
    pub quality_levels: Vec<f64>,
    /// Primary metric for analysis.
    pub metric: Metric,
    /// Viewing condition.
    pub viewing: ViewingCondition,
    /// Whether to include other codecs of the same format.
    pub include_same_format: bool,
    /// Whether to include codecs of different formats.
    pub include_other_formats: bool,
    /// Maximum images to process.
    pub limit: Option<usize>,
    /// Output directory for reports.
    pub output_dir: PathBuf,
    /// Whether to delete results for excluded codecs after comparison.
    pub delete_excluded: bool,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            quality_levels: STANDARD_QUALITY_LEVELS.to_vec(),
            metric: Metric::Ssimulacra2,
            viewing: ViewingCondition::desktop(),
            include_same_format: true,
            include_other_formats: true,
            limit: None,
            output_dir: PathBuf::from("./compare_output"),
            delete_excluded: false,
        }
    }
}

/// Builder for comparing a codec against all others.
pub struct CompareAgainstAll {
    codec_id: String,
    codec_version: String,
    encode_fn: Option<codec_eval::eval::session::EncodeFn>,
    decode_fn: Option<codec_eval::eval::session::DecodeFn>,
    corpus_path: Option<PathBuf>,
    options: CompareOptions,
    format: Option<String>,
}

impl CompareAgainstAll {
    /// Create a new comparison builder for the given codec.
    pub fn new(codec_id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            codec_id: codec_id.into(),
            codec_version: version.into(),
            encode_fn: None,
            decode_fn: None,
            corpus_path: None,
            options: CompareOptions::default(),
            format: None,
        }
    }

    /// Set the encode function for the codec.
    pub fn with_encode(mut self, encode_fn: codec_eval::eval::session::EncodeFn) -> Self {
        self.encode_fn = Some(encode_fn);
        self
    }

    /// Set the decode function for the codec.
    pub fn with_decode(mut self, decode_fn: codec_eval::eval::session::DecodeFn) -> Self {
        self.decode_fn = Some(decode_fn);
        self
    }

    /// Set the codec's output format (e.g., "jpeg", "webp", "avif").
    pub fn with_format(mut self, format: impl Into<String>) -> Self {
        self.format = Some(format.into());
        self
    }

    /// Set the corpus path.
    pub fn on_corpus(mut self, path: impl Into<PathBuf>) -> Self {
        self.corpus_path = Some(path.into());
        self
    }

    /// Set quality levels to test.
    pub fn with_quality_levels(mut self, levels: Vec<f64>) -> Self {
        self.options.quality_levels = levels;
        self
    }

    /// Set the primary metric.
    pub fn with_metric(mut self, metric: Metric) -> Self {
        self.options.metric = metric;
        self
    }

    /// Only compare against same-format codecs.
    pub fn same_format_only(mut self) -> Self {
        self.options.include_same_format = true;
        self.options.include_other_formats = false;
        self
    }

    /// Only compare against different-format codecs.
    pub fn other_formats_only(mut self) -> Self {
        self.options.include_same_format = false;
        self.options.include_other_formats = true;
        self
    }

    /// Limit the number of images to process.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.options.limit = Some(limit);
        self
    }

    /// Set the output directory.
    pub fn output_to(mut self, path: impl Into<PathBuf>) -> Self {
        self.options.output_dir = path.into();
        self
    }

    /// Delete results for excluded codecs after comparison.
    pub fn delete_excluded(mut self) -> Self {
        self.options.delete_excluded = true;
        self
    }

    /// Run the comparison.
    pub fn run(self) -> Result<CompareResult> {
        let encode_fn = self.encode_fn.ok_or_else(|| {
            CompareError::EncoderNotAvailable("encode function not provided".to_string())
        })?;
        let decode_fn = self.decode_fn.ok_or_else(|| {
            CompareError::EncoderNotAvailable("decode function not provided".to_string())
        })?;
        let corpus_path = self
            .corpus_path
            .ok_or_else(|| CompareError::ImageLoad("corpus path not provided".to_string()))?;

        // Discover corpus
        let corpus = Corpus::discover(&corpus_path)?;
        let images = &corpus.images;
        let image_count = self.options.limit.unwrap_or(images.len()).min(images.len());

        // Setup eval session
        std::fs::create_dir_all(&self.options.output_dir)?;
        let eval_config = EvalConfig::builder()
            .report_dir(&self.options.output_dir)
            .viewing(self.options.viewing.clone())
            .metrics(MetricConfig::perceptual())
            .quality_levels(self.options.quality_levels.clone())
            .build();

        let mut session = EvalSession::new(eval_config);

        // Register the subject codec
        session.add_codec_with_decode(&self.codec_id, &self.codec_version, encode_fn, decode_fn);

        // Register comparison codecs based on options
        let format = self.format.as_deref();

        if self.options.include_same_format || self.options.include_other_formats {
            register_comparison_codecs(
                &mut session,
                format,
                self.options.include_same_format,
                self.options.include_other_formats,
                &self.codec_id,
            );
        }

        // Run evaluation
        let mut corpus_report = CorpusReport::new("compare".to_string());

        for (i, corpus_image) in images.iter().take(image_count).enumerate() {
            let path = corpus_image.full_path(&corpus.root_path);
            let name = corpus_image.name();

            // Load image
            let img = match image::open(&path) {
                Ok(img) => img,
                Err(_) => continue,
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
            if let Ok(report) = session.evaluate_image(&name, image_data) {
                corpus_report.images.push(report);
            }
        }

        // Optionally delete excluded codecs from results
        if self.options.delete_excluded {
            // Filter results to only include relevant codecs
            // (This is a no-op for now as we only register what we want)
        }

        // Extract RD points
        let rd_points = extract_rd_points(&corpus_report, self.options.metric);

        // Compute Pareto front
        let pareto = ParetoFront::compute(&rd_points);

        // Compute BD-rates vs subject codec
        let bd_rates = compute_bd_rates(&pareto, &self.codec_id);

        // Generate reports
        let generator =
            ReportGenerator::new(&self.options.output_dir).with_metric(self.options.metric);
        let _ = generator.generate(&corpus_report);

        Ok(CompareResult {
            subject_codec: self.codec_id,
            corpus_report,
            pareto,
            bd_rates,
            output_dir: self.options.output_dir,
        })
    }
}

/// Result of a self-comparison.
pub struct CompareResult {
    /// The subject codec ID.
    pub subject_codec: String,
    /// Full corpus report.
    pub corpus_report: CorpusReport,
    /// Computed Pareto front.
    pub pareto: ParetoFront,
    /// BD-Rate vs subject codec (negative = subject is better).
    pub bd_rates: HashMap<String, f64>,
    /// Output directory.
    pub output_dir: PathBuf,
}

impl CompareResult {
    /// Get BD-Rate for each other codec vs the subject.
    pub fn bd_rates(&self) -> &HashMap<String, f64> {
        &self.bd_rates
    }

    /// Check if the subject codec is on the Pareto front.
    pub fn subject_on_pareto(&self) -> bool {
        self.pareto
            .points
            .iter()
            .any(|p| p.codec == self.subject_codec)
    }

    /// Get the subject codec's average quality at each BPP.
    pub fn subject_rd_curve(&self) -> Vec<(f64, f64)> {
        self.pareto
            .points
            .iter()
            .filter(|p| p.codec == self.subject_codec)
            .map(|p| (p.bpp, p.quality))
            .collect()
    }

    /// Write comparison charts to a directory.
    pub fn write_charts(&self, output: impl AsRef<Path>) -> Result<()> {
        let output = output.as_ref();
        std::fs::create_dir_all(output)?;

        // The charts were already generated during run()
        // Just copy them if output differs
        if output != self.output_dir {
            for entry in std::fs::read_dir(&self.output_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map(|e| e == "svg").unwrap_or(false) {
                    let dest = output.join(path.file_name().unwrap());
                    std::fs::copy(&path, dest)?;
                }
            }
        }

        Ok(())
    }

    /// Print a summary table.
    pub fn print_summary(&self) {
        println!("\n{:=<60}", "");
        println!("COMPARISON RESULTS FOR: {}", self.subject_codec);
        println!("{:=<60}", "");

        println!("\nBD-Rate (negative = subject is better):");
        println!("{:-<40}", "");

        let mut sorted: Vec<_> = self.bd_rates.iter().collect();
        sorted.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap());

        for (codec, rate) in sorted {
            let status = if *rate < -5.0 {
                "BETTER"
            } else if *rate > 5.0 {
                "WORSE"
            } else {
                "SIMILAR"
            };
            println!("  {:20} {:+8.1}%  ({})", codec, rate, status);
        }

        println!("{:-<40}", "");
        println!("\nSubject on Pareto front: {}", self.subject_on_pareto());
    }
}

/// Register comparison codecs based on format filtering.
fn register_comparison_codecs(
    session: &mut EvalSession,
    subject_format: Option<&str>,
    include_same: bool,
    include_other: bool,
    exclude_codec: &str,
) {
    // JPEG codecs
    let is_jpeg = subject_format
        .map(|f| f == "jpeg" || f == "jpg")
        .unwrap_or(false);
    if (is_jpeg && include_same) || (!is_jpeg && include_other) {
        let mozjpeg = encoders::jpeg::MozJpegCodec::new();
        if mozjpeg.is_available() && mozjpeg.id() != exclude_codec {
            session.add_codec_with_decode(
                mozjpeg.id(),
                mozjpeg.version(),
                mozjpeg.encode_fn(),
                mozjpeg.decode_fn(),
            );
        }

        let jpegli = encoders::jpeg::JpegliCodec::new();
        if jpegli.is_available() && jpegli.id() != exclude_codec {
            session.add_codec_with_decode(
                jpegli.id(),
                jpegli.version(),
                jpegli.encode_fn(),
                jpegli.decode_fn(),
            );
        }
    }

    // WebP codec
    let is_webp = subject_format.map(|f| f == "webp").unwrap_or(false);
    if (is_webp && include_same) || (!is_webp && include_other) {
        let webp = encoders::webp::WebPCodec::new();
        if webp.is_available() && webp.id() != exclude_codec {
            session.add_codec_with_decode(
                webp.id(),
                webp.version(),
                webp.encode_fn(),
                webp.decode_fn(),
            );
        }
    }

    // AVIF codecs
    let is_avif = subject_format.map(|f| f == "avif").unwrap_or(false);
    if (is_avif && include_same) || (!is_avif && include_other) {
        for codec in encoders::avif::AvifCodec::all() {
            if codec.is_available() && codec.id() != exclude_codec {
                session.add_codec_with_decode(
                    codec.id(),
                    codec.version(),
                    codec.encode_fn(),
                    codec.decode_fn(),
                );
            }
        }
    }
}

/// Extract RD points from corpus report.
fn extract_rd_points(corpus: &CorpusReport, metric: Metric) -> Vec<RDPoint> {
    let mut points = Vec::new();

    for image in &corpus.images {
        for result in &image.results {
            let quality = match metric {
                Metric::Ssimulacra2 => result.metrics.ssimulacra2,
                Metric::Dssim => result.metrics.dssim.map(|d| -d),
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

/// Compute BD-rates vs subject codec.
fn compute_bd_rates(pareto: &ParetoFront, subject: &str) -> HashMap<String, f64> {
    let mut rates = HashMap::new();

    let subject_points: Vec<_> = pareto
        .points
        .iter()
        .filter(|p| p.codec == subject)
        .collect();

    if subject_points.len() < 2 {
        return rates;
    }

    let subject_rd: Vec<(f64, f64)> = subject_points.iter().map(|p| (p.quality, p.bpp)).collect();

    // Get unique codecs
    let codecs: std::collections::HashSet<_> = pareto.points.iter().map(|p| &p.codec).collect();

    for codec in codecs {
        if codec == subject {
            continue;
        }

        let codec_points: Vec<_> = pareto.points.iter().filter(|p| &p.codec == codec).collect();

        if codec_points.len() < 2 {
            continue;
        }

        let codec_rd: Vec<(f64, f64)> = codec_points.iter().map(|p| (p.quality, p.bpp)).collect();

        if let Some(rate) = bd_rate(&subject_rd, &codec_rd) {
            rates.insert(codec.clone(), rate);
        }
    }

    rates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_options_default() {
        let opts = CompareOptions::default();
        assert!(opts.include_same_format);
        assert!(opts.include_other_formats);
        assert!(!opts.delete_excluded);
    }

    #[test]
    fn test_compare_builder() {
        let builder = CompareAgainstAll::new("test-codec", "1.0")
            .same_format_only()
            .with_limit(10);

        assert_eq!(builder.codec_id, "test-codec");
        assert_eq!(builder.options.limit, Some(10));
        assert!(builder.options.include_same_format);
        assert!(!builder.options.include_other_formats);
    }
}
