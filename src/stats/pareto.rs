//! Pareto front calculation for rate-distortion analysis.
//!
//! A Pareto front identifies the set of non-dominated points where no other
//! point is better on all objectives. For codec comparison, this helps find
//! the best codec at each quality/size trade-off.

use serde::{Deserialize, Serialize};

/// A point on a rate-distortion curve.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RDPoint {
    /// Codec identifier.
    pub codec: String,

    /// Quality setting used.
    pub quality_setting: f64,

    /// Bits per pixel (rate).
    pub bpp: f64,

    /// Quality metric value (higher = better, e.g., SSIMULACRA2).
    /// For DSSIM, negate before adding.
    pub quality: f64,

    /// Optional encode time in milliseconds.
    pub encode_time_ms: Option<f64>,

    /// Optional image name.
    pub image: Option<String>,
}

impl RDPoint {
    /// Create a new RD point.
    #[must_use]
    pub fn new(codec: impl Into<String>, quality_setting: f64, bpp: f64, quality: f64) -> Self {
        Self {
            codec: codec.into(),
            quality_setting,
            bpp,
            quality,
            encode_time_ms: None,
            image: None,
        }
    }

    /// Check if this point dominates another.
    ///
    /// A point dominates another if it's better on at least one objective
    /// and not worse on any objective.
    ///
    /// Objectives:
    /// - Lower bpp is better (smaller files)
    /// - Higher quality is better
    #[must_use]
    pub fn dominates(&self, other: &Self) -> bool {
        let better_or_equal_bpp = self.bpp <= other.bpp;
        let better_or_equal_quality = self.quality >= other.quality;
        let strictly_better = self.bpp < other.bpp || self.quality > other.quality;

        better_or_equal_bpp && better_or_equal_quality && strictly_better
    }
}

/// Pareto front of rate-distortion points.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ParetoFront {
    /// Points on the Pareto front (non-dominated).
    pub points: Vec<RDPoint>,
}

impl ParetoFront {
    /// Compute the Pareto front from a set of points.
    ///
    /// Returns a new `ParetoFront` containing only the non-dominated points.
    #[must_use]
    pub fn compute(points: &[RDPoint]) -> Self {
        let mut front = Vec::new();

        for point in points {
            // Check if any existing front point dominates this one
            let is_dominated = front.iter().any(|p: &RDPoint| p.dominates(point));

            if !is_dominated {
                // Remove any front points that this point dominates
                front.retain(|p| !point.dominates(p));
                front.push(point.clone());
            }
        }

        // Sort by bpp for easy plotting
        front.sort_by(|a, b| {
            a.bpp
                .partial_cmp(&b.bpp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Self { points: front }
    }

    /// Check if the front is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Get the number of points on the front.
    #[must_use]
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Get points that achieve at least the target quality.
    #[must_use]
    pub fn at_quality(&self, min_quality: f64) -> Vec<&RDPoint> {
        self.points
            .iter()
            .filter(|p| p.quality >= min_quality)
            .collect()
    }

    /// Get points that achieve at most the target bpp.
    #[must_use]
    pub fn at_bpp(&self, max_bpp: f64) -> Vec<&RDPoint> {
        self.points.iter().filter(|p| p.bpp <= max_bpp).collect()
    }

    /// Get the best point (highest quality) at or below the target bpp.
    #[must_use]
    pub fn best_at_bpp(&self, max_bpp: f64) -> Option<&RDPoint> {
        self.points
            .iter()
            .filter(|p| p.bpp <= max_bpp)
            .max_by(|a, b| {
                a.quality
                    .partial_cmp(&b.quality)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get the most efficient point (lowest bpp) at or above the target quality.
    #[must_use]
    pub fn best_at_quality(&self, min_quality: f64) -> Option<&RDPoint> {
        self.points
            .iter()
            .filter(|p| p.quality >= min_quality)
            .min_by(|a, b| {
                a.bpp
                    .partial_cmp(&b.bpp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    /// Get unique codecs on the Pareto front.
    #[must_use]
    pub fn codecs(&self) -> Vec<&str> {
        let mut codecs: Vec<&str> = self.points.iter().map(|p| p.codec.as_str()).collect();
        codecs.sort();
        codecs.dedup();
        codecs
    }

    /// Filter to points from a specific codec.
    #[must_use]
    pub fn filter_codec(&self, codec: &str) -> Vec<&RDPoint> {
        self.points.iter().filter(|p| p.codec == codec).collect()
    }

    /// Compute per-codec Pareto fronts.
    #[must_use]
    pub fn per_codec(points: &[RDPoint]) -> std::collections::HashMap<String, ParetoFront> {
        use std::collections::HashMap;

        let mut by_codec: HashMap<String, Vec<RDPoint>> = HashMap::new();

        for point in points {
            by_codec
                .entry(point.codec.clone())
                .or_default()
                .push(point.clone());
        }

        by_codec
            .into_iter()
            .map(|(codec, pts)| (codec, Self::compute(&pts)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dominates() {
        let p1 = RDPoint::new("a", 80.0, 1.0, 90.0); // bpp=1, quality=90
        let p2 = RDPoint::new("b", 80.0, 2.0, 85.0); // bpp=2, quality=85

        // p1 dominates p2 (lower bpp AND higher quality)
        assert!(p1.dominates(&p2));
        assert!(!p2.dominates(&p1));
    }

    #[test]
    fn test_no_dominance() {
        let p1 = RDPoint::new("a", 80.0, 1.0, 85.0); // bpp=1, quality=85
        let p2 = RDPoint::new("b", 80.0, 2.0, 90.0); // bpp=2, quality=90

        // Neither dominates (trade-off: p1 is smaller, p2 is higher quality)
        assert!(!p1.dominates(&p2));
        assert!(!p2.dominates(&p1));
    }

    #[test]
    fn test_pareto_front_basic() {
        let points = vec![
            RDPoint::new("a", 80.0, 1.0, 80.0),
            RDPoint::new("b", 80.0, 2.0, 90.0),
            RDPoint::new("c", 80.0, 3.0, 85.0), // Dominated by b
            RDPoint::new("d", 80.0, 0.5, 70.0),
        ];

        let front = ParetoFront::compute(&points);

        // Should have 3 points on front: d (smallest), a, b (highest quality)
        // c is dominated by b (same or better on both)
        assert_eq!(front.len(), 3);

        let codecs = front.codecs();
        assert!(codecs.contains(&"a"));
        assert!(codecs.contains(&"b"));
        assert!(codecs.contains(&"d"));
        assert!(!codecs.contains(&"c"));
    }

    #[test]
    fn test_pareto_best_at_bpp() {
        let points = vec![
            RDPoint::new("a", 80.0, 1.0, 80.0),
            RDPoint::new("b", 80.0, 2.0, 90.0),
            RDPoint::new("c", 80.0, 0.5, 70.0),
        ];

        let front = ParetoFront::compute(&points);

        // Best at bpp <= 1.0 should be "a" (higher quality than "c")
        let best = front.best_at_bpp(1.0).unwrap();
        assert_eq!(best.codec, "a");

        // Best at bpp <= 2.0 should be "b" (highest quality)
        let best = front.best_at_bpp(2.0).unwrap();
        assert_eq!(best.codec, "b");
    }

    #[test]
    fn test_pareto_best_at_quality() {
        let points = vec![
            RDPoint::new("a", 80.0, 1.0, 80.0),
            RDPoint::new("b", 80.0, 2.0, 90.0),
            RDPoint::new("c", 80.0, 0.5, 70.0),
        ];

        let front = ParetoFront::compute(&points);

        // Most efficient at quality >= 80 should be "a"
        let best = front.best_at_quality(80.0).unwrap();
        assert_eq!(best.codec, "a");

        // Most efficient at quality >= 85 should be "b"
        let best = front.best_at_quality(85.0).unwrap();
        assert_eq!(best.codec, "b");
    }
}
