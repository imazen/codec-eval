//! SSIMULACRA2 metric calculation.
//!
//! SSIMULACRA2 is a perceptual image quality metric that correlates well with
//! human visual perception. Higher scores indicate better quality.
//!
//! Score interpretation:
//! - 100: Identical
//! - > 90: Imperceptible difference
//! - > 80: Marginal difference
//! - > 70: Subtle difference
//! - > 50: Noticeable difference
//! - <= 50: Degraded
//!
//! # Implementation
//!
//! This module uses `fast-ssim2` for SIMD-accelerated SSIMULACRA2 calculation.
//! It provides significantly better performance than the reference `ssimulacra2`
//! implementation while producing identical results.
//!
//! # ICC Profile Support
//!
//! When comparing images with embedded ICC profiles, use [`calculate_ssimulacra2_icc`]
//! to ensure accurate color space conversion before comparison. This is critical for:
//!
//! - XYB JPEGs from jpegli (which embed custom ICC profiles)
//! - Wide-gamut images (Display P3, Rec.2020)
//! - Any image with non-sRGB color space
//!
//! Without proper ICC handling, scores can be off by 1-2 points at high quality levels.

use fast_ssim2::compute_ssimulacra2;
use imgref::ImgVec;

use super::icc::ColorProfile;
use crate::error::{Error, Result};

/// Calculate SSIMULACRA2 between two images.
///
/// # Arguments
///
/// * `reference` - Reference image as RGB8 pixel data (row-major, 3 bytes per pixel).
/// * `test` - Test image as RGB8 pixel data (row-major, 3 bytes per pixel).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Returns
///
/// SSIMULACRA2 score where higher is better (100 = identical).
///
/// # Errors
///
/// Returns an error if the images have different sizes or if calculation fails.
///
/// # Performance
///
/// This function uses `fast-ssim2` with SIMD acceleration. For the fastest
/// performance, enable the `unsafe-simd` feature on fast-ssim2 (requires
/// unsafe code, but significantly faster on modern CPUs).
pub fn calculate_ssimulacra2(
    reference: &[u8],
    test: &[u8],
    width: usize,
    height: usize,
) -> Result<f64> {
    if reference.len() != test.len() {
        return Err(Error::DimensionMismatch {
            expected: (width, height),
            actual: (test.len() / 3 / height, height),
        });
    }

    let expected_len = width * height * 3;
    if reference.len() != expected_len {
        return Err(Error::MetricCalculation {
            metric: "SSIMULACRA2".to_string(),
            reason: format!(
                "Invalid image size: expected {} bytes, got {}",
                expected_len,
                reference.len()
            ),
        });
    }

    // Convert flat RGB8 buffer to [u8; 3] array for fast-ssim2
    let ref_pixels: Vec<[u8; 3]> = reference
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();

    let test_pixels: Vec<[u8; 3]> = test
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();

    let ref_img = ImgVec::new(ref_pixels, width, height);
    let test_img = ImgVec::new(test_pixels, width, height);

    // fast-ssim2 uses ImgRef, so convert ImgVec to ImgRef
    compute_ssimulacra2(ref_img.as_ref(), test_img.as_ref()).map_err(|e| {
        Error::MetricCalculation {
            metric: "SSIMULACRA2".to_string(),
            reason: format!("Failed to compute SSIMULACRA2: {e:?}"),
        }
    })
}

/// Calculate SSIMULACRA2 with ICC profile support.
///
/// This function transforms both images to sRGB before comparison, ensuring
/// accurate results even when images have non-sRGB color profiles.
///
/// # Arguments
///
/// * `reference` - Reference image as RGB8 pixel data.
/// * `reference_profile` - Color profile of the reference image.
/// * `test` - Test image as RGB8 pixel data.
/// * `test_profile` - Color profile of the test image.
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Returns
///
/// SSIMULACRA2 score where higher is better (100 = identical).
///
/// # Example
///
/// ```ignore
/// use codec_eval::metrics::{ssimulacra2::calculate_ssimulacra2_icc, ColorProfile};
///
/// // For XYB JPEG with embedded ICC profile
/// let score = calculate_ssimulacra2_icc(
///     &reference_rgb,
///     &ColorProfile::Srgb,
///     &decoded_jpeg_rgb,
///     &ColorProfile::Icc(jpeg_icc_data),
///     width,
///     height,
/// )?;
/// ```
pub fn calculate_ssimulacra2_icc(
    reference: &[u8],
    reference_profile: &ColorProfile,
    test: &[u8],
    test_profile: &ColorProfile,
    width: usize,
    height: usize,
) -> Result<f64> {
    let (ref_srgb, test_srgb) =
        super::icc::prepare_for_comparison(reference, reference_profile, test, test_profile)?;

    calculate_ssimulacra2(&ref_srgb, &test_srgb, width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_images() {
        let data: Vec<u8> = (0..100 * 100 * 3).map(|i| (i % 256) as u8).collect();
        let score = calculate_ssimulacra2(&data, &data, 100, 100).unwrap();
        // Identical images should have score close to 100
        assert!(
            score > 99.0,
            "Identical images should have score ~100, got {score}"
        );
    }

    #[test]
    fn test_different_images() {
        let ref_data: Vec<u8> = vec![100u8; 100 * 100 * 3];
        let test_data: Vec<u8> = vec![200u8; 100 * 100 * 3];
        let score = calculate_ssimulacra2(&ref_data, &test_data, 100, 100).unwrap();
        // Very different images should have low score
        assert!(
            score < 80.0,
            "Very different images should have low score, got {score}"
        );
    }

    #[test]
    fn test_size_mismatch() {
        let small: Vec<u8> = vec![128u8; 50 * 50 * 3];
        let large: Vec<u8> = vec![128u8; 100 * 100 * 3];
        let result = calculate_ssimulacra2(&small, &large, 100, 100);
        assert!(result.is_err());
    }
}
