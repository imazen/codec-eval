//! DSSIM (Structural Dissimilarity) metric calculation.
//!
//! Wraps the `dssim-core` crate for perceptual image comparison.
//!
//! # ICC Profile Support
//!
//! Use [`calculate_dssim_icc`] for images with non-sRGB color profiles.

use dssim_core::Dssim;
use imgref::ImgVec;
use rgb::RGBA;

use super::icc::ColorProfile;
use crate::error::{Error, Result};
use crate::viewing::ViewingCondition;

/// Calculate DSSIM between two images.
///
/// # Arguments
///
/// * `reference` - Reference image as RGBA f32 values (0.0-1.0).
/// * `test` - Test image as RGBA f32 values (0.0-1.0).
/// * `viewing` - Viewing condition for PPD-based adjustment (currently unused
///   by dssim-core, but reserved for future use).
///
/// # Returns
///
/// DSSIM value where 0 = identical, higher = more different.
/// Typical thresholds:
/// - < 0.0003: Imperceptible
/// - < 0.0007: Marginal
/// - < 0.0015: Subtle
/// - < 0.003: Noticeable
/// - >= 0.003: Degraded
///
/// # Errors
///
/// Returns an error if the images have different dimensions or if DSSIM
/// calculation fails.
pub fn calculate_dssim(
    reference: &ImgVec<RGBA<f32>>,
    test: &ImgVec<RGBA<f32>>,
    _viewing: &ViewingCondition,
) -> Result<f64> {
    if reference.width() != test.width() || reference.height() != test.height() {
        return Err(Error::DimensionMismatch {
            expected: (reference.width() as u32, reference.height() as u32),
            actual: (test.width() as u32, test.height() as u32),
        });
    }

    let dssim = Dssim::new();

    let ref_image = dssim
        .create_image(reference)
        .ok_or_else(|| Error::MetricCalculation {
            metric: "DSSIM".to_string(),
            reason: "Failed to create reference image".to_string(),
        })?;

    let test_image = dssim
        .create_image(test)
        .ok_or_else(|| Error::MetricCalculation {
            metric: "DSSIM".to_string(),
            reason: "Failed to create test image".to_string(),
        })?;

    let (dssim_val, _ssim_maps) = dssim.compare(&ref_image, test_image);

    Ok(f64::from(dssim_val))
}

/// Convert a single sRGB u8 component to linear f32.
///
/// Applies the sRGB transfer function (inverse gamma) to convert from
/// gamma-encoded sRGB to linear light values.
#[inline]
fn srgb_to_linear(srgb: u8) -> f32 {
    let s = f32::from(srgb) / 255.0;
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Convert RGB8 image data to the format needed for DSSIM calculation.
///
/// Applies proper sRGB-to-linear conversion (inverse gamma) as required
/// by dssim-core. This matches dssim-core's `ToRGBAPLU::to_rgblu()` behavior.
///
/// # Arguments
///
/// * `data` - RGB8 pixel data in row-major order (sRGB gamma-encoded).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Returns
///
/// An `ImgVec<RGBA<f32>>` with linear light values suitable for DSSIM calculation.
#[must_use]
pub fn rgb8_to_dssim_image(data: &[u8], width: usize, height: usize) -> ImgVec<RGBA<f32>> {
    let pixels: Vec<RGBA<f32>> = data
        .chunks_exact(3)
        .map(|rgb| RGBA {
            r: srgb_to_linear(rgb[0]),
            g: srgb_to_linear(rgb[1]),
            b: srgb_to_linear(rgb[2]),
            a: 1.0,
        })
        .collect();

    ImgVec::new(pixels, width, height)
}

/// Convert RGBA8 image data to the format needed for DSSIM calculation.
///
/// Applies proper sRGB-to-linear conversion (inverse gamma) for RGB channels.
/// Alpha channel is normalized linearly (0-255 → 0.0-1.0).
///
/// # Arguments
///
/// * `data` - RGBA8 pixel data in row-major order (sRGB gamma-encoded RGB + linear alpha).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Returns
///
/// An `ImgVec<RGBA<f32>>` with linear light RGB values suitable for DSSIM calculation.
#[must_use]
pub fn rgba8_to_dssim_image(data: &[u8], width: usize, height: usize) -> ImgVec<RGBA<f32>> {
    let pixels: Vec<RGBA<f32>> = data
        .chunks_exact(4)
        .map(|rgba| RGBA {
            r: srgb_to_linear(rgba[0]),
            g: srgb_to_linear(rgba[1]),
            b: srgb_to_linear(rgba[2]),
            a: f32::from(rgba[3]) / 255.0, // Alpha is linear, not gamma-encoded
        })
        .collect();

    ImgVec::new(pixels, width, height)
}

/// Calculate DSSIM with ICC profile support.
///
/// This function transforms both images to sRGB before comparison.
///
/// # Arguments
///
/// * `reference` - Reference image as RGB8 pixel data.
/// * `reference_profile` - Color profile of the reference image.
/// * `test` - Test image as RGB8 pixel data.
/// * `test_profile` - Color profile of the test image.
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
/// * `viewing` - Viewing condition for PPD-based adjustment.
pub fn calculate_dssim_icc(
    reference: &[u8],
    reference_profile: &ColorProfile,
    test: &[u8],
    test_profile: &ColorProfile,
    width: usize,
    height: usize,
    viewing: &ViewingCondition,
) -> Result<f64> {
    let (ref_srgb, test_srgb) =
        super::icc::prepare_for_comparison(reference, reference_profile, test, test_profile)?;

    let ref_img = rgb8_to_dssim_image(&ref_srgb, width, height);
    let test_img = rgb8_to_dssim_image(&test_srgb, width, height);

    calculate_dssim(&ref_img, &test_img, viewing)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_images() {
        let pixels: Vec<RGBA<f32>> = (0..100 * 100)
            .map(|_| RGBA {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            })
            .collect();
        let img = ImgVec::new(pixels, 100, 100);

        let dssim = calculate_dssim(&img, &img, &ViewingCondition::desktop()).unwrap();
        assert!(
            dssim < 0.0001,
            "Identical images should have near-zero DSSIM"
        );
    }

    #[test]
    fn test_different_images() {
        let ref_pixels: Vec<RGBA<f32>> = (0..100 * 100)
            .map(|_| RGBA {
                r: 0.3,
                g: 0.3,
                b: 0.3,
                a: 1.0,
            })
            .collect();
        let test_pixels: Vec<RGBA<f32>> = (0..100 * 100)
            .map(|_| RGBA {
                r: 0.7,
                g: 0.7,
                b: 0.7,
                a: 1.0,
            })
            .collect();

        let ref_img = ImgVec::new(ref_pixels, 100, 100);
        let test_img = ImgVec::new(test_pixels, 100, 100);

        let dssim = calculate_dssim(&ref_img, &test_img, &ViewingCondition::desktop()).unwrap();
        assert!(dssim > 0.0, "Different images should have non-zero DSSIM");
    }

    #[test]
    fn test_dimension_mismatch() {
        let small: Vec<RGBA<f32>> = (0..50 * 50)
            .map(|_| RGBA {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            })
            .collect();
        let large: Vec<RGBA<f32>> = (0..100 * 100)
            .map(|_| RGBA {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 1.0,
            })
            .collect();

        let small_img = ImgVec::new(small, 50, 50);
        let large_img = ImgVec::new(large, 100, 100);

        let result = calculate_dssim(&small_img, &large_img, &ViewingCondition::desktop());
        assert!(matches!(result, Err(Error::DimensionMismatch { .. })));
    }

    #[test]
    fn test_rgb8_conversion() {
        let rgb_data = vec![255u8, 0, 0, 0, 255, 0]; // Red, Green pixels
        let img = rgb8_to_dssim_image(&rgb_data, 2, 1);

        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 1);
        let pixels: Vec<_> = img.pixels().collect();
        assert!((pixels[0].r - 1.0).abs() < 0.001);
        assert!((pixels[1].g - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_rgba8_conversion() {
        let rgba_data = vec![255u8, 0, 0, 128, 0, 255, 0, 255]; // Semi-transparent red, opaque green
        let img = rgba8_to_dssim_image(&rgba_data, 2, 1);

        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 1);
        let pixels: Vec<_> = img.pixels().collect();
        assert!((pixels[0].a - 0.502).abs() < 0.01);
        assert!((pixels[1].a - 1.0).abs() < 0.001);
    }
}
