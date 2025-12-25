//! Butteraugli metric calculation.
//!
//! Butteraugli is a perceptual image quality metric developed by Google.
//! Lower scores indicate better quality (more similar to reference).
//!
//! Score interpretation:
//! - < 1.0: Imperceptible difference
//! - < 2.0: Marginal difference
//! - < 3.0: Subtle difference
//! - < 5.0: Noticeable difference
//! - >= 5.0: Degraded

use butteraugli_oxide::{compute_butteraugli, ButteraugliParams};

use crate::error::{Error, Result};

/// Calculate Butteraugli distance between two images.
///
/// # Arguments
///
/// * `reference` - Reference image as RGB8 pixel data.
/// * `test` - Test image as RGB8 pixel data.
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Returns
///
/// Butteraugli score where lower is better (<1.0 = imperceptible).
///
/// # Errors
///
/// Returns an error if the images have different sizes or if calculation fails.
pub fn calculate_butteraugli(
    reference: &[u8],
    test: &[u8],
    width: usize,
    height: usize,
) -> Result<f64> {
    if reference.len() != test.len() {
        return Err(Error::DimensionMismatch {
            expected: (width as u32, height as u32),
            actual: ((test.len() / 3 / height) as u32, height as u32),
        });
    }

    let expected_len = width * height * 3;
    if reference.len() != expected_len {
        return Err(Error::MetricCalculation {
            metric: "Butteraugli".to_string(),
            reason: format!(
                "Invalid image size: expected {} bytes, got {}",
                expected_len,
                reference.len()
            ),
        });
    }

    let params = ButteraugliParams::default();
    let result = compute_butteraugli(reference, test, width, height, &params);

    Ok(result.score)
}

/// Calculate Butteraugli with custom intensity target.
///
/// The intensity target affects how the metric perceives differences
/// at different brightness levels.
///
/// # Arguments
///
/// * `reference` - Reference image as RGB8 pixel data.
/// * `test` - Test image as RGB8 pixel data.
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
/// * `intensity_target` - Target display intensity in nits (default: 80.0).
///
/// # Returns
///
/// Butteraugli score where lower is better.
pub fn calculate_butteraugli_with_intensity(
    reference: &[u8],
    test: &[u8],
    width: usize,
    height: usize,
    intensity_target: f32,
) -> Result<f64> {
    if reference.len() != test.len() {
        return Err(Error::DimensionMismatch {
            expected: (width as u32, height as u32),
            actual: ((test.len() / 3 / height) as u32, height as u32),
        });
    }

    let expected_len = width * height * 3;
    if reference.len() != expected_len {
        return Err(Error::MetricCalculation {
            metric: "Butteraugli".to_string(),
            reason: format!(
                "Invalid image size: expected {} bytes, got {}",
                expected_len,
                reference.len()
            ),
        });
    }

    let params = ButteraugliParams::default().with_intensity_target(intensity_target);
    let result = compute_butteraugli(reference, test, width, height, &params);

    Ok(result.score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identical_images() {
        let data: Vec<u8> = (0..100 * 100 * 3).map(|i| (i % 256) as u8).collect();
        let score = calculate_butteraugli(&data, &data, 100, 100).unwrap();
        // Identical images should have score close to 0
        assert!(score < 0.01, "Identical images should have score ~0, got {score}");
    }

    #[test]
    fn test_different_images() {
        let ref_data: Vec<u8> = vec![100u8; 100 * 100 * 3];
        let test_data: Vec<u8> = vec![200u8; 100 * 100 * 3];
        let score = calculate_butteraugli(&ref_data, &test_data, 100, 100).unwrap();
        // Very different images should have high score
        assert!(score > 1.0, "Very different images should have high score, got {score}");
    }

    #[test]
    fn test_size_mismatch() {
        let small: Vec<u8> = vec![128u8; 50 * 50 * 3];
        let large: Vec<u8> = vec![128u8; 100 * 100 * 3];
        let result = calculate_butteraugli(&small, &large, 100, 100);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_intensity() {
        let data: Vec<u8> = (0..100 * 100 * 3).map(|i| (i % 256) as u8).collect();
        let score = calculate_butteraugli_with_intensity(&data, &data, 100, 100, 250.0).unwrap();
        assert!(score < 0.01, "Identical images should have score ~0 at any intensity");
    }
}
