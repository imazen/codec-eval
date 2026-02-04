//! Fixed-frame R-D curve parameterization with corner-based angles.
//!
//! Every encode lives in a triangle: the worst corner (max bpp, zero quality)
//! is the origin, and the angle from that corner describes position on the
//! rate-distortion tradeoff. This is comparable across codecs, corpora, and
//! resolutions because the frame is fixed by the metric scales and a practical
//! bpp ceiling.
//!
//! ## Fixed frame (web targeting)
//!
//! | Axis | Min | Max | Notes |
//! |------|-----|-----|-------|
//! | bpp  | 0   | 4.0 | Practical web ceiling |
//! | s2   | 0   | 100 | SSIMULACRA2 scale |
//! | ba   | 0   | 15  | Butteraugli practical floor (inverted) |
//!
//! ## Corner angle
//!
//! `θ = atan2(quality_norm * aspect, 1.0 - bpp_norm)`
//!
//! The aspect ratio is calibrated from the reference codec knee
//! (mozjpeg 4:2:0 on CID22) so that the knee lands at exactly 45°.
//!
//! - θ < 0°  → worse than the worst corner (negative quality)
//! - θ = 0°  → worst corner (max bpp, zero quality)
//! - θ < 45° → compression-efficient (below the knee)
//! - θ = 45° → reference knee (balanced tradeoff)
//! - θ ≈ 52° → ideal diagonal (zero bpp, perfect quality)
//! - θ > 52° → quality-dominated (spending bits for diminishing returns)
//! - θ = 90° → no compression (max bpp, max quality)
//! - θ > 90° → over-budget (bpp exceeds frame ceiling)
//!
//! The **knee** (45° tangent on the corpus-aggregate curve) is a landmark
//! within this system, not the origin. Its angle tells you where the
//! "balanced tradeoff" falls for a given codec.
//!
//! ## Dual-metric angles
//!
//! SSIMULACRA2 and Butteraugli produce different angles for the same encode.
//! Comparing `theta_s2` and `theta_ba` reveals what kind of artifacts the
//! codec configuration produces at that operating point.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

// ---------------------------------------------------------------------------
// Fixed frame
// ---------------------------------------------------------------------------

/// Fixed normalization frame for web-targeted R-D analysis.
///
/// Uses metric-native scales and an aspect ratio calibrated so the
/// reference knee (mozjpeg 4:2:0 on CID22) lands at exactly 45 degrees.
/// Angles are comparable across codecs and corpora.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FixedFrame {
    /// Maximum bpp (practical ceiling). Default: 4.0 for web.
    pub bpp_max: f64,
    /// SSIMULACRA2 scale maximum. Always 100.
    pub s2_max: f64,
    /// Butteraugli practical worst-case. Default: 15.0.
    pub ba_max: f64,
    /// Quality-axis stretch factor. Calibrated from reference knee so
    /// that `atan2(q_norm * aspect, 1 - bpp_norm) = 45 deg` at the knee.
    pub aspect: f64,
}

impl FixedFrame {
    /// Standard web-targeting frame.
    ///
    /// Aspect ratio calibrated from CID22-training mozjpeg 4:2:0 s2 knee
    /// at (0.7274 bpp, s2=65.10):
    /// `aspect = (1 - 0.7274/4.0) / (65.10/100.0) = 1.2568`
    pub const WEB: Self = Self {
        bpp_max: 4.0,
        s2_max: 100.0,
        ba_max: 15.0,
        aspect: (1.0 - 0.7274 / 4.0) / (65.10 / 100.0),
    };

    /// Compute the corner angle for an SSIMULACRA2 measurement.
    ///
    /// Origin is the worst corner: (bpp_max, s2=0).
    /// The aspect ratio stretches the quality axis so the reference
    /// knee is at 45 degrees. Angles can exceed 90 degrees or go
    /// below 0 degrees for extreme encodes.
    #[must_use]
    pub fn s2_angle(&self, bpp: f64, s2: f64) -> f64 {
        let bpp_norm = bpp / self.bpp_max;
        let s2_norm = s2 / self.s2_max;
        (s2_norm * self.aspect).atan2(1.0 - bpp_norm).to_degrees()
    }

    /// Compute the corner angle for a Butteraugli measurement.
    ///
    /// Butteraugli is inverted: lower = better. We normalize so that
    /// ba=0 means quality_norm=1.0, ba=ba_max means quality_norm=0.0.
    /// Same aspect ratio as s2 for comparable dual-angle analysis.
    #[must_use]
    pub fn ba_angle(&self, bpp: f64, ba: f64) -> f64 {
        let bpp_norm = bpp / self.bpp_max;
        let ba_norm = 1.0 - ba / self.ba_max;
        (ba_norm * self.aspect).atan2(1.0 - bpp_norm).to_degrees()
    }

    /// Compute dual-angle position for an encode.
    #[must_use]
    pub fn position(&self, bpp: f64, s2: f64, ba: f64) -> RDPosition {
        RDPosition {
            theta_s2: self.s2_angle(bpp, s2),
            theta_ba: self.ba_angle(bpp, ba),
            bpp,
            ssimulacra2: s2,
            butteraugli: ba,
        }
    }
}

impl Default for FixedFrame {
    fn default() -> Self {
        Self::WEB
    }
}

// ---------------------------------------------------------------------------
// Normalization (retained for knee detection on corpus-aggregate data)
// ---------------------------------------------------------------------------

/// Range for normalizing an axis to [0, 1].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AxisRange {
    pub min: f64,
    pub max: f64,
}

impl AxisRange {
    #[must_use]
    pub fn new(min: f64, max: f64) -> Self {
        debug_assert!(max > min, "AxisRange max must exceed min");
        Self { min, max }
    }

    #[must_use]
    pub fn normalize(&self, value: f64) -> f64 {
        (value - self.min) / (self.max - self.min)
    }

    #[must_use]
    pub fn denormalize(&self, norm: f64) -> f64 {
        norm * (self.max - self.min) + self.min
    }

    #[must_use]
    pub fn span(&self) -> f64 {
        self.max - self.min
    }
}

/// Direction of a quality metric.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityDirection {
    HigherIsBetter,
    LowerIsBetter,
}

/// Normalization context for knee detection (uses per-curve observed ranges).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct NormalizationContext {
    pub bpp_range: AxisRange,
    pub quality_range: AxisRange,
    pub direction: QualityDirection,
}

impl NormalizationContext {
    #[must_use]
    pub fn normalize_bpp(&self, bpp: f64) -> f64 {
        self.bpp_range.normalize(bpp)
    }

    #[must_use]
    pub fn normalize_quality(&self, raw_quality: f64) -> f64 {
        match self.direction {
            QualityDirection::HigherIsBetter => self.quality_range.normalize(raw_quality),
            QualityDirection::LowerIsBetter => 1.0 - self.quality_range.normalize(raw_quality),
        }
    }
}

// ---------------------------------------------------------------------------
// Knee point (landmark within the fixed frame)
// ---------------------------------------------------------------------------

/// The 45° tangent point on a corpus-aggregate R-D curve.
///
/// Computed using per-curve normalization (where the slope = 1 in normalized
/// space), then placed in the fixed frame as a landmark angle.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RDKnee {
    /// Bits per pixel at the knee (raw).
    pub bpp: f64,

    /// Quality metric value at the knee (raw units).
    pub quality: f64,

    /// Angle of this knee in the fixed-frame corner system (degrees).
    /// Computed from `FixedFrame::s2_angle` or `FixedFrame::ba_angle`.
    pub fixed_angle: f64,

    /// The per-curve normalization context used for knee detection.
    pub norm: NormalizationContext,
}

// ---------------------------------------------------------------------------
// Calibration
// ---------------------------------------------------------------------------

/// Dual-metric calibration with knee landmarks in the fixed frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RDCalibration {
    /// The fixed frame used for angle computation.
    pub frame: FixedFrame,

    /// Knee in SSIMULACRA2 space.
    pub ssimulacra2: RDKnee,

    /// Knee in Butteraugli space.
    pub butteraugli: RDKnee,

    /// Which corpus was used.
    pub corpus: String,

    /// Codec this calibration applies to.
    pub codec: String,

    /// Number of images averaged.
    pub image_count: usize,

    /// ISO 8601 timestamp.
    pub computed_at: String,
}

impl RDCalibration {
    /// The bpp range where the two knees disagree.
    #[must_use]
    pub fn disagreement_range(&self) -> (f64, f64) {
        let a = self.ssimulacra2.bpp;
        let b = self.butteraugli.bpp;
        (a.min(b), a.max(b))
    }

    /// Compute dual-angle position using the fixed frame.
    #[must_use]
    pub fn position(&self, bpp: f64, s2: f64, ba: f64) -> RDPosition {
        self.frame.position(bpp, s2, ba)
    }
}

// ---------------------------------------------------------------------------
// Position in fixed-frame corner space
// ---------------------------------------------------------------------------

/// An encode's position in the fixed-frame corner coordinate system.
///
/// Both angles are measured from the worst corner (max bpp, zero quality).
/// Higher angle = better (closer to the ideal of zero-cost perfect quality).
///
/// Comparing the two angles reveals artifact character:
/// - `theta_s2 ≈ theta_ba` → uniform quality tradeoff
/// - `theta_s2 > theta_ba` → better structural fidelity than local contrast
/// - `theta_s2 < theta_ba` → better local contrast than structural fidelity
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RDPosition {
    /// Corner angle in SSIMULACRA2 space (degrees, 0–90).
    pub theta_s2: f64,

    /// Corner angle in Butteraugli space (degrees, 0–90).
    pub theta_ba: f64,

    /// Raw bits per pixel.
    pub bpp: f64,

    /// Raw SSIMULACRA2 score (0–100, higher is better).
    pub ssimulacra2: f64,

    /// Raw Butteraugli distance (0+, lower is better).
    pub butteraugli: f64,
}

impl RDPosition {
    /// In the disagreement zone between the two knees.
    #[must_use]
    pub fn in_disagreement_zone(&self, cal: &RDCalibration) -> bool {
        let (lo, hi) = cal.disagreement_range();
        self.bpp >= lo && self.bpp <= hi
    }

    /// Which angular bin (by s2 angle).
    #[must_use]
    pub fn bin(&self, scheme: &BinScheme) -> AngleBin {
        scheme.bin_for(self.theta_s2)
    }

    /// Dual-metric bin.
    #[must_use]
    pub fn dual_bin(&self, scheme: &BinScheme) -> DualAngleBin {
        DualAngleBin {
            s2: scheme.bin_for(self.theta_s2),
            ba: scheme.bin_for(self.theta_ba),
        }
    }
}

// ---------------------------------------------------------------------------
// Angular binning
// ---------------------------------------------------------------------------

/// Defines how the [0°, 90°] range is divided into bins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinScheme {
    /// Center of the first bin (degrees).
    pub start: f64,
    /// Width of each bin (degrees).
    pub width: f64,
    /// Number of bins.
    pub count: usize,
}

impl BinScheme {
    /// Cover [lo, hi] with `count` equal-width bins.
    #[must_use]
    pub fn range(lo: f64, hi: f64, count: usize) -> Self {
        let width = (hi - lo) / count as f64;
        Self {
            start: lo + width / 2.0,
            width,
            count,
        }
    }

    /// Default: 18 bins of 5° each covering [0°, 90°].
    #[must_use]
    pub fn default_18() -> Self {
        Self::range(0.0, 90.0, 18)
    }

    /// Fine: 36 bins of 2.5° each covering [0°, 90°].
    #[must_use]
    pub fn fine_36() -> Self {
        Self::range(0.0, 90.0, 36)
    }

    /// Determine which bin an angle falls into.
    #[must_use]
    pub fn bin_for(&self, angle_deg: f64) -> AngleBin {
        let first_edge = self.start - self.width / 2.0;
        let offset = angle_deg - first_edge;
        let idx = (offset / self.width).floor();
        let idx = (idx.clamp(0.0, (self.count - 1) as f64)) as usize;
        let center = self.start + idx as f64 * self.width;
        AngleBin {
            index: idx,
            center,
            width: self.width,
        }
    }

    /// Iterate over all bins.
    pub fn bins(&self) -> impl Iterator<Item = AngleBin> + '_ {
        (0..self.count).map(move |i| {
            let center = self.start + i as f64 * self.width;
            AngleBin {
                index: i,
                center,
                width: self.width,
            }
        })
    }
}

/// A single angular bin.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct AngleBin {
    pub index: usize,
    pub center: f64,
    pub width: f64,
}

impl AngleBin {
    #[must_use]
    pub fn lo(&self) -> f64 {
        self.center - self.width / 2.0
    }

    #[must_use]
    pub fn hi(&self) -> f64 {
        self.center + self.width / 2.0
    }

    #[must_use]
    pub fn contains(&self, angle_deg: f64) -> bool {
        angle_deg >= self.lo() && angle_deg < self.hi()
    }
}

/// Dual-metric bin.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DualAngleBin {
    pub s2: AngleBin,
    pub ba: AngleBin,
}

// ---------------------------------------------------------------------------
// Codec configuration tracking
// ---------------------------------------------------------------------------

/// A single tuning parameter value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum ParamValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    Text(String),
}

impl std::fmt::Display for ParamValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Int(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::Bool(v) => write!(f, "{v}"),
            Self::Text(v) => write!(f, "{v}"),
        }
    }
}

/// The full set of tuning knobs that produced a particular encode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodecConfig {
    pub codec: String,
    pub version: String,
    pub params: BTreeMap<String, ParamValue>,
}

impl CodecConfig {
    #[must_use]
    pub fn new(codec: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            codec: codec.into(),
            version: version.into(),
            params: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn with_param(mut self, key: impl Into<String>, value: ParamValue) -> Self {
        self.params.insert(key.into(), value);
        self
    }

    #[must_use]
    pub fn fingerprint(&self) -> String {
        let params: Vec<String> = self
            .params
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        format!("{}@{} [{}]", self.codec, self.version, params.join(", "))
    }
}

// ---------------------------------------------------------------------------
// Pareto frontier
// ---------------------------------------------------------------------------

/// A point on the configuration-aware Pareto frontier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredRDPoint {
    pub position: RDPosition,
    pub config: CodecConfig,
    pub image: Option<String>,
    pub encode_time_ms: Option<f64>,
    pub decode_time_ms: Option<f64>,
}

/// Pareto frontier with angular binning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfiguredParetoFront {
    pub calibration: RDCalibration,
    pub scheme: BinScheme,
    pub points: Vec<ConfiguredRDPoint>,
}

impl ConfiguredParetoFront {
    /// Compute non-dominated front (bpp vs s2).
    #[must_use]
    pub fn compute(
        points: Vec<ConfiguredRDPoint>,
        calibration: RDCalibration,
        scheme: BinScheme,
    ) -> Self {
        let mut front: Vec<ConfiguredRDPoint> = Vec::new();

        for point in &points {
            let dominated = front.iter().any(|p| {
                p.position.bpp <= point.position.bpp
                    && p.position.ssimulacra2 >= point.position.ssimulacra2
                    && (p.position.bpp < point.position.bpp
                        || p.position.ssimulacra2 > point.position.ssimulacra2)
            });

            if !dominated {
                front.retain(|p| {
                    !(point.position.bpp <= p.position.bpp
                        && point.position.ssimulacra2 >= p.position.ssimulacra2
                        && (point.position.bpp < p.position.bpp
                            || point.position.ssimulacra2 > p.position.ssimulacra2))
                });
                front.push(point.clone());
            }
        }

        front.sort_by(|a, b| {
            a.position
                .bpp
                .partial_cmp(&b.position.bpp)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Self {
            calibration,
            scheme,
            points: front,
        }
    }

    #[must_use]
    pub fn best_config_for_s2(&self, min_s2: f64) -> Option<&ConfiguredRDPoint> {
        self.points
            .iter()
            .filter(|p| p.position.ssimulacra2 >= min_s2)
            .min_by(|a, b| {
                a.position
                    .bpp
                    .partial_cmp(&b.position.bpp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    #[must_use]
    pub fn best_config_for_ba(&self, max_ba: f64) -> Option<&ConfiguredRDPoint> {
        self.points
            .iter()
            .filter(|p| p.position.butteraugli <= max_ba)
            .min_by(|a, b| {
                a.position
                    .bpp
                    .partial_cmp(&b.position.bpp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    #[must_use]
    pub fn best_config_for_bpp(&self, max_bpp: f64) -> Option<&ConfiguredRDPoint> {
        self.points
            .iter()
            .filter(|p| p.position.bpp <= max_bpp)
            .max_by(|a, b| {
                a.position
                    .ssimulacra2
                    .partial_cmp(&b.position.ssimulacra2)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    }

    #[must_use]
    pub fn in_bin(&self, bin: &AngleBin) -> Vec<&ConfiguredRDPoint> {
        self.points
            .iter()
            .filter(|p| bin.contains(p.position.theta_s2))
            .collect()
    }

    #[must_use]
    pub fn coverage(&self) -> Vec<(AngleBin, usize)> {
        self.scheme
            .bins()
            .map(|bin| {
                let count = self
                    .points
                    .iter()
                    .filter(|p| bin.contains(p.position.theta_s2))
                    .count();
                (bin, count)
            })
            .collect()
    }

    #[must_use]
    pub fn empty_bins(&self) -> Vec<AngleBin> {
        self.coverage()
            .into_iter()
            .filter(|(_, count)| *count == 0)
            .map(|(bin, _)| bin)
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Corpus aggregate and knee computation
// ---------------------------------------------------------------------------

/// A single encode result from one image at one quality level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncodeResult {
    pub bpp: f64,
    pub ssimulacra2: f64,
    pub butteraugli: f64,
    pub image: String,
    pub config: CodecConfig,
}

/// Aggregated R-D data from a corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusAggregate {
    pub corpus: String,
    pub codec: String,
    /// Averaged R-D points sorted by bpp: (bpp, mean_s2, mean_butteraugli).
    pub curve: Vec<(f64, f64, f64)>,
    pub image_count: usize,
}

impl CorpusAggregate {
    /// Find the SSIMULACRA2 knee and express it in the fixed frame.
    #[must_use]
    pub fn ssimulacra2_knee(&self, frame: &FixedFrame) -> Option<RDKnee> {
        self.find_knee_for(
            QualityDirection::HigherIsBetter,
            |(_b, s, _ba)| *s,
            |bpp, quality| frame.s2_angle(bpp, quality),
        )
    }

    /// Find the Butteraugli knee and express it in the fixed frame.
    #[must_use]
    pub fn butteraugli_knee(&self, frame: &FixedFrame) -> Option<RDKnee> {
        self.find_knee_for(
            QualityDirection::LowerIsBetter,
            |(_b, _s, ba)| *ba,
            |bpp, quality| frame.ba_angle(bpp, quality),
        )
    }

    /// Compute the full dual-metric calibration.
    #[must_use]
    pub fn calibrate(&self, frame: &FixedFrame) -> Option<RDCalibration> {
        let s2_knee = self.ssimulacra2_knee(frame)?;
        let ba_knee = self.butteraugli_knee(frame)?;

        Some(RDCalibration {
            frame: *frame,
            ssimulacra2: s2_knee,
            butteraugli: ba_knee,
            corpus: self.corpus.clone(),
            codec: self.codec.clone(),
            image_count: self.image_count,
            computed_at: String::new(),
        })
    }

    fn find_knee_for(
        &self,
        direction: QualityDirection,
        extract: impl Fn(&(f64, f64, f64)) -> f64,
        compute_fixed_angle: impl Fn(f64, f64) -> f64,
    ) -> Option<RDKnee> {
        if self.curve.len() < 3 {
            return None;
        }

        let bpp_vals: Vec<f64> = self.curve.iter().map(|(b, _, _)| *b).collect();
        let q_vals: Vec<f64> = self.curve.iter().map(&extract).collect();

        let bpp_range = AxisRange::new(
            *bpp_vals.iter().min_by(|a, b| a.partial_cmp(b).unwrap())?,
            *bpp_vals.iter().max_by(|a, b| a.partial_cmp(b).unwrap())?,
        );
        let quality_range = AxisRange::new(
            *q_vals.iter().min_by(|a, b| a.partial_cmp(b).unwrap())?,
            *q_vals.iter().max_by(|a, b| a.partial_cmp(b).unwrap())?,
        );

        let norm = NormalizationContext {
            bpp_range,
            quality_range,
            direction,
        };

        find_knee(&self.curve, &norm, &extract, &compute_fixed_angle)
    }
}

/// Find the knee (45° tangent) on a per-curve normalized R-D curve,
/// then express it in the fixed frame.
fn find_knee(
    curve: &[(f64, f64, f64)],
    norm: &NormalizationContext,
    extract_quality: &impl Fn(&(f64, f64, f64)) -> f64,
    compute_fixed_angle: &impl Fn(f64, f64) -> f64,
) -> Option<RDKnee> {
    if curve.len() < 2 {
        return None;
    }

    let mut slopes: Vec<(usize, f64)> = Vec::new();
    for i in 0..curve.len() - 1 {
        let bpp0 = norm.normalize_bpp(curve[i].0);
        let bpp1 = norm.normalize_bpp(curve[i + 1].0);
        let q0 = norm.normalize_quality(extract_quality(&curve[i]));
        let q1 = norm.normalize_quality(extract_quality(&curve[i + 1]));

        let d_bpp = bpp1 - bpp0;
        if d_bpp.abs() < 1e-12 {
            continue;
        }

        slopes.push((i, (q1 - q0) / d_bpp));
    }

    if slopes.is_empty() {
        return None;
    }

    let crossing_idx = slopes
        .iter()
        .position(|(_, slope)| *slope <= 1.0)
        .unwrap_or(slopes.len() / 2);

    let (seg_idx, _) = slopes[crossing_idx];
    let bpp = (curve[seg_idx].0 + curve[seg_idx + 1].0) / 2.0;
    let quality =
        (extract_quality(&curve[seg_idx]) + extract_quality(&curve[seg_idx + 1])) / 2.0;

    Some(RDKnee {
        bpp,
        quality,
        fixed_angle: compute_fixed_angle(bpp, quality),
        norm: *norm,
    })
}

// ---------------------------------------------------------------------------
// SVG plotting
// ---------------------------------------------------------------------------

/// Generate an SVG plot of the R-D curve with corner angle grid and knee markers.
///
/// Plots (bpp, s2) with angle reference lines radiating from the worst corner,
/// and marks the knee positions.
#[must_use]
pub fn plot_rd_svg(
    curve: &[(f64, f64, f64)],
    calibration: &RDCalibration,
    title: &str,
) -> String {
    let frame = &calibration.frame;
    let margin = 60.0_f64;
    let plot_w = 600.0_f64;
    let plot_h = 400.0_f64;
    let total_w = plot_w + margin * 2.0;
    let total_h = plot_h + margin * 2.0;

    // Coordinate transforms: data → SVG pixel
    // bpp axis: 0 → margin, bpp_max → margin + plot_w
    // s2 axis:  0 → margin + plot_h, s2_max → margin (SVG y is inverted)
    let x_of = |bpp: f64| -> f64 { margin + (bpp / frame.bpp_max) * plot_w };
    let y_of = |s2: f64| -> f64 { margin + plot_h - (s2.max(0.0) / frame.s2_max) * plot_h };

    // Corner origin in SVG space: (bpp_max, s2=0) → bottom-right
    let cx = x_of(frame.bpp_max);
    let cy = y_of(0.0);

    let mut svg = String::with_capacity(8192);

    // Header
    let _ = write!(
        svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {total_w} {total_h}" font-family="monospace" font-size="11">"##
    );

    // Background
    let _ = write!(
        svg,
        r##"<rect width="{total_w}" height="{total_h}" fill="#1a1a2e"/>"##
    );

    // Plot area background
    let _ = write!(
        svg,
        r##"<rect x="{margin}" y="{margin}" width="{plot_w}" height="{plot_h}" fill="#16213e" stroke="#333" stroke-width="1"/>"##
    );

    // Angle reference lines from the corner
    for deg in (0..=90).step_by(15) {
        let rad = (deg as f64).to_radians();
        let q_norm = rad.sin();
        let r_norm = rad.cos(); // 1.0 - bpp_norm → bpp_norm = 1.0 - r_norm

        // Line from corner to the edge of the plot
        // Extend to hit plot boundary
        let scale = if r_norm.abs() > 1e-6 {
            (1.0 / r_norm).min(if q_norm.abs() > 1e-6 {
                1.0 / q_norm
            } else {
                f64::MAX
            })
        } else if q_norm.abs() > 1e-6 {
            1.0 / q_norm
        } else {
            1.0
        };

        let bpp_far = frame.bpp_max * (1.0 - r_norm * scale).clamp(0.0, 1.0);
        let s2_far = (frame.s2_max * q_norm * scale).clamp(0.0, frame.s2_max);

        let opacity = if deg == 45 { "0.4" } else { "0.15" };
        let color = if deg == 45 { "#ffd700" } else { "#888" };

        let _ = write!(
            svg,
            r##"<line x1="{}" y1="{}" x2="{}" y2="{}" stroke="{color}" stroke-width="1" stroke-dasharray="4,4" opacity="{opacity}"/>"##,
            cx, cy,
            x_of(bpp_far), y_of(s2_far)
        );

        // Angle label near the corner
        let label_dist = 35.0;
        let lx = cx - label_dist * r_norm;
        let ly = cy - label_dist * q_norm;
        let _ = write!(
            svg,
            r##"<text x="{lx:.0}" y="{ly:.0}" fill="#666" text-anchor="middle" font-size="9">{deg}°</text>"##
        );
    }

    // Grid lines
    for bpp_tick in [0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5] {
        let x = x_of(bpp_tick);
        let _ = write!(
            svg,
            r##"<line x1="{x}" y1="{margin}" x2="{x}" y2="{}" stroke="#333" stroke-width="0.5"/>"##,
            margin + plot_h
        );
        let _ = write!(
            svg,
            r##"<text x="{x}" y="{}" fill="#888" text-anchor="middle">{bpp_tick}</text>"##,
            margin + plot_h + 16.0
        );
    }
    for s2_tick in [0.0, 20.0, 40.0, 60.0, 80.0, 100.0] {
        let y = y_of(s2_tick);
        let _ = write!(
            svg,
            r##"<line x1="{margin}" y1="{y}" x2="{}" y2="{y}" stroke="#333" stroke-width="0.5"/>"##,
            margin + plot_w
        );
        let _ = write!(
            svg,
            r##"<text x="{}" y="{}" fill="#888" text-anchor="end">{s2_tick:.0}</text>"##,
            margin - 6.0,
            y + 4.0
        );
    }

    // R-D curve (s2)
    if curve.len() >= 2 {
        let mut path = String::from("M");
        for (i, (bpp, s2, _ba)) in curve.iter().enumerate() {
            let sep = if i == 0 { "" } else { " L" };
            let _ = write!(path, "{sep}{:.1},{:.1}", x_of(*bpp), y_of(*s2));
        }
        let _ = write!(
            svg,
            r##"<path d="{path}" fill="none" stroke="#e74c3c" stroke-width="2.5" stroke-linejoin="round"/>"##
        );

        // Data points
        for (bpp, s2, _ba) in curve {
            let _ = write!(
                svg,
                r##"<circle cx="{:.1}" cy="{:.1}" r="3" fill="#e74c3c" opacity="0.8"/>"##,
                x_of(*bpp),
                y_of(*s2)
            );
        }
    }

    // Knee markers
    let s2_knee = &calibration.ssimulacra2;
    let kx = x_of(s2_knee.bpp);
    let ky = y_of(s2_knee.quality);
    let _ = write!(
        svg,
        r##"<circle cx="{kx:.1}" cy="{ky:.1}" r="7" fill="none" stroke="#ffd700" stroke-width="2.5"/>"##
    );
    let _ = write!(
        svg,
        r##"<text x="{:.0}" y="{:.0}" fill="#ffd700" font-size="10">s2 knee {:.1}° ({:.2} bpp, s2={:.1})</text>"##,
        kx + 12.0, ky - 4.0, s2_knee.fixed_angle, s2_knee.bpp, s2_knee.quality
    );

    let ba_knee = &calibration.butteraugli;
    // Find s2 value at the ba knee bpp (interpolate on the curve)
    let s2_at_ba_knee = interpolate_curve_s2(curve, ba_knee.bpp).unwrap_or(50.0);
    let bkx = x_of(ba_knee.bpp);
    let bky = y_of(s2_at_ba_knee);
    let _ = write!(
        svg,
        r##"<circle cx="{bkx:.1}" cy="{bky:.1}" r="7" fill="none" stroke="#3498db" stroke-width="2.5"/>"##
    );
    let _ = write!(
        svg,
        r##"<text x="{:.0}" y="{:.0}" fill="#3498db" font-size="10">ba knee {:.1}° ({:.2} bpp, ba={:.2})</text>"##,
        bkx + 12.0, bky + 14.0, ba_knee.fixed_angle, ba_knee.bpp, ba_knee.quality
    );

    // Disagreement range shading
    let (dis_lo, dis_hi) = calibration.disagreement_range();
    let _ = write!(
        svg,
        r##"<rect x="{:.1}" y="{margin}" width="{:.1}" height="{plot_h}" fill="#ffd700" opacity="0.06"/>"##,
        x_of(dis_lo),
        x_of(dis_hi) - x_of(dis_lo)
    );

    // Axis labels
    let _ = write!(
        svg,
        r##"<text x="{:.0}" y="{}" fill="#ccc" text-anchor="middle" font-size="12">bpp</text>"##,
        margin + plot_w / 2.0,
        margin + plot_h + 35.0
    );
    let _ = write!(
        svg,
        r##"<text x="{}" y="{:.0}" fill="#ccc" text-anchor="middle" font-size="12" transform="rotate(-90,{},{:.0})">SSIMULACRA2</text>"##,
        margin - 40.0,
        margin + plot_h / 2.0,
        margin - 40.0,
        margin + plot_h / 2.0
    );

    // Title
    let _ = write!(
        svg,
        r##"<text x="{:.0}" y="{}" fill="#eee" text-anchor="middle" font-size="14" font-weight="bold">{title}</text>"##,
        margin + plot_w / 2.0,
        margin - 15.0
    );

    // Corner marker
    let _ = write!(
        svg,
        r##"<circle cx="{cx:.0}" cy="{cy:.0}" r="4" fill="#ff6b6b"/>"##
    );
    let _ = write!(
        svg,
        r##"<text x="{:.0}" y="{:.0}" fill="#ff6b6b" font-size="9" text-anchor="end">origin</text>"##,
        cx - 8.0, cy + 4.0
    );

    svg.push_str("</svg>");
    svg
}

/// Linearly interpolate s2 at a given bpp on the aggregate curve.
fn interpolate_curve_s2(curve: &[(f64, f64, f64)], target_bpp: f64) -> Option<f64> {
    if curve.len() < 2 {
        return None;
    }
    for w in curve.windows(2) {
        let (b0, s0, _) = w[0];
        let (b1, s1, _) = w[1];
        if target_bpp >= b0 && target_bpp <= b1 && (b1 - b0).abs() > 1e-12 {
            let t = (target_bpp - b0) / (b1 - b0);
            return Some(s0 + t * (s1 - s0));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

/// Measured defaults from corpus calibration runs (2026-02-03).
///
/// Codec: mozjpeg 4:2:0 progressive with optimized scans.
/// Quality sweep: 10–98 step 4 (23 levels per image).
/// Fixed frame: bpp_max=4.0, s2_max=100, ba_max=15.
pub mod defaults {
    use super::{
        AxisRange, FixedFrame, NormalizationContext, QualityDirection, RDCalibration, RDKnee,
    };

    /// MozJPEG 4:2:0 progressive on CID22-training (209 images, 512x512).
    ///
    /// s2 knee at 0.73 bpp (s2=65.10) -> fixed-frame angle 38.5 deg.
    /// ba knee at 0.70 bpp (ba=4.38) -> fixed-frame angle 40.7 deg.
    /// Disagreement range: 0.70-0.73 bpp (metrics nearly agree).
    #[must_use]
    pub fn mozjpeg_cid22() -> RDCalibration {
        let frame = FixedFrame::WEB;
        RDCalibration {
            frame,
            ssimulacra2: RDKnee {
                bpp: 0.7274,
                quality: 65.10,
                fixed_angle: frame.s2_angle(0.7274, 65.10),
                norm: NormalizationContext {
                    bpp_range: AxisRange::new(0.1760, 3.6274),
                    quality_range: AxisRange::new(-8.48, 87.99),
                    direction: QualityDirection::HigherIsBetter,
                },
            },
            butteraugli: RDKnee {
                bpp: 0.7048,
                quality: 4.378,
                fixed_angle: frame.ba_angle(0.7048, 4.378),
                norm: NormalizationContext {
                    bpp_range: AxisRange::new(0.1760, 3.6274),
                    quality_range: AxisRange::new(1.854, 11.663),
                    direction: QualityDirection::LowerIsBetter,
                },
            },
            corpus: "CID22-training".into(),
            codec: "mozjpeg-420-prog".into(),
            image_count: 209,
            computed_at: "2026-02-03T22:56:01Z".into(),
        }
    }

    /// MozJPEG 4:2:0 progressive on CLIC2025-training (32 images, ~2048px).
    ///
    /// s2 knee at 0.46 bpp (s2=58.95) -> fixed-frame angle 33.7 deg.
    /// ba knee at 0.39 bpp (ba=5.19) -> fixed-frame angle 36.0 deg.
    /// Disagreement range: 0.39-0.46 bpp.
    #[must_use]
    pub fn mozjpeg_clic2025() -> RDCalibration {
        let frame = FixedFrame::WEB;
        RDCalibration {
            frame,
            ssimulacra2: RDKnee {
                bpp: 0.4623,
                quality: 58.95,
                fixed_angle: frame.s2_angle(0.4623, 58.95),
                norm: NormalizationContext {
                    bpp_range: AxisRange::new(0.1194, 3.0694),
                    quality_range: AxisRange::new(-16.94, 87.63),
                    direction: QualityDirection::HigherIsBetter,
                },
            },
            butteraugli: RDKnee {
                bpp: 0.3948,
                quality: 5.192,
                fixed_angle: frame.ba_angle(0.3948, 5.192),
                norm: NormalizationContext {
                    bpp_range: AxisRange::new(0.1194, 3.0694),
                    quality_range: AxisRange::new(1.895, 13.264),
                    direction: QualityDirection::LowerIsBetter,
                },
            },
            corpus: "CLIC2025-training".into(),
            codec: "mozjpeg-420-prog".into(),
            image_count: 32,
            computed_at: "2026-02-03T23:09:01Z".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::{
        defaults, AngleBin, AxisRange, BinScheme, CodecConfig, ConfiguredParetoFront,
        ConfiguredRDPoint, CorpusAggregate, FixedFrame, NormalizationContext, ParamValue,
        QualityDirection,
    };

    fn make_test_curve() -> Vec<(f64, f64, f64)> {
        vec![
            (0.10, 25.0, 8.0),
            (0.20, 40.0, 5.5),
            (0.30, 52.0, 3.8),
            (0.50, 62.0, 2.5),
            (0.70, 70.0, 1.8),
            (1.00, 78.0, 1.2),
            (1.50, 84.0, 0.8),
            (2.00, 88.0, 0.6),
            (3.00, 92.0, 0.4),
        ]
    }

    #[test]
    fn test_fixed_frame_s2_corner() {
        let f = FixedFrame::WEB;
        // Worst corner: (bpp_max, 0) → atan2(0, 0) ≈ 0°
        assert!(f.s2_angle(4.0, 0.0).abs() < 0.01);
        // Ideal diagonal: (0, s2_max) → atan2(1*aspect, 1) ≈ 51.5°
        let ideal = f.s2_angle(0.0, 100.0);
        assert!(ideal > 50.0 && ideal < 53.0, "ideal angle: {ideal}");
        // Reference knee: (0.7274, 65.10) → exactly 45°
        assert!((f.s2_angle(0.7274, 65.10) - 45.0).abs() < 0.1);
        // No compression: (bpp_max, s2_max) → atan2(aspect, 0) = 90°
        assert!((f.s2_angle(4.0, 100.0) - 90.0).abs() < 0.01);
        // Negative s2 → negative angle (allowed)
        assert!(f.s2_angle(2.0, -10.0) < 0.0);
        // Over-budget bpp → angle > 90° (allowed)
        assert!(f.s2_angle(5.0, 50.0) > 90.0);
    }

    #[test]
    fn test_fixed_frame_ba_corner() {
        let f = FixedFrame::WEB;
        // Worst corner: (bpp_max, ba_max) → ba_norm=0, atan2(0, 0) = 0°
        assert!(f.ba_angle(4.0, 15.0).abs() < 0.01);
        // Ideal diagonal: (0, ba=0) → ba_norm=1, atan2(aspect, 1) ≈ 51.5°
        let ideal = f.ba_angle(0.0, 0.0);
        assert!(ideal > 50.0 && ideal < 53.0, "ba ideal angle: {ideal}");
    }

    #[test]
    fn test_fixed_frame_comparable() {
        let f = FixedFrame::WEB;
        // Two encodes with same q_norm*aspect/(1-bpp_norm) ratio → same angle
        // At the reference knee: ratio = 1.0 → 45°
        let a = f.s2_angle(0.7274, 65.10); // the reference knee
        assert!((a - 45.0).abs() < 0.1);
        // Same proportional tradeoff at a different scale
        // s2_norm * aspect / (1 - bpp_norm) should be the same ratio
        // At knee: 0.651 * 1.257 / 0.818 = 1.0
        // At (2.0, 50.0): 0.50 * 1.257 / 0.50 = 1.257 → angle > 45°
        let b = f.s2_angle(2.0, 50.0);
        assert!(b > 45.0, "should be above knee: {b}");
    }

    #[test]
    fn test_axis_range_normalize() {
        let r = AxisRange::new(0.0, 10.0);
        assert!((r.normalize(5.0) - 0.5).abs() < 1e-10);
    }

    #[test]
    fn test_axis_range_roundtrip() {
        let r = AxisRange::new(2.0, 8.0);
        let val = 5.5;
        assert!((r.denormalize(r.normalize(val)) - val).abs() < 1e-10);
    }

    #[test]
    fn test_quality_direction_higher_is_better() {
        let ctx = NormalizationContext {
            bpp_range: AxisRange::new(0.0, 3.0),
            quality_range: AxisRange::new(20.0, 100.0),
            direction: QualityDirection::HigherIsBetter,
        };
        assert!((ctx.normalize_quality(100.0) - 1.0).abs() < 1e-10);
        assert!(ctx.normalize_quality(20.0).abs() < 1e-10);
    }

    #[test]
    fn test_quality_direction_lower_is_better() {
        let ctx = NormalizationContext {
            bpp_range: AxisRange::new(0.0, 3.0),
            quality_range: AxisRange::new(0.5, 12.0),
            direction: QualityDirection::LowerIsBetter,
        };
        assert!((ctx.normalize_quality(0.5) - 1.0).abs() < 1e-10);
        assert!(ctx.normalize_quality(12.0).abs() < 1e-10);
    }

    #[test]
    fn test_knee_detection_s2() {
        let curve = make_test_curve();
        let agg = CorpusAggregate {
            corpus: "test".into(),
            codec: "test-codec".into(),
            curve,
            image_count: 1,
        };

        let knee = agg.ssimulacra2_knee(&FixedFrame::WEB).expect("should find knee");
        assert!(knee.bpp > 0.2, "knee bpp too low: {}", knee.bpp);
        assert!(knee.bpp < 2.0, "knee bpp too high: {}", knee.bpp);
        assert!(knee.quality > 40.0, "knee s2 too low: {}", knee.quality);
        assert!(knee.quality < 90.0, "knee s2 too high: {}", knee.quality);
        // Fixed-frame angle should be in a reasonable range
        assert!(knee.fixed_angle > 20.0, "angle too low: {}", knee.fixed_angle);
        assert!(knee.fixed_angle < 70.0, "angle too high: {}", knee.fixed_angle);
    }

    #[test]
    fn test_knee_detection_ba() {
        let curve = make_test_curve();
        let agg = CorpusAggregate {
            corpus: "test".into(),
            codec: "test-codec".into(),
            curve,
            image_count: 1,
        };

        let knee = agg.butteraugli_knee(&FixedFrame::WEB).expect("should find knee");
        assert!(knee.bpp > 0.2);
        assert!(knee.bpp < 2.0);
        assert!(knee.fixed_angle > 20.0);
        assert!(knee.fixed_angle < 70.0);
    }

    #[test]
    fn test_calibration_disagreement_range() {
        let curve = make_test_curve();
        let agg = CorpusAggregate {
            corpus: "test".into(),
            codec: "test-codec".into(),
            curve,
            image_count: 1,
        };

        let cal = agg.calibrate(&FixedFrame::WEB).expect("should calibrate");
        let (lo, hi) = cal.disagreement_range();
        assert!(lo <= hi);
        assert!(lo > 0.0);
    }

    #[test]
    fn test_defaults_knee_angles() {
        let cal = defaults::mozjpeg_cid22();
        // s2 knee should be at exactly 45° (this is the reference knee)
        assert!(
            (cal.ssimulacra2.fixed_angle - 45.0).abs() < 0.5,
            "s2 knee angle {:.1}° should be ~45°",
            cal.ssimulacra2.fixed_angle
        );
        // ba knee should be near 45° but not necessarily exact
        assert!(
            cal.butteraugli.fixed_angle > 40.0 && cal.butteraugli.fixed_angle < 55.0,
            "ba knee angle {:.1}° outside expected 40-55° range",
            cal.butteraugli.fixed_angle
        );
        // Both knees should be within 10° of each other for mozjpeg
        let diff = (cal.ssimulacra2.fixed_angle - cal.butteraugli.fixed_angle).abs();
        assert!(
            diff < 10.0,
            "knee angle difference {:.1}° too large (s2={:.1}°, ba={:.1}°)",
            diff, cal.ssimulacra2.fixed_angle, cal.butteraugli.fixed_angle
        );
    }

    #[test]
    fn test_bin_scheme_range() {
        let scheme = BinScheme::default_18();
        assert_eq!(scheme.count, 18);
        assert!((scheme.width - 5.0).abs() < 1e-10);

        let bins: Vec<AngleBin> = scheme.bins().collect();
        assert_eq!(bins.len(), 18);
        assert!((bins[0].center - 2.5).abs() < 1e-10);
        assert!((bins[17].center - 87.5).abs() < 1e-10);
    }

    #[test]
    fn test_bin_assignment() {
        let scheme = BinScheme::default_18();
        let bin = scheme.bin_for(45.0);
        assert!(bin.contains(45.0));
    }

    #[test]
    fn test_codec_config_fingerprint() {
        let config = CodecConfig::new("mozjpeg-rs", "0.5.0")
            .with_param("quality", ParamValue::Int(75))
            .with_param("trellis", ParamValue::Bool(true));
        let fp = config.fingerprint();
        assert!(fp.contains("mozjpeg-rs"));
        assert!(fp.contains("quality=75"));
    }

    #[test]
    fn test_configured_pareto_front() {
        let cal = defaults::mozjpeg_cid22();

        let points: Vec<ConfiguredRDPoint> = vec![
            ConfiguredRDPoint {
                position: cal.position(0.3, 50.0, 4.0),
                config: CodecConfig::new("test", "1.0")
                    .with_param("q", ParamValue::Int(30)),
                image: None,
                encode_time_ms: None,
                decode_time_ms: None,
            },
            ConfiguredRDPoint {
                position: cal.position(0.5, 65.0, 2.5),
                config: CodecConfig::new("test", "1.0")
                    .with_param("q", ParamValue::Int(50)),
                image: None,
                encode_time_ms: None,
                decode_time_ms: None,
            },
            ConfiguredRDPoint {
                position: cal.position(1.0, 80.0, 1.0),
                config: CodecConfig::new("test", "1.0")
                    .with_param("q", ParamValue::Int(80)),
                image: None,
                encode_time_ms: None,
                decode_time_ms: None,
            },
            ConfiguredRDPoint {
                position: cal.position(0.6, 60.0, 3.0),
                config: CodecConfig::new("test", "1.0")
                    .with_param("q", ParamValue::Int(45)),
                image: None,
                encode_time_ms: None,
                decode_time_ms: None,
            },
        ];

        let scheme = BinScheme::default_18();
        let front = ConfiguredParetoFront::compute(points, cal, scheme);

        // Dominated point should be removed
        assert_eq!(front.points.len(), 3);

        // All angles should be positive for these well-behaved test points
        for p in &front.points {
            assert!(p.position.theta_s2 > 0.0, "s2 angle: {}", p.position.theta_s2);
            assert!(p.position.theta_ba > 0.0, "ba angle: {}", p.position.theta_ba);
        }

        let best = front.best_config_for_s2(70.0).unwrap();
        assert_eq!(best.config.params.get("q"), Some(&ParamValue::Int(80)));

        let best = front.best_config_for_bpp(0.5).unwrap();
        assert_eq!(best.config.params.get("q"), Some(&ParamValue::Int(50)));
    }
}
