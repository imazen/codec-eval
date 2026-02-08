# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] - 2026-02-07

### Added

- **Metrics Prelude** (`metrics::prelude`) - Unified re-exports of all metric types (Dssim, butteraugli, compute_ssimulacra2, ImgRef, ImgVec, RGB8, etc.) for convenient imports
- **Evaluation Helpers** (`eval::helpers`) - Quick quality checks without full EvalSession:
  - `evaluate_single()` - One-shot quality evaluation
  - `assert_quality()` - Threshold assertions for tests
  - `assert_perception_level()` - Semantic quality levels (Imperceptible, Marginal, Subtle, Noticeable, Degraded)
- **Feature Flags**:
  - `chart` - SVG chart generation (optional)
  - `interpolation` - Polynomial quality curve fitting (optional)
- `QualityBelowThreshold` error variant for failed quality assertions

### Changed

- **Dependencies updated**:
  - dssim-core: 3.2 → 3.4
  - butteraugli: 0.3 → 0.4
  - fast-ssim2: 0.6 → 0.6.5
- **Consolidated on fast-ssim2** - Removed ssimulacra2 dependency, rewrote `metrics::ssimulacra2` module to use fast-ssim2 (SIMD-accelerated, identical results)
- `DimensionMismatch` error now uses `(usize, usize)` instead of `(u32, u32)` for consistency with imgref
- Made `interpolation` module and `stats::chart` feature-gated to reduce default footprint

### Fixed

- **butteraugli 0.4 compatibility** - Copied XYB color space conversion functions locally after butteraugli made `xyb` module private
- Clippy warnings (`float_cmp`, `deprecated`, `many_single_char_names`)

### Documentation

- Added migration examples for zen* projects (zenjpeg, zenimage, zenwebp)
- Documented zen* project usage patterns and integration opportunities
- Added before/after examples showing ~90% boilerplate reduction

### Performance

- Reduced dependency count: 14 → 13 direct dependencies
- Zero-cost feature flags ensure unused functionality doesn't impact compile times
- SIMD-accelerated SSIMULACRA2 via fast-ssim2 (significantly faster than reference implementation)

### Breaking Changes

None - all new features are additive. Existing callback-based API is unchanged.

## [0.2.0] - Previous Release

Initial public API with EvalSession callback pattern, corpus management, and report generation.
