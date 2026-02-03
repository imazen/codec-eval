//! Statistical analysis and Pareto front calculation.
//!
//! This module provides tools for analyzing codec comparison results,
//! including Pareto front calculation for rate-distortion analysis.
//!
//! ## Core Statistics
//!
//! - [`Summary`]: Descriptive statistics (mean, median, std_dev, percentiles)
//! - [`median`], [`mean`], [`std_dev`]: Basic statistical functions
//! - [`percentile`], [`percentile_u32`]: Percentile calculation (R-7 interpolation)
//! - [`trimmed_mean`]: Robust mean excluding outliers
//! - [`iqr`]: Interquartile range
//!
//! ## Rate-Distortion Analysis
//!
//! - [`bd_rate`]: Bjontegaard Delta Rate calculation
//! - [`ParetoFront`]: Pareto-optimal points on RD curve

pub mod chart;
mod pareto;
pub mod rd_knee;

pub use chart::{ChartConfig, ChartPoint, ChartSeries, generate_svg};
pub use pareto::{ParetoFront, RDPoint};
pub use rd_knee::{
    AngleBin, AxisRange, BinScheme, CodecConfig, ConfiguredParetoFront, ConfiguredRDPoint,
    CorpusAggregate, DualAngleBin, EncodeResult, FixedFrame, NormalizationContext, ParamValue,
    QualityDirection, RDCalibration, RDKnee, RDPosition, plot_rd_svg,
};

use serde::{Deserialize, Serialize};

/// Descriptive statistics for a set of measurements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Summary {
    /// Number of values.
    pub count: usize,
    /// Mean value.
    pub mean: f64,
    /// Median value.
    pub median: f64,
    /// Standard deviation.
    pub std_dev: f64,
    /// Minimum value.
    pub min: f64,
    /// Maximum value.
    pub max: f64,
    /// 5th percentile.
    pub p5: f64,
    /// 25th percentile.
    pub p25: f64,
    /// 75th percentile.
    pub p75: f64,
    /// 95th percentile.
    pub p95: f64,
}

impl Summary {
    /// Compute summary statistics for a slice of values.
    ///
    /// Returns `None` if the slice is empty.
    #[must_use]
    pub fn compute(values: &[f64]) -> Option<Self> {
        if values.is_empty() {
            return None;
        }

        let mut sorted = values.to_vec();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        let count = sorted.len();
        let sum: f64 = sorted.iter().sum();
        let mean = sum / count as f64;

        let variance: f64 = sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / count as f64;
        let std_dev = variance.sqrt();

        let median = percentile_sorted(&sorted, 0.5);
        let min = sorted[0];
        let max = sorted[count - 1];

        Some(Self {
            count,
            mean,
            median,
            std_dev,
            min,
            max,
            p5: percentile_sorted(&sorted, 0.05),
            p25: percentile_sorted(&sorted, 0.25),
            p75: percentile_sorted(&sorted, 0.75),
            p95: percentile_sorted(&sorted, 0.95),
        })
    }
}

//=============================================================================
// Core Statistical Functions
//=============================================================================

/// Compute median of a slice.
///
/// For even-length slices, returns the average of the two middle values.
///
/// # Example
///
/// ```
/// use codec_eval::stats::median;
///
/// assert_eq!(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
/// assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
/// ```
#[must_use]
pub fn median(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let mid = sorted.len() / 2;
    if sorted.len() % 2 == 0 {
        (sorted[mid - 1] + sorted[mid]) / 2.0
    } else {
        sorted[mid]
    }
}

/// Compute arithmetic mean.
///
/// # Example
///
/// ```
/// use codec_eval::stats::mean;
///
/// assert!((mean(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 3.0).abs() < 0.001);
/// ```
#[must_use]
pub fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    values.iter().sum::<f64>() / values.len() as f64
}

/// Compute sample standard deviation.
///
/// Uses Bessel's correction (N-1 denominator) for sample standard deviation.
///
/// # Example
///
/// ```
/// use codec_eval::stats::std_dev;
///
/// let values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
/// assert!((std_dev(&values) - 2.138).abs() < 0.001);
/// ```
#[must_use]
pub fn std_dev(values: &[f64]) -> f64 {
    if values.len() < 2 {
        return 0.0;
    }
    let m = mean(values);
    let variance = values.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (values.len() - 1) as f64;
    variance.sqrt()
}

/// Compute percentile using linear interpolation (R-7 method).
///
/// This is the default method used by R, NumPy, and Excel.
/// The percentile `p` should be in the range 0.0 to 1.0.
///
/// # Example
///
/// ```
/// use codec_eval::stats::percentile;
///
/// let values = [1.0, 2.0, 3.0, 4.0, 5.0];
/// assert!((percentile(&values, 0.5) - 3.0).abs() < 0.001);  // median
/// assert!((percentile(&values, 0.25) - 2.0).abs() < 0.001); // Q1
/// assert!((percentile(&values, 0.75) - 4.0).abs() < 0.001); // Q3
/// ```
#[must_use]
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    percentile_sorted(&sorted, p)
}

/// Compute percentile for u32 values.
///
/// Returns the interpolated percentile rounded to the nearest integer.
///
/// # Example
///
/// ```
/// use codec_eval::stats::percentile_u32;
///
/// let values = [10, 20, 30, 40, 50];
/// assert_eq!(percentile_u32(&values, 0.5), 30);
/// ```
#[must_use]
pub fn percentile_u32(values: &[u32], p: f64) -> u32 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort();
    let pos = p.clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = pos.floor() as usize;
    let upper = (lower + 1).min(sorted.len() - 1);
    let frac = pos - lower as f64;
    let result = sorted[lower] as f64 * (1.0 - frac) + sorted[upper] as f64 * frac;
    result.round() as u32
}

/// Compute trimmed mean (removes top and bottom `trim_pct` of values).
///
/// This provides a robust estimate of central tendency that is less
/// affected by outliers than the arithmetic mean.
///
/// # Arguments
///
/// * `values` - The values to analyze
/// * `trim_pct` - Fraction to trim from each end (e.g., 0.1 = 10% from each end)
///
/// # Example
///
/// ```
/// use codec_eval::stats::trimmed_mean;
///
/// // Outliers at ends don't affect trimmed mean much
/// let values = [1.0, 10.0, 11.0, 12.0, 13.0, 100.0];
/// let tm = trimmed_mean(&values, 0.2);  // Trim 20% from each end
/// assert!((tm - 11.5).abs() < 0.001);   // Average of middle values
/// ```
#[must_use]
pub fn trimmed_mean(values: &[f64], trim_pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let trim_count = (sorted.len() as f64 * trim_pct.clamp(0.0, 0.5)) as usize;
    if trim_count * 2 >= sorted.len() {
        return median(values);
    }
    let trimmed = &sorted[trim_count..sorted.len() - trim_count];
    mean(trimmed)
}

/// Compute interquartile range (IQR = Q3 - Q1).
///
/// The IQR is a robust measure of spread that is not affected by outliers.
///
/// # Example
///
/// ```
/// use codec_eval::stats::iqr;
///
/// let values = [1.0, 2.0, 3.0, 4.0, 5.0];
/// assert!((iqr(&values) - 2.0).abs() < 0.001);  // Q3(4) - Q1(2) = 2
/// ```
#[must_use]
pub fn iqr(values: &[f64]) -> f64 {
    percentile(values, 0.75) - percentile(values, 0.25)
}

/// Internal: Calculate percentile from pre-sorted values.
/// Accepts percentile in 0-100 range for backward compatibility with Summary.
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    // Normalize to 0-1 range if given as percentage
    let p = if p > 1.0 { p / 100.0 } else { p };
    let p = p.clamp(0.0, 1.0);

    let idx = p * (sorted.len() - 1) as f64;
    let lower = idx.floor() as usize;
    let upper = idx.ceil() as usize;
    let frac = idx - lower as f64;

    if lower == upper {
        sorted[lower]
    } else {
        sorted[lower] * (1.0 - frac) + sorted[upper] * frac
    }
}

/// Calculate BD-Rate (Bjontegaard Delta Rate).
///
/// BD-Rate measures the average bitrate difference between two rate-distortion
/// curves at the same quality level. A negative value means the test curve
/// is more efficient (lower bitrate at same quality).
///
/// # Arguments
///
/// * `reference` - Reference curve points (bitrate, quality).
/// * `test` - Test curve points (bitrate, quality).
///
/// # Returns
///
/// BD-Rate as a percentage. Negative = test is better.
#[must_use]
pub fn bd_rate(reference: &[(f64, f64)], test: &[(f64, f64)]) -> Option<f64> {
    if reference.len() < 4 || test.len() < 4 {
        return None;
    }

    // Sort by quality
    let mut ref_sorted: Vec<_> = reference.to_vec();
    let mut test_sorted: Vec<_> = test.to_vec();
    ref_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    test_sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Find overlapping quality range
    let min_quality = ref_sorted[0].1.max(test_sorted[0].1);
    let max_quality = ref_sorted.last()?.1.min(test_sorted.last()?.1);

    if min_quality >= max_quality {
        return None;
    }

    // Use log-rate for integration
    let ref_log: Vec<_> = ref_sorted.iter().map(|(r, q)| (r.ln(), *q)).collect();
    let test_log: Vec<_> = test_sorted.iter().map(|(r, q)| (r.ln(), *q)).collect();

    // Integrate using trapezoidal rule (simplified)
    let ref_area = integrate_curve(&ref_log, min_quality, max_quality);
    let test_area = integrate_curve(&test_log, min_quality, max_quality);

    let avg_ref = ref_area / (max_quality - min_quality);
    let avg_test = test_area / (max_quality - min_quality);

    // BD-Rate = (10^(avg_test - avg_ref) - 1) * 100
    let bd = (10_f64.powf(avg_test - avg_ref) - 1.0) * 100.0;

    Some(bd)
}

/// Simple trapezoidal integration of a curve.
fn integrate_curve(points: &[(f64, f64)], min_x: f64, max_x: f64) -> f64 {
    let mut area = 0.0;

    for window in points.windows(2) {
        let (y0, x0) = window[0];
        let (y1, x1) = window[1];

        // Skip segments outside range
        if x1 < min_x || x0 > max_x {
            continue;
        }

        // Clip to range
        let x0 = x0.max(min_x);
        let x1 = x1.min(max_x);

        // Trapezoidal area
        area += (y0 + y1) / 2.0 * (x1 - x0);
    }

    area
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_summary_compute() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let summary = Summary::compute(&values).unwrap();

        assert_eq!(summary.count, 5);
        assert!((summary.mean - 3.0).abs() < 0.001);
        assert!((summary.median - 3.0).abs() < 0.001);
        assert!((summary.min - 1.0).abs() < 0.001);
        assert!((summary.max - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_summary_empty() {
        assert!(Summary::compute(&[]).is_none());
    }

    #[test]
    fn test_percentile() {
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&values, 0.0) - 1.0).abs() < 0.001);
        assert!((percentile(&values, 0.5) - 3.0).abs() < 0.001);
        assert!((percentile(&values, 1.0) - 5.0).abs() < 0.001);
    }

    #[test]
    fn test_median() {
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0, 5.0]), 3.0);
        assert_eq!(median(&[1.0, 2.0, 3.0, 4.0]), 2.5);
        assert_eq!(median(&[5.0]), 5.0);
        assert_eq!(median(&[]), 0.0);
    }

    #[test]
    fn test_trimmed_mean() {
        // With outliers
        let values = [1.0, 10.0, 11.0, 12.0, 13.0, 100.0];
        let tm = trimmed_mean(&values, 0.2);
        assert!((tm - 11.5).abs() < 0.001);

        // Regular case
        let values2 = [1.0, 2.0, 3.0, 4.0, 5.0];
        let tm2 = trimmed_mean(&values2, 0.2);
        assert!((tm2 - 3.0).abs() < 0.001);
    }

    #[test]
    fn test_iqr() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((iqr(&values) - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_percentile_u32() {
        let values = [10, 20, 30, 40, 50];
        assert_eq!(percentile_u32(&values, 0.0), 10);
        assert_eq!(percentile_u32(&values, 0.5), 30);
        assert_eq!(percentile_u32(&values, 1.0), 50);
    }

    #[test]
    fn test_bd_rate_same_curve() {
        let curve = vec![
            (1000.0, 30.0),
            (2000.0, 35.0),
            (4000.0, 40.0),
            (8000.0, 45.0),
        ];

        let bd = bd_rate(&curve, &curve);
        assert!(bd.is_some());
        assert!(bd.unwrap().abs() < 0.1); // Should be ~0 for same curve
    }
}
