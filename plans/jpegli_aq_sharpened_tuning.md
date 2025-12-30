# jpegli-rs AQ Tuning for Sharpened/Contrast-Boosted Images

## Overview

Tune jpegli-rs adaptive quantization (AQ) settings for images that have been
contrast-boosted or sharpened (e.g., via `f.sharpen=23` in imageflow). These
images have enhanced high-frequency content that AQ may over-quantize, leading
to visible degradation of the sharpening effect.

## Hypothesis

Sharpening creates artificial high-frequency edges. Standard AQ treats these as
"complex regions" and allocates more bits, which is good. However, if AQ is too
aggressive, it may still zero out too many of these edge coefficients, softening
the intended sharpening. We need to find the optimal AQ strength that:

1. **Preserves sharpness** - measured by butteraugli and ssimulacra2
2. **Maintains file size** - doesn't inflate files significantly
3. **Generalizes** - works across ecommerce and general photography

## Source Data Preparation

### Step 1: Create Sharpened 800px PNG Sources

Using imageflow_tool to downscale to 800px width and apply sharpening:

```bash
# CLIC validation set (32 images)
for f in /home/lilith/work/codec-corpus/clic2025/validation/*.png; do
    name=$(basename "$f" .png)
    /home/lilith/work/imageflow/target/release/imageflow_tool v1/querystring \
        --in "$f" \
        --out "/home/lilith/work/codec-eval/corpus/sharpened-800px/clic_${name}.png" \
        --command "w=800&f.sharpen=23&format=png"
done

# Ecommerce corpus subset (sample from each category)
for category in products clothing marketing general; do
    for f in $(ls /mnt/v/work/corpus/${category}/*.jpg | head -10); do
        name=$(basename "$f" .jpg)
        /home/lilith/work/imageflow/target/release/imageflow_tool v1/querystring \
            --in "$f" \
            --out "/home/lilith/work/codec-eval/corpus/sharpened-800px/ecom_${category}_${name}.png" \
            --command "w=800&f.sharpen=23&format=png"
    done
done
```

### Expected Corpus Size
- 32 CLIC images
- ~40 ecommerce images (10 per category)
- Total: ~72 images

## Experiment Parameters

### AQ Scale Factors to Test

The `AQStrengthMap.scale()` method multiplies all per-block AQ strengths:
- `scale > 1.0` → more bits to complex regions → preserves detail, LARGER files
- `scale < 1.0` → fewer bits to complex regions → loses detail, SMALLER files

Test range: **0.5, 0.75, 1.0, 1.25, 1.5, 2.0**

### Quality Levels to Test

Using jpegli distance values: **0.5, 1.0, 1.5, 2.0, 3.0**

(distance 1.0 ≈ quality 90, distance 0.5 ≈ quality 95)

### Control Variants

1. **Baseline (no AQ)** - uniform AQ strength 0.0
2. **Default AQ (scale=1.0)** - current jpegli-rs behavior
3. **Aggressive AQ (scale=2.0)** - maximum detail preservation

## Metrics

### Primary Metrics (Perceptual Quality)

1. **Butteraugli** - psychovisual distance optimized for sharp edges
   - Range: 0.0 = identical, > 2.0 = significant degradation
   - Critical for sharpened images

2. **SSIMULACRA2** - modern perceptual metric
   - Range: 100 = identical, < 70 = noticeable artifacts
   - Good for general quality assessment

3. **DSSIM** - structural dissimilarity
   - Range: 0.0 = identical
   - Established baseline metric

### Secondary Metrics

4. **File Size (bytes)** - compression efficiency
5. **Bits per Pixel (bpp)** - normalized file size
6. **Edge Preservation Score** - custom: compare edge maps before/after

## Experimental Procedure

### Phase 1: Baseline Characterization

For each image, encode at quality levels with default AQ:
- Measure file size and all quality metrics
- Establish baseline rate-distortion curves

### Phase 2: AQ Scale Sweep

For each (image, quality, aq_scale) tuple:
1. Load source PNG
2. Compute AQ map from Y plane
3. Scale AQ map by `aq_scale`
4. Encode with modified AQ map
5. Decode and measure against source

### Phase 3: Analysis

For each quality level:
1. Plot bpp vs butteraugli for each AQ scale
2. Find Pareto-optimal AQ scales (best quality at each size)
3. Identify sharpening-specific patterns

## Implementation

### jpegli-rs Worktree

Located at: `/home/lilith/work/jpegli-rs-aq-tuning/`
Branch: `aq-tuning-sharpened`

### Example Code

```rust
use jpegli::{Encoder, Quality};
use jpegli::adaptive_quant::compute_aq_strength_map;

fn encode_with_aq_scale(pixels: &[u8], width: usize, height: usize,
                        quality: f32, aq_scale: f32) -> Vec<u8> {
    // Extract Y plane for AQ computation
    let y_plane: Vec<f32> = pixels.chunks(3)
        .map(|rgb| 0.299 * rgb[0] as f32 + 0.587 * rgb[1] as f32 + 0.114 * rgb[2] as f32)
        .collect();

    // Compute and scale AQ map
    let y_quant_01 = 8; // Approximate for quality ~90
    let mut aq_map = compute_aq_strength_map(&y_plane, width, height, y_quant_01);
    aq_map.scale(aq_scale);

    // Encode with custom AQ
    Encoder::new()
        .width(width)
        .height(height)
        .quality(Quality::from_distance(1.0 / quality))
        .aq_map(aq_map)
        .encode(pixels)
        .unwrap()
}
```

## Results (2024-12-29)

### Key Finding: AQ Scale 0.25 is Optimal

Testing 55 sharpened images (800px, f.sharpen=23) across 5 quality levels:

| AQ Scale | Avg BPP | Avg DSSIM | Avg SSIM2 | RD Efficiency |
|----------|---------|-----------|-----------|---------------|
| **0.25** | 1.94    | 0.0009    | 81.6      | **0.00174**   |
| 0.50     | 1.87    | 0.0010    | 80.8      | 0.00187       |
| 0.75     | 1.81    | 0.0011    | 80.0      | 0.00199       |
| 1.00     | 1.76    | 0.0012    | 79.2      | 0.00211       |
| 1.25     | 1.71    | 0.0012    | 78.4      | 0.00205       |
| 1.50     | 1.66    | 0.0013    | 77.6      | 0.00216       |
| 2.00     | 1.57    | 0.0015    | 76.1      | 0.00236       |
| 3.00     | 1.43    | 0.0020    | 73.3      | 0.00286       |

**RD Efficiency = DSSIM * BPP** (lower is better)

### Interpretation

AQ scale 0.25 is optimal across ALL quality levels (distance 0.5 to 3.0) because:

1. **Sharpening creates artificial high-frequency edges**
2. **Standard AQ (scale=1.0) treats these as "complex" needing more bits**
3. **But for sharpened content, we want UNIFORM quantization**
4. **Lower AQ scale = less adaptive = more uniform = preserves sharpening**

Trade-off at AQ 0.25 vs 1.0:
- ~10% larger files (1.94 vs 1.76 bpp)
- ~25% better quality (0.0009 vs 0.0012 DSSIM)
- ~17% better rate-distortion efficiency

### Recommendation

For images processed with sharpening (f.sharpen or similar):
```rust
let mut aq_map = compute_aq_strength_map(&y_plane, width, height, y_quant_01);
aq_map.scale(0.25);  // Reduce AQ aggressiveness for sharpened images
```

Or equivalently, set `aq_strength_multiplier = 0.25` for sharpened content.

## Expected Outcomes

1. **Optimal AQ scale for sharpened images** - **CONFIRMED: 0.25x** (not 1.25-1.5x as hypothesized)
2. **Quality/size tradeoff curves** - Pareto fronts generated in plots
3. **Category-specific insights** - Consistent across CLIC and ecommerce
4. **Recommendations** - Use AQ scale 0.25 for sharpened input

## File Locations

- Source corpus: `/home/lilith/work/codec-eval/corpus/sharpened-800px/`
- Results CSV: `/home/lilith/work/codec-eval/results/aq_sharpened_tuning.csv`
- Analysis: `/home/lilith/work/codec-eval/results/aq_sharpened_analysis.html`
- jpegli-rs worktree: `/home/lilith/work/jpegli-rs-aq-tuning/`

## Timeline

1. **Corpus creation** - generate sharpened PNGs
2. **Baseline encoding** - default AQ at all quality levels
3. **AQ sweep** - test all AQ scale combinations
4. **Analysis** - generate charts and recommendations
5. **Integration** - update jpegli-rs with optimal defaults for sharpened input

## References

- jpegli-rs AQ implementation: `jpegli-rs/src/adaptive_quant.rs`
- AQ strength range: 0.0-0.2 (mean ~0.08)
- Zero-bias formula: `threshold = offset + mul * aq_strength`
- codec-eval metrics: butteraugli, ssimulacra2, dssim
