# codec-eval Dependency Analysis

## Direct Dependencies (14 crates)

### Metrics Crates (4)
- **butteraugli** `0.4.0` - Perceptual quality metric
- **dssim-core** `3.4.0` - Structural dissimilarity
- **ssimulacra2** `0.5.1` - SSIMULACRA2 reference impl
- **fast-ssim2** `0.6.5` - SSIMULACRA2 SIMD-accelerated

### Image Handling (2)
- **imgref** `1.12.0` - Image reference types
- **rgb** `0.8.52` - RGB/RGBA pixel types

### Utilities (5)
- **rayon** `1.11.0` - Parallel iterators
- **chrono** `0.4.43` - Date/time (for reports)
- **csv** `1.4.0` - CSV import/export
- **serde** `1.0.228` - Serialization
- **serde_json** `1.0.149` - JSON reports

### Error Handling (1)
- **thiserror** `2.0.18` - Error derive macros

### Optional (2)
- **moxcms** `0.7.11` - ICC color profiles (feature: icc)
- **jpeg-decoder** `0.3.2` - JPEG decode (feature: jpeg-decode)

## Metrics Crate Dependencies

### butteraugli v0.4.0 → 5 deps
```
├── imgref v1.12.0          (shared with codec-eval)
├── multiversion v0.8.0     (SIMD dispatch)
├── rgb v0.8.52             (shared with codec-eval)
├── simd_aligned v0.6.1     (SIMD memory alignment)
└── wide v0.7.33            (SIMD abstractions)
```

### dssim-core v3.4.0 → 4 deps
```
├── imgref v1.12.0          (shared)
├── itertools v0.14.0       (iterator utilities)
├── rayon v1.11.0           (shared)
└── rgb v0.8.52             (shared)
```

### ssimulacra2 v0.5.1 → 3 deps + 1 build
```
├── num-traits v0.2.19      (numeric traits)
├── rayon v1.11.0           (shared)
├── thiserror v2.0.18       (shared)
└── yuvxyb v0.4.2           (color space conversion)
[build] yuvxyb-math v0.1.0
```

### fast-ssim2 v0.6.5 → 7 deps + 1 build
```
├── imgref v1.12.0          (shared)
├── multiversion v0.8.0     (SIMD dispatch)
├── num-traits v0.2.19      (shared with ssimulacra2)
├── safe_unaligned_simd v0.2.4  (safe SIMD ops)
├── thiserror v2.0.18       (shared)
├── wide v1.1.1             (SIMD - newer than butteraugli's 0.7)
└── yuvxyb v0.4.2           (shared with ssimulacra2)
[build] yuvxyb-math v0.1.0
```

## Shared Dependencies (Good News!)

Most deps are shared across metrics crates, reducing total footprint:

### Core Image Types (fully shared)
- `imgref` - Used by butteraugli, dssim-core, fast-ssim2
- `rgb` - Used by butteraugli, dssim-core, codec-eval

### Parallel Processing (fully shared)
- `rayon` - Used by dssim-core, ssimulacra2, fast-ssim2, codec-eval

### Error Handling (fully shared)
- `thiserror` - Used by ssimulacra2, fast-ssim2, codec-eval

### SIMD Infrastructure (partially shared)
- `multiversion` v0.8.0 - Used by butteraugli, fast-ssim2
- `wide` - Used by butteraugli (v0.7) and fast-ssim2 (v1.1) - **2 versions!**
- `yuvxyb` v0.4.2 - Used by ssimulacra2, fast-ssim2

### Unique Dependencies per Crate
- butteraugli: `simd_aligned`
- dssim-core: `itertools`
- ssimulacra2/fast-ssim2: `num-traits`, `yuvxyb`
- fast-ssim2: `safe_unaligned_simd`

## Version Conflicts

### ⚠️ wide: 2 versions
- butteraugli uses `0.7.33`
- fast-ssim2 uses `1.1.1`

This means both versions are compiled, though they're small crates (~10KB each).

### ✅ Everything else: single version

## Total Dependency Count

Running `cargo tree | wc -l` gives rough dep count:

```bash
# All dependencies (transitive)
codec-eval: ~80 total crates

# Unique crates (deduped)
codec-eval: ~60 unique crates
```

## Compile Time Impact

From cold build (rough estimate):
- codec-eval alone: ~2-3 seconds
- With all metrics: ~5-7 seconds
- Full workspace (codec-compare): ~10-15 seconds

Fast rebuilds after changes: <1 second

## What zen* Projects Get

When a zen* project depends on codec-eval, they get:

### Always Included (default features)
- All 4 metrics crates (butteraugli, dssim-core, ssimulacra2, fast-ssim2)
- Image handling (imgref, rgb)
- Error handling (thiserror)
- Serialization (serde, serde_json)
- CSV support (csv)
- Parallel processing (rayon)
- Date/time (chrono)
- ICC color profiles (moxcms)
- JPEG decoding (jpeg-decoder)

### Optional (via features)
- Chart generation (adds no new deps - pure Rust SVG generation)
- Interpolation (adds no new deps - pure math)

## Optimization Opportunities

### 1. Remove SIMD Duplication
The `wide` version conflict (0.7 vs 1.1) is minor but could be eliminated by:
- Updating butteraugli to use wide 1.x
- OR: using fast-ssim2's SIMD abstractions in butteraugli

### 2. Feature-gate heavy deps?
Currently everything is always included. Could consider:
- Making metrics opt-in (butteraugli, dssim, ssimulacra2 features)
- But this complicates the API and our goal is simplicity

### 3. Consider no_std for core?
Most deps support no_std:
- imgref: ✅ no_std
- rgb: ✅ no_std
- dssim-core: ❌ needs std (rayon)
- butteraugli: ❌ needs std (SIMD, floats)

Not worth pursuing since metrics need std.

## Comparison with zen* Direct Dependencies

### Before (zenimage example)
```toml
[dependencies]
ssimulacra2 = "0.5"
dssim-core = "3.3"
butteraugli = "0.4"
imgref = "1.10"
rgb = "0.8"
# = 5 separate dep declarations
# = potential version conflicts
```

### After (with codec-eval)
```toml
[dependencies]
codec-eval = "0.3"
# = 1 dep declaration
# = consistent versions
# + evaluation helpers
# + error types
# + viewing conditions
```

### Net Change
- **Remove**: 3-5 direct metric dependencies
- **Add**: 1 codec-eval dependency
- **Gain**: helpers, error handling, viewing conditions
- **Total deps**: Similar (same underlying crates)
- **Compile time**: Similar or slightly better (deduplication)

## Dependency Health

All dependencies are:
- ✅ Actively maintained
- ✅ Pure Rust (no C deps)
- ✅ Well-tested
- ✅ Used in production

Metrics crates maintained by:
- butteraugli: Imazen (us)
- dssim-core: Kornel Lesiński (ImageOptim author)
- ssimulacra2: Cloudinary
- fast-ssim2: Imazen (us)

## Summary

✅ **Clean dependency tree** - Good sharing, minimal duplication  
✅ **Reasonable size** - ~60 unique crates  
✅ **Fast compilation** - 5-7 seconds cold, <1s incremental  
✅ **All pure Rust** - No C/C++ build dependencies  
✅ **Well maintained** - Active upstream projects  

⚠️ **Minor issue**: wide 0.7 vs 1.1 (both compiled)  
✅ **Easily fixed**: Update butteraugli to wide 1.x  

**For zen* projects**: Similar total deps, but better organization and no version conflicts.
