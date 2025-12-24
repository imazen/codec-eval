//! Evaluation session with callback-based codec interface.
//!
//! This module provides [`EvalSession`], the main entry point for codec evaluation.
//! External crates provide encode/decode callbacks, and the session handles
//! metrics calculation, caching, and report generation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use imgref::ImgVec;
use rgb::{RGB8, RGBA8};

use crate::error::Result;
use crate::eval::report::{CodecResult, CorpusReport, ImageReport};
use crate::metrics::dssim::rgb8_to_dssim_image;
use crate::metrics::{calculate_psnr, MetricConfig, MetricResult};
use crate::viewing::ViewingCondition;

/// Image data accepted by the evaluation session.
///
/// Supports both `imgref::ImgVec` types and raw slices for flexibility.
#[derive(Clone)]
pub enum ImageData {
    /// RGB8 image using imgref.
    Rgb8(ImgVec<RGB8>),

    /// RGBA8 image using imgref.
    Rgba8(ImgVec<RGBA8>),

    /// RGB8 raw slice with dimensions.
    RgbSlice {
        /// Pixel data in row-major order.
        data: Vec<u8>,
        /// Image width.
        width: usize,
        /// Image height.
        height: usize,
    },

    /// RGBA8 raw slice with dimensions.
    RgbaSlice {
        /// Pixel data in row-major order.
        data: Vec<u8>,
        /// Image width.
        width: usize,
        /// Image height.
        height: usize,
    },
}

impl ImageData {
    /// Get image width.
    #[must_use]
    pub fn width(&self) -> usize {
        match self {
            Self::Rgb8(img) => img.width(),
            Self::Rgba8(img) => img.width(),
            Self::RgbSlice { width, .. } => *width,
            Self::RgbaSlice { width, .. } => *width,
        }
    }

    /// Get image height.
    #[must_use]
    pub fn height(&self) -> usize {
        match self {
            Self::Rgb8(img) => img.height(),
            Self::Rgba8(img) => img.height(),
            Self::RgbSlice { height, .. } => *height,
            Self::RgbaSlice { height, .. } => *height,
        }
    }

    /// Convert to RGB8 slice representation.
    #[must_use]
    pub fn to_rgb8_vec(&self) -> Vec<u8> {
        match self {
            Self::Rgb8(img) => {
                img.pixels()
                    .flat_map(|p| [p.r, p.g, p.b])
                    .collect()
            }
            Self::Rgba8(img) => {
                img.pixels()
                    .flat_map(|p| [p.r, p.g, p.b])
                    .collect()
            }
            Self::RgbSlice { data, .. } => data.clone(),
            Self::RgbaSlice { data, width, height } => {
                let mut rgb = Vec::with_capacity(width * height * 3);
                for chunk in data.chunks_exact(4) {
                    rgb.push(chunk[0]);
                    rgb.push(chunk[1]);
                    rgb.push(chunk[2]);
                }
                rgb
            }
        }
    }
}

/// Request for a single encode operation.
#[derive(Debug, Clone)]
pub struct EncodeRequest {
    /// Quality setting (0-100, codec-specific interpretation).
    pub quality: f64,

    /// Additional codec-specific parameters.
    pub params: HashMap<String, String>,
}

impl EncodeRequest {
    /// Create a new encode request with the given quality.
    #[must_use]
    pub fn new(quality: f64) -> Self {
        Self {
            quality,
            params: HashMap::new(),
        }
    }

    /// Add a codec-specific parameter.
    #[must_use]
    pub fn with_param(mut self, key: &str, value: &str) -> Self {
        self.params.insert(key.to_string(), value.to_string());
        self
    }
}

/// Encode callback type.
///
/// Takes image data and encode request, returns encoded bytes.
pub type EncodeFn = Box<dyn Fn(&ImageData, &EncodeRequest) -> Result<Vec<u8>> + Send + Sync>;

/// Decode callback type.
///
/// Takes encoded bytes, returns decoded image data.
pub type DecodeFn = Box<dyn Fn(&[u8]) -> Result<ImageData> + Send + Sync>;

/// Configuration for an evaluation session.
#[derive(Debug, Clone)]
pub struct EvalConfig {
    /// Directory for report output (CSV, JSON).
    pub report_dir: PathBuf,

    /// Directory for caching encoded files.
    pub cache_dir: Option<PathBuf>,

    /// Viewing condition for perceptual metrics.
    pub viewing: ViewingCondition,

    /// Which metrics to calculate.
    pub metrics: MetricConfig,

    /// Quality levels to sweep.
    pub quality_levels: Vec<f64>,
}

impl EvalConfig {
    /// Create a new configuration builder.
    #[must_use]
    pub fn builder() -> EvalConfigBuilder {
        EvalConfigBuilder::default()
    }
}

/// Builder for [`EvalConfig`].
#[derive(Debug, Default)]
pub struct EvalConfigBuilder {
    report_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
    viewing: Option<ViewingCondition>,
    metrics: Option<MetricConfig>,
    quality_levels: Option<Vec<f64>>,
}

impl EvalConfigBuilder {
    /// Set the report output directory.
    #[must_use]
    pub fn report_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.report_dir = Some(path.into());
        self
    }

    /// Set the cache directory.
    #[must_use]
    pub fn cache_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.cache_dir = Some(path.into());
        self
    }

    /// Set the viewing condition.
    #[must_use]
    pub fn viewing(mut self, viewing: ViewingCondition) -> Self {
        self.viewing = Some(viewing);
        self
    }

    /// Set which metrics to calculate.
    #[must_use]
    pub fn metrics(mut self, metrics: MetricConfig) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Set quality levels to sweep.
    #[must_use]
    pub fn quality_levels(mut self, levels: Vec<f64>) -> Self {
        self.quality_levels = Some(levels);
        self
    }

    /// Build the configuration.
    ///
    /// # Panics
    ///
    /// Panics if `report_dir` is not set.
    #[must_use]
    pub fn build(self) -> EvalConfig {
        EvalConfig {
            report_dir: self.report_dir.expect("report_dir is required"),
            cache_dir: self.cache_dir,
            viewing: self.viewing.unwrap_or_default(),
            metrics: self.metrics.unwrap_or_else(MetricConfig::all),
            quality_levels: self.quality_levels.unwrap_or_else(|| {
                vec![50.0, 60.0, 70.0, 80.0, 85.0, 90.0, 95.0]
            }),
        }
    }
}

/// Registered codec entry.
struct CodecEntry {
    id: String,
    version: String,
    encode: EncodeFn,
    decode: Option<DecodeFn>,
}

/// Evaluation session for codec comparison.
///
/// # Example
///
/// ```rust,ignore
/// use codec_eval::{EvalSession, EvalConfig, ViewingCondition, ImageData};
///
/// let config = EvalConfig::builder()
///     .report_dir("./reports")
///     .viewing(ViewingCondition::desktop())
///     .build();
///
/// let mut session = EvalSession::new(config);
///
/// session.add_codec("my-codec", "1.0.0", Box::new(|image, request| {
///     // Encode the image
///     Ok(encoded_bytes)
/// }));
///
/// let report = session.evaluate_image("test.png", image_data)?;
/// ```
pub struct EvalSession {
    config: EvalConfig,
    codecs: Vec<CodecEntry>,
}

impl EvalSession {
    /// Create a new evaluation session.
    #[must_use]
    pub fn new(config: EvalConfig) -> Self {
        Self {
            config,
            codecs: Vec::new(),
        }
    }

    /// Register a codec with an encode callback.
    pub fn add_codec(&mut self, id: &str, version: &str, encode: EncodeFn) -> &mut Self {
        self.codecs.push(CodecEntry {
            id: id.to_string(),
            version: version.to_string(),
            encode,
            decode: None,
        });
        self
    }

    /// Register a codec with both encode and decode callbacks.
    pub fn add_codec_with_decode(
        &mut self,
        id: &str,
        version: &str,
        encode: EncodeFn,
        decode: DecodeFn,
    ) -> &mut Self {
        self.codecs.push(CodecEntry {
            id: id.to_string(),
            version: version.to_string(),
            encode,
            decode: Some(decode),
        });
        self
    }

    /// Get the number of registered codecs.
    #[must_use]
    pub fn codec_count(&self) -> usize {
        self.codecs.len()
    }

    /// Evaluate a single image across all registered codecs.
    ///
    /// # Arguments
    ///
    /// * `name` - Image name or identifier.
    /// * `image` - The image data to evaluate.
    ///
    /// # Returns
    ///
    /// An [`ImageReport`] containing results for all codec/quality combinations.
    pub fn evaluate_image(&self, name: &str, image: ImageData) -> Result<ImageReport> {
        let width = image.width() as u32;
        let height = image.height() as u32;
        let mut report = ImageReport::new(name.to_string(), width, height);

        let reference_rgb = image.to_rgb8_vec();

        for codec in &self.codecs {
            for &quality in &self.config.quality_levels {
                let request = EncodeRequest::new(quality);

                // Encode
                let start = Instant::now();
                let encoded = (codec.encode)(&image, &request)?;
                let encode_time = start.elapsed();

                // Calculate metrics
                let metrics = if let Some(ref decode) = codec.decode {
                    // Decode and compare
                    let start = Instant::now();
                    let decoded = decode(&encoded)?;
                    let decode_time = start.elapsed();

                    let decoded_rgb = decoded.to_rgb8_vec();
                    let metrics = self.calculate_metrics(&reference_rgb, &decoded_rgb, width, height)?;

                    report.results.push(CodecResult {
                        codec_id: codec.id.clone(),
                        codec_version: codec.version.clone(),
                        quality,
                        file_size: encoded.len(),
                        bits_per_pixel: (encoded.len() * 8) as f64 / (width as f64 * height as f64),
                        encode_time,
                        decode_time: Some(decode_time),
                        metrics: metrics.clone(),
                        perception: metrics.perception_level(),
                        cached_path: None,
                        codec_params: request.params,
                    });
                    continue;
                } else {
                    // No decoder, just record file size
                    MetricResult::default()
                };

                report.results.push(CodecResult {
                    codec_id: codec.id.clone(),
                    codec_version: codec.version.clone(),
                    quality,
                    file_size: encoded.len(),
                    bits_per_pixel: (encoded.len() * 8) as f64 / (width as f64 * height as f64),
                    encode_time,
                    decode_time: None,
                    metrics,
                    perception: None,
                    cached_path: None,
                    codec_params: request.params,
                });
            }
        }

        Ok(report)
    }

    /// Calculate metrics between reference and test images.
    fn calculate_metrics(
        &self,
        reference: &[u8],
        test: &[u8],
        width: u32,
        height: u32,
    ) -> Result<MetricResult> {
        let mut result = MetricResult::default();

        if self.config.metrics.psnr {
            result.psnr = Some(calculate_psnr(
                reference,
                test,
                width as usize,
                height as usize,
            ));
        }

        if self.config.metrics.dssim {
            let ref_img = rgb8_to_dssim_image(reference, width as usize, height as usize);
            let test_img = rgb8_to_dssim_image(test, width as usize, height as usize);
            result.dssim = Some(crate::metrics::dssim::calculate_dssim(
                &ref_img,
                &test_img,
                &self.config.viewing,
            )?);
        }

        Ok(result)
    }

    /// Write an image report to the configured report directory.
    pub fn write_image_report(&self, report: &ImageReport) -> Result<()> {
        std::fs::create_dir_all(&self.config.report_dir)?;

        let json_path = self.config.report_dir.join(format!("{}.json", report.name));
        let json = serde_json::to_string_pretty(report)?;
        std::fs::write(json_path, json)?;

        Ok(())
    }

    /// Write a corpus report to the configured report directory.
    pub fn write_corpus_report(&self, report: &CorpusReport) -> Result<()> {
        std::fs::create_dir_all(&self.config.report_dir)?;

        let json_path = self.config.report_dir.join(format!("{}.json", report.name));
        let json = serde_json::to_string_pretty(report)?;
        std::fs::write(json_path, json)?;

        // Also write CSV summary
        let csv_path = self.config.report_dir.join(format!("{}.csv", report.name));
        self.write_csv_summary(report, &csv_path)?;

        Ok(())
    }

    /// Write a CSV summary of the corpus report.
    fn write_csv_summary(&self, report: &CorpusReport, path: &Path) -> Result<()> {
        let mut wtr = csv::Writer::from_path(path)?;

        // Header
        wtr.write_record([
            "image",
            "codec",
            "version",
            "quality",
            "file_size",
            "bpp",
            "encode_ms",
            "decode_ms",
            "dssim",
            "psnr",
            "perception",
        ])?;

        for img in &report.images {
            for result in &img.results {
                wtr.write_record([
                    &img.name,
                    &result.codec_id,
                    &result.codec_version,
                    &result.quality.to_string(),
                    &result.file_size.to_string(),
                    &format!("{:.4}", result.bits_per_pixel),
                    &result.encode_time.as_millis().to_string(),
                    &result.decode_time.map_or(String::new(), |d| d.as_millis().to_string()),
                    &result.metrics.dssim.map_or(String::new(), |d| format!("{:.6}", d)),
                    &result.metrics.psnr.map_or(String::new(), |p| format!("{:.2}", p)),
                    &result.perception.map_or(String::new(), |p| p.code().to_string()),
                ])?;
            }
        }

        wtr.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image(width: usize, height: usize) -> ImageData {
        let data: Vec<u8> = (0..width * height * 3)
            .map(|i| (i % 256) as u8)
            .collect();
        ImageData::RgbSlice { data, width, height }
    }

    #[test]
    fn test_image_data_dimensions() {
        let img = create_test_image(100, 50);
        assert_eq!(img.width(), 100);
        assert_eq!(img.height(), 50);
    }

    #[test]
    fn test_encode_request() {
        let req = EncodeRequest::new(80.0)
            .with_param("subsampling", "4:2:0");
        assert!((req.quality - 80.0).abs() < f64::EPSILON);
        assert_eq!(req.params.get("subsampling"), Some(&"4:2:0".to_string()));
    }

    #[test]
    fn test_eval_config_builder() {
        let config = EvalConfig::builder()
            .report_dir("/tmp/reports")
            .cache_dir("/tmp/cache")
            .viewing(ViewingCondition::laptop())
            .quality_levels(vec![50.0, 75.0, 90.0])
            .build();

        assert_eq!(config.report_dir, PathBuf::from("/tmp/reports"));
        assert_eq!(config.cache_dir, Some(PathBuf::from("/tmp/cache")));
        assert!((config.viewing.acuity_ppd - 60.0).abs() < f64::EPSILON);
        assert_eq!(config.quality_levels.len(), 3);
    }

    #[test]
    fn test_session_add_codec() {
        let config = EvalConfig::builder()
            .report_dir("/tmp/test")
            .build();

        let mut session = EvalSession::new(config);
        session.add_codec("test", "1.0", Box::new(|_, _| Ok(vec![0u8; 100])));

        assert_eq!(session.codec_count(), 1);
    }
}
