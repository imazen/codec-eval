//! Multi-codec image comparison library.
//!
//! Provides unified wrappers for comparing image codecs across formats:
//! - JPEG: MozJPEG, jpegli, libjpeg-turbo
//! - WebP: libwebp
//! - AVIF: libaom, rav1e, SVT-AV1
//!
//! # Example
//!
//! ```rust,ignore
//! use codec_compare::{CodecRegistry, CompareConfig};
//!
//! let config = CompareConfig::default();
//! let mut registry = CodecRegistry::new(config);
//!
//! // Register all available codecs
//! registry.register_all();
//!
//! // Run comparison
//! let results = registry.compare_image("test.png")?;
//! ```

pub mod compare;
pub mod encoders;
pub mod quality_predictor;
pub mod registry;
pub mod report;

pub use compare::{CompareAgainstAll, CompareOptions, CompareResult};
pub use registry::{CodecRegistry, CompareConfig};

use thiserror::Error;

/// Errors from codec comparison operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CompareError {
    #[error("Encoder not available: {0}")]
    EncoderNotAvailable(String),

    #[error("Encoding failed for {codec}: {message}")]
    EncodingFailed { codec: String, message: String },

    #[error("Decoding failed for {codec}: {message}")]
    DecodingFailed { codec: String, message: String },

    #[error("Image loading failed: {0}")]
    ImageLoad(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Codec eval error: {0}")]
    CodecEval(#[from] codec_eval::error::Error),
}

/// Result type for comparison operations.
pub type Result<T> = std::result::Result<T, CompareError>;
