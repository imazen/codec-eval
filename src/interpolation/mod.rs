//! Quality interpolation and polynomial fitting.
//!
//! This module provides tools for interpolating quality values between
//! measured data points, enabling smooth quality curves from sparse samples.
//!
//! ## Key Types
//!
//! - [`InterpolationConfig`]: Configuration for polynomial fitting
//! - [`GapPolynomial`]: Power-law polynomial for a quality range
//! - [`InterpolationTable`]: Collection of polynomials for a codec/condition
//!
//! ## Methodology
//!
//! The interpolation uses power-law fitting: `y = a * x^b + c`
//!
//! This form works well for quality-to-metric relationships because:
//! - Quality improvements have diminishing returns at high quality
//! - The relationship is monotonic but non-linear
//! - Power law captures the "elbow" in quality curves
//!
//! ### Cross-Validation
//!
//! To validate fits, we use leave-one-out cross-validation:
//! 1. For each gap between measured quality values
//! 2. Skip one internal point and fit on remaining points
//! 3. Validate by predicting the skipped point
//! 4. Average adjacent polynomial fits for smooth transitions

use serde::{Deserialize, Serialize};

/// Configuration for polynomial interpolation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationConfig {
    /// Minimum exponent for power law fitting (default: 0.5)
    pub min_exponent: f64,
    /// Maximum exponent for power law fitting (default: 3.0)
    pub max_exponent: f64,
    /// Exponent search step size (default: 0.1)
    pub exponent_step: f64,
    /// Minimum R² for valid fit (default: 0.90)
    pub min_r_squared: f64,
}

impl Default for InterpolationConfig {
    fn default() -> Self {
        Self {
            min_exponent: 0.5,
            max_exponent: 3.0,
            exponent_step: 0.1,
            min_r_squared: 0.90,
        }
    }
}

/// Power-law polynomial for quality interpolation: `y = a * x^b + c`
///
/// Each polynomial covers a specific quality range `[q_low, q_high]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GapPolynomial {
    /// Lower bound of quality range
    pub q_low: u32,
    /// Upper bound of quality range
    pub q_high: u32,
    /// Coefficient a in `a * x^b + c`
    pub a: f64,
    /// Exponent b in `a * x^b + c`
    pub b: f64,
    /// Offset c in `a * x^b + c`
    pub c: f64,
    /// Coefficient of determination (1.0 = perfect fit)
    pub r_squared: f64,
    /// Error when predicting the validation (skipped) point
    pub validation_error: f64,
}

impl GapPolynomial {
    /// Interpolate value for a given input.
    ///
    /// # Example
    ///
    /// ```
    /// use codec_eval::interpolation::GapPolynomial;
    ///
    /// let poly = GapPolynomial {
    ///     q_low: 50,
    ///     q_high: 90,
    ///     a: 0.001,
    ///     b: 2.0,
    ///     c: 0.5,
    ///     r_squared: 0.98,
    ///     validation_error: 0.001,
    /// };
    ///
    /// let result = poly.interpolate(70.0);
    /// assert!(result > 0.0 && result <= 100.0);
    /// ```
    #[must_use]
    pub fn interpolate(&self, x: f64) -> f64 {
        (self.a * x.powf(self.b) + self.c).clamp(0.0, 100.0)
    }

    /// Check if this polynomial covers the given quality value.
    #[must_use]
    pub fn covers(&self, q: u32) -> bool {
        q >= self.q_low && q <= self.q_high
    }
}

/// Collection of gap polynomials for interpolation.
///
/// Stores polynomials for a specific codec and viewing condition,
/// allowing quality interpolation across the full quality range.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterpolationTable {
    /// Codec identifier (e.g., "mozjpeg", "avif")
    pub codec: String,
    /// Condition identifier (e.g., "desktop-1x", "phone-2x")
    pub condition: String,
    /// Polynomials covering different quality ranges
    pub polynomials: Vec<GapPolynomial>,
}

impl InterpolationTable {
    /// Create a new interpolation table.
    #[must_use]
    pub fn new(codec: impl Into<String>, condition: impl Into<String>) -> Self {
        Self {
            codec: codec.into(),
            condition: condition.into(),
            polynomials: Vec::new(),
        }
    }

    /// Find the polynomial that covers the given quality.
    #[must_use]
    pub fn find_polynomial(&self, q: u32) -> Option<&GapPolynomial> {
        self.polynomials.iter().find(|p| p.covers(q))
    }

    /// Interpolate value, falling back to identity if no polynomial found.
    #[must_use]
    pub fn interpolate(&self, x: f64) -> f64 {
        let q = x.round() as u32;
        if let Some(poly) = self.find_polynomial(q) {
            poly.interpolate(x)
        } else {
            x // fallback to identity
        }
    }
}

//=============================================================================
// Fitting Functions
//=============================================================================

/// Fit a power-law polynomial `y = a * x^b + c` using grid search.
///
/// Returns `(a, b, c, r_squared)` if a valid fit is found.
///
/// The algorithm:
/// 1. For each exponent b in `[min_exponent, max_exponent]`
/// 2. Transform x → x^b
/// 3. Fit linear regression for a and c
/// 4. Compute R² and keep best fit
#[must_use]
#[allow(clippy::many_single_char_names, clippy::similar_names)] // Standard math notation
pub fn fit_power_law(
    points: &[(f64, f64)],
    config: &InterpolationConfig,
) -> Option<(f64, f64, f64, f64)> {
    if points.len() < 3 {
        return None;
    }

    let mut best_fit: Option<(f64, f64, f64, f64)> = None;
    let mut b = config.min_exponent;

    while b <= config.max_exponent {
        // Transform: let x' = x^b, then fit y = a*x' + c (linear regression)
        let x_transformed: Vec<f64> = points.iter().map(|(x, _)| x.powf(b)).collect();
        let y: Vec<f64> = points.iter().map(|(_, y)| *y).collect();

        // Linear regression for a and c
        let n = points.len() as f64;
        let sum_x: f64 = x_transformed.iter().sum();
        let sum_y: f64 = y.iter().sum();
        let sum_xy: f64 = x_transformed.iter().zip(&y).map(|(x, y)| x * y).sum();
        let sum_x2: f64 = x_transformed.iter().map(|x| x * x).sum();

        let denom = n * sum_x2 - sum_x * sum_x;
        if denom.abs() < 1e-10 {
            b += config.exponent_step;
            continue;
        }

        let a = (n * sum_xy - sum_x * sum_y) / denom;
        let c = (sum_y - a * sum_x) / n;

        // Compute R²
        let y_mean = sum_y / n;
        let ss_tot: f64 = y.iter().map(|yi| (yi - y_mean).powi(2)).sum();
        let ss_res: f64 = x_transformed
            .iter()
            .zip(&y)
            .map(|(xi, yi)| (yi - (a * xi + c)).powi(2))
            .sum();

        let r_squared = if ss_tot > 0.0 {
            1.0 - ss_res / ss_tot
        } else {
            0.0
        };

        if best_fit.is_none() || r_squared > best_fit.unwrap().3 {
            best_fit = Some((a, b, c, r_squared));
        }

        b += config.exponent_step;
    }

    best_fit
}

/// Fit a gap polynomial by skipping one point for validation.
///
/// # Arguments
///
/// * `points` - (x, y) pairs sorted by x (e.g., quality, metric)
/// * `skip_idx` - Index of point to skip for validation
/// * `config` - Fitting configuration
///
/// # Returns
///
/// The fitted polynomial with validation error, or `None` if fitting fails.
#[must_use]
pub fn fit_gap_polynomial(
    points: &[(u32, f64)],
    skip_idx: usize,
    config: &InterpolationConfig,
) -> Option<GapPolynomial> {
    if points.len() < 4 || skip_idx >= points.len() {
        return None;
    }

    let skipped = points[skip_idx];
    let training: Vec<(f64, f64)> = points
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != skip_idx)
        .map(|(_, (q, d))| (*q as f64, *d))
        .collect();

    let (a, b, c, r_squared) = fit_power_law(&training, config)?;

    // Validate by predicting the skipped point
    let predicted = a * (skipped.0 as f64).powf(b) + c;
    let validation_error = (predicted - skipped.1).abs();

    let q_low = points.first()?.0;
    let q_high = points.last()?.0;

    Some(GapPolynomial {
        q_low,
        q_high,
        a,
        b,
        c,
        r_squared,
        validation_error,
    })
}

/// Compute gap polynomials for all quality gaps, averaging adjacent fits.
///
/// This creates a smooth interpolation by:
/// 1. For each internal point, fit a polynomial by skipping it
/// 2. Average adjacent polynomial coefficients for smooth transitions
///
/// # Arguments
///
/// * `points` - (x, y) pairs sorted by x (e.g., quality, metric)
/// * `config` - Fitting configuration
///
/// # Example
///
/// ```
/// use codec_eval::interpolation::{compute_gap_polynomials, InterpolationConfig};
///
/// let points = vec![
///     (30, 0.010),
///     (50, 0.005),
///     (70, 0.002),
///     (80, 0.001),
///     (90, 0.0005),
/// ];
///
/// let polys = compute_gap_polynomials(&points, &InterpolationConfig::default());
/// assert!(!polys.is_empty());
/// ```
#[must_use]
pub fn compute_gap_polynomials(
    points: &[(u32, f64)],
    config: &InterpolationConfig,
) -> Vec<GapPolynomial> {
    if points.len() < 4 {
        return Vec::new();
    }

    let mut gap_polys = Vec::new();

    // For each internal point (not first or last), fit by skipping it
    for skip_idx in 1..points.len() - 1 {
        let q_low = points[skip_idx - 1].0;
        let q_high = points[skip_idx + 1].0;

        // Skip if gap is too small (consecutive values)
        if q_high - q_low <= 2 {
            continue;
        }

        // Fit polynomial by skipping this point
        if let Some(poly) = fit_gap_polynomial(points, skip_idx, config) {
            gap_polys.push((skip_idx, poly));
        }
    }

    // Average adjacent polynomials for each gap
    let mut result = Vec::new();
    for i in 0..gap_polys.len() {
        let (idx, poly) = &gap_polys[i];

        // Find adjacent polynomials to average with
        let mut a_sum = poly.a;
        let mut b_sum = poly.b;
        let mut c_sum = poly.c;
        let mut count = 1.0;

        // Average with previous if exists and overlaps
        if i > 0 {
            let (prev_idx, prev_poly) = &gap_polys[i - 1];
            if idx - prev_idx <= 2 {
                a_sum += prev_poly.a;
                b_sum += prev_poly.b;
                c_sum += prev_poly.c;
                count += 1.0;
            }
        }

        // Average with next if exists and overlaps
        if i + 1 < gap_polys.len() {
            let (next_idx, next_poly) = &gap_polys[i + 1];
            if next_idx - idx <= 2 {
                a_sum += next_poly.a;
                b_sum += next_poly.b;
                c_sum += next_poly.c;
                count += 1.0;
            }
        }

        result.push(GapPolynomial {
            q_low: poly.q_low,
            q_high: poly.q_high,
            a: a_sum / count,
            b: b_sum / count,
            c: c_sum / count,
            r_squared: poly.r_squared,
            validation_error: poly.validation_error,
        });
    }

    result
}

/// Linear interpolation to find x value for a target y.
///
/// Given (x, y) points where y typically decreases as x increases
/// (e.g., quality vs DSSIM), finds the x value that would produce
/// the target y.
///
/// # Arguments
///
/// * `target_y` - The target y value to find
/// * `points` - (x, y) pairs sorted by x
///
/// # Returns
///
/// The interpolated x value, or the closest point's x if target is outside range.
#[must_use]
pub fn linear_interpolate(target_y: f64, points: &[(u32, f64)]) -> Option<f64> {
    if points.is_empty() {
        return None;
    }
    if points.len() == 1 {
        return Some(points[0].0 as f64);
    }

    // Find two adjacent points that bracket the target
    for i in 0..points.len() - 1 {
        let (x1, y1) = points[i];
        let (x2, y2) = points[i + 1];

        // Check if target falls between these y values
        let in_range = (y1 <= target_y && target_y <= y2) || (y2 <= target_y && target_y <= y1);

        if in_range && (y2 - y1).abs() > 1e-12 {
            let t = (target_y - y1) / (y2 - y1);
            let interp_x = x1 as f64 + t * (x2 as f64 - x1 as f64);
            return Some(interp_x.clamp(0.0, 100.0));
        }
    }

    // Target outside range - return closest
    points
        .iter()
        .min_by(|a, b| {
            (a.1 - target_y)
                .abs()
                .partial_cmp(&(b.1 - target_y).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|(x, _)| *x as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gap_polynomial_interpolate() {
        let poly = GapPolynomial {
            q_low: 50,
            q_high: 90,
            a: 0.0001,
            b: 2.0,
            c: 0.0,
            r_squared: 0.99,
            validation_error: 0.001,
        };

        let result = poly.interpolate(70.0);
        assert!((result - 0.49).abs() < 0.01); // 0.0001 * 70^2 = 0.49
    }

    #[test]
    fn test_gap_polynomial_covers() {
        let poly = GapPolynomial {
            q_low: 50,
            q_high: 90,
            a: 1.0,
            b: 1.0,
            c: 0.0,
            r_squared: 0.99,
            validation_error: 0.001,
        };

        assert!(poly.covers(50));
        assert!(poly.covers(70));
        assert!(poly.covers(90));
        assert!(!poly.covers(49));
        assert!(!poly.covers(91));
    }

    #[test]
    fn test_fit_power_law() {
        // y = 2 * x^1 + 5 (linear)
        let points = vec![(10.0, 25.0), (20.0, 45.0), (30.0, 65.0), (40.0, 85.0)];

        let config = InterpolationConfig::default();
        let fit = fit_power_law(&points, &config);
        assert!(fit.is_some());

        let (_a, b, _c, r_squared) = fit.unwrap();
        assert!(r_squared > 0.99);
        // Should find b ≈ 1.0 for linear
        assert!((b - 1.0).abs() < 0.2);
    }

    #[test]
    fn test_compute_gap_polynomials() {
        let points = vec![
            (30, 0.010),
            (50, 0.005),
            (70, 0.002),
            (80, 0.001),
            (90, 0.0005),
        ];

        let config = InterpolationConfig::default();
        let polys = compute_gap_polynomials(&points, &config);

        // Should have at least one polynomial
        assert!(!polys.is_empty());

        // All should have reasonable R²
        for poly in &polys {
            assert!(poly.r_squared > 0.5);
        }
    }

    #[test]
    fn test_linear_interpolate() {
        // Decreasing y as x increases (typical quality vs DSSIM)
        let points = vec![(50, 0.010), (70, 0.005), (90, 0.002)];

        // Find quality for DSSIM = 0.007 (between 50 and 70)
        let x = linear_interpolate(0.007, &points);
        assert!(x.is_some());
        let x = x.unwrap();
        assert!(x > 50.0 && x < 70.0);
    }

    #[test]
    fn test_linear_interpolate_outside_range() {
        let points = vec![(50, 0.010), (90, 0.002)];

        // Target above range
        let x = linear_interpolate(0.015, &points);
        assert!(x.is_some());
        assert_eq!(x.unwrap(), 50.0); // Closest to high DSSIM

        // Target below range
        let x = linear_interpolate(0.001, &points);
        assert!(x.is_some());
        assert_eq!(x.unwrap(), 90.0); // Closest to low DSSIM
    }

    #[test]
    fn test_interpolation_table() {
        let mut table = InterpolationTable::new("mozjpeg", "desktop-1x");
        table.polynomials.push(GapPolynomial {
            q_low: 50,
            q_high: 90,
            a: 1.0,
            b: 1.0,
            c: 0.0,
            r_squared: 0.99,
            validation_error: 0.001,
        });

        // In range - uses polynomial
        let result = table.interpolate(70.0);
        assert!((result - 70.0).abs() < 0.01);

        // Out of range - falls back to identity
        let result = table.interpolate(30.0);
        assert_eq!(result, 30.0);
    }
}
