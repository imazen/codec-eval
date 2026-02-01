//! Codec registry for managing and running comparisons.

use std::path::PathBuf;

use codec_eval::eval::{EvalConfig, EvalSession, ImageData};
use codec_eval::metrics::MetricConfig;
use codec_eval::viewing::ViewingCondition;

use crate::Result;
use crate::encoders::{self, CodecImpl, STANDARD_QUALITY_LEVELS};

/// Configuration for codec comparison.
#[derive(Debug, Clone)]
pub struct CompareConfig {
    /// Output directory for reports.
    pub output_dir: PathBuf,

    /// Quality levels to test (0-100).
    pub quality_levels: Vec<f64>,

    /// Viewing condition for perceptual metrics.
    pub viewing: ViewingCondition,

    /// Metric configuration.
    pub metrics: MetricConfig,

    /// Which formats to include.
    pub formats: FormatSelection,

    /// AVIF encoder speed (0-10, lower = slower/better).
    pub avif_speed: u8,
}

impl Default for CompareConfig {
    fn default() -> Self {
        Self {
            output_dir: PathBuf::from("./reports"),
            quality_levels: STANDARD_QUALITY_LEVELS.to_vec(),
            viewing: ViewingCondition::desktop(),
            metrics: MetricConfig::perceptual(),
            formats: FormatSelection::default(),
            avif_speed: 6,
        }
    }
}

impl CompareConfig {
    /// Create a new configuration with the given output directory.
    pub fn new(output_dir: impl Into<PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
            ..Default::default()
        }
    }

    /// Set quality levels.
    pub fn with_quality_levels(mut self, levels: Vec<f64>) -> Self {
        self.quality_levels = levels;
        self
    }

    /// Set viewing condition.
    pub fn with_viewing(mut self, viewing: ViewingCondition) -> Self {
        self.viewing = viewing;
        self
    }

    /// Set metric configuration.
    pub fn with_metrics(mut self, metrics: MetricConfig) -> Self {
        self.metrics = metrics;
        self
    }

    /// Set format selection.
    pub fn with_formats(mut self, formats: FormatSelection) -> Self {
        self.formats = formats;
        self
    }

    /// Set AVIF speed.
    pub fn with_avif_speed(mut self, speed: u8) -> Self {
        self.avif_speed = speed.min(10);
        self
    }
}

/// Which formats to include in the comparison.
#[derive(Debug, Clone, Default)]
pub struct FormatSelection {
    /// Include JPEG codecs (mozjpeg, jpegli).
    pub jpeg: bool,
    /// Include WebP.
    pub webp: bool,
    /// Include AVIF (all encoders).
    pub avif: bool,
    /// Include JPEG XL.
    pub jpegxl: bool,
}

impl FormatSelection {
    /// Include all available formats.
    pub fn all() -> Self {
        Self {
            jpeg: true,
            webp: true,
            avif: true,
            jpegxl: true,
        }
    }

    /// Include only JPEG formats.
    pub fn jpeg_only() -> Self {
        Self {
            jpeg: true,
            webp: false,
            avif: false,
            jpegxl: false,
        }
    }

    /// Include only next-gen formats (WebP, AVIF, JPEG XL).
    pub fn next_gen() -> Self {
        Self {
            jpeg: false,
            webp: true,
            avif: true,
            jpegxl: true,
        }
    }
}

/// Registry of codecs for comparison.
pub struct CodecRegistry {
    config: CompareConfig,
    codecs: Vec<Box<dyn CodecImpl>>,
    session: EvalSession,
}

impl CodecRegistry {
    /// Create a new registry with the given configuration.
    pub fn new(config: CompareConfig) -> Self {
        let eval_config = EvalConfig::builder()
            .report_dir(&config.output_dir)
            .viewing(config.viewing.clone())
            .metrics(config.metrics.clone())
            .quality_levels(config.quality_levels.clone())
            .build();

        Self {
            config,
            codecs: Vec::new(),
            session: EvalSession::new(eval_config),
        }
    }

    /// Register all available codecs based on the format selection.
    pub fn register_all(&mut self) {
        if self.config.formats.jpeg {
            self.register_jpeg();
        }
        if self.config.formats.webp {
            self.register_webp();
        }
        if self.config.formats.avif {
            self.register_avif();
        }
        if self.config.formats.jpegxl {
            self.register_jpegxl();
        }
    }

    /// Register JPEG codecs (all variants: 420/444, progressive/baseline).
    pub fn register_jpeg(&mut self) {
        // Register all MozJPEG variants
        for codec in encoders::jpeg::MozJpegCodec::all_variants() {
            if codec.is_available() {
                self.register_codec(Box::new(codec));
            }
        }

        // Register all jpegli variants
        for codec in encoders::jpeg::JpegliCodec::all_variants() {
            if codec.is_available() {
                self.register_codec(Box::new(codec));
            }
        }
    }

    /// Register WebP codec.
    pub fn register_webp(&mut self) {
        let webp = encoders::webp::WebPCodec::new();
        if webp.is_available() {
            self.register_codec(Box::new(webp));
        }
    }

    /// Register AVIF codecs.
    pub fn register_avif(&mut self) {
        for codec in encoders::avif::AvifCodec::all() {
            let codec = codec.with_speed(self.config.avif_speed);
            if codec.is_available() {
                self.register_codec(Box::new(codec));
            }
        }
    }

    /// Register JPEG XL codec.
    pub fn register_jpegxl(&mut self) {
        let jpegxl = encoders::jpegxl::JpegxlCodec::new();
        if jpegxl.is_available() {
            self.register_codec(Box::new(jpegxl));
        }
    }

    /// Register a specific codec.
    pub fn register_codec(&mut self, codec: Box<dyn CodecImpl>) {
        let id = codec.id().to_string();
        let version = codec.version().to_string();
        let encode_fn = codec.encode_fn();
        let decode_fn = codec.decode_fn();

        self.session
            .add_codec_with_decode(&id, &version, encode_fn, decode_fn);
        self.codecs.push(codec);
    }

    /// Get a reference to the eval session.
    pub fn session(&self) -> &EvalSession {
        &self.session
    }

    /// Get a mutable reference to the eval session.
    pub fn session_mut(&mut self) -> &mut EvalSession {
        &mut self.session
    }

    /// Get list of registered codec IDs.
    pub fn registered_codecs(&self) -> Vec<&str> {
        self.codecs.iter().map(|c| c.id()).collect()
    }

    /// Get the configuration.
    pub fn config(&self) -> &CompareConfig {
        &self.config
    }

    /// Evaluate a single image across all registered codecs.
    pub fn evaluate_image(
        &mut self,
        name: &str,
        image: ImageData,
    ) -> Result<codec_eval::eval::ImageReport> {
        let report = self.session.evaluate_image(name, image)?;
        Ok(report)
    }

    /// Write an image report to disk.
    pub fn write_image_report(&self, report: &codec_eval::eval::ImageReport) -> Result<()> {
        self.session.write_image_report(report)?;
        Ok(())
    }

    /// Write a corpus report to disk.
    pub fn write_corpus_report(&self, report: &codec_eval::eval::CorpusReport) -> Result<()> {
        self.session.write_corpus_report(report)?;
        Ok(())
    }
}
