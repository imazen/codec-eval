//! ICC color profile handling for accurate metric calculation.
//!
//! This module provides ICC profile transformation to ensure images are in
//! sRGB color space before computing perceptual quality metrics.
//!
//! # Why ICC Profiles Matter
//!
//! When comparing image quality, both images must be in the same color space.
//! Many encoded images (especially XYB JPEGs from jpegli) embed non-sRGB ICC
//! profiles. Without proper color management:
//!
//! - Metrics will report incorrect scores
//! - SSIMULACRA2 can be off by 1-2 points (1-2% error at high quality)
//! - Colors may appear shifted even when compression is lossless
//!
//! # Implementation Notes
//!
//! This module uses moxcms, a pure-Rust CMS that most closely matches libjxl's
//! skcms library. According to experiments (see ssimulacra2-fork/EXPERIMENTS.md):
//!
//! | CMS Backend | SSIMULACRA2 Score | Difference from skcms |
//! |-------------|-------------------|----------------------|
//! | libjxl (skcms) | 88.48 | — (reference) |
//! | moxcms Linear | 86.96 | -1.52 (1.7%) |
//! | lcms2 Perceptual | 85.97 | -2.51 (2.8%) |
//!
//! The remaining gap is likely due to JPEG decoder differences, not CMS.

use crate::error::{Error, Result};

/// Color profile information for an image.
#[derive(Debug, Clone, Default)]
pub enum ColorProfile {
    /// Standard sRGB color space (no transformation needed).
    #[default]
    Srgb,
    /// Embedded ICC profile data.
    Icc(Vec<u8>),
}

impl ColorProfile {
    /// Check if this is the sRGB profile.
    #[must_use]
    pub fn is_srgb(&self) -> bool {
        matches!(self, Self::Srgb)
    }

    /// Create from ICC profile bytes, or None if no profile (assumes sRGB).
    #[must_use]
    pub fn from_icc_bytes(icc: Option<&[u8]>) -> Self {
        match icc {
            Some(data) if !data.is_empty() => Self::Icc(data.to_vec()),
            _ => Self::Srgb,
        }
    }
}

/// Transform RGB pixels from source ICC profile to sRGB.
///
/// # Arguments
///
/// * `rgb` - RGB8 pixel data (3 bytes per pixel)
/// * `profile` - Source color profile
///
/// # Returns
///
/// Transformed RGB8 pixels in sRGB color space, or the original if already sRGB.
#[cfg(feature = "icc")]
pub fn transform_to_srgb(rgb: &[u8], profile: &ColorProfile) -> Result<Vec<u8>> {
    use moxcms::{ColorProfile as MoxProfile, Layout, TransformOptions};

    match profile {
        ColorProfile::Srgb => Ok(rgb.to_vec()),
        ColorProfile::Icc(icc_data) => {
            let input_profile =
                MoxProfile::new_from_slice(icc_data).map_err(|e| Error::MetricCalculation {
                    metric: "ICC".to_string(),
                    reason: format!("Failed to parse ICC profile: {e}"),
                })?;

            let srgb = MoxProfile::new_srgb();

            // Use Linear interpolation (default) - closest match to skcms
            // See EXPERIMENTS.md for comparison results
            let transform = input_profile
                .create_transform_8bit(Layout::Rgb, &srgb, Layout::Rgb, TransformOptions::default())
                .map_err(|e| Error::MetricCalculation {
                    metric: "ICC".to_string(),
                    reason: format!("Failed to create ICC transform: {e}"),
                })?;

            let mut output = vec![0u8; rgb.len()];
            transform
                .transform(rgb, &mut output)
                .map_err(|e| Error::MetricCalculation {
                    metric: "ICC".to_string(),
                    reason: format!("Failed to apply ICC transform: {e}"),
                })?;

            Ok(output)
        }
    }
}

/// Transform RGB pixels from source ICC profile to sRGB (no-op without icc feature).
#[cfg(not(feature = "icc"))]
pub fn transform_to_srgb(rgb: &[u8], profile: &ColorProfile) -> Result<Vec<u8>> {
    match profile {
        ColorProfile::Srgb => Ok(rgb.to_vec()),
        ColorProfile::Icc(_) => Err(Error::MetricCalculation {
            metric: "ICC".to_string(),
            reason: "ICC profile support requires the 'icc' feature".to_string(),
        }),
    }
}

/// Transform two images to sRGB and return them.
///
/// This is the main entry point for ICC-aware metric calculation.
/// Both images are transformed to sRGB before comparison.
pub fn prepare_for_comparison(
    reference: &[u8],
    reference_profile: &ColorProfile,
    test: &[u8],
    test_profile: &ColorProfile,
) -> Result<(Vec<u8>, Vec<u8>)> {
    let ref_srgb = transform_to_srgb(reference, reference_profile)?;
    let test_srgb = transform_to_srgb(test, test_profile)?;
    Ok((ref_srgb, test_srgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_srgb_passthrough() {
        let rgb = vec![100u8, 150, 200, 50, 100, 150];
        let result = transform_to_srgb(&rgb, &ColorProfile::Srgb).unwrap();
        assert_eq!(result, rgb);
    }

    #[test]
    fn test_color_profile_default() {
        assert!(ColorProfile::default().is_srgb());
    }

    #[test]
    fn test_from_icc_bytes_none() {
        let profile = ColorProfile::from_icc_bytes(None);
        assert!(profile.is_srgb());
    }

    #[test]
    fn test_from_icc_bytes_empty() {
        let profile = ColorProfile::from_icc_bytes(Some(&[]));
        assert!(profile.is_srgb());
    }

    #[test]
    fn test_from_icc_bytes_data() {
        let profile = ColorProfile::from_icc_bytes(Some(&[1, 2, 3, 4]));
        assert!(!profile.is_srgb());
    }
}
