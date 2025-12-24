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
//! - [`metrics`]: Quality metrics (DSSIM, PSNR)
//! - [`eval`]: Evaluation session and report generation
//! - [`corpus`]: Test corpus management
//! - [`import`]: CSV import for third-party results
//! - [`stats`]: Statistical analysis and Pareto front

pub mod corpus;
pub mod error;
pub mod eval;
pub mod import;
pub mod metrics;
pub mod stats;
pub mod viewing;

// Re-export commonly used types
pub use error::{Error, Result};
pub use eval::{
    session::{EvalConfig, EvalSession, ImageData},
    report::{CorpusReport, ImageReport, CodecResult},
};
pub use metrics::{MetricConfig, MetricResult};
pub use viewing::ViewingCondition;
pub use corpus::{Corpus, CorpusImage, ImageCategory, SparseCheckout, SparseFilter, SparseStatus};
pub use import::{CsvImporter, CsvSchema, ExternalResult};
pub use stats::{ParetoFront, RDPoint, Summary};
