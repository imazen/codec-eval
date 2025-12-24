//! Statistical analysis and Pareto front calculation.
//!
//! This module provides tools for analyzing codec comparison results,
//! including Pareto front calculation for rate-distortion analysis.

mod pareto;

pub use pareto::{ParetoFront, RDPoint};

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

        let median = percentile(&sorted, 50.0);
        let min = sorted[0];
        let max = sorted[count - 1];

        Some(Self {
            count,
            mean,
            median,
            std_dev,
            min,
            max,
            p5: percentile(&sorted, 5.0),
            p25: percentile(&sorted, 25.0),
            p75: percentile(&sorted, 75.0),
            p95: percentile(&sorted, 95.0),
        })
    }
}

/// Calculate a percentile from sorted values.
fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }

    let p = p.clamp(0.0, 100.0) / 100.0;
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
        let sorted = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        assert!((percentile(&sorted, 0.0) - 1.0).abs() < 0.001);
        assert!((percentile(&sorted, 50.0) - 3.0).abs() < 0.001);
        assert!((percentile(&sorted, 100.0) - 5.0).abs() < 0.001);
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
