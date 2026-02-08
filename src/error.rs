//! Error types for codec-eval operations.

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for codec-eval operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur during codec evaluation.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    /// Failed to load an image file.
    #[error("Image load failed: {path}: {reason}")]
    ImageLoad {
        /// Path to the image that failed to load.
        path: PathBuf,
        /// Reason for the failure.
        reason: String,
    },

    /// Error from a codec during encoding or decoding.
    #[error("Codec error ({codec}): {message}")]
    Codec {
        /// Codec identifier.
        codec: String,
        /// Error message from the codec.
        message: String,
    },

    /// Image dimensions don't match between reference and test images.
    #[error("Dimension mismatch: expected {expected:?}, got {actual:?}")]
    DimensionMismatch {
        /// Expected dimensions (width, height).
        expected: (usize, usize),
        /// Actual dimensions (width, height).
        actual: (usize, usize),
    },

    /// Failed to calculate a quality metric.
    #[error("Metric calculation failed: {metric}: {reason}")]
    MetricCalculation {
        /// Name of the metric that failed.
        metric: String,
        /// Reason for the failure.
        reason: String,
    },

    /// Error in corpus management.
    #[error("Corpus error: {0}")]
    Corpus(String),

    /// Error importing CSV data.
    #[error("CSV import error at line {line}: {reason}")]
    CsvImport {
        /// Line number where the error occurred.
        line: usize,
        /// Reason for the failure.
        reason: String,
    },

    /// Invalid quality value provided.
    #[error("Invalid quality value: {0} (expected 0.0-100.0 or codec-specific range)")]
    InvalidQuality(f64),

    /// Quality metric is below the specified threshold.
    #[error("{metric} quality below threshold: {value} (threshold: {threshold})")]
    QualityBelowThreshold {
        /// Name of the quality metric.
        metric: String,
        /// Actual value.
        value: f64,
        /// Threshold that was not met.
        threshold: f64,
    },

    /// Unsupported image or codec format.
    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    /// Error writing report files.
    #[error("Report error: {0}")]
    Report(String),

    /// Cache-related error.
    #[error("Cache error: {0}")]
    Cache(String),

    /// I/O error wrapper.
    #[error(transparent)]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// CSV error.
    #[error("CSV error: {0}")]
    Csv(#[from] csv::Error),
}
