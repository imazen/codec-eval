//! Viewing condition modeling for perceptual quality assessment.
//!
//! This module provides the [`ViewingCondition`] type which models how an image
//! will be viewed, affecting perceptual quality thresholds.
//!
//! ## Key Concepts
//!
//! - **acuity_ppd**: The viewer's visual acuity in pixels per degree. This is
//!   determined by the display's pixel density and viewing distance.
//! - **browser_dppx**: The browser/OS device pixel ratio (e.g., 2.0 for retina).
//! - **image_intrinsic_dppx**: The image's intrinsic pixels per CSS pixel (for srcset).
//! - **ppd**: The effective pixels per degree for this specific image viewing.

use serde::{Deserialize, Serialize};

/// Viewing condition for perceptual quality assessment.
///
/// Models how an image will be viewed, which affects whether compression
/// artifacts will be perceptible.
///
/// # Example
///
/// ```
/// use codec_eval::ViewingCondition;
///
/// // Desktop viewing with 2x retina display showing a 2x srcset image
/// let condition = ViewingCondition::desktop()
///     .with_browser_dppx(2.0)
///     .with_image_intrinsic_dppx(2.0);
///
/// // The effective PPD accounts for the srcset ratio
/// let ppd = condition.effective_ppd();
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ViewingCondition {
    /// Viewer's visual acuity in pixels per degree.
    ///
    /// This is the baseline PPD for the display and viewing distance.
    /// Typical values:
    /// - Desktop at arm's length: ~40 PPD
    /// - Laptop: ~60 PPD
    /// - Smartphone held close: ~90+ PPD
    pub acuity_ppd: f64,

    /// Browser/OS device pixel ratio.
    ///
    /// For retina/HiDPI displays, this is typically 2.0 or 3.0.
    /// For standard displays, this is 1.0.
    pub browser_dppx: Option<f64>,

    /// Image's intrinsic pixels per CSS pixel.
    ///
    /// For srcset images:
    /// - A 1x image has `intrinsic_dppx = 1.0`
    /// - A 2x image has `intrinsic_dppx = 2.0`
    ///
    /// This affects the effective resolution at which the image is displayed.
    pub image_intrinsic_dppx: Option<f64>,

    /// Override or computed PPD for this specific viewing.
    ///
    /// If `Some`, this value is used directly instead of computing from
    /// the other fields.
    pub ppd: Option<f64>,
}

impl ViewingCondition {
    /// Create a new viewing condition with the given acuity PPD.
    ///
    /// # Arguments
    ///
    /// * `acuity_ppd` - The viewer's visual acuity in pixels per degree.
    #[must_use]
    pub fn new(acuity_ppd: f64) -> Self {
        Self {
            acuity_ppd,
            browser_dppx: None,
            image_intrinsic_dppx: None,
            ppd: None,
        }
    }

    /// Desktop viewing condition (acuity ~40 PPD).
    ///
    /// Represents viewing a standard desktop monitor at arm's length
    /// (approximately 24 inches / 60 cm).
    #[must_use]
    pub fn desktop() -> Self {
        Self::new(40.0)
    }

    /// Laptop viewing condition (acuity ~60 PPD).
    ///
    /// Represents viewing a laptop screen at a typical distance
    /// (approximately 18 inches / 45 cm).
    #[must_use]
    pub fn laptop() -> Self {
        Self::new(60.0)
    }

    /// Smartphone viewing condition (acuity ~90 PPD).
    ///
    /// Represents viewing a smartphone held at reading distance
    /// (approximately 12 inches / 30 cm).
    #[must_use]
    pub fn smartphone() -> Self {
        Self::new(90.0)
    }

    /// Set the browser/OS device pixel ratio.
    ///
    /// # Arguments
    ///
    /// * `dppx` - Device pixel ratio (e.g., 2.0 for retina).
    #[must_use]
    pub fn with_browser_dppx(mut self, dppx: f64) -> Self {
        self.browser_dppx = Some(dppx);
        self
    }

    /// Set the image's intrinsic pixels per CSS pixel.
    ///
    /// # Arguments
    ///
    /// * `dppx` - Intrinsic DPI ratio (e.g., 2.0 for a 2x srcset image).
    #[must_use]
    pub fn with_image_intrinsic_dppx(mut self, dppx: f64) -> Self {
        self.image_intrinsic_dppx = Some(dppx);
        self
    }

    /// Override the computed PPD with a specific value.
    ///
    /// # Arguments
    ///
    /// * `ppd` - The PPD value to use.
    #[must_use]
    pub fn with_ppd_override(mut self, ppd: f64) -> Self {
        self.ppd = Some(ppd);
        self
    }

    /// Compute the effective PPD for metric adjustment.
    ///
    /// If `ppd` is set, returns that value directly. Otherwise, computes
    /// the effective PPD from the acuity and dppx values.
    ///
    /// The formula is:
    /// ```text
    /// effective_ppd = acuity_ppd * (image_intrinsic_dppx / browser_dppx)
    /// ```
    ///
    /// This accounts for how srcset images are scaled on HiDPI displays.
    #[must_use]
    pub fn effective_ppd(&self) -> f64 {
        if let Some(ppd) = self.ppd {
            return ppd;
        }

        let browser = self.browser_dppx.unwrap_or(1.0);
        let intrinsic = self.image_intrinsic_dppx.unwrap_or(1.0);

        // When intrinsic > browser, image pixels are smaller than device pixels,
        // making artifacts less visible (higher effective PPD).
        // When intrinsic < browser, image pixels are larger, artifacts more visible.
        self.acuity_ppd * (intrinsic / browser)
    }
}

impl Default for ViewingCondition {
    fn default() -> Self {
        Self::desktop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_desktop_defaults() {
        let v = ViewingCondition::desktop();
        assert!((v.acuity_ppd - 40.0).abs() < f64::EPSILON);
        assert!(v.browser_dppx.is_none());
        assert!(v.image_intrinsic_dppx.is_none());
        assert!(v.ppd.is_none());
    }

    #[test]
    fn test_effective_ppd_no_dppx() {
        let v = ViewingCondition::desktop();
        assert!((v.effective_ppd() - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_effective_ppd_with_retina() {
        // 2x image on 2x display = same effective PPD
        let v = ViewingCondition::desktop()
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(2.0);
        assert!((v.effective_ppd() - 40.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_effective_ppd_1x_on_2x() {
        // 1x image on 2x display = half effective PPD (artifacts more visible)
        let v = ViewingCondition::desktop()
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(1.0);
        assert!((v.effective_ppd() - 20.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_effective_ppd_2x_on_1x() {
        // 2x image on 1x display = double effective PPD (artifacts less visible)
        let v = ViewingCondition::desktop()
            .with_browser_dppx(1.0)
            .with_image_intrinsic_dppx(2.0);
        assert!((v.effective_ppd() - 80.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_ppd_override() {
        let v = ViewingCondition::desktop()
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(1.0)
            .with_ppd_override(100.0);
        assert!((v.effective_ppd() - 100.0).abs() < f64::EPSILON);
    }
}
