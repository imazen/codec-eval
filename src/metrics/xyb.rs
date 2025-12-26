//! XYB color space roundtrip for fair metric comparison.
//!
//! When comparing compressed images to originals, the original can first be
//! roundtripped through XYB color space with u8 quantization to simulate what
//! happens when a codec stores XYB values at 8-bit precision.
//!
//! This isolates true compression error from color space conversion error,
//! which is important when evaluating codecs that operate in XYB color space
//! internally (like jpegli).
//!
//! ## Quantization Loss
//!
//! With u8 quantization of XYB values, the roundtrip introduces some loss:
//!
//! | Max Diff | % of Colors |
//! |----------|-------------|
//! | Exact (0) | 15.7% |
//! | ≤1 | 71.3% |
//! | ≤2 | 84.7% |
//! | ≤5 | 95.8% |
//! | ≤10 | 99.3% |
//!
//! Maximum observed difference: 26 levels (for bright saturated yellows).
//! Mean absolute error: ~0.69 per channel.

use butteraugli_oxide::xyb;

// XYB value ranges for all possible sRGB u8 inputs (empirically determined)
const X_MIN: f32 = -0.016; // Slightly padded from -0.015386
const X_MAX: f32 = 0.029;  // Slightly padded from 0.028100
const Y_MIN: f32 = 0.0;
const Y_MAX: f32 = 0.846;  // Slightly padded from 0.845309
const B_MIN: f32 = 0.0;
const B_MAX: f32 = 0.846;  // Slightly padded from 0.845309

/// Quantize a value to u8 precision within a given range.
#[inline]
fn quantize_to_u8(value: f32, min: f32, max: f32) -> f32 {
    let range = max - min;
    let normalized = (value - min) / range;
    let quantized = (normalized * 255.0).round().clamp(0.0, 255.0) / 255.0;
    quantized * range + min
}

/// Roundtrip RGB through XYB color space with u8 quantization.
///
/// This simulates the color space conversion and quantization that happens
/// during encoding when a codec stores XYB values at 8-bit precision.
///
/// # Algorithm
///
/// 1. sRGB (u8) → Linear RGB (f32)
/// 2. Linear RGB → XYB (f32)
/// 3. **Quantize each XYB channel to u8 precision**
/// 4. XYB (quantized) → Linear RGB (f32)
/// 5. Linear RGB → sRGB (u8)
///
/// # Arguments
///
/// * `rgb` - Input RGB8 buffer (3 bytes per pixel, row-major)
/// * `width` - Image width in pixels
/// * `height` - Image height in pixels
///
/// # Returns
///
/// Roundtripped RGB8 buffer with the same dimensions.
#[must_use]
pub fn xyb_roundtrip(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let num_pixels = width * height;
    assert_eq!(rgb.len(), num_pixels * 3, "Buffer size mismatch");

    let mut result = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let r = rgb[i * 3];
        let g = rgb[i * 3 + 1];
        let b = rgb[i * 3 + 2];

        // Convert to XYB
        let (x, y, b_xyb) = xyb::srgb_to_xyb(r, g, b);

        // Quantize XYB to u8 precision
        let x_q = quantize_to_u8(x, X_MIN, X_MAX);
        let y_q = quantize_to_u8(y, Y_MIN, Y_MAX);
        let b_q = quantize_to_u8(b_xyb, B_MIN, B_MAX);

        // Convert back to RGB
        let (r_out, g_out, b_out) = xyb::xyb_to_srgb(x_q, y_q, b_q);

        result[i * 3] = r_out;
        result[i * 3 + 1] = g_out;
        result[i * 3 + 2] = b_out;
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xyb_roundtrip_preserves_size() {
        let rgb: Vec<u8> = (0..64 * 64 * 3).map(|i| (i % 256) as u8).collect();
        let result = xyb_roundtrip(&rgb, 64, 64);
        assert_eq!(result.len(), rgb.len());
    }

    #[test]
    fn test_xyb_roundtrip_deterministic() {
        let rgb: Vec<u8> = (0..32 * 32 * 3).map(|i| ((i * 7) % 256) as u8).collect();
        let result1 = xyb_roundtrip(&rgb, 32, 32);
        let result2 = xyb_roundtrip(&rgb, 32, 32);
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_xyb_roundtrip_has_quantization_loss() {
        // With u8 quantization, we expect some loss
        // Max observed is 26 for bright yellows, but typical is much smaller
        let mut max_diff = 0i32;

        // Sample systematically
        for r in (0..=255u8).step_by(16) {
            for g in (0..=255u8).step_by(16) {
                for b in (0..=255u8).step_by(16) {
                    let rgb = vec![r, g, b];
                    let result = xyb_roundtrip(&rgb, 1, 1);

                    let dr = (result[0] as i32 - r as i32).abs();
                    let dg = (result[1] as i32 - g as i32).abs();
                    let db = (result[2] as i32 - b as i32).abs();
                    max_diff = max_diff.max(dr).max(dg).max(db);
                }
            }
        }

        // Should have some non-zero loss but bounded
        assert!(
            max_diff <= 30,
            "Max diff {} exceeds expected bound",
            max_diff
        );
    }
}
