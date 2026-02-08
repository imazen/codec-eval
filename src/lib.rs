//! # codec-eval
//!
//! Image codec comparison and evaluation library.
//!
//! This library provides an **API-first design** where external crates provide
//! encode/decode callbacks, and this library handles quality metrics, viewing
//! conditions, and report generation.
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use codec_eval::{EvalSession, EvalConfig, ViewingCondition, ImageData};
//!
//! let config = EvalConfig::builder()
//!     .report_dir("./reports")
//!     .viewing(ViewingCondition::desktop())
//!     .build();
//!
//! let mut session = EvalSession::new(config);
//!
//! session.add_codec("my-codec", "1.0.0", Box::new(|image, request| {
//!     // Your encoding logic here
//!     Ok(encoded_bytes)
//! }));
//!
//! let report = session.evaluate_image("test.png", image_data)?;
//! ```
//!
//! ## Modules
//!
//! - [`error`]: Error types for the library
//! - [`viewing`]: Viewing condition modeling for perceptual metrics
//! - [`metrics`]: Quality metrics (DSSIM, SSIMULACRA2, Butteraugli, PSNR)
//! - [`eval`]: Evaluation session and report generation
//! - [`corpus`]: Test corpus management
//! - [`import`]: CSV import for third-party results
//! - [`stats`]: Statistical analysis and Pareto front
//! - [`interpolation`]: Quality interpolation and polynomial fitting

pub mod corpus;
#[cfg(feature = "jpeg-decode")]
pub mod decode;
pub mod error;
pub mod eval;
pub mod import;
#[cfg(feature = "interpolation")]
pub mod interpolation;
pub mod metrics;
pub mod stats;
pub mod viewing;

// Re-export commonly used types for codec evaluation
pub use corpus::{Corpus, CorpusImage, ImageCategory};
pub use error::{Error, Result};
pub use eval::{
    // Evaluation helpers (lightweight API for zen* projects)
    assert_perception_level, assert_quality, evaluate_single,
    // Session-based evaluation (full API)
    CodecResult, CorpusReport, EvalConfig, EvalSession, ImageReport, ImageData,
};
pub use import::{CsvImporter, ExternalResult};
pub use metrics::{MetricConfig, MetricResult, PerceptionLevel};
pub use stats::{ParetoFront, RDPoint, Summary};
pub use viewing::{ViewingCondition, REFERENCE_PPD};

// Advanced/specialized re-exports (less commonly used)

/// Sparse corpus checkout types (for large test corpora).
#[cfg(feature = "default")]
pub use corpus::{SparseCheckout, SparseFilter, SparseStatus};

/// CSV schema for importing external results.
#[cfg(feature = "default")]
pub use import::CsvSchema;

/// ICC color profile support (requires `icc` feature).
#[cfg(feature = "icc")]
pub use metrics::ColorProfile;

/// XYB color space roundtrip (for testing XYB-based codecs).
pub use metrics::xyb_roundtrip;

/// Statistical functions (mean, median, percentile, etc.).
pub use stats::{iqr, mean, median, percentile, percentile_u32, std_dev, trimmed_mean};

/// Viewing condition simulation parameters.
pub use viewing::{SimulationMode, SimulationParams};

// Feature-gated re-exports

/// Chart generation types (requires `chart` feature).
#[cfg(feature = "chart")]
pub use stats::{generate_svg, ChartConfig, ChartPoint, ChartSeries};

/// Polynomial interpolation for quality curves (requires `interpolation` feature).
#[cfg(feature = "interpolation")]
pub use interpolation::{
    compute_gap_polynomials, fit_gap_polynomial, fit_power_law, linear_interpolate,
    GapPolynomial, InterpolationConfig, InterpolationTable,
};
