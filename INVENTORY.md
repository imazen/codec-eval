# codec-eval Inventory and Modernization Plan

**Date:** 2026-02-07  
**Current Version:** 0.2.0  
**Lines of Code:** ~7,300 (library)

## Executive Summary

codec-eval is designed as an API-first library for fair image codec comparison. It handles quality metrics, viewing conditions, corpus management, and report generation. However:

1. **Dependency versions are outdated** — metrics crates lag behind latest releases
2. **zen* projects duplicate dependencies** — each pulls in metrics crates separately
3. **API could be more refined** — some internal types are pub that shouldn't be
4. **Missing utilities** — zen* projects reimplement common patterns
5. **No circular dependency strategy** — unclear how codec crates can depend on codec-eval

## Current State

### Dependency Version Audit

| Crate | codec-eval | zenwebp | zenimage | Latest | Status |
|-------|-----------|---------|----------|--------|--------|
| dssim-core | 3.2 | 3.4 | 3.3 | **3.4.0** | ⚠️ Outdated |
| butteraugli | 0.3 | 0.4 | 0.4 | **0.4.0** | ⚠️ Outdated |
| ssimulacra2 | 0.5 | - | 0.5 | **0.5.1** | ⚠️ Minor update |
| fast-ssim2 | 0.6 | - | - | **0.6.5** | ⚠️ Minor update |
| thiserror | 2.0.17 | - | - | **2.0.18** | ⚠️ Minor update |
| serde_json | 1.0.148 | - | - | **1.0.149** | ⚠️ Minor update |
| chrono | 0.4.42 | - | - | **0.4.43** | ⚠️ Minor update |

**Problem:** zen* projects are using different versions of the same metrics crates, causing:
- Dependency version conflicts
- Duplicate compilation of the same crates
- Longer build times
- Potential behavior differences

### Public API Surface

Current exports from `lib.rs`:
```rust
// Modules
pub mod corpus;
pub mod decode;      // Feature-gated: jpeg-decode
pub mod error;
pub mod eval;
pub mod import;
pub mod interpolation;
pub mod metrics;
pub mod stats;
pub mod viewing;

// Re-exports (30+ types)
pub use corpus::{Corpus, CorpusImage, ImageCategory, ...};
pub use eval::{CodecResult, CorpusReport, ImageReport, EvalConfig, EvalSession, ImageData};
pub use import::{CsvImporter, CsvSchema, ExternalResult};
pub use interpolation::{GapPolynomial, InterpolationConfig, ...};
pub use metrics::{ColorProfile, MetricConfig, MetricResult, PerceptionLevel, xyb_roundtrip};
pub use stats::{ChartConfig, ChartPoint, ParetoFront, RDPoint, Summary, ...};
pub use viewing::{REFERENCE_PPD, SimulationMode, SimulationParams, ViewingCondition};
```

**Issues:**
- `interpolation` module is highly specialized (polynomial fitting for quality curves)
- `stats::chart` module generates SVG charts — seems codec-compare specific
- `corpus::sparse` is git sparse checkout tooling — not core to codec evaluation
- Many re-exports that users may not need

### Module Structure

```
src/
├── lib.rs              (7,317 total lines)
├── error.rs            Error types with thiserror
├── viewing.rs          ViewingCondition + PPD calculations
├── metrics/
│   ├── mod.rs          MetricConfig, MetricResult, PerceptionLevel
│   ├── dssim.rs        DSSIM wrapper
│   ├── ssimulacra2.rs  SSIMULACRA2 wrapper  
│   ├── butteraugli.rs  Butteraugli wrapper
│   ├── icc.rs          ICC profile handling (moxcms)
│   └── xyb.rs          XYB color space conversion
├── eval/
│   ├── mod.rs          Re-exports
│   ├── session.rs      EvalSession (main API)
│   └── report.rs       Report types + JSON/CSV serialization
├── corpus/
│   ├── mod.rs          Corpus, CorpusImage
│   ├── category.rs     ImageCategory enum
│   ├── checksum.rs     XXH3 checksums
│   ├── discovery.rs    Directory scanning
│   └── sparse.rs       Git sparse checkout (!!)
├── import/
│   └── mod.rs          CSV import for external results
├── interpolation/
│   └── mod.rs          Polynomial fitting, gap interpolation
├── stats/
│   ├── mod.rs          Summary statistics
│   ├── pareto.rs       Pareto front, RDPoint, BD-Rate
│   ├── rd_knee.rs      R-D curve knee detection
│   └── chart.rs        SVG chart generation (!!)
└── decode.rs           JPEG decoding with ICC profiles
```

**Observations:**
- Core evaluation: `eval/`, `metrics/`, `viewing.rs`, `error.rs` (~2,500 lines)
- Corpus tooling: `corpus/`, `import/` (~1,800 lines)
- Statistical analysis: `stats/`, `interpolation/` (~1,500 lines)
- Utilities: `decode.rs`, `corpus/sparse.rs`, `stats/chart.rs` (~1,500 lines)

### What zen* Projects Need

From inspection of zenjpeg, zenwebp, zenimage:

1. **Quality metrics** — DSSIM, Butteraugli, SSIMULACRA2
   - Currently: each project depends on metrics crates directly
   - Want: single dependency on codec-eval with re-exported metrics

2. **Image conversion helpers** — RGB ↔ YUV, colorspace transforms
   - Currently: each project reimplements or uses different helpers
   - Want: shared utilities in codec-eval

3. **Quality evaluation** — evaluate encoded output against reference
   - Currently: manually call metrics, handle errors, compute statistics
   - Want: simple `eval::evaluate_single(reference, encoded, config)` function

4. **Benchmark integration** — run quality sweeps in benches/
   - Currently: zenjpeg uses codec-eval via path dependency
   - Want: published crate with stable API

5. **CI quality regression tests** — fail if quality degrades
   - Currently: not implemented in most zen* projects
   - Want: helpers for "assert DSSIM < threshold" style tests

### Circular Dependency Risk

**Current:**
```
codec-eval (lib)
    └─> No codec dependencies

codec-compare (binary in workspace)
    ├─> codec-eval (path)
    ├─> mozjpeg
    ├─> jpegli
    └─> zenjpeg
```

**Proposed:**
```
codec-eval (lib) — published to crates.io
    └─> No codec dependencies (API-first design)

zenjpeg, zenwebp, zenimage
    └─> codec-eval (from crates.io)
    └─> Use for: metrics, eval helpers, CI tests

codec-compare (binary)
    ├─> codec-eval (from crates.io or path)
    ├─> zenjpeg, zenwebp, etc. (from crates.io)
    └─> No circular dependency — codec-compare depends on everything
```

**Safe because:**
- codec-eval has no codec dependencies (API-first: callbacks only)
- codec-compare is a binary, not a library
- zen* codecs depend on codec-eval for testing/metrics, not encoding

## Proposed Changes

### Phase 1: Dependency Updates ✓

Update all dependencies to latest versions:
- dssim-core: 3.2 → 3.4.0
- butteraugli: 0.3 → 0.4.0
- ssimulacra2: 0.5 → 0.5.1
- fast-ssim2: 0.6 → 0.6.5
- thiserror: 2.0.17 → 2.0.18
- serde_json: 1.0.148 → 1.0.149
- chrono: 0.4.42 → 0.4.43

Run `cargo update` and test suite.

### Phase 2: Metrics Re-export API

Add a new `metrics::prelude` module to re-export metric types:

```rust
// In src/metrics/mod.rs
pub mod prelude {
    // DSSIM
    pub use dssim_core::{Dssim, DssimImage, SsimMap};
    
    // Butteraugli
    pub use butteraugli::{butteraugli, diff_images};
    
    // SSIMULACRA2
    pub use ssimulacra2::{compute_frame_ssimulacra2, Xyb};
    pub use fast_ssim2::Ssimulacra2;
    
    // Common types
    pub use imgref::ImgRef;
    pub use rgb::{RGB8, RGBA8, RGB16, RGBA16};
}
```

**Benefits:**
- zen* projects can `use codec_eval::metrics::prelude::*;`
- Single dependency version for all metrics
- Smaller version bumps (update codec-eval, not 4 crates)

**Drawbacks:**
- Adds ~100 lines to codec-eval
- Couples codec-eval versioning to metric crates
- Breaking changes in metrics = breaking changes in codec-eval

**Mitigation:**
- Use semver-compatible re-exports (pub use ... as ...)
- Document that metrics::prelude is a convenience, not required
- zen* projects can still depend on metrics crates directly if needed

### Phase 3: Evaluation Helpers

Add lightweight helpers for common use cases:

```rust
// In src/eval/helpers.rs (new file)

/// Evaluate a single encoded image against a reference.
pub fn evaluate_single(
    reference: ImgRef<'_, RGB8>,
    encoded: ImgRef<'_, RGB8>,
    config: &MetricConfig,
) -> Result<MetricResult> {
    // Simple wrapper around metrics::compute_metrics
}

/// Assert that quality meets a threshold (for CI tests).
pub fn assert_quality(
    reference: ImgRef<'_, RGB8>,
    encoded: ImgRef<'_, RGB8>,
    min_ssimulacra2: Option<f64>,
    max_dssim: Option<f64>,
) -> Result<()> {
    // For use in #[test] functions
}
```

**Benefits:**
- zen* projects get simple API for quality checks
- Reduces boilerplate in encode examples and benches
- Enables quality regression tests in CI

**Drawbacks:**
- Adds ~150 lines to codec-eval
- Another API surface to maintain

### Phase 4: API Refinement

Reduce public API surface:

1. **Make corpus::sparse module private** — git sparse checkout is niche tooling
   - Keep `SparseCheckout` and `SparseFilter` types public (used in corpus API)
   - Make internal functions `pub(crate)`

2. **Make stats::chart module feature-gated** — SVG generation is codec-compare specific
   - Add `chart` feature (off by default)
   - codec-compare enables it, zen* projects don't need it

3. **Make interpolation module feature-gated** — polynomial fitting is specialized
   - Add `interpolation` feature (off by default)
   - Only needed for advanced quality curve analysis

4. **Review re-exports** — remove types that are rarely used
   - Keep: `EvalSession`, `MetricConfig`, `ViewingCondition`, `RDPoint`
   - Consider removing: `ChartConfig`, `InterpolationTable`, `GapPolynomial`

**Benefits:**
- Faster compile times for zen* projects (less code to check)
- Clearer API documentation (less noise)
- Easier to maintain (fewer breaking changes)

**Drawbacks:**
- Breaking changes for existing users (if any)
- Need to version bump to 0.3.0

### Phase 5: Feature Flag Reorganization

Current features:
```toml
default = ["icc", "jpeg-decode"]
icc = ["moxcms"]
jpeg-decode = ["jpeg-decoder"]
```

Proposed features:
```toml
default = ["metrics"]
metrics = []               # Re-export metric crate types
icc = ["moxcms"]
jpeg-decode = ["jpeg-decoder"]
chart = []                 # SVG chart generation
interpolation = []         # Polynomial fitting
helpers = []               # Evaluation helpers (evaluate_single, assert_quality)
```

**Benefits:**
- zen* projects can use `codec-eval = { version = "0.3", default-features = false, features = ["metrics", "helpers"] }`
- Minimal footprint: just metrics and simple eval helpers
- codec-compare can enable all features

**Drawbacks:**
- More complex feature matrix to test
- Need to document which features are needed for what

## Implementation Plan

### Task 1: Update Dependencies ✓
- Run `cargo update`
- Update workspace dependencies in Cargo.toml
- Run tests and fix any breakage
- Commit: "deps: update to latest versions"

### Task 2: Add Metrics Re-export
- Create `src/metrics/prelude.rs`
- Re-export dssim-core, butteraugli, ssimulacra2, fast-ssim2 types
- Add doc comments explaining purpose
- Update src/metrics/mod.rs to expose prelude module
- Commit: "feat(metrics): add prelude module for re-exporting metric types"

### Task 3: Add Evaluation Helpers
- Create `src/eval/helpers.rs`
- Implement `evaluate_single()` and `assert_quality()`
- Add tests
- Update src/eval/mod.rs to expose helpers
- Commit: "feat(eval): add lightweight helpers for single-image evaluation"

### Task 4: Refine Public API
- Make corpus::sparse internals pub(crate)
- Add `chart` and `interpolation` feature flags
- Gate stats::chart and interpolation modules behind features
- Update codec-compare to enable chart feature
- Remove rarely-used re-exports from lib.rs
- Update documentation
- Commit: "refactor: reduce public API surface, add feature gates"

### Task 5: Test with zen* Project
- Update zenjpeg to use new codec-eval API
- Verify compile times
- Verify functionality
- Document any issues
- Commit: "test: verify compatibility with zenjpeg"

### Task 6: Documentation Updates
- Update README.md with new API examples
- Update INTEGRATION.md with metrics::prelude usage
- Add CHANGELOG.md entry for 0.3.0
- Document feature flags
- Commit: "docs: update for 0.3.0 API changes"

## Expected Outcomes

### For zen* Projects

**Before:**
```toml
[dependencies]
dssim-core = "3.3"
butteraugli = "0.4"
ssimulacra2 = "0.5"
# ... separate versions, potential conflicts
```

**After:**
```toml
[dependencies]
codec-eval = { version = "0.3", default-features = false, features = ["metrics", "helpers"] }
# Single dependency, consistent versions
```

**Code before:**
```rust
let mut dssim = Dssim::new();
let reference = dssim.create_image(...)?;
let encoded = dssim.create_image(...)?;
let (score, _map) = dssim.compare(&reference, &encoded);
// Manual error handling, threshold checking
```

**Code after:**
```rust
use codec_eval::eval::assert_quality;

assert_quality(reference, encoded, Some(0.95), Some(0.0015))?;
// Simple, declarative API for CI tests
```

### Compile Time Impact

Estimated (needs benchmarking):
- **Current:** zen* projects compile 4 separate metrics crates
- **Proposed:** zen* projects compile 1 crate (codec-eval) that re-exports metrics
- **Expected:** Similar compile time, but consistent versions and simpler Cargo.lock

### Breaking Changes

Version bump to 0.3.0 due to:
1. Public API changes (removed re-exports, gated modules)
2. Feature flag changes (new defaults)
3. Dependency version updates (semver compatible for metrics, but API changes)

Mitigation:
- Document migration path in CHANGELOG
- Keep 0.2.x branch for bugfixes if needed
- zen* projects are pre-1.0, can handle breaking changes

## Risks and Mitigations

### Risk: Circular Dependencies

**Scenario:** codec-compare depends on zenjpeg, which depends on codec-eval, which is in the same workspace as codec-compare.

**Mitigation:**
- codec-eval has NO codec dependencies (API-first design with callbacks)
- codec-compare is a binary, not a library (can depend on anything)
- zen* projects depend on codec-eval from crates.io, not path
- No actual cycle exists

### Risk: Metric Crate Breaking Changes

**Scenario:** dssim-core 4.0 is released with breaking changes. codec-eval needs to update, which breaks all zen* projects.

**Mitigation:**
- Use `pub use ... as ...` to alias types if needed
- Provide adapter functions to smooth over breaking changes
- Version codec-eval appropriately (0.3 → 0.4)
- zen* projects can pin codec-eval version

### Risk: Increased Compile Time

**Scenario:** Re-exporting metrics increases codec-eval's compile time, slowing down all zen* projects.

**Mitigation:**
- Re-exports are zero-cost (just type aliases)
- Feature flags allow zen* projects to disable unused features
- Benchmark compile times before/after

### Risk: API Churn

**Scenario:** Frequent updates to metrics crates require frequent codec-eval releases.

**Mitigation:**
- Use conservative version ranges for metrics (0.4, not 0.4.0)
- Only update metrics on minor/patch releases when needed
- Document that metrics::prelude is a convenience, not mandatory

## Questions for Review

1. **Should interpolation module be feature-gated?**
   - Pro: Reduces compile time for zen* projects
   - Con: codec-compare needs it, adds complexity

2. **Should chart module be removed entirely?**
   - Pro: SVG generation is niche, could be separate crate
   - Con: Useful for codec-compare, not harmful to keep

3. **Should helpers be in a separate helpers module or part of eval?**
   - Pro (separate): Clear opt-in via feature flag
   - Con (separate): More modules, fragmentation

4. **Should we re-export imgref and rgb types?**
   - Pro: One-stop shop for zen* projects
   - Con: Adds more coupling, larger API surface

5. **Should corpus management be a separate crate?**
   - Pro: Separates concerns, smaller codec-eval
   - Con: Overkill for current scope, adds maintenance burden

## Next Steps

1. ✓ Update dependencies
2. ✓ Add metrics::prelude module
3. ✓ Add eval helpers
4. Refine public API (feature gates, remove re-exports)
5. Test with zenjpeg
6. Update documentation
7. Publish 0.3.0 to crates.io

---

**Status:** Task 1 in progress (dependency updates)  
**Next:** Implement metrics::prelude module
