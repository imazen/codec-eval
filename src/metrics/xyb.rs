//! XYB color space roundtrip for fair metric comparison.
//!
//! When comparing compressed images to originals, the original can first be
//! roundtripped through XYB color space (RGB → XYB → RGB) to isolate true
//! compression error from color space conversion error.
//!
//! **Note:** With butteraugli-oxide's XYB implementation, the roundtrip is
//! **lossless** for all 16.7 million possible RGB colors. This means the
//! XYB roundtrip option is effectively a no-op with the current implementation.
//! However, it may be useful for comparing against codecs that use a different
//! (lossy) XYB implementation internally.

use butteraugli_oxide::xyb;

/// Roundtrip RGB through XYB color space.
///
/// This simulates the color space conversion that happens during encoding,
/// allowing metrics to measure only the compression loss and not the
/// unavoidable color space conversion loss.
///
/// # Algorithm
///
/// 1. sRGB (u8) → Linear RGB (f32)
/// 2. Linear RGB → XYB (f32)
/// 3. XYB → Linear RGB (f32)
/// 4. Linear RGB → sRGB (u8)
///
/// # Lossless Property
///
/// With butteraugli-oxide's implementation, this roundtrip is **lossless**
/// for all 16.7 million possible RGB colors. The f32 precision is sufficient
/// to perfectly reconstruct the original u8 values after the round trip.
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

        // Convert to XYB and back
        let (x, y, b_xyb) = xyb::srgb_to_xyb(r, g, b);
        let (r_out, g_out, b_out) = xyb::xyb_to_srgb(x, y, b_xyb);

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
    fn test_xyb_roundtrip_lossless() {
        // Verify that XYB roundtrip is lossless for a representative sample
        // (Full 16.7M color test is in bench_tests below)
        let test_colors = [
            [255u8, 0, 0],     // Red
            [0u8, 255, 0],     // Green
            [0u8, 0, 255],     // Blue
            [255u8, 255, 0],   // Yellow
            [0u8, 255, 255],   // Cyan
            [255u8, 0, 255],   // Magenta
            [0u8, 0, 0],       // Black
            [255u8, 255, 255], // White
            [128u8, 128, 128], // Gray
            [200u8, 150, 130], // Skin tone
            [135u8, 180, 230], // Sky blue
        ];

        for color in &test_colors {
            let rgb = vec![color[0], color[1], color[2]];
            let result = xyb_roundtrip(&rgb, 1, 1);
            assert_eq!(
                &result[..],
                &rgb[..],
                "XYB roundtrip should be lossless for {:?}",
                color
            );
        }
    }

    /// Exhaustive test: verify lossless roundtrip for ALL 16.7M colors
    /// Run with: cargo test --release test_xyb_roundtrip_exhaustive
    #[test]
    #[ignore] // Takes ~1.5 seconds in release mode
    fn test_xyb_roundtrip_exhaustive() {
        let mut failures = 0u64;
        for r in 0..=255u8 {
            for g in 0..=255u8 {
                for b in 0..=255u8 {
                    let rgb = vec![r, g, b];
                    let result = xyb_roundtrip(&rgb, 1, 1);
                    if result[0] != r || result[1] != g || result[2] != b {
                        failures += 1;
                    }
                }
            }
        }
        assert_eq!(failures, 0, "XYB roundtrip should be lossless for all colors");
    }
}
