//! Re-exports of metric crate types for convenience.
//!
//! This module provides a single place to import common types from the metrics
//! crates (dssim-core, butteraugli, ssimulacra2, fast-ssim2), allowing zen*
//! projects to depend only on codec-eval instead of each metric crate separately.
//!
//! # Example
//!
//! ```rust
//! use codec_eval::metrics::prelude::*;
//!
//! // Now you have access to:
//! // - Dssim, DssimImage (from dssim-core)
//! // - butteraugli, ButteraugliParams (from butteraugli)
//! // - compute_frame_ssimulacra2, Xyb (from ssimulacra2)
//! // - Ssimulacra2 (from fast-ssim2)
//! // - ImgRef, ImgVec (from imgref)
//! // - RGB8, RGBA8, RGB16, RGBA16 (from rgb)
//! ```
//!
//! # Note
//!
//! This is a convenience module. You can still depend on the individual metric
//! crates directly if you prefer. This module just provides a single dependency
//! point with consistent versions.

// ============================================================================
// DSSIM (Structural Similarity)
// ============================================================================

/// DSSIM metric calculator (from dssim-core).
///
/// Used for computing structural dissimilarity between images.
pub use dssim_core::Dssim;

/// DSSIM image wrapper (from dssim-core).
///
/// Wrap your images in this type before passing to Dssim::compare.
pub use dssim_core::DssimImage;

/// SSIM heatmap (from dssim-core).
///
/// Returned by Dssim::compare as the second element of the tuple.
#[doc(hidden)]
pub use dssim_core::SsimMap;

// ============================================================================
// Butteraugli (Perceptual Quality)
// ============================================================================

/// Butteraugli metric function (from butteraugli).
///
/// Compare two images and return a perceptual quality score.
/// Scores < 1.0 indicate imperceptible difference.
pub use butteraugli::butteraugli;

/// Butteraugli parameters (from butteraugli).
///
/// Configure intensity target, HF asymmetry, and other parameters.
pub use butteraugli::ButteraugliParams;

/// Butteraugli result (from butteraugli).
///
/// Contains the overall score and optional diffmap.
pub use butteraugli::ButteraugliResult;

// ============================================================================
// SSIMULACRA2 (Best Correlation with Human Perception)
// ============================================================================

/// Compute SSIMULACRA2 score with SIMD acceleration (from fast-ssim2).
///
/// This is the recommended SSIMULACRA2 implementation, providing significantly
/// better performance than the reference implementation while producing
/// identical results.
pub use fast_ssim2::compute_ssimulacra2;

/// SSIMULACRA2 configuration for customizing computation (from fast-ssim2).
pub use fast_ssim2::Ssimulacra2Config;

/// Precomputed reference for repeated comparisons (from fast-ssim2).
///
/// Use this when comparing multiple distorted images against the same reference
/// to avoid recomputing the reference image's analysis.
pub use fast_ssim2::Ssimulacra2Reference;

// ============================================================================
// Image Types (imgref, rgb)
// ============================================================================

/// Image reference type (from imgref).
///
/// Zero-copy view into an image buffer.
pub use imgref::ImgRef;

/// Owned image type (from imgref).
///
/// Owns its pixel buffer.
pub use imgref::ImgVec;

/// RGB pixel type with u8 components (from rgb).
pub use rgb::RGB8;

/// RGBA pixel type with u8 components (from rgb).
pub use rgb::RGBA8;

/// RGB pixel type with u16 components (from rgb).
pub use rgb::RGB16;

/// RGBA pixel type with u16 components (from rgb).
pub use rgb::RGBA16;

/// Generic RGB pixel type (from rgb).
pub use rgb::RGB;

/// Generic RGBA pixel type (from rgb).
pub use rgb::RGBA;
