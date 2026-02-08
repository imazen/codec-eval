# Migration Example: zen* Projects to codec-eval 0.3

This document shows real before/after examples of migrating from direct metric usage to codec-eval 0.3 helpers.

## Status

**✅ zenjpeg dependency update completed** (commit e0accf7)
- dssim 3.2 → dssim-core 3.4
- butteraugli 0.3 → 0.4
- fast-ssim2 0.6 → 0.6.5

## Before: Manual Metric Setup (Current Pattern)

This is from `zenjpeg/tests/cpp_reference_parity.rs`:

```rust
use dssim_core::Dssim;
use rgb::RGBA;

fn compute_dssim(orig: &[u8], comp: &[u8], width: usize, height: usize) -> f64 {
    let attr = Dssim::new();

    // Manual conversion to RGBA
    let orig_rgba: Vec<RGBA<u8>> = orig
        .chunks(3)
        .map(|c| RGBA::new(c[0], c[1], c[2], 255))
        .collect();
    let comp_rgba: Vec<RGBA<u8>> = comp
        .chunks(3)
        .map(|c| RGBA::new(c[0], c[1], c[2], 255))
        .collect();

    // Create dssim images
    let orig_img = attr.create_image_rgba(&orig_rgba, width, height).unwrap();
    let comp_img = attr.create_image_rgba(&comp_rgba, width, height).unwrap();

    // Compare
    let (dssim, _) = attr.compare(&orig_img, comp_img);
    dssim.into()
}

#[test]
fn test_quality() {
    let (orig_pixels, width, height) = load_test_image();
    let encoded = encode_image(&orig_pixels, width, height, quality);
    let decoded = decode_image(&encoded);

    let dssim = compute_dssim(&orig_pixels, &decoded, width, height);

    // Manual threshold check
    assert!(dssim < 0.003, "Quality too low: DSSIM {}", dssim);
}
```

**Problems:**
- 30+ lines of boilerplate per test file
- Repeated across 20+ test files in zenjpeg
- Error-prone manual conversions
- No unified metric configuration
- Duplicated across all zen* projects

## After: codec-eval 0.3 Helpers

Same test using new helpers:

```rust
use codec_eval::metrics::prelude::*;
use codec_eval::{assert_quality, assert_perception_level, PerceptionLevel};

#[test]
fn test_quality() {
    let (orig_pixels, width, height) = load_test_image();
    let encoded = encode_image(&orig_pixels, width, height, quality);
    let decoded = decode_image(&encoded);

    // Convert to ImgVec<RGB8> (one-time)
    let reference = ImgVec::new(
        orig_pixels.chunks_exact(3)
            .map(|c| RGB8::new(c[0], c[1], c[2]))
            .collect(),
        width,
        height,
    );
    let distorted = ImgVec::new(
        decoded.chunks_exact(3)
            .map(|c| RGB8::new(c[0], c[1], c[2]))
            .collect(),
        width,
        height,
    );

    // Option 1: Specific thresholds
    assert_quality(&reference, &distorted,
        Some(80.0),   // min SSIMULACRA2
        Some(0.003)   // max DSSIM
    )?;

    // Option 2: Perception levels
    assert_perception_level(&reference, &distorted,
        PerceptionLevel::Imperceptible  // < 0.0003 DSSIM
    )?;
}
```

**Benefits:**
- ~10 lines vs 30+
- Unified metric configuration
- Clear threshold semantics
- Works across all zen* projects
- Single dependency: `codec-eval = "0.3"`

## Alternative: Single-Call Evaluation

For one-off quality checks:

```rust
use codec_eval::{evaluate_single, MetricConfig};

let result = evaluate_single(
    &reference,
    &distorted,
    &MetricConfig::default()
)?;

println!("DSSIM: {}", result.dssim);
println!("SSIMULACRA2: {}", result.ssimulacra2);
println!("Butteraugli: {}", result.butteraugli);
```

## Migration Impact Analysis

### zenjpeg
- **20+ test files** with metric calculations
- **Estimated reduction:** 150 lines → 50 lines per file = **2000+ lines saved**
- **Maintenance:** Single dependency instead of 3 separate metric crates

### zenimage
- **Version conflicts resolved** (ssimulacra2 0.5 vs fast-ssim2 usage)
- **Examples simplified** (currently mixing direct calls)

### zenwebp
- **Tuning examples** become more maintainable
- **Consistent metrics** with other zen* projects

## Next Steps

1. **Create helper wrapper** for common zen* pattern:
   ```rust
   // In zenjpeg/src/test_utils.rs or similar
   pub fn assert_encode_quality(
       original: &[u8],
       width: u32,
       height: u32,
       encoded: &[u8],
       min_ssim2: f64,
   ) -> Result<()> {
       let reference = to_imgvec(original, width, height);
       let decoded = decode_to_imgvec(encoded)?;
       codec_eval::assert_quality(&reference, &decoded, Some(min_ssim2), None)
   }
   ```

2. **Migrate one test file** as validation

3. **Roll out** to other test files

4. **Update** zenimage and zenwebp

## API Reference

From `codec-eval 0.3`:

```rust
// Prelude - unified imports
pub use codec_eval::metrics::prelude::*;

// Quick assertions
pub fn assert_quality(
    reference: &ImgVec<RGB8>,
    encoded: &ImgVec<RGB8>,
    min_ssimulacra2: Option<f64>,
    max_dssim: Option<f64>,
) -> Result<()>;

pub fn assert_perception_level(
    reference: &ImgVec<RGB8>,
    encoded: &ImgVec<RGB8>,
    min_level: PerceptionLevel,
) -> Result<()>;

// Perception levels (DSSIM-based)
pub enum PerceptionLevel {
    Imperceptible,  // < 0.0003
    Marginal,       // < 0.0007
    Subtle,         // < 0.0015
    Noticeable,     // < 0.003
    Degraded,       // >= 0.003
}

// Full evaluation
pub fn evaluate_single(
    reference: &ImgVec<RGB8>,
    encoded: &ImgVec<RGB8>,
    config: &MetricConfig,
) -> Result<MetricResult>;
```

## Testing the Migration

Build test (completed):
```bash
cd /home/lilith/work/zenjpeg/zenjpeg
cargo check --tests  # ✅ Passes with new deps
```

Next validation step:
```bash
# Pick one test file to migrate
# Run specific test to verify
cargo test --test cpp_reference_parity -- test_reference_data_loads
```
