//! Report types for evaluation results.
//!
//! This module defines the data structures for evaluation reports that can be
//! serialized to JSON or CSV.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::metrics::{MetricResult, PerceptionLevel};

/// Result from evaluating a single codec on a single image at a single quality.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecResult {
    /// Codec identifier.
    pub codec_id: String,

    /// Codec version string.
    pub codec_version: String,

    /// Quality setting used.
    pub quality: f64,

    /// Encoded file size in bytes.
    pub file_size: usize,

    /// Bits per pixel of the encoded image.
    pub bits_per_pixel: f64,

    /// Encoding time.
    #[serde(with = "duration_millis")]
    pub encode_time: Duration,

    /// Decoding time (if decoder was provided).
    #[serde(with = "duration_millis_option")]
    pub decode_time: Option<Duration>,

    /// Quality metrics comparing decoded to reference.
    pub metrics: MetricResult,

    /// Perception level based on metrics.
    pub perception: Option<PerceptionLevel>,

    /// Path to cached encoded file (if caching enabled).
    pub cached_path: Option<PathBuf>,

    /// Additional codec-specific parameters used.
    #[serde(default)]
    pub codec_params: HashMap<String, String>,
}

impl CodecResult {
    /// Calculate compression ratio (original size / encoded size).
    #[must_use]
    pub fn compression_ratio(&self, original_size: usize) -> f64 {
        if self.file_size == 0 {
            0.0
        } else {
            original_size as f64 / self.file_size as f64
        }
    }
}

/// Report for a single image evaluated across multiple codecs and quality levels.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageReport {
    /// Image name or identifier.
    pub name: String,

    /// Path to the source image.
    pub source_path: Option<PathBuf>,

    /// Image dimensions.
    pub width: u32,
    pub height: u32,

    /// Uncompressed image size in bytes (estimated).
    pub uncompressed_size: usize,

    /// Results for each codec/quality combination.
    pub results: Vec<CodecResult>,

    /// When this report was generated.
    #[serde(with = "chrono_serde")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl ImageReport {
    /// Create a new image report.
    #[must_use]
    pub fn new(name: String, width: u32, height: u32) -> Self {
        Self {
            name,
            source_path: None,
            width,
            height,
            uncompressed_size: (width as usize) * (height as usize) * 3,
            results: Vec::new(),
            timestamp: chrono::Utc::now(),
        }
    }

    /// Get results for a specific codec.
    pub fn results_for_codec(&self, codec_id: &str) -> impl Iterator<Item = &CodecResult> {
        self.results.iter().filter(move |r| r.codec_id == codec_id)
    }

    /// Get the best result (highest quality metric) at or below a target file size.
    #[must_use]
    pub fn best_at_size(&self, max_bytes: usize) -> Option<&CodecResult> {
        self.results
            .iter()
            .filter(|r| r.file_size <= max_bytes)
            .max_by(|a, b| {
                // Compare by DSSIM (lower is better), so we invert
                let a_quality = a.metrics.dssim.map(|d| -d).unwrap_or(f64::NEG_INFINITY);
                let b_quality = b.metrics.dssim.map(|d| -d).unwrap_or(f64::NEG_INFINITY);
                a_quality.partial_cmp(&b_quality).unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get the smallest file that achieves at least the target quality.
    #[must_use]
    pub fn smallest_at_quality(&self, max_dssim: f64) -> Option<&CodecResult> {
        self.results
            .iter()
            .filter(|r| r.metrics.dssim.map_or(false, |d| d <= max_dssim))
            .min_by_key(|r| r.file_size)
    }
}

/// Report for a corpus of images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusReport {
    /// Corpus name or identifier.
    pub name: String,

    /// Individual image reports.
    pub images: Vec<ImageReport>,

    /// When this report was generated.
    #[serde(with = "chrono_serde")]
    pub timestamp: chrono::DateTime<chrono::Utc>,

    /// Configuration used for this evaluation.
    pub config_summary: String,
}

impl CorpusReport {
    /// Create a new corpus report.
    #[must_use]
    pub fn new(name: String) -> Self {
        Self {
            name,
            images: Vec::new(),
            timestamp: chrono::Utc::now(),
            config_summary: String::new(),
        }
    }

    /// Total number of codec results across all images.
    #[must_use]
    pub fn total_results(&self) -> usize {
        self.images.iter().map(|img| img.results.len()).sum()
    }

    /// Get unique codec IDs in this report.
    #[must_use]
    pub fn codec_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self
            .images
            .iter()
            .flat_map(|img| img.results.iter().map(|r| r.codec_id.clone()))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }
}

// Custom serialization for Duration as milliseconds
mod duration_millis {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.as_millis().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

mod duration_millis_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        duration.map(|d| d.as_millis()).serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis: Option<u64> = Option::deserialize(deserializer)?;
        Ok(millis.map(Duration::from_millis))
    }
}

mod chrono_serde {
    use chrono::{DateTime, Utc};
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(dt: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        dt.to_rfc3339().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Utc>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        DateTime::parse_from_rfc3339(&s)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_report_new() {
        let report = ImageReport::new("test.png".to_string(), 1920, 1080);
        assert_eq!(report.name, "test.png");
        assert_eq!(report.width, 1920);
        assert_eq!(report.height, 1080);
        assert_eq!(report.uncompressed_size, 1920 * 1080 * 3);
    }

    #[test]
    fn test_codec_result_compression_ratio() {
        let result = CodecResult {
            codec_id: "test".to_string(),
            codec_version: "1.0".to_string(),
            quality: 80.0,
            file_size: 1000,
            bits_per_pixel: 0.5,
            encode_time: Duration::from_millis(100),
            decode_time: None,
            metrics: MetricResult::default(),
            perception: None,
            cached_path: None,
            codec_params: HashMap::new(),
        };

        assert!((result.compression_ratio(10000) - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_corpus_report_codec_ids() {
        let mut report = CorpusReport::new("test".to_string());
        let mut img = ImageReport::new("img1.png".to_string(), 100, 100);
        img.results.push(CodecResult {
            codec_id: "mozjpeg".to_string(),
            codec_version: "4.0".to_string(),
            quality: 80.0,
            file_size: 1000,
            bits_per_pixel: 0.8,
            encode_time: Duration::from_millis(50),
            decode_time: None,
            metrics: MetricResult::default(),
            perception: None,
            cached_path: None,
            codec_params: HashMap::new(),
        });
        img.results.push(CodecResult {
            codec_id: "webp".to_string(),
            codec_version: "1.0".to_string(),
            quality: 80.0,
            file_size: 900,
            bits_per_pixel: 0.72,
            encode_time: Duration::from_millis(60),
            decode_time: None,
            metrics: MetricResult::default(),
            perception: None,
            cached_path: None,
            codec_params: HashMap::new(),
        });
        report.images.push(img);

        let ids = report.codec_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&"mozjpeg".to_string()));
        assert!(ids.contains(&"webp".to_string()));
    }
}
