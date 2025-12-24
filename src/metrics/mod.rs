//! Quality metrics for image comparison.
//!
//! This module provides perceptual quality metrics for comparing reference
//! and test images. Supported metrics:
//!
//! - **DSSIM**: Structural dissimilarity metric (lower is better, 0 = identical)
//! - **PSNR**: Peak Signal-to-Noise Ratio (higher is better)
//!
//! ## Perception Thresholds
//!
//! Based on empirical data from imageflow:
//!
//! | Level | DSSIM | Description |
//! |-------|-------|-------------|
//! | Imperceptible | < 0.0003 | Visually identical |
//! | Marginal | < 0.0007 | Only A/B comparison reveals |
//! | Subtle | < 0.0015 | Barely noticeable |
//! | Noticeable | < 0.003 | Visible on inspection |
//! | Degraded | >= 0.003 | Clearly visible artifacts |

pub mod dssim;

use serde::{Deserialize, Serialize};

/// Configuration for which metrics to calculate.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricConfig {
    /// Calculate DSSIM (structural dissimilarity).
    pub dssim: bool,
    /// Calculate PSNR (peak signal-to-noise ratio).
    pub psnr: bool,
}

impl MetricConfig {
    /// Calculate all available metrics.
    #[must_use]
    pub fn all() -> Self {
        Self {
            dssim: true,
            psnr: true,
        }
    }

    /// Fast metric set (PSNR only).
    #[must_use]
    pub fn fast() -> Self {
        Self {
            dssim: false,
            psnr: true,
        }
    }

    /// Perceptual metrics only (DSSIM).
    #[must_use]
    pub fn perceptual() -> Self {
        Self {
            dssim: true,
            psnr: false,
        }
    }
}

/// Results from metric calculations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetricResult {
    /// DSSIM value (lower is better, 0 = identical).
    pub dssim: Option<f64>,
    /// PSNR value in dB (higher is better).
    pub psnr: Option<f64>,
}

impl MetricResult {
    /// Get the perception level based on DSSIM value.
    #[must_use]
    pub fn perception_level(&self) -> Option<PerceptionLevel> {
        self.dssim.map(PerceptionLevel::from_dssim)
    }
}

/// Perceptual quality level based on metric thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PerceptionLevel {
    /// DSSIM < 0.0003 - Visually identical.
    Imperceptible,
    /// DSSIM < 0.0007 - Only A/B comparison reveals difference.
    Marginal,
    /// DSSIM < 0.0015 - Barely noticeable.
    Subtle,
    /// DSSIM < 0.003 - Visible on inspection.
    Noticeable,
    /// DSSIM >= 0.003 - Clearly visible artifacts.
    Degraded,
}

impl PerceptionLevel {
    /// Determine perception level from DSSIM value.
    #[must_use]
    pub fn from_dssim(dssim: f64) -> Self {
        if dssim < 0.0003 {
            Self::Imperceptible
        } else if dssim < 0.0007 {
            Self::Marginal
        } else if dssim < 0.0015 {
            Self::Subtle
        } else if dssim < 0.003 {
            Self::Noticeable
        } else {
            Self::Degraded
        }
    }

    /// Get the maximum DSSIM value for this perception level.
    #[must_use]
    pub fn max_dssim(self) -> f64 {
        match self {
            Self::Imperceptible => 0.0003,
            Self::Marginal => 0.0007,
            Self::Subtle => 0.0015,
            Self::Noticeable => 0.003,
            Self::Degraded => f64::INFINITY,
        }
    }

    /// Get a short code for this level.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Imperceptible => "IMP",
            Self::Marginal => "MAR",
            Self::Subtle => "SUB",
            Self::Noticeable => "NOT",
            Self::Degraded => "DEG",
        }
    }
}

impl std::fmt::Display for PerceptionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Imperceptible => write!(f, "Imperceptible"),
            Self::Marginal => write!(f, "Marginal"),
            Self::Subtle => write!(f, "Subtle"),
            Self::Noticeable => write!(f, "Noticeable"),
            Self::Degraded => write!(f, "Degraded"),
        }
    }
}

/// Calculate PSNR between two images.
///
/// # Arguments
///
/// * `reference` - Reference image pixel data (RGB8, row-major).
/// * `test` - Test image pixel data (RGB8, row-major).
/// * `width` - Image width in pixels.
/// * `height` - Image height in pixels.
///
/// # Returns
///
/// PSNR value in decibels. Higher is better. Returns `f64::INFINITY` if
/// images are identical.
#[must_use]
pub fn calculate_psnr(reference: &[u8], test: &[u8], width: usize, height: usize) -> f64 {
    assert_eq!(reference.len(), test.len());
    assert_eq!(reference.len(), width * height * 3);

    let mut mse_sum: f64 = 0.0;
    let pixel_count = (width * height * 3) as f64;

    for (r, t) in reference.iter().zip(test.iter()) {
        let diff = f64::from(*r) - f64::from(*t);
        mse_sum += diff * diff;
    }

    let mse = mse_sum / pixel_count;

    if mse == 0.0 {
        f64::INFINITY
    } else {
        10.0 * (255.0_f64 * 255.0 / mse).log10()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perception_level_thresholds() {
        assert_eq!(PerceptionLevel::from_dssim(0.0001), PerceptionLevel::Imperceptible);
        assert_eq!(PerceptionLevel::from_dssim(0.0003), PerceptionLevel::Marginal);
        assert_eq!(PerceptionLevel::from_dssim(0.0005), PerceptionLevel::Marginal);
        assert_eq!(PerceptionLevel::from_dssim(0.0007), PerceptionLevel::Subtle);
        assert_eq!(PerceptionLevel::from_dssim(0.001), PerceptionLevel::Subtle);
        assert_eq!(PerceptionLevel::from_dssim(0.0015), PerceptionLevel::Noticeable);
        assert_eq!(PerceptionLevel::from_dssim(0.002), PerceptionLevel::Noticeable);
        assert_eq!(PerceptionLevel::from_dssim(0.003), PerceptionLevel::Degraded);
        assert_eq!(PerceptionLevel::from_dssim(0.01), PerceptionLevel::Degraded);
    }

    #[test]
    fn test_psnr_identical() {
        let data = vec![128u8; 100 * 100 * 3];
        let psnr = calculate_psnr(&data, &data, 100, 100);
        assert!(psnr.is_infinite());
    }

    #[test]
    fn test_psnr_different() {
        let reference = vec![100u8; 100 * 100 * 3];
        let test = vec![110u8; 100 * 100 * 3];
        let psnr = calculate_psnr(&reference, &test, 100, 100);
        // PSNR for constant difference of 10: 10 * log10(255^2 / 100) ≈ 28.13
        assert!(psnr > 28.0);
        assert!(psnr < 29.0);
    }

    #[test]
    fn test_metric_config_all() {
        let config = MetricConfig::all();
        assert!(config.dssim);
        assert!(config.psnr);
    }

    #[test]
    fn test_metric_config_fast() {
        let config = MetricConfig::fast();
        assert!(!config.dssim);
        assert!(config.psnr);
    }
}
