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

/// Pre-defined viewing condition presets for common scenarios.
///
/// These presets model real-world viewing scenarios including srcset
/// image delivery on various devices.
///
/// ## Terminology
///
/// - **Native**: srcset matches device DPPX (1x on 1x, 2x on 2x, etc.)
/// - **Undersized**: srcset is smaller than device (browser upscales, artifacts amplified)
/// - **Oversized**: srcset is larger than device (browser downscales, artifacts hidden)
///
/// ## Preset PPD Values
///
/// | Device | Base PPD | Typical DPPX | Viewing Distance |
/// |--------|----------|--------------|------------------|
/// | Desktop | 40 | 1.0 | ~24" / 60cm |
/// | Laptop | 70 | 2.0 | ~18" / 45cm |
/// | Phone | 95 | 3.0 | ~12" / 30cm |
pub mod presets {
    use super::ViewingCondition;

    //=========================================================================
    // Native Conditions (srcset matches device DPPX)
    //=========================================================================

    /// Desktop monitor at arm's length, 1x srcset on 1x display.
    ///
    /// This is the most demanding condition - artifacts are most visible.
    /// Effective PPD: 40
    #[must_use]
    pub fn native_desktop() -> ViewingCondition {
        ViewingCondition::new(40.0)
            .with_browser_dppx(1.0)
            .with_image_intrinsic_dppx(1.0)
    }

    /// Laptop/retina screen, 2x srcset on 2x display.
    ///
    /// Common premium laptop viewing condition.
    /// Effective PPD: 70
    #[must_use]
    pub fn native_laptop() -> ViewingCondition {
        ViewingCondition::new(70.0)
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(2.0)
    }

    /// Smartphone, 3x srcset on 3x display.
    ///
    /// High-DPI phone with matching srcset.
    /// Effective PPD: 95
    #[must_use]
    pub fn native_phone() -> ViewingCondition {
        ViewingCondition::new(95.0)
            .with_browser_dppx(3.0)
            .with_image_intrinsic_dppx(3.0)
    }

    //=========================================================================
    // Undersized Conditions (browser upscales, artifacts amplified)
    //=========================================================================

    /// 1x srcset shown on 3x phone display (0.33x ratio).
    ///
    /// Worst case: massive upscaling makes artifacts very visible.
    /// Effective PPD: ~32 (95 * 1/3)
    #[must_use]
    pub fn srcset_1x_on_phone() -> ViewingCondition {
        ViewingCondition::new(95.0)
            .with_browser_dppx(3.0)
            .with_image_intrinsic_dppx(1.0)
    }

    /// 1x srcset shown on 2x laptop display (0.5x ratio).
    ///
    /// Common when srcset is misconfigured or unavailable.
    /// Effective PPD: 35 (70 * 1/2)
    #[must_use]
    pub fn srcset_1x_on_laptop() -> ViewingCondition {
        ViewingCondition::new(70.0)
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(1.0)
    }

    /// 2x srcset shown on 3x phone display (0.67x ratio).
    ///
    /// Moderate upscaling on high-DPI phone.
    /// Effective PPD: ~63 (95 * 2/3)
    #[must_use]
    pub fn srcset_2x_on_phone() -> ViewingCondition {
        ViewingCondition::new(95.0)
            .with_browser_dppx(3.0)
            .with_image_intrinsic_dppx(2.0)
    }

    //=========================================================================
    // Oversized Conditions (browser downscales, artifacts hidden)
    //=========================================================================

    /// 2x srcset shown on 1x desktop display (2.0x ratio).
    ///
    /// Downscaling hides artifacts, but wastes bandwidth.
    /// Effective PPD: 80 (40 * 2)
    #[must_use]
    pub fn srcset_2x_on_desktop() -> ViewingCondition {
        ViewingCondition::new(40.0)
            .with_browser_dppx(1.0)
            .with_image_intrinsic_dppx(2.0)
    }

    /// 2x srcset shown on 1.5x laptop display (1.33x ratio).
    ///
    /// Slight oversizing on mid-DPI laptop.
    /// Effective PPD: ~93 (70 * 2/1.5)
    #[must_use]
    pub fn srcset_2x_on_laptop_1_5x() -> ViewingCondition {
        ViewingCondition::new(70.0)
            .with_browser_dppx(1.5)
            .with_image_intrinsic_dppx(2.0)
    }

    /// 3x srcset shown on 3x phone display.
    ///
    /// Native phone viewing, same as native_phone().
    /// Effective PPD: 95
    #[must_use]
    pub fn srcset_3x_on_phone() -> ViewingCondition {
        native_phone()
    }

    //=========================================================================
    // Preset Collections
    //=========================================================================

    /// All standard presets for comprehensive analysis.
    ///
    /// Returns conditions ordered from most demanding (lowest effective PPD)
    /// to least demanding (highest effective PPD).
    #[must_use]
    pub fn all() -> Vec<ViewingCondition> {
        vec![
            srcset_1x_on_phone(),    // ~32 PPD - most demanding
            srcset_1x_on_laptop(),   // 35 PPD
            native_desktop(),        // 40 PPD
            srcset_2x_on_phone(),    // ~63 PPD
            native_laptop(),         // 70 PPD
            srcset_2x_on_desktop(),  // 80 PPD
            srcset_2x_on_laptop_1_5x(), // ~93 PPD
            native_phone(),          // 95 PPD - least demanding
        ]
    }

    /// Key presets for compact analysis tables.
    ///
    /// Covers the main device types at native resolution.
    #[must_use]
    pub fn key() -> Vec<ViewingCondition> {
        vec![
            native_desktop(),
            native_laptop(),
            native_phone(),
        ]
    }

    /// Baseline condition for quality mapping (native laptop).
    ///
    /// This is a good middle-ground for quality calibration:
    /// - More forgiving than desktop (70 vs 40 PPD)
    /// - Representative of premium laptop viewing
    /// - 2x srcset is common for web images
    #[must_use]
    pub fn baseline() -> ViewingCondition {
        native_laptop()
    }

    /// Most demanding condition for diminishing returns analysis.
    ///
    /// Native desktop is where artifacts are most visible,
    /// making it ideal for determining quality upper bounds.
    #[must_use]
    pub fn demanding() -> ViewingCondition {
        native_desktop()
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

    #[test]
    fn test_presets_native() {
        let desktop = presets::native_desktop();
        assert!((desktop.effective_ppd() - 40.0).abs() < 0.1);

        let laptop = presets::native_laptop();
        assert!((laptop.effective_ppd() - 70.0).abs() < 0.1);

        let phone = presets::native_phone();
        assert!((phone.effective_ppd() - 95.0).abs() < 0.1);
    }

    #[test]
    fn test_presets_undersized() {
        // 1x on 3x phone = 95 * (1/3) ≈ 31.67
        let v = presets::srcset_1x_on_phone();
        assert!(v.effective_ppd() < 35.0);
        assert!(v.effective_ppd() > 30.0);

        // 1x on 2x laptop = 70 * (1/2) = 35
        let v = presets::srcset_1x_on_laptop();
        assert!((v.effective_ppd() - 35.0).abs() < 0.1);
    }

    #[test]
    fn test_presets_oversized() {
        // 2x on 1x desktop = 40 * (2/1) = 80
        let v = presets::srcset_2x_on_desktop();
        assert!((v.effective_ppd() - 80.0).abs() < 0.1);
    }

    #[test]
    fn test_presets_all_ordered() {
        let all = presets::all();
        assert!(all.len() >= 5);

        // Should be ordered by effective PPD (ascending)
        for i in 0..all.len() - 1 {
            assert!(
                all[i].effective_ppd() <= all[i + 1].effective_ppd(),
                "Presets should be ordered by effective PPD"
            );
        }
    }
}
