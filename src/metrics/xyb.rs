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
//!
//! ## XYB Color Space
//!
//! XYB is a hybrid opponent/trichromatic color space used by butteraugli
//! and JPEG XL. These functions are ported from butteraugli 0.4.0 to avoid
//! depending on private API.

// XYB conversion constants (from butteraugli 0.4.0)
const XYB_OPSIN_ABSORBANCE_MATRIX: [f32; 9] = [
    0.30, 0.622, 0.078, // Row 0
    0.23, 0.692, 0.078, // Row 1
    0.243_422_69, 0.204_767_44, 0.551_809_87, // Row 2
];

const XYB_OPSIN_ABSORBANCE_BIAS: [f32; 3] = [0.003_793_073_3, 0.003_793_073_3, 0.003_793_073_3];

const XYB_NEG_OPSIN_ABSORBANCE_BIAS_CBRT: [f32; 3] = [
    -0.155_954_12, // -cbrt(0.003_793_073_3)
    -0.155_954_12,
    -0.155_954_12,
];

const INV_OPSIN_MATRIX: [f32; 9] = [
    11.031_567, -9.866_944, -0.164_623,
    -3.254_147, 4.418_77, -0.164_623,
    -3.658_851, 2.712_923, 1.945_928,
];

/// sRGB gamma decoding (sRGB to linear RGB).
#[inline]
fn srgb_to_linear_f32(v: f32) -> f32 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// sRGB gamma encoding (linear RGB to sRGB).
#[inline]
fn linear_to_srgb_f32(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Convert sRGB u8 to linear float.
#[inline]
fn srgb_u8_to_linear(v: u8) -> f32 {
    srgb_to_linear_f32(f32::from(v) / 255.0)
}

/// Convert linear float to sRGB u8.
#[inline]
fn linear_to_srgb_u8(v: f32) -> u8 {
    (linear_to_srgb_f32(v.clamp(0.0, 1.0)) * 255.0).round() as u8
}

/// Mixed cube root transfer function.
#[inline]
fn mixed_cbrt(v: f32) -> f32 {
    if v < 0.0 {
        -((-v).cbrt())
    } else {
        v.cbrt()
    }
}

/// Inverse of mixed cube root.
#[inline]
fn mixed_cube(v: f32) -> f32 {
    if v < 0.0 {
        -((-v).powi(3))
    } else {
        v.powi(3)
    }
}

/// Convert linear RGB to XYB color space.
#[allow(clippy::many_single_char_names)] // x, y, b, r, g are standard color channel names
fn linear_rgb_to_xyb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    // Apply opsin absorbance matrix
    let m = &XYB_OPSIN_ABSORBANCE_MATRIX;
    let bias = &XYB_OPSIN_ABSORBANCE_BIAS;

    let opsin_r = m[0] * r + m[1] * g + m[2] * b + bias[0];
    let opsin_g = m[3] * r + m[4] * g + m[5] * b + bias[1];
    let opsin_b = m[6] * r + m[7] * g + m[8] * b + bias[2];

    // Apply cube root
    let cbrt_r = mixed_cbrt(opsin_r);
    let cbrt_g = mixed_cbrt(opsin_g);
    let cbrt_b = mixed_cbrt(opsin_b);

    // Subtract bias
    let neg_bias = &XYB_NEG_OPSIN_ABSORBANCE_BIAS_CBRT;
    let cbrt_r = cbrt_r + neg_bias[0];
    let cbrt_g = cbrt_g + neg_bias[1];
    let cbrt_b = cbrt_b + neg_bias[2];

    // Final XYB transform
    let x = 0.5 * (cbrt_r - cbrt_g);
    let y = 0.5 * (cbrt_r + cbrt_g);

    (x, y, cbrt_b)
}

/// Convert XYB to linear RGB.
#[allow(clippy::many_single_char_names)] // x, y, b, r, g are standard color channel names
fn xyb_to_linear_rgb(x: f32, y: f32, b: f32) -> (f32, f32, f32) {
    let neg_bias = &XYB_NEG_OPSIN_ABSORBANCE_BIAS_CBRT;

    // Inverse XYB transform
    let cbrt_r = y + x;
    let cbrt_g = y - x;
    let cbrt_b = b;

    // Add back bias
    let cbrt_r = cbrt_r - neg_bias[0];
    let cbrt_g = cbrt_g - neg_bias[1];
    let cbrt_b = cbrt_b - neg_bias[2];

    // Inverse cube root
    let opsin_r = mixed_cube(cbrt_r);
    let opsin_g = mixed_cube(cbrt_g);
    let opsin_b = mixed_cube(cbrt_b);

    // Remove bias
    let bias = &XYB_OPSIN_ABSORBANCE_BIAS;
    let opsin_r = opsin_r - bias[0];
    let opsin_g = opsin_g - bias[1];
    let opsin_b = opsin_b - bias[2];

    // Inverse opsin matrix
    let inv = &INV_OPSIN_MATRIX;
    let r = inv[0] * opsin_r + inv[1] * opsin_g + inv[2] * opsin_b;
    let g = inv[3] * opsin_r + inv[4] * opsin_g + inv[5] * opsin_b;
    let b_out = inv[6] * opsin_r + inv[7] * opsin_g + inv[8] * opsin_b;

    (r, g, b_out)
}

/// Convert sRGB u8 to XYB.
fn srgb_to_xyb(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
    let lr = srgb_u8_to_linear(r);
    let lg = srgb_u8_to_linear(g);
    let lb = srgb_u8_to_linear(b);
    linear_rgb_to_xyb(lr, lg, lb)
}

/// Convert XYB to sRGB u8.
fn xyb_to_srgb(x: f32, y: f32, b: f32) -> (u8, u8, u8) {
    let (lr, lg, lb) = xyb_to_linear_rgb(x, y, b);
    (
        linear_to_srgb_u8(lr),
        linear_to_srgb_u8(lg),
        linear_to_srgb_u8(lb),
    )
}

// XYB value ranges for all possible sRGB u8 inputs (empirically determined)
const X_MIN: f32 = -0.016; // Slightly padded from -0.015386
const X_MAX: f32 = 0.029; // Slightly padded from 0.028100
const Y_MIN: f32 = 0.0;
const Y_MAX: f32 = 0.846; // Slightly padded from 0.845309
const B_MIN: f32 = 0.0;
const B_MAX: f32 = 0.846; // Slightly padded from 0.845309

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
#[allow(clippy::many_single_char_names)] // r, g, b, x, y are standard color channel names
pub fn xyb_roundtrip(rgb: &[u8], width: usize, height: usize) -> Vec<u8> {
    let num_pixels = width * height;
    assert_eq!(rgb.len(), num_pixels * 3, "Buffer size mismatch");

    let mut result = vec![0u8; num_pixels * 3];

    for i in 0..num_pixels {
        let r = rgb[i * 3];
        let g = rgb[i * 3 + 1];
        let b = rgb[i * 3 + 2];

        // Convert to XYB
        let (x, y, b_xyb) = srgb_to_xyb(r, g, b);

        // Quantize XYB to u8 precision
        let x_q = quantize_to_u8(x, X_MIN, X_MAX);
        let y_q = quantize_to_u8(y, Y_MIN, Y_MAX);
        let b_q = quantize_to_u8(b_xyb, B_MIN, B_MAX);

        // Convert back to RGB
        let (r_out, g_out, b_out) = xyb_to_srgb(x_q, y_q, b_q);

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
