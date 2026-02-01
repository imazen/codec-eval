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
//!
//! ## Simulation Modes
//!
//! When simulating viewing conditions for metric calculation, there are two approaches:
//!
//! - [`SimulationMode::Accurate`]: Simulate browser behavior exactly, including upscaling
//!   undersized images. This matches real-world viewing but introduces resampling artifacts.
//!
//! - [`SimulationMode::DownsampleOnly`]: Never upsample images. For undersized images,
//!   adjust the effective PPD instead. This avoids simulation artifacts but requires
//!   metric threshold adjustment.

use serde::{Deserialize, Serialize};

/// How to handle image scaling during viewing simulation.
///
/// When calculating perceptual metrics, we need to simulate how images appear
/// on different devices. This affects whether we resample images or adjust
/// metric thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SimulationMode {
    /// Simulate browser behavior exactly.
    ///
    /// - Undersized images (ratio < 1): Upsample to simulate browser upscaling
    /// - Oversized images (ratio > 1): Downsample to simulate browser downscaling
    ///
    /// This matches real-world viewing but introduces resampling artifacts
    /// that may affect metric accuracy.
    #[default]
    Accurate,

    /// Never upsample, only downsample.
    ///
    /// - Undersized images: Keep at native resolution, adjust effective PPD
    /// - Oversized images: Downsample normally
    ///
    /// This avoids introducing upsampling artifacts in the simulation.
    /// The effective PPD is adjusted to account for the missing upscale,
    /// making metric thresholds more lenient for undersized images.
    DownsampleOnly,
}

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

    /// Compute the srcset ratio (intrinsic / browser).
    ///
    /// - ratio < 1: Image is undersized, browser upscales
    /// - ratio = 1: Native resolution
    /// - ratio > 1: Image is oversized, browser downscales
    #[must_use]
    pub fn srcset_ratio(&self) -> f64 {
        let browser = self.browser_dppx.unwrap_or(1.0);
        let intrinsic = self.image_intrinsic_dppx.unwrap_or(1.0);
        intrinsic / browser
    }

    /// Compute simulation parameters for a given image size.
    ///
    /// Returns the scale factor to apply and the adjusted PPD for metrics.
    ///
    /// # Arguments
    ///
    /// * `image_width` - Original image width in pixels
    /// * `image_height` - Original image height in pixels
    /// * `mode` - Simulation mode (accurate or downsample-only)
    ///
    /// # Example
    ///
    /// ```
    /// use codec_eval::viewing::{ViewingCondition, SimulationMode};
    ///
    /// let condition = ViewingCondition::desktop()
    ///     .with_browser_dppx(2.0)
    ///     .with_image_intrinsic_dppx(1.0); // undersized
    ///
    /// let params = condition.simulation_params(1000, 800, SimulationMode::DownsampleOnly);
    /// assert_eq!(params.scale_factor, 1.0); // No upscaling
    /// assert!(params.adjusted_ppd < 40.0);  // Adjusted for missing upscale
    /// ```
    #[must_use]
    pub fn simulation_params(
        &self,
        image_width: u32,
        image_height: u32,
        mode: SimulationMode,
    ) -> SimulationParams {
        let ratio = self.srcset_ratio();
        let base_ppd = self.acuity_ppd;

        match mode {
            SimulationMode::Accurate => {
                // Full simulation: scale by ratio
                let scale_factor = ratio;
                let target_width = (image_width as f64 * scale_factor).round() as u32;
                let target_height = (image_height as f64 * scale_factor).round() as u32;

                SimulationParams {
                    scale_factor,
                    target_width,
                    target_height,
                    adjusted_ppd: self.effective_ppd(),
                    requires_upscale: ratio < 1.0,
                    requires_downscale: ratio > 1.0,
                }
            }
            SimulationMode::DownsampleOnly => {
                if ratio >= 1.0 {
                    // Oversized: downsample normally
                    let scale_factor = ratio;
                    let target_width = (image_width as f64 * scale_factor).round() as u32;
                    let target_height = (image_height as f64 * scale_factor).round() as u32;

                    SimulationParams {
                        scale_factor,
                        target_width,
                        target_height,
                        adjusted_ppd: self.effective_ppd(),
                        requires_upscale: false,
                        requires_downscale: ratio > 1.0,
                    }
                } else {
                    // Undersized: keep original size, adjust PPD instead
                    // The effective PPD is reduced because we're not simulating the upscale
                    // that would make artifacts more visible
                    let adjusted_ppd = base_ppd * ratio;

                    SimulationParams {
                        scale_factor: 1.0,
                        target_width: image_width,
                        target_height: image_height,
                        adjusted_ppd,
                        requires_upscale: false, // We skip upscaling
                        requires_downscale: false,
                    }
                }
            }
        }
    }
}

/// Parameters for viewing simulation.
///
/// Describes how to transform an image and adjust metrics for a viewing condition.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SimulationParams {
    /// Scale factor to apply to the image (1.0 = no scaling).
    pub scale_factor: f64,

    /// Target width after scaling.
    pub target_width: u32,

    /// Target height after scaling.
    pub target_height: u32,

    /// Adjusted PPD for metric thresholds.
    ///
    /// In downsample-only mode, this may differ from effective_ppd()
    /// to compensate for skipped upscaling.
    pub adjusted_ppd: f64,

    /// Whether the simulation requires upscaling.
    ///
    /// In downsample-only mode, this is always false.
    pub requires_upscale: bool,

    /// Whether the simulation requires downscaling.
    pub requires_downscale: bool,
}

/// Reference PPD for metric threshold normalization.
///
/// Desktop viewing at arm's length (~24"/60cm) is the most demanding
/// common viewing condition, so we use it as the baseline.
pub const REFERENCE_PPD: f64 = 40.0;

impl SimulationParams {
    /// Check if any scaling is required.
    #[must_use]
    pub fn requires_scaling(&self) -> bool {
        self.requires_upscale || self.requires_downscale
    }

    /// Get the scale factor clamped to downscale-only (max 1.0).
    #[must_use]
    pub fn downscale_only_factor(&self) -> f64 {
        self.scale_factor.min(1.0)
    }

    /// Compute threshold multiplier for metric values.
    ///
    /// This accounts for how viewing conditions affect artifact visibility.
    /// Higher PPD = smaller angular size = artifacts less visible = more lenient thresholds.
    ///
    /// The multiplier is relative to [`REFERENCE_PPD`] (40, desktop viewing).
    ///
    /// # Returns
    ///
    /// - 1.0 at reference PPD (40)
    /// - > 1.0 for higher PPD (more lenient, e.g., 1.75 at 70 PPD)
    /// - < 1.0 for lower PPD (stricter, e.g., 0.5 at 20 PPD)
    ///
    /// # Example
    ///
    /// ```
    /// use codec_eval::viewing::{ViewingCondition, SimulationMode, REFERENCE_PPD};
    ///
    /// // Desktop at reference PPD
    /// let condition = ViewingCondition::new(40.0);
    /// let params = condition.simulation_params(1000, 800, SimulationMode::Accurate);
    /// assert!((params.threshold_multiplier() - 1.0).abs() < 0.01);
    ///
    /// // Laptop at 70 PPD - more lenient
    /// let condition = ViewingCondition::new(70.0);
    /// let params = condition.simulation_params(1000, 800, SimulationMode::Accurate);
    /// assert!(params.threshold_multiplier() > 1.5);
    /// ```
    #[must_use]
    pub fn threshold_multiplier(&self) -> f64 {
        self.adjusted_ppd / REFERENCE_PPD
    }

    /// Adjust a DSSIM threshold for this viewing condition.
    ///
    /// Higher PPD allows higher DSSIM values (artifacts less visible).
    ///
    /// # Arguments
    ///
    /// * `base_threshold` - Threshold at reference PPD (e.g., 0.0003 for imperceptible)
    ///
    /// # Example
    ///
    /// ```
    /// use codec_eval::viewing::{ViewingCondition, SimulationMode};
    ///
    /// let condition = ViewingCondition::new(70.0); // laptop
    /// let params = condition.simulation_params(1000, 800, SimulationMode::Accurate);
    ///
    /// // Imperceptible threshold at reference is 0.0003
    /// let adjusted = params.adjust_dssim_threshold(0.0003);
    /// assert!(adjusted > 0.0003); // More lenient at higher PPD
    /// ```
    #[must_use]
    pub fn adjust_dssim_threshold(&self, base_threshold: f64) -> f64 {
        base_threshold * self.threshold_multiplier()
    }

    /// Adjust a Butteraugli threshold for this viewing condition.
    ///
    /// Higher PPD allows higher Butteraugli values (artifacts less visible).
    ///
    /// # Arguments
    ///
    /// * `base_threshold` - Threshold at reference PPD (e.g., 1.0 for imperceptible)
    #[must_use]
    pub fn adjust_butteraugli_threshold(&self, base_threshold: f64) -> f64 {
        base_threshold * self.threshold_multiplier()
    }

    /// Adjust a SSIMULACRA2 threshold for this viewing condition.
    ///
    /// Higher PPD allows lower SSIMULACRA2 scores (artifacts less visible).
    /// Note: SSIMULACRA2 is inverted (higher = better), so we divide.
    ///
    /// # Arguments
    ///
    /// * `base_threshold` - Threshold at reference PPD (e.g., 90.0 for imperceptible)
    #[must_use]
    pub fn adjust_ssimulacra2_threshold(&self, base_threshold: f64) -> f64 {
        // SSIMULACRA2: higher is better, so higher PPD means we can accept lower scores
        // But we need to be careful not to go below 0
        let multiplier = self.threshold_multiplier();
        if multiplier >= 1.0 {
            // Higher PPD: can accept lower scores
            // 90 at 40 PPD → ~51 at 70 PPD (90 - (90-0) * (1 - 1/1.75))
            base_threshold - (100.0 - base_threshold) * (1.0 - 1.0 / multiplier)
        } else {
            // Lower PPD: need higher scores
            // 90 at 40 PPD → 95 at 20 PPD
            base_threshold + (100.0 - base_threshold) * (1.0 / multiplier - 1.0)
        }
        .clamp(0.0, 100.0)
    }

    /// Check if a DSSIM value is acceptable for this viewing condition.
    ///
    /// # Arguments
    ///
    /// * `dssim` - Measured DSSIM value
    /// * `base_threshold` - Threshold at reference PPD
    #[must_use]
    pub fn dssim_acceptable(&self, dssim: f64, base_threshold: f64) -> bool {
        dssim < self.adjust_dssim_threshold(base_threshold)
    }

    /// Check if a Butteraugli value is acceptable for this viewing condition.
    #[must_use]
    pub fn butteraugli_acceptable(&self, butteraugli: f64, base_threshold: f64) -> bool {
        butteraugli < self.adjust_butteraugli_threshold(base_threshold)
    }

    /// Check if a SSIMULACRA2 value is acceptable for this viewing condition.
    #[must_use]
    pub fn ssimulacra2_acceptable(&self, ssimulacra2: f64, base_threshold: f64) -> bool {
        ssimulacra2 > self.adjust_ssimulacra2_threshold(base_threshold)
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
            srcset_1x_on_phone(),       // ~32 PPD - most demanding
            srcset_1x_on_laptop(),      // 35 PPD
            native_desktop(),           // 40 PPD
            srcset_2x_on_phone(),       // ~63 PPD
            native_laptop(),            // 70 PPD
            srcset_2x_on_desktop(),     // 80 PPD
            srcset_2x_on_laptop_1_5x(), // ~93 PPD
            native_phone(),             // 95 PPD - least demanding
        ]
    }

    /// Key presets for compact analysis tables.
    ///
    /// Covers the main device types at native resolution.
    #[must_use]
    pub fn key() -> Vec<ViewingCondition> {
        vec![native_desktop(), native_laptop(), native_phone()]
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

    #[test]
    fn test_srcset_ratio() {
        // Native: ratio = 1
        let v = ViewingCondition::desktop();
        assert!((v.srcset_ratio() - 1.0).abs() < 0.001);

        // Undersized: 1x on 2x = 0.5
        let v = ViewingCondition::desktop()
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(1.0);
        assert!((v.srcset_ratio() - 0.5).abs() < 0.001);

        // Oversized: 2x on 1x = 2.0
        let v = ViewingCondition::desktop()
            .with_browser_dppx(1.0)
            .with_image_intrinsic_dppx(2.0);
        assert!((v.srcset_ratio() - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_simulation_accurate_undersized() {
        // 1x on 2x display (undersized)
        let v = ViewingCondition::new(40.0)
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(1.0);

        let params = v.simulation_params(1000, 800, SimulationMode::Accurate);

        // Should upscale to simulate browser behavior
        assert!((params.scale_factor - 0.5).abs() < 0.001);
        assert_eq!(params.target_width, 500);
        assert_eq!(params.target_height, 400);
        assert!(params.requires_upscale); // ratio < 1 means browser upscales
        assert!(!params.requires_downscale);
    }

    #[test]
    fn test_simulation_accurate_oversized() {
        // 2x on 1x display (oversized)
        let v = ViewingCondition::new(40.0)
            .with_browser_dppx(1.0)
            .with_image_intrinsic_dppx(2.0);

        let params = v.simulation_params(1000, 800, SimulationMode::Accurate);

        // Should downscale
        assert!((params.scale_factor - 2.0).abs() < 0.001);
        assert_eq!(params.target_width, 2000);
        assert_eq!(params.target_height, 1600);
        assert!(!params.requires_upscale);
        assert!(params.requires_downscale);
    }

    #[test]
    fn test_simulation_downsample_only_undersized() {
        // 1x on 2x display (undersized) with downsample-only mode
        let v = ViewingCondition::new(40.0)
            .with_browser_dppx(2.0)
            .with_image_intrinsic_dppx(1.0);

        let params = v.simulation_params(1000, 800, SimulationMode::DownsampleOnly);

        // Should NOT upscale, keep original size
        assert!((params.scale_factor - 1.0).abs() < 0.001);
        assert_eq!(params.target_width, 1000);
        assert_eq!(params.target_height, 800);
        assert!(!params.requires_upscale);
        assert!(!params.requires_downscale);

        // PPD should be adjusted to compensate (reduced)
        assert!((params.adjusted_ppd - 20.0).abs() < 0.1); // 40 * 0.5 = 20
    }

    #[test]
    fn test_simulation_downsample_only_oversized() {
        // 2x on 1x display (oversized) - should still downscale
        let v = ViewingCondition::new(40.0)
            .with_browser_dppx(1.0)
            .with_image_intrinsic_dppx(2.0);

        let params = v.simulation_params(1000, 800, SimulationMode::DownsampleOnly);

        // Should downscale (oversized images are fine to downscale)
        assert!((params.scale_factor - 2.0).abs() < 0.001);
        assert_eq!(params.target_width, 2000);
        assert_eq!(params.target_height, 1600);
        assert!(!params.requires_upscale);
        assert!(params.requires_downscale);
    }

    #[test]
    fn test_simulation_params_helpers() {
        let params = SimulationParams {
            scale_factor: 0.5,
            target_width: 500,
            target_height: 400,
            adjusted_ppd: 20.0,
            requires_upscale: true,
            requires_downscale: false,
        };

        assert!(params.requires_scaling());
        assert!((params.downscale_only_factor() - 0.5).abs() < 0.001);

        let params2 = SimulationParams {
            scale_factor: 2.0,
            target_width: 2000,
            target_height: 1600,
            adjusted_ppd: 80.0,
            requires_upscale: false,
            requires_downscale: true,
        };

        assert!(params2.requires_scaling());
        assert!((params2.downscale_only_factor() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_threshold_multiplier() {
        // Reference PPD = 40
        let params_ref = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 40.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        assert!((params_ref.threshold_multiplier() - 1.0).abs() < 0.001);

        // Higher PPD = more lenient
        let params_high = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 80.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        assert!((params_high.threshold_multiplier() - 2.0).abs() < 0.001);

        // Lower PPD = stricter
        let params_low = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 20.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        assert!((params_low.threshold_multiplier() - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_adjust_dssim_threshold() {
        let base_threshold = 0.0003; // imperceptible at reference

        // At reference PPD, threshold unchanged
        let params_ref = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 40.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        assert!((params_ref.adjust_dssim_threshold(base_threshold) - 0.0003).abs() < 0.00001);

        // At higher PPD (laptop), more lenient
        let params_laptop = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 70.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        let adjusted = params_laptop.adjust_dssim_threshold(base_threshold);
        assert!(adjusted > 0.0003); // More lenient
        assert!((adjusted - 0.000525).abs() < 0.0001); // 0.0003 * 1.75
    }

    #[test]
    fn test_adjust_ssimulacra2_threshold() {
        let base_threshold = 90.0; // imperceptible at reference

        // At reference PPD, threshold unchanged
        let params_ref = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 40.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        assert!((params_ref.adjust_ssimulacra2_threshold(base_threshold) - 90.0).abs() < 0.1);

        // At higher PPD, can accept lower scores
        let params_high = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 80.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        let adjusted = params_high.adjust_ssimulacra2_threshold(base_threshold);
        assert!(adjusted < 90.0); // Can accept lower score

        // At lower PPD, need higher scores
        let params_low = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 20.0,
            requires_upscale: false,
            requires_downscale: false,
        };
        let adjusted = params_low.adjust_ssimulacra2_threshold(base_threshold);
        assert!(adjusted > 90.0); // Need higher score
    }

    #[test]
    fn test_metric_acceptable() {
        let params = SimulationParams {
            scale_factor: 1.0,
            target_width: 1000,
            target_height: 800,
            adjusted_ppd: 70.0, // laptop, more lenient
            requires_upscale: false,
            requires_downscale: false,
        };

        // DSSIM: 0.0004 would fail at reference (40 PPD) but pass at 70 PPD
        // Threshold at 70 PPD = 0.0003 * 1.75 = 0.000525
        assert!(params.dssim_acceptable(0.0004, 0.0003));
        assert!(!params.dssim_acceptable(0.0006, 0.0003));

        // Butteraugli: 1.5 would fail at reference but pass at 70 PPD
        // Threshold at 70 PPD = 1.0 * 1.75 = 1.75
        assert!(params.butteraugli_acceptable(1.5, 1.0));

        // SSIMULACRA2: at 70 PPD (multiplier 1.75), threshold is ~85.7
        // So 86 passes but 85 fails
        assert!(params.ssimulacra2_acceptable(86.0, 90.0));
        assert!(!params.ssimulacra2_acceptable(84.0, 90.0));
    }
}
