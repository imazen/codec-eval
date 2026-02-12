# Codec Comparison Guide

[![CI](https://github.com/imazen/codec-eval/actions/workflows/ci.yml/badge.svg)](https://github.com/imazen/codec-eval/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)

A practical guide to comparing image codecs fairly, with metrics accuracy data, viewing condition considerations, and scientific methodology.

## For Codec Developers

**Integrating your codec?** See **[INTEGRATION.md](INTEGRATION.md)** for:
- Wiring up encode/decode callbacks
- MozJPEG, Jpegli, AVIF examples
- CI quality regression testing
- Interpreting DSSIM thresholds

**Want to improve this tool?** See **[CONTRIBUTING.md](CONTRIBUTING.md)**. We actively want input from codec developers—you know your domain better than we do.

```bash
# Quick start
cargo add codec-eval --git https://github.com/imazen/codec-eval

# Or use the CLI
cargo install --git https://github.com/imazen/codec-eval codec-eval-cli
```

### The codec-eval Library

**API-first design**: You provide encode/decode callbacks, the library handles everything else.

```rust
use codec_eval::{EvalSession, EvalConfig, ViewingCondition};

let config = EvalConfig::builder()
    .report_dir("./reports")
    .viewing(ViewingCondition::desktop())
    .quality_levels(vec![60.0, 80.0, 95.0])
    .build();

let mut session = EvalSession::new(config);

session.add_codec("my-codec", "1.0", Box::new(|image, request| {
    my_codec::encode(image, request.quality)
}));

let report = session.evaluate_corpus("./test_images")?;
```

**Features:**
- DSSIM, Butteraugli, and SSIMULACRA2 metrics (PSNR for legacy comparisons)
- Viewing condition modeling (desktop, mobile, retina)
- Automatic corpus download and caching via [codec-corpus](https://crates.io/crates/codec-corpus)
- CSV import for third-party benchmark results
- Pareto front analysis and BD-Rate calculation
- JSON/CSV report generation
- Optional: SVG charts, polynomial interpolation

### codec-iter: Fast Encoder Iteration

A CLI tool for rapid encoder development feedback. Change code, run eval, see if things got better — in under a second for small image sets.

```bash
# Quick eval: 3 representative images, 3 quality levels, ~360ms
just eval

# Compare subsampling modes
just eval-sweep

# Full sweep: subsampling × XYB, side-by-side
just eval-sweep-full

# Save a baseline, then iterate against it
just eval-baseline
# ... make encoder changes ...
just eval
```

`codec-iter` uses [zencodecs](https://github.com/imazen/zencodecs) for encode/decode and [fast-ssim2](https://github.com/imazen/fast-ssim2) for SSIMULACRA2 scoring. Source images are loaded from PNG (cached as PPM for faster reloads via [zenpnm](https://github.com/imazen/zenpnm)). Image tiers are pre-selected from CID22 via [glassa](https://github.com/imazen/glassa) clustering — 3/5/15 representative images depending on `--limit`.

**Subcommands:**

- `eval` — Encode, decode, compute SSIM2. Compares against stored baseline if one exists; auto-saves on first run.
- `sweep` — Cartesian product of config options (subsampling, XYB). Shows ranked comparison table.
- `baseline save` / `baseline show` — Manage stored baselines (JSON in `./baselines/`).

**Quality presets:** `quick` (3 points: 75/85/95), `standard` (8 points: 50–95), `dense` (25 points: 50–98, step 2).

See `just --list` for all recipes, or `cargo run -p codec-iter -- --help` for full CLI options.

### Quick Quality Checks (New in 0.3)

For simple tests without full corpus evaluation:

```rust
use codec_eval::{assert_quality, metrics::prelude::*};

// Load images
let reference = ImgVec::new(/* ... */, width, height);
let encoded = ImgVec::new(/* ... */, width, height);

// Assert quality thresholds
assert_quality(&reference, &encoded,
    Some(80.0),   // min SSIMULACRA2 score
    Some(0.003)   // max DSSIM
)?;

// Or use semantic levels
assert_perception_level(&reference, &encoded,
    PerceptionLevel::Imperceptible  // DSSIM < 0.0003
)?;
```

**Unified imports** via `metrics::prelude`:
```rust
use codec_eval::metrics::prelude::*;
// Now have: Dssim, butteraugli, compute_ssimulacra2,
//           ImgRef, ImgVec, RGB8, RGBA8, etc.
```

See [INTEGRATION.md](INTEGRATION.md) for detailed examples.

---

## Quick Reference

| Metric | Correlation with Human Perception | Best For |
|--------|-----------------------------------|----------|
| PSNR | ~67% | Legacy benchmarks only |
| SSIM/DSSIM | ~82% | Quick approximation |
| Butteraugli | 80-91% | High-quality threshold (score < 1.0) |
| SSIMULACRA2 | 87-98% | **Recommended** — best overall accuracy |
| VMAF | ~90% | Video, large datasets |

---

## Fair Comparison Principles

Based on [Kornel Lesiński's guide](https://kornel.ski/en/faircomparison):

### 1. Never Convert Between Lossy Formats

```
❌ JPEG → WebP → AVIF (each conversion adds artifacts)
✓  PNG/TIFF → WebP
✓  PNG/TIFF → AVIF
```

Always start from a lossless source. Converting lossy→lossy compounds artifacts and skews results.

### 2. Standardize Encoder Settings

Don't compare `mozjpeg -quality 80` against `cjxl -quality 80` — quality scales differ between encoders.

Instead, match by:
- **File size** — encode to the same byte budget, compare quality
- **Quality metric** — encode to the same SSIMULACRA2 score, compare file size

### 3. Use Multiple Images

A single test image can favor certain codecs. Use diverse datasets:
- [Kodak](https://github.com/imazen/codec-corpus) — 24 classic benchmark images
- [CLIC 2025](https://github.com/imazen/codec-corpus) — 62 high-resolution images
- [CID22](https://github.com/imazen/codec-corpus) — 250 perceptual quality research images

These datasets are automatically downloaded and cached via the `codec-corpus` crate:
```rust
use codec_eval::corpus::Corpus;

let kodak = Corpus::get_dataset("kodak")?;
let clic = Corpus::get_dataset("clic2025/training")?;
```

### 4. Test at Multiple Quality Levels

Codec rankings change across the quality spectrum:
- **High quality** (SSIMULACRA2 > 80): Differences minimal
- **Medium quality** (60-80): Most visible differences
- **Low quality** (< 50): Edge cases, artifacts become dominant

### 5. Consider Encode/Decode Speed

A codec that's 5% smaller but 100x slower may not be practical. Report:
- Encode time (CPU seconds)
- Decode time (critical for web)
- Memory usage

---

## Quality Metrics Deep Dive

### SSIMULACRA2 (Recommended)

The current best metric for perceptual quality assessment.

| Score | Quality Level | Typical Use Case |
|-------|---------------|------------------|
| < 30 | Poor | Thumbnails, previews |
| 40-50 | Low | Aggressive compression |
| 50-70 | Medium | General web images |
| 70-80 | Good | Photography sites |
| 80-85 | Very High | Professional/archival |
| > 85 | Excellent | Near-lossless |

**Accuracy**: 87% overall, up to 98% on high-confidence comparisons.

**Tool**: [ssimulacra2_rs](https://github.com/rust-av/ssimulacra2)

```bash
ssimulacra2 original.png compressed.jpg
```

### DSSIM

Structural similarity, derived from SSIM but outputs distance (lower = better).

**Accuracy**: Validated against [TID2013](http://www.ponomarenko.info/tid2013.htm) database:
- Spearman correlation: -0.84 to -0.95 (varies by distortion type)
- Best on: Noise, compression artifacts, blur
- Weaker on: Exotic distortions, color shifts

**Tool**: [dssim](https://github.com/kornelski/dssim)

```bash
dssim original.png compressed.jpg
```

| DSSIM Score | Approximate Quality |
|-------------|---------------------|
| < 0.001 | Visually identical |
| 0.001-0.01 | Excellent |
| 0.01-0.05 | Good |
| 0.05-0.10 | Acceptable |
| > 0.10 | Noticeable artifacts |

**Note**: Values are not directly comparable between DSSIM versions. Always report version.

### Butteraugli

Google's perceptual metric, good for high-quality comparisons.

**Accuracy**: 80-91% (varies by image type).

**Best for**: Determining if compression is "transparent" (score < 1.0).

**Limitation**: Less reliable for heavily compressed images.

### VMAF

Netflix's Video Multi-Method Assessment Fusion.

**Accuracy**: ~90% for video, slightly less for still images.

**Best for**: Large-scale automated testing, video frames.

### PSNR (Avoid)

Peak Signal-to-Noise Ratio — purely mathematical, ignores perception.

**Accuracy**: ~67% — only slightly better than chance.

**Use only**: For backwards compatibility with legacy benchmarks.

---

## R-D Angle: Fixed-Frame Rate-Distortion Parameterization

A coordinate system for describing where an encode sits on the rate-distortion tradeoff. Every encode gets an angle measured from the worst corner of a fixed frame. The reference codec's knee (balanced tradeoff point) lands at exactly 45 degrees.

### The formula

```
theta = atan2(quality_norm * aspect, 1.0 - bpp_norm)
```

Where:
- `bpp_norm = bpp / bpp_max` (how much of the budget you're using)
- `quality_norm = metric_value / metric_max` (how much quality you're getting)
- `aspect` = quality-axis stretch factor, calibrated so the reference knee = 45 deg

For SSIMULACRA2 (higher is better), `quality_norm = s2 / s2_max`.

For Butteraugli (lower is better), `quality_norm = 1.0 - ba / ba_max`.

### The fixed frame (web targeting)

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

### What the angles mean

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

### Calibrated reference numbers

Computed from full corpus evaluation with mozjpeg 4:2:0 progressive, quality sweep 10-98, step 2.

**CID22-training** (209 images, 512x512):

| Metric | Knee bpp | Knee value | Angle |
|--------|----------|------------|-------|
| SSIMULACRA2 | 0.7274 | 65.10 | 45.0 deg |
| Butteraugli | 0.7048 | 4.378 | 47.2 deg |

Disagreement range: 0.70-0.73 bpp (the two metrics nearly agree on where the knee is).

**CLIC2025-training** (32 images, ~2048px):

| Metric | Knee bpp | Knee value | Angle |
|--------|----------|------------|-------|
| SSIMULACRA2 | 0.4623 | 58.95 | 40.0 deg |
| Butteraugli | 0.3948 | 5.192 | 42.4 deg |

Disagreement range: 0.39-0.46 bpp.

CLIC2025 knees are at lower angles because the larger images (~2048px vs 512px) have more pixels per bit — the curve shifts left, and the balanced point is cheaper.

### How to determine the angle for a JPEG

**Method 1: From corpus averages (fast, approximate)**

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

**Method 2: From a per-image Pareto set (precise)**

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

**Method 3: Angle of a specific encode on the per-image curve**

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

### Dual-metric comparison

Every encode has two angles: one from SSIMULACRA2 and one from Butteraugli. Comparing them reveals what kind of artifacts the codec produces:

- `theta_s2 > theta_ba`: The encode looks better structurally (SSIMULACRA2 is happy) than it does perceptually (Butteraugli sees local contrast issues). Common with aggressive chroma subsampling.
- `theta_s2 < theta_ba`: Butteraugli is more forgiving than SSIMULACRA2 at this operating point. The artifacts present are local-contrast-friendly but structurally visible.
- `theta_s2 ≈ theta_ba`: Both metrics agree on the quality level. The encode is well-balanced.

### Knee detection algorithm

The knee is found by:

1. Normalizing the corpus-aggregate R-D curve to [0, 1] on both axes (per-curve normalization, using observed min/max bpp and quality values).
2. Computing the normalized slope between adjacent points.
3. Finding the point closest to where the normalized slope crosses 1.0 (equal quality gain per bit spent).
4. Interpolating linearly between the two bracketing points for a smooth result.
5. Mapping the raw (bpp, quality) result back to the fixed-frame angle using the aspect ratio.

The per-curve normalization in step 1 is independent of the fixed frame — it uses the actual observed range of the data. The fixed frame and aspect ratio only enter in step 5, when converting the raw knee coordinates to a comparable angle.

### Source files

- `src/stats/rd_knee.rs` — All types, angle computation, knee detection, SVG plotting
- `crates/codec-compare/src/rd_calibrate.rs` — Calibration binary (corpus sweep)

---

## Viewing Conditions

### Pixels Per Degree (PPD)

The number of pixels that fit in one degree of visual field. Critical for assessing when compression artifacts become visible.

| PPD | Context | Notes |
|-----|---------|-------|
| 30 | 1080p at arm's length | Casual viewing |
| 60 | 20/20 vision threshold | Most artifacts visible |
| 80 | Average human acuity limit | Diminishing returns above this |
| 120 | 4K at close range | Overkill for most content |
| 159 | iPhone 15 Pro | "Retina" display density |

**Formula**:
```
PPD = (distance_inches × resolution_ppi × π) / (180 × viewing_distance_inches)
```

### Device Categories

| Device Type | Typical PPD | Compression Tolerance |
|-------------|-------------|----------------------|
| Desktop monitor | 40-80 | Medium quality acceptable |
| Laptop | 80-120 | Higher quality needed |
| Smartphone | 120-160 | Very high quality or artifacts visible |
| 4K TV at 3m | 30-40 | More compression acceptable |

### Practical Implications

1. **Mobile-first sites** need higher quality settings (SSIMULACRA2 > 70)
2. **Desktop sites** can use more aggressive compression (SSIMULACRA2 50-70)
3. **Thumbnails** can be heavily compressed regardless of device
4. **Hero images** on retina displays need minimal compression

---

## Scientific Methodology

### ITU-R BT.500

The international standard for subjective video/image quality assessment.

**Key elements**:
- Controlled viewing conditions (luminance, distance, display calibration)
- Non-expert viewers (15-30 recommended)
- 5-grade Mean Opinion Score (MOS):
  - 5: Excellent
  - 4: Good
  - 3: Fair
  - 2: Poor
  - 1: Bad
- Statistical analysis with confidence intervals

**When to use**: Final validation of codec choices, publishing research.

### Presentation Methods

| Method | Description | Best For |
|--------|-------------|----------|
| **DSIS** | Show reference, then test image | Impairment detection |
| **DSCQS** | Side-by-side, both unlabeled | Quality comparison |
| **2AFC** | "Which is better?" forced choice | Fine discrimination |
| **Flicker test** | Rapid A/B alternation | Detecting subtle differences |

---

## Human A/B Testing

When metrics aren't enough, subjective testing provides ground truth. But poorly designed studies produce unreliable data.

### Study Design

**Randomization**:
- Randomize presentation order (left/right, first/second)
- Randomize image order across participants
- Balance codec appearances to avoid order effects

**Blinding**:
- Participants must not know which codec produced which image
- Use neutral labels ("Image A" / "Image B")
- Don't reveal hypothesis until after data collection

**Controls**:
- Include known quality differences as sanity checks
- Add duplicate pairs to measure participant consistency
- Include "same image" pairs to detect bias

### Sample Size

| Comparison Type | Minimum N | Recommended N |
|-----------------|-----------|---------------|
| Large quality difference (obvious) | 15 | 20-30 |
| Medium difference (noticeable) | 30 | 50-80 |
| Small difference (subtle) | 80 | 150+ |

**Power analysis**: For 80% power to detect a 0.5 MOS difference with SD=1.0, you need ~64 participants per condition.

### Participant Screening

**Pre-study**:
- Visual acuity test (corrected 20/40 or better)
- Color vision screening (Ishihara plates)
- Display calibration verification

**Exclusion criteria** (define before data collection):
- Failed attention checks (> 20% incorrect on known pairs)
- Inconsistent responses (< 60% agreement on duplicate pairs)
- Response time outliers (< 200ms suggests random clicking)
- Incomplete sessions (< 80% of trials)

### Attention Checks

Embed these throughout the study:

```
Types of attention checks:
1. Obvious pairs    - Original vs heavily compressed (SSIMULACRA2 < 30)
2. Identical pairs  - Same image twice (should report "same" or 50/50 split)
3. Reversed pairs   - Same comparison shown twice, order flipped
4. Instructed response - "For this pair, select the LEFT image"
```

**Threshold**: Exclude participants who fail > 2 attention checks or > 20% of obvious pairs.

### Bias Detection & Correction

**Position bias**: Tendency to favor left/right or first/second.
- Detect: Chi-square test on position choices across all trials
- Correct: Counter-balance positions; exclude participants with > 70% same-side choices

**Fatigue effects**: Quality judgments degrade over time.
- Detect: Compare accuracy on attention checks early vs late in session
- Correct: Limit sessions to 15-20 minutes; analyze by time block

**Anchoring**: First few images bias subsequent judgments.
- Detect: Compare ratings for same image shown early vs late
- Correct: Use practice trials (discard data); randomize order

**Central tendency**: Avoiding extreme ratings.
- Detect: Histogram of ratings (should use full scale)
- Correct: Use forced choice (2AFC) instead of rating scales

### Statistical Analysis

**For rating data (MOS)**:
```
1. Calculate mean and 95% CI per condition
2. Check normality (Shapiro-Wilk) - often violated
3. Use robust methods:
   - Trimmed means (10-20% trim)
   - Bootstrap confidence intervals
   - Non-parametric tests (Wilcoxon, Kruskal-Wallis)
4. Report effect sizes (Cohen's d, or MOS difference)
```

**For forced choice (2AFC)**:
```
1. Calculate preference percentage per pair
2. Binomial test for significance (H0: 50%)
3. Apply multiple comparison correction:
   - Bonferroni (conservative)
   - Holm-Bonferroni (less conservative)
   - Benjamini-Hochberg FDR (for many comparisons)
4. Report: "Codec A preferred 67% of time (p < 0.01, N=100)"
```

**Outlier handling**:
```
1. Define criteria BEFORE analysis (pre-registration)
2. Report both with and without outlier exclusion
3. Use robust statistics that down-weight outliers
4. Never exclude based on "inconvenient" results
```

### Reporting Results

**Always include**:
- Sample size (N) and exclusion count with reasons
- Confidence intervals, not just p-values
- Effect sizes in meaningful units (MOS points, % preference)
- Individual data points or distributions (not just means)
- Attention check pass rates
- Participant demographics (if relevant to display/vision)

**Example**:
> "N=87 participants completed the study (12 excluded: 8 failed attention checks, 4 incomplete).
> Codec A was preferred over Codec B in 62% of comparisons (95% CI: 55-69%, p=0.003, binomial test).
> This corresponds to a mean quality difference of 0.4 MOS points (95% CI: 0.2-0.6)."

### Common Pitfalls

| Pitfall | Problem | Solution |
|---------|---------|----------|
| Small N | Underpowered, unreliable | Power analysis before study |
| No attention checks | Can't detect random responders | Embed 10-15% check trials |
| Post-hoc exclusion | Cherry-picking results | Pre-register exclusion criteria |
| Only reporting means | Hides variability | Show distributions + CI |
| Multiple comparisons | Inflated false positives | Apply correction (Bonferroni, FDR) |
| Unbalanced design | Confounds codec with position/order | Full counterbalancing |
| Lab-only testing | May not generalize | Include diverse participants/displays |

### Real-World Studies

**Cloudinary 2021** (1.4 million opinions):
- JPEG XL: 10-15% better than AVIF at web quality levels
- AVIF: Best for low-bandwidth scenarios
- WebP: Solid middle ground
- All modern codecs beat JPEG by 25-35%

---

## Recommended Workflow

### For Quick Comparisons

```bash
# 1. Encode to same file size
convert source.png -define webp:target-size=50000 output.webp
cjxl source.png output.jxl --target_size 50000

# 2. Measure with SSIMULACRA2
ssimulacra2 source.png output.webp
ssimulacra2 source.png output.jxl
```

### For Thorough Evaluation

1. **Gather diverse test images** from [codec-corpus](https://github.com/imazen/codec-corpus)
2. **Create quality ladder** (10 quality levels per codec)
3. **Compute metrics** for each combination
4. **Plot rate-distortion curves** (file size vs quality)
5. **Consider encode/decode speed**
6. **Validate with subjective testing** if publishing results

---

## Tools & Implementations

### SSIMULACRA2

| Implementation | Type | Install | Notes |
|----------------|------|---------|-------|
| [ssimulacra2_rs](https://crates.io/crates/ssimulacra2_rs) | CLI (Rust) | `cargo install ssimulacra2_rs` | **Recommended** |
| [ssimulacra2](https://crates.io/crates/ssimulacra2) | Library (Rust) | `cargo add ssimulacra2` | For integration |
| [ssimulacra2-cuda](https://crates.io/crates/ssimulacra2-cuda) | GPU (CUDA) | `cargo install ssimulacra2-cuda` | Fast batch processing |
| [libjxl](https://github.com/libjxl/libjxl) | CLI (C++) | Build from source | Original implementation |

```bash
# Install CLI
cargo install ssimulacra2_rs

# Usage
ssimulacra2_rs original.png compressed.jpg
# Output: 76.543210 (higher = better, scale 0-100)
```

### DSSIM

| Implementation | Type | Install | Notes |
|----------------|------|---------|-------|
| [dssim](https://github.com/kornelski/dssim) | CLI (Rust) | `cargo install dssim` | **Recommended** |

```bash
# Install
cargo install dssim

# Basic comparison (lower = better)
dssim original.png compressed.jpg
# Output: 0.02341

# Generate difference visualization
dssim -o difference.png original.png compressed.jpg
```

**Accuracy**: Validated against TID2013 database. Spearman correlation -0.84 to -0.95 depending on distortion type.

### Butteraugli

| Implementation | Type | Install | Notes |
|----------------|------|---------|-------|
| [butteraugli](https://github.com/google/butteraugli) | CLI (C++) | Build from source | Original |
| [libjxl](https://github.com/libjxl/libjxl) | CLI (C++) | Build from source | Includes butteraugli |

### VMAF

| Implementation | Type | Install | Notes |
|----------------|------|---------|-------|
| [libvmaf](https://github.com/Netflix/vmaf) | CLI + Library | Package manager or build | Official Netflix implementation |

```bash
# Ubuntu/Debian
apt install libvmaf-dev

# Usage (via ffmpeg)
ffmpeg -i original.mp4 -i compressed.mp4 -lavfi libvmaf -f null -
```

### Image Processing

| Tool | Purpose | Link |
|------|---------|------|
| [imageflow](https://github.com/imazen/imageflow) | High-performance image processing with quality calibration | Rust + C ABI |
| [libvips](https://github.com/libvips/libvips) | Fast image processing library | C + bindings |
| [sharp](https://github.com/lovell/sharp) | Node.js image processing (uses libvips) | npm |

---

## References

### Methodology
- [How to Compare Images Fairly](https://kornel.ski/en/faircomparison) — Kornel Lesiński
- [ITU-R BT.500-15](https://www.itu.int/rec/R-REC-BT.500) — Subjective quality assessment
- [The Netflix Tech Blog: VMAF](https://netflixtechblog.com/toward-a-practical-perceptual-video-quality-metric-653f208b9652)

### Studies
- [Image Codec Comparison](https://storage.googleapis.com/avif-comparison/index.html) — Google
- [Cloudinary Image Format Study](https://cloudinary.com/blog/image-format-study) — 1.4M opinions
- [Are We Compressed Yet?](https://arewecompressedyet.com/) — Video codec comparison

### Test Images
- [codec-corpus](https://github.com/imazen/codec-corpus) — Reference images for calibration

---

## Contributing

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for the full guide.

**We especially want contributions from:**
- **Codec developers** (mozjpeg, jpegli, libavif, webp, etc.) — integration examples, quality scale docs, edge cases
- **Metrics researchers** — new metrics, calibration data, perception thresholds
- **Anyone** — docs, tests, bug reports, benchmark results

This project is designed to be community-driven. Fork it, experiment, share what you learn.

---

## License

This guide is released under [CC0](https://creativecommons.org/publicdomain/zero/1.0/) — use freely without attribution.
