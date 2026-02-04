# R-D Angle: Fixed-Frame Rate-Distortion Parameterization

A coordinate system for describing where an encode sits on the rate-distortion tradeoff. Every encode gets an angle measured from the worst corner of a fixed frame. The reference codec's knee (balanced tradeoff point) lands at exactly 45 degrees.

## The formula

```
theta = atan2(quality_norm * aspect, 1.0 - bpp_norm)
```

Where:
- `bpp_norm = bpp / bpp_max` (how much of the budget you're using)
- `quality_norm = metric_value / metric_max` (how much quality you're getting)
- `aspect` = quality-axis stretch factor, calibrated so the reference knee = 45 deg

For SSIMULACRA2 (higher is better), `quality_norm = s2 / s2_max`.

For Butteraugli (lower is better), `quality_norm = 1.0 - ba / ba_max`.

## The fixed frame (web targeting)

| Parameter | Value | Rationale |
|-----------|-------|-----------|
| `bpp_max` | 4.0 | Practical web ceiling. Few images exceed this. |
| `s2_max` | 100.0 | SSIMULACRA2 scale maximum. |
| `ba_max` | 15.0 | Butteraugli score where quality is essentially destroyed. |
| `aspect` | 1.2568 | Calibrated from CID22-training mozjpeg s2 knee. |

The aspect ratio is derived from the reference knee at `(0.7274 bpp, s2=65.10)`:

```
aspect = (1 - bpp_knee / bpp_max) / (s2_knee / s2_max)
       = (1 - 0.7274 / 4.0) / (65.10 / 100.0)
       = 0.81815 / 0.651
       = 1.2568
```

This stretches the quality axis so that the reference knee's quality displacement equals its bpp displacement in the angle computation, producing exactly 45 degrees.

## What the angles mean

| Angle | Meaning |
|-------|---------|
| < 0 deg | Worse than the worst corner. Negative quality (s2 below 0). |
| 0 deg | Worst corner: max bpp, zero quality. |
| ~10-25 deg | Aggressive compression. Thumbnails, heavy lossy. |
| **45 deg** | **Reference knee. Balanced tradeoff for mozjpeg/CID22.** |
| ~52 deg | Ideal diagonal (0 bpp, perfect quality). Theoretical limit. |
| 60-80 deg | Quality-dominated. Large files, diminishing quality returns. |
| 90 deg | No compression: max bpp, max quality. |
| > 90 deg | Over-budget. bpp exceeds the frame ceiling. |

The knee is where the R-D curve transitions from "every extra bit buys meaningful quality" to "you're spending bits for marginal gains." It's the point where the normalized slope of the curve equals 1.0 — equal return on both axes.

Angles below 45 deg are compression-efficient. Angles above 45 deg are quality-focused. The ideal diagonal at ~52 deg represents "perfect quality at zero cost" — unachievable, but it's the geometric ceiling for good encodes.

## Calibrated reference numbers

Computed from full corpus evaluation with mozjpeg 4:2:0 progressive, quality sweep 10-98, step 2.

### CID22-training (209 images, 512x512)

| Metric | Knee bpp | Knee value | Angle |
|--------|----------|------------|-------|
| SSIMULACRA2 | 0.7274 | 65.10 | 45.0 deg |
| Butteraugli | 0.7048 | 4.378 | 47.2 deg |

Disagreement range: 0.70-0.73 bpp (the two metrics nearly agree on where the knee is).

### CLIC2025-training (32 images, ~2048px)

| Metric | Knee bpp | Knee value | Angle |
|--------|----------|------------|-------|
| SSIMULACRA2 | 0.4623 | 58.95 | 40.0 deg |
| Butteraugli | 0.3948 | 5.192 | 42.4 deg |

Disagreement range: 0.39-0.46 bpp.

CLIC2025 knees are at lower angles because the larger images (~2048px vs 512px) have more pixels per bit — the curve shifts left, and the balanced point is cheaper.

## How to determine the angle for a JPEG

### Method 1: From corpus averages (fast, approximate)

If you know the bpp and SSIMULACRA2 score for an encode, just compute:

```rust
use codec_eval::stats::FixedFrame;

let frame = FixedFrame::WEB;
let angle = frame.s2_angle(bpp, ssimulacra2_score);
```

This tells you where the encode sits relative to the reference knee. If `angle < 45`, you're compressing harder than the balanced point. If `angle > 45`, you're spending more bits than necessary for the quality gain.

For Butteraugli:
```rust
let angle = frame.ba_angle(bpp, butteraugli_score);
```

### Method 2: From a per-image Pareto set (precise)

If you have a quality sweep for the specific image (multiple quality settings, each with bpp and metric scores), you can:

1. Build the Pareto front from the sweep data.
2. Find the knee of the per-image R-D curve using `CorpusAggregate` with a single image.
3. Compare the per-image knee angle to the corpus reference (45 deg).

```rust
use codec_eval::stats::rd_knee::{CorpusAggregate, FixedFrame};

// curve: Vec<(bpp, ssimulacra2, butteraugli)> sorted by bpp
let agg = CorpusAggregate {
    corpus: "single-image".into(),
    codec: "mozjpeg-420-prog".into(),
    curve,
    image_count: 1,
};

let frame = FixedFrame::WEB;

// Find this image's knee
if let Some(knee) = agg.ssimulacra2_knee(&frame) {
    println!("Image knee at {:.1} deg ({:.2} bpp, s2={:.1})",
        knee.fixed_angle, knee.bpp, knee.quality);

    // Compare to corpus reference (45 deg)
    if knee.fixed_angle < 40.0 {
        println!("This image compresses efficiently — knee is below average");
    } else if knee.fixed_angle > 50.0 {
        println!("This image is hard to compress — needs more bits for quality");
    }
}
```

A per-image knee at 35 deg means the image reaches diminishing returns earlier (compresses well). A knee at 55 deg means the image needs more bits to look good.

### Method 3: Angle of a specific encode on the per-image curve

Given a specific encode (one quality setting), compute its angle and compare to the image's own knee:

```rust
let frame = FixedFrame::WEB;

// The specific encode
let encode_angle = frame.s2_angle(encode_bpp, encode_s2);

// The image's knee (from a previous sweep)
let image_knee_angle = image_knee.fixed_angle;

if encode_angle < image_knee_angle {
    // Below this image's knee: compression-efficient territory
    // You could increase quality without wasting bits
} else {
    // Above this image's knee: quality-dominated territory
    // Reducing quality would save significant bits
}
```

## Dual-metric comparison

Every encode has two angles: one from SSIMULACRA2 and one from Butteraugli. Comparing them reveals what kind of artifacts the codec produces:

- `theta_s2 > theta_ba`: The encode looks better structurally (SSIMULACRA2 is happy) than it does perceptually (Butteraugli sees local contrast issues). Common with aggressive chroma subsampling.
- `theta_s2 < theta_ba`: Butteraugli is more forgiving than SSIMULACRA2 at this operating point. The artifacts present are local-contrast-friendly but structurally visible.
- `theta_s2 ≈ theta_ba`: Both metrics agree on the quality level. The encode is well-balanced.

## Knee detection algorithm

The knee is found by:

1. Normalizing the corpus-aggregate R-D curve to [0, 1] on both axes (per-curve normalization, using observed min/max bpp and quality values).
2. Computing the normalized slope between adjacent points.
3. Finding the point closest to where the normalized slope crosses 1.0 (equal quality gain per bit spent).
4. Interpolating linearly between the two bracketing points for a smooth result.
5. Mapping the raw (bpp, quality) result back to the fixed-frame angle using the aspect ratio.

The per-curve normalization in step 1 is independent of the fixed frame — it uses the actual observed range of the data. The fixed frame and aspect ratio only enter in step 5, when converting the raw knee coordinates to a comparable angle.

## Source files

- `src/stats/rd_knee.rs` — All types, angle computation, knee detection, SVG plotting
- `crates/codec-compare/src/rd_calibrate.rs` — Calibration binary (corpus sweep)
- `docs/rd-angle.md` — This document

Calibration data (CSV with per-image measurements):
- `/mnt/v/output/codec-eval/calibration/cid22-full-mozjpeg-420.csv` (9405 measurements, 209 images)
- `/mnt/v/output/codec-eval/calibration/clic2025-mozjpeg-420.csv` (1440 measurements, 32 images)
