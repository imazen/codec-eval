//! Lightweight evaluation helpers for codec testing.
//!
//! These helpers provide simple APIs for common use cases:
//! - Evaluate a single encoded image against a reference
//! - Assert quality thresholds in CI tests
//! - Quick quality checks during development
//!
//! # Example
//!
//! ```rust,ignore
//! use codec_eval::eval::helpers::{evaluate_single, assert_quality};
//! use codec_eval::metrics::MetricConfig;
//! use imgref::ImgVec;
//! use rgb::RGB8;
//!
//! # fn encode_image(img: &ImgVec<RGB8>) -> Vec<u8> { vec![] }
//! # fn decode_image(data: &[u8]) -> ImgVec<RGB8> { ImgVec::new(vec![], 8, 8) }
//! // Evaluate quality
//! let reference: ImgVec<RGB8> = // ...
//! # ImgVec::new(vec![], 8, 8);
//! let encoded_data = encode_image(&reference);
//! let decoded = decode_image(&encoded_data);
//!
//! let config = MetricConfig::perceptual();
//! let result = evaluate_single(&reference, &decoded, &config).unwrap();
//!
//! println!("DSSIM: {:?}", result.dssim);
//! println!("SSIMULACRA2: {:?}", result.ssimulacra2);
//!
//! // Assert quality in tests
//! assert_quality(&reference, &decoded, Some(80.0), Some(0.002)).unwrap();
//! ```

use crate::error::{Error, Result};
use crate::metrics::{
    self, butteraugli, dssim, ssimulacra2, MetricConfig, MetricResult, PerceptionLevel,
};
use crate::viewing::ViewingCondition;
use imgref::ImgVec;
use rgb::{RGB8, RGBA};

/// Convert RGB8 image to RGBA<f32> with linear RGB values.
///
/// This applies sRGB gamma decoding (sRGB → linear RGB) and adds alpha = 1.0.
fn rgb8_to_rgba_f32(img: &ImgVec<RGB8>) -> ImgVec<RGBA<f32>> {
    let pixels: Vec<RGBA<f32>> = img
        .pixels()
        .map(|p| {
            let r = srgb_to_linear(p.r);
            let g = srgb_to_linear(p.g);
            let b = srgb_to_linear(p.b);
            RGBA::new(r, g, b, 1.0)
        })
        .collect();
    ImgVec::new(pixels, img.width(), img.height())
}

/// Apply sRGB gamma decoding (sRGB u8 → linear f32).
#[inline]
fn srgb_to_linear(srgb: u8) -> f32 {
    let s = f32::from(srgb) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Evaluate a single encoded image against a reference.
///
/// This is a convenience wrapper around the individual metric calculation functions.
/// It's designed for simple use cases where you want to quickly evaluate the quality
/// of an encoded image without setting up a full `EvalSession`.
///
/// # Arguments
///
/// * `reference` - Reference image (original)
/// * `encoded` - Encoded/decoded image to compare
/// * `config` - Which metrics to calculate
///
/// # Returns
///
/// `MetricResult` containing the calculated metric values.
///
/// # Errors
///
/// Returns an error if:
/// - Images have different dimensions
/// - Images are too small for the metric (minimum 8x8 for butteraugli)
/// - Metric calculation fails
///
/// # Example
///
/// ```rust,ignore
/// use codec_eval::eval::helpers::evaluate_single;
/// use codec_eval::metrics::MetricConfig;
///
/// let config = MetricConfig::perceptual();
/// let result = evaluate_single(&reference, &encoded, &config)?;
///
/// if let Some(dssim) = result.dssim {
///     println!("DSSIM: {:.6}", dssim);
/// }
/// ```
pub fn evaluate_single(
    reference: &ImgVec<RGB8>,
    encoded: &ImgVec<RGB8>,
    config: &MetricConfig,
) -> Result<MetricResult> {
    // Validate dimensions match
    if reference.width() != encoded.width() || reference.height() != encoded.height() {
        return Err(Error::DimensionMismatch {
            expected: (reference.width(), reference.height()),
            actual: (encoded.width(), encoded.height()),
        });
    }

    let width = reference.width();
    let height = reference.height();

    // Apply XYB roundtrip to reference if requested
    let reference_img: ImgVec<RGB8>;
    let reference_final = if config.xyb_roundtrip {
        let ref_bytes: Vec<u8> = reference
            .pixels()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect();
        let roundtripped = metrics::xyb_roundtrip(&ref_bytes, width, height);
        let pixels: Vec<RGB8> = roundtripped
            .chunks_exact(3)
            .map(|chunk| RGB8::new(chunk[0], chunk[1], chunk[2]))
            .collect();
        reference_img = ImgVec::new(pixels, width, height);
        &reference_img
    } else {
        reference
    };

    let mut result = MetricResult::default();

    // Calculate requested metrics
    // DSSIM requires RGBA<f32> format
    if config.dssim {
        let ref_rgba = rgb8_to_rgba_f32(reference_final);
        let enc_rgba = rgb8_to_rgba_f32(encoded);
        let viewing = ViewingCondition::desktop();
        result.dssim = Some(dssim::calculate_dssim(&ref_rgba, &enc_rgba, &viewing)?);
    }

    // SSIMULACRA2 and Butteraugli use raw u8 buffers
    if config.ssimulacra2 || config.butteraugli || config.psnr {
        let ref_buf: Vec<u8> = reference_final
            .pixels()
            .flat_map(|p| [p.r, p.g, p.b])
            .collect();
        let enc_buf: Vec<u8> = encoded.pixels().flat_map(|p| [p.r, p.g, p.b]).collect();

        if config.ssimulacra2 {
            result.ssimulacra2 =
                Some(ssimulacra2::calculate_ssimulacra2(&ref_buf, &enc_buf, width, height)?);
        }

        if config.butteraugli {
            result.butteraugli =
                Some(butteraugli::calculate_butteraugli(&ref_buf, &enc_buf, width, height)?);
        }

        if config.psnr {
            result.psnr = Some(metrics::calculate_psnr(&ref_buf, &enc_buf, width, height));
        }
    }

    Ok(result)
}

/// Assert that quality meets specified thresholds.
///
/// This is designed for use in CI tests and benchmarks. It calculates quality
/// metrics and fails if they don't meet the specified thresholds.
///
/// # Arguments
///
/// * `reference` - Reference image (original)
/// * `encoded` - Encoded/decoded image to compare
/// * `min_ssimulacra2` - Minimum acceptable SSIMULACRA2 score (optional)
/// * `max_dssim` - Maximum acceptable DSSIM value (optional)
///
/// # Returns
///
/// `Ok(())` if quality meets all specified thresholds.
///
/// # Errors
///
/// Returns an error if:
/// - Images have different dimensions
/// - Quality is below the specified thresholds
/// - Metric calculation fails
///
/// # Example
///
/// ```rust,ignore
/// use codec_eval::eval::helpers::assert_quality;
///
/// // Assert SSIMULACRA2 >= 80.0 and DSSIM <= 0.002
/// assert_quality(&reference, &encoded, Some(80.0), Some(0.002))?;
///
/// // Assert only SSIMULACRA2 >= 90.0
/// assert_quality(&reference, &encoded, Some(90.0), None)?;
///
/// // Assert only DSSIM <= 0.001
/// assert_quality(&reference, &encoded, None, Some(0.001))?;
/// ```
pub fn assert_quality(
    reference: &ImgVec<RGB8>,
    encoded: &ImgVec<RGB8>,
    min_ssimulacra2: Option<f64>,
    max_dssim: Option<f64>,
) -> Result<()> {
    // Build config based on what thresholds are specified
    let config = MetricConfig {
        dssim: max_dssim.is_some(),
        ssimulacra2: min_ssimulacra2.is_some(),
        butteraugli: false,
        psnr: false,
        xyb_roundtrip: false,
    };

    let result = evaluate_single(reference, encoded, &config)?;

    // Check thresholds
    if let Some(threshold) = min_ssimulacra2 {
        if let Some(score) = result.ssimulacra2 {
            if score < threshold {
                return Err(Error::QualityBelowThreshold {
                    metric: "SSIMULACRA2".to_string(),
                    value: score,
                    threshold,
                });
            }
        }
    }

    if let Some(threshold) = max_dssim {
        if let Some(score) = result.dssim {
            if score > threshold {
                return Err(Error::QualityBelowThreshold {
                    metric: "DSSIM".to_string(),
                    value: score,
                    threshold,
                });
            }
        }
    }

    Ok(())
}

/// Assert that quality is at the specified perception level or better.
///
/// This is a more semantic way to assert quality thresholds based on
/// perceptual categories rather than raw metric values.
///
/// # Arguments
///
/// * `reference` - Reference image (original)
/// * `encoded` - Encoded/decoded image to compare
/// * `min_level` - Minimum acceptable perception level
///
/// # Returns
///
/// `Ok(())` if quality is at the specified level or better.
///
/// # Errors
///
/// Returns an error if:
/// - Images have different dimensions
/// - Quality is below the specified perception level
/// - Metric calculation fails
///
/// # Example
///
/// ```rust,ignore
/// use codec_eval::eval::helpers::assert_perception_level;
/// use codec_eval::metrics::PerceptionLevel;
///
/// // Assert quality is at least "Subtle" (DSSIM < 0.0015)
/// assert_perception_level(&reference, &encoded, PerceptionLevel::Subtle)?;
///
/// // Assert quality is "Imperceptible" (DSSIM < 0.0003)
/// assert_perception_level(&reference, &encoded, PerceptionLevel::Imperceptible)?;
/// ```
pub fn assert_perception_level(
    reference: &ImgVec<RGB8>,
    encoded: &ImgVec<RGB8>,
    min_level: PerceptionLevel,
) -> Result<()> {
    let config = MetricConfig {
        dssim: true,
        ssimulacra2: false,
        butteraugli: false,
        psnr: false,
        xyb_roundtrip: false,
    };

    let result = evaluate_single(reference, encoded, &config)?;

    if let Some(dssim) = result.dssim {
        let actual_level = PerceptionLevel::from_dssim(dssim);
        let min_level_value = min_level as u8;
        let actual_level_value = actual_level as u8;

        if actual_level_value > min_level_value {
            return Err(Error::QualityBelowThreshold {
                metric: format!("PerceptionLevel (DSSIM {dssim:.6})"),
                value: actual_level_value.into(),
                threshold: min_level_value.into(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_image(width: usize, height: usize, pattern: u8) -> ImgVec<RGB8> {
        let pixels: Vec<RGB8> = (0..width * height)
            .map(|i| {
                let base = (i + usize::from(pattern)) % 256;
                RGB8::new(base as u8, (base + 50) as u8, (base + 100) as u8)
            })
            .collect();
        ImgVec::new(pixels, width, height)
    }

    #[test]
    fn test_evaluate_single_identical() {
        let img = create_test_image(64, 64, 0);
        let config = MetricConfig::perceptual();

        let result = evaluate_single(&img, &img, &config).unwrap();

        // Identical images should have perfect scores
        assert!(result.dssim.unwrap() < 0.0001);
        assert!(result.ssimulacra2.unwrap() > 99.0);
        assert!(result.butteraugli.unwrap() < 0.1);
    }

    #[test]
    fn test_evaluate_single_dimension_mismatch() {
        let img1 = create_test_image(64, 64, 0);
        let img2 = create_test_image(32, 32, 0);
        let config = MetricConfig::perceptual();

        let result = evaluate_single(&img1, &img2, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_assert_quality_pass() {
        let img = create_test_image(64, 64, 0);

        // Identical images should easily pass these thresholds
        assert!(assert_quality(&img, &img, Some(90.0), Some(0.001)).is_ok());
    }

    #[test]
    fn test_assert_quality_fail_ssimulacra2() {
        let img1 = create_test_image(64, 64, 0);
        let img2 = create_test_image(64, 64, 50);

        // Different images won't meet high SSIMULACRA2 threshold
        assert!(assert_quality(&img1, &img2, Some(99.0), None).is_err());
    }

    #[test]
    fn test_assert_perception_level() {
        let img = create_test_image(64, 64, 0);

        // Identical images should be imperceptible
        assert!(assert_perception_level(&img, &img, PerceptionLevel::Imperceptible).is_ok());
    }
}
