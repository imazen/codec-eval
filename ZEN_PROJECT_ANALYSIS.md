# zen* Projects - codec-eval Usage Analysis

## Summary

Analyzed 9 zen* projects to understand current metric usage and opportunities for codec-eval adoption.

## Projects Overview

| Project | Metrics Used | codec-eval | Notes |
|---------|--------------|------------|-------|
| **zenjpeg** | dssim 3.2, butteraugli 0.3, codec-eval (path) | ✅ Dev only | Heavy test usage |
| **zenimage** | ssimulacra2 0.5, dssim-core 3.3, butteraugli 0.4 | ❌ | Uses fast-ssim2 in examples |
| **zenwebp** | butteraugli 0.4, dssim-core 3.4 | ❌ | Examples for tuning |
| **zenjpeg-dispatch** | dssim 3.2, butteraugli (path), codec-eval (path) | ✅ Dev only | CUDA metrics too |
| **zenavif** | None | ❌ | No metrics |
| **zengif** | None | ❌ | No metrics |
| **zenpnm** | None | ❌ | No metrics |
| **zencodecs** | None | ❌ | No metrics |
| **zendiff** | N/A | N/A | No Cargo.toml |

## Detailed Analysis

### zenjpeg (Primary User)

**Current dependencies:**
```toml
dssim = "3.2"
codec-eval = { path = "/home/lilith/work/codec-eval" }  # Local dev only
butteraugli = { version = "0.3", features = ["unsafe-perf"] }
```

**Usage locations:**
- 20+ test files using DSSIM
- Quality verification tests (roundtrip_quality.rs)
- C++ parity comparisons
- Codec coverage tests

**Usage pattern:**
```rust
use dssim::Dssim;
use rgb::RGBA8;

const MAX_DSSIM_Q90: f64 = 0.005;

let mut dssim = Dssim::new();
let ref_img = dssim.create_image(...)?;
let test_img = dssim.create_image(...)?;
let (score, _) = dssim.compare(&ref_img, test_img);
assert!(score < MAX_DSSIM_Q90, "Quality too low");
```

**Migration opportunity:**
```rust
use codec_eval::eval::assert_quality;

assert_quality(&reference, &encoded, None, Some(0.005))?;
// Much simpler! Handles conversions internally
```

**Issues:**
- Uses outdated dssim 3.2 (should be dssim-core 3.4)
- Uses butteraugli 0.3 (should be 0.4)
- codec-eval path dependency (should use crates.io when published)
- Lots of boilerplate for simple quality checks

**Benefits of codec-eval 0.3:**
- ✅ One dependency instead of 3
- ✅ Simple assertion API for tests
- ✅ Consistent versions
- ✅ No manual image conversion

### zenimage

**Current dependencies:**
```toml
ssimulacra2 = "0.5"
dssim-core = "3.3"
butteraugli = "0.4"
```

**Usage pattern (from example):**
```rust
use fast_ssim2::{compute_frame_ssimulacra2, ColorPrimaries, Rgb, TransferCharacteristic};
// Note: Uses fast-ssim2 directly, not ssimulacra2!
```

**Issues:**
- Depends on ssimulacra2 but examples use fast-ssim2
- Outdated dssim-core 3.3 (should be 3.4)
- Different versions than codec-eval

**Migration benefits:**
- ✅ Remove version conflicts
- ✅ Align with fast-ssim2 (already used in examples)
- ✅ Get helpers for quality checks

### zenwebp

**Current dependencies:**
```toml
butteraugli = "0.4.0"
dssim-core = "3.4"
```

**Usage locations:**
- 2 examples for parameter tuning (tune_psy_rd, tune_psy_strength)

**Usage pattern:**
```rust
use butteraugli::{butteraugli, ButteraugliParams};
use imgref::Img;
use rgb::RGB8;

let ref_img = Img::new(ref_pixels, width, height);
let test_img = Img::new(test_pixels, width, height);
let params = ButteraugliParams::default();
let result = butteraugli(ref_img.as_ref(), test_img.as_ref(), &params)?;
println!("Butteraugli: {}", result.score);
```

**Issues:**
- Up-to-date versions (good!)
- Only used in examples (not critical path)
- Boilerplate for simple comparisons

**Migration benefits:**
- ✅ Single dependency
- ✅ Could use helpers for simpler code

### zenjpeg-dispatch (Advanced User)

**Current dependencies:**
```toml
dssim = "3.2"
butteraugli = { path = "../butteraugli/butteraugli" }
codec-eval = { path = "../codec-eval" }

# GPU features
ssimulacra2-cuda = { path = "../turbo-metrics/crates/ssimulacra2-cuda", optional = true }
dssim-cuda = { path = "../turbo-metrics/crates/dssim-cuda", optional = true }
butteraugli-cuda = { path = "../turbo-metrics/crates/butteraugli-cuda", optional = true }
```

**Usage:**
- 11 examples with metrics
- CUDA-accelerated metrics (optional)
- Both CPU and GPU paths

**Issues:**
- Path dependencies everywhere
- Outdated dssim 3.2
- Complex setup

**Migration notes:**
- ⚠️ Advanced use case with CUDA
- Could still benefit from codec-eval for CPU path
- CUDA metrics are separate concern

### Non-Metric Projects

**zenavif, zengif, zenpnm, zencodecs** - No metrics dependencies

These projects don't currently use quality metrics. Could benefit from codec-eval if:
- Adding quality regression tests
- Implementing parameter tuning
- Comparing with reference codecs

## Version Conflicts Summary

Current version fragmentation across zen* projects:

| Dependency | zenjpeg | zenimage | zenwebp | zenjpeg-dispatch | codec-eval 0.3 |
|------------|---------|----------|---------|------------------|----------------|
| dssim | 3.2 | 3.3 | 3.4 | 3.2 | **3.4** |
| butteraugli | 0.3 | 0.4 | 0.4 | path | **0.4** |
| ssimulacra2 | - | 0.5 | - | - | **removed** |
| fast-ssim2 | - | (used) | - | - | **0.6.5** |

**Problems:**
- 4 different dssim versions in use
- 2 butteraugli versions
- ssimulacra2 vs fast-ssim2 confusion

**After codec-eval adoption:**
- ✅ All projects use same versions
- ✅ No version conflicts
- ✅ Single source of truth

## Migration Priority

### High Priority (Heavy Users)

**1. zenjpeg** - Immediate benefit
- 20+ test files to simplify
- Outdated dependencies
- Already uses codec-eval (path)
- **Action**: Update to codec-eval 0.3, use helpers

**2. zenimage** - Version cleanup
- Conflicting deps (ssimulacra2 vs fast-ssim2)
- Examples already use fast-ssim2
- **Action**: Switch to codec-eval, remove conflicts

### Medium Priority

**3. zenwebp** - Example improvement
- Only 2 examples use metrics
- Already has correct versions
- **Action**: Optional, could simplify examples

**4. zenjpeg-dispatch** - Partial migration
- Complex CUDA setup
- Could use codec-eval for CPU fallback
- **Action**: Update dependencies, keep CUDA separate

### Low Priority

**5. Other projects** - Future enhancement
- zenavif, zengif, zenpnm, zencodecs
- No current metric usage
- **Action**: Consider for future quality testing

## Migration Template

### Before (zenjpeg example)
```rust
// Cargo.toml
[dependencies]
dssim = "3.2"
butteraugli = { version = "0.3", features = ["unsafe-perf"] }

// test.rs
use dssim::Dssim;
use rgb::RGBA8;

let mut dssim = Dssim::new();
let ref_rgba: Vec<RGBA8> = convert_to_rgba(&reference);
let test_rgba: Vec<RGBA8> = convert_to_rgba(&encoded);
let ref_img = dssim.create_image(&ref_rgba, width, height).unwrap();
let test_img = dssim.create_image(&test_rgba, width, height).unwrap();
let (score, _map) = dssim.compare(&ref_img, test_img);
assert!(score < 0.005, "DSSIM {} exceeds threshold", score);
```

### After (with codec-eval 0.3)
```rust
// Cargo.toml
[dependencies]
codec-eval = "0.3"

// test.rs
use codec_eval::eval::assert_quality;

assert_quality(&reference, &encoded, None, Some(0.005))?;
// That's it! Handles conversions, metrics, errors
```

## Recommended Migration Steps

### Phase 1: Update zenjpeg (Test in Real World)
1. Update codec-eval path to version 0.3
2. Convert 2-3 test files to use helpers
3. Verify all tests still pass
4. Measure compile time change
5. Document any issues

### Phase 2: Update zenimage (Resolve Conflicts)
1. Add codec-eval 0.3 dependency
2. Remove ssimulacra2, dssim-core, butteraugli
3. Update examples to use codec-eval::metrics::prelude
4. Test all functionality

### Phase 3: Optional Updates
1. zenwebp: Simplify examples (optional)
2. zenjpeg-dispatch: Update CPU path
3. Other projects: Add quality testing

## Expected Impact

### Compile Time
- **Before**: Each project compiles 3-4 metric crates separately
- **After**: Single codec-eval with shared deps
- **Change**: Similar total time, but consistent across projects

### Code Reduction
- **zenjpeg tests**: ~50% less boilerplate
- **Examples**: ~30% simpler
- **Maintenance**: Single version to update

### Dependency Count
- **Before**: 3-4 direct metric deps per project
- **After**: 1 codec-eval dep
- **Savings**: ~2-3 deps per project

## Issues to Watch For

### Potential Problems
1. **API changes** - Helper API different from direct dssim
2. **Compile time** - Need to verify no regression
3. **Feature requirements** - Some projects may need chart/interpolation
4. **Path dependencies** - zenjpeg-dispatch has complex setup

### Mitigation
- Test with zenjpeg first (already uses codec-eval)
- Keep direct metric deps available via prelude
- Document migration path clearly
- Be ready to adjust helpers if needed

## Conclusion

**Strong case for migration:**
- ✅ 3 projects actively use metrics (zenjpeg, zenimage, zenwebp)
- ✅ Version conflicts exist (dssim 3.2/3.3/3.4)
- ✅ Boilerplate reduction opportunity
- ✅ Already proven in zenjpeg (path dep)

**Recommended approach:**
1. Publish codec-eval 0.3
2. Migrate zenjpeg first (already using it)
3. Observe for issues
4. Roll out to zenimage, zenwebp
5. Document lessons learned

**Next step:** Test migration with zenjpeg to validate approach.
