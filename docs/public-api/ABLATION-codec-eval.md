# codec-eval Public API Ablation Report

**Date:** 2026-06-10  
**Snapshot commit:** edd5136cd73c (main; snapshot regenerated at this commit)  
**Crate version:** 0.3.2 (pre-1.0 — B-class items are a **minor** version bump, not major)  
**Snapshot items:** 2,677 default / 3,008 all-features

---

## Grep evidence template

```
ugrep -r "SYMBOL" ~/work/ \
  --include="*.rs" \
  --exclude-dir={target,.jj,codec-eval,downloaded-crates,zen-arm-src,zenjpeg-perm-corpus,.jplag,retired}
```

All hit counts below are from a single scan run on 2026-06-10. "As of this scan."

---

## Consumer inventory

Active external consumers found (non-archived, non-retired):

| Repo | Symbols used |
|------|-------------|
| `zen/zenjpeg` (tests + examples) | `EvalConfig EvalSession ImageData ViewingCondition`, `stats::rd_knee::{BinScheme CorpusAggregate FixedFrame RDKnee}` |
| `zen/mozjpeg-rs` (examples + tests) | `EvalConfig EvalSession ImageData MetricConfig ViewingCondition`, `decode::jpeg_decode_callback` |
| `jpegbs/jpegli-rs` (tests) | `EvalConfig EvalSession ImageData ViewingCondition` |
| `codec-eval/crates/codec-compare` (workspace-excluded bin) | `Corpus CorpusReport EvalConfig EvalSession ImageData ImageReport`, `eval::session::{DecodeFn EncodeFn EncodeRequest}`, `metrics::{MetricConfig dssim ssimulacra2}`, `stats::{ParetoFront RDPoint bd_rate}`, `stats::chart::{ChartConfig ChartPoint ChartSeries generate_svg}`, `stats::rd_knee::{CorpusAggregate FixedFrame RDCalibration}`, `viewing::ViewingCondition`, `error::{Error Result}` |
| `codec-eval/crates/codec-eval-cli` (in-repo bin) | `corpus::{Corpus ImageCategory}`, `import::{CsvImporter CsvSchema ExternalResult}`, `stats::{ParetoFront RDPoint Summary}`, `corpus::sparse::{SparseCheckout SparseFilter preview_patterns}` |

Zenmetrics sweep tooling: **zero hits**. Benchmarks/justfiles across `~/work/zen`: **zero Rust hits** for `codec_eval`.

---

## Summary counts

| Category | Items | % of 2,677 default | Proposed action |
|----------|-------|--------------------|-----------------|
| **Core eval + corpus + metrics** (definitively consumed) | ~1,800 | ~67% | KEEP |
| **`metrics::prelude`** (third-party re-exports, zero external consumers) | 34 | 1.3% | **A** |
| **`viewing::presets`** (pub mod, zero external consumers) | 28 | 1.0% | **A** |
| **`corpus::compute_checksum`** (re-exported file-hash helper, zero external consumers) | 5 | 0.2% | **A** |
| **`stats::rd_knee::defaults` submod** (zero external consumers) | 6 | 0.2% | **A** |
| **`metrics::calculate_psnr`** (free fn in metrics module, zero external consumers; PSNR already covered via `MetricConfig`) | 20 | 0.7% | **A** |
| **`EvalConfigBuilder` / `CsvSchemaBuilder`** (builder structs never imported externally — accessed via `EvalConfig::builder()` / `CsvSchema::builder()`) | ~40 | 1.5% | **A** |
| **`CorpusMetadata`** (pub struct, zero external consumers) | ~15 | 0.6% | **A** |
| Subtotal flagged for **A** | ~148 | **~5.5%** | |
| Subtotal flagged for **B** | **0** | 0% | none |

Total A-class proposals: approximately 148 items (~5.5% of 2,677 default).  
Total B-class proposals: 0. Conservative default held throughout.

---

## Module tables

### `stats::rd_knee` — 1,438 items (53.7% of surface)

The dominant module by item count. Nearly all items are consumed:
- `FixedFrame BinScheme CorpusAggregate RDKnee RDCalibration` — used by both zenjpeg and codec-compare; **KEEP**
- `ConfiguredParetoFront CodecConfig ConfiguredRDPoint EncodeResult` — used by codec-compare; **KEEP**
- `NormalizationContext RDPosition QualityDirection AxisRange AngleBin DualAngleBin ParamValue plot_rd_svg` — used by codec-compare; **KEEP**

One sub-area flagged:

| Item | External consumers | Action |
|------|--------------------|--------|
| `rd_knee::defaults` submod (6 items: `mozjpeg_cid22 mozjpeg_clic2025` etc.) | 0 | **A** — `#[doc(hidden)]` |

### `metrics::prelude` — 34 items

Re-exports of `dssim-core`, `butteraugli`, `fast-ssim2`, `imgref`, `rgb` types. The stated purpose (single-dep point for zen* projects) is not currently exercised by any active consumer. No file in the org imports `codec_eval::metrics::prelude`.

| Item group | External consumers | Action |
|------------|--------------------|--------|
| `Dssim DssimImage` (dssim-core) | 0 | **A** |
| `butteraugli ButteraugliParams ButteraugliResult` | 0 | **A** |
| `compute_ssimulacra2 Ssimulacra2Config Ssimulacra2Reference` | 0 | **A** |
| `ImgRef ImgVec RGB* RGBA*` (imgref/rgb) | 0 | **A** |
| `SsimMap` (already `#[doc(hidden)]` internally) | 0 | KEEP current |

Note: the individual submodules `metrics::dssim` and `metrics::ssimulacra2` ARE used by codec-compare to call `dssim::rgb8_to_dssim_image`, `dssim::calculate_dssim`, and `ssimulacra2::calculate_ssimulacra2`. Those submodules are **KEEP**. Only the `prelude` convenience re-export layer is flagged.

### `viewing::presets` — 28 items

| Items | External consumers | Action |
|-------|--------------------|--------|
| `native_desktop native_laptop native_phone srcset_*_on_* all baseline demanding key` | 0 | **A** — `#[doc(hidden)]` on `pub mod presets` |

`ViewingCondition` itself has `.desktop() .phone()` convenience constructors used extensively — those are **KEEP** on the struct. Only the separate `presets` sub-namespace is flagged.

### `metrics::calculate_psnr` — ~20 items

A free function in `metrics/mod.rs`. PSNR is exposed via `MetricConfig { psnr: bool }` and `MetricResult { psnr: Option<f64> }` (both well-consumed). The raw `calculate_psnr(&[u8], &[u8], usize, usize)` entry point has zero external consumers.

| Item | External consumers | Action |
|------|--------------------|--------|
| `metrics::calculate_psnr` (free fn + auto-impls) | 0 | **A** |

### `corpus::compute_checksum` — 5 items

File-path checksum helper re-exported from `corpus::`. Used internally by `Corpus::compute_checksums()`. No external callers. `Corpus::compute_checksums()` is itself a legitimate public API.

| Item | External consumers | Action |
|------|--------------------|--------|
| `corpus::compute_checksum` (free fn) | 0 | **A** |

### Builder structs — `EvalConfigBuilder`, `CsvSchemaBuilder` — ~40 items

Both are `pub` structs with `pub fn` chains (builder pattern). Neither is imported directly by any external consumer — callers use `EvalConfig::builder()` and `CsvSchema::builder()` which return the builder. The builder types themselves appear in the public surface as named types, but consumers never name them in `use` statements.

| Item | External consumers | Action |
|------|--------------------|--------|
| `eval::session::EvalConfigBuilder` (struct + methods) | 0 (never named, only returned) | **A** |
| `import::CsvSchemaBuilder` (struct + methods) | 0 (never named, only returned) | **A** |

Caveat: marking the builder return type `#[doc(hidden)]` while keeping the builder methods documented on the source type is standard Rust practice and non-breaking.

### `corpus::CorpusMetadata` — ~15 items

Embedded in `Corpus { metadata: CorpusMetadata }` as a public field, but never accessed by name in any consumer. No external code constructs or pattern-matches on it.

| Item | External consumers | Action |
|------|--------------------|--------|
| `corpus::CorpusMetadata` (struct + fields) | 0 | **A** |

---

## Top-10 flagged items digest

1. `metrics::prelude` (34 items) — entire module. Zero external consumers. `pub mod prelude` → `#[doc(hidden)] pub mod prelude`.
2. `viewing::presets` (28 items) — entire submodule. Zero external consumers. `pub mod presets` → `#[doc(hidden)] pub mod presets`.
3. `EvalConfigBuilder` (~20 items) — builder struct, never named externally. Add `#[doc(hidden)]`.
4. `CsvSchemaBuilder` (~18 items) — same pattern as above.
5. `CorpusMetadata` (~15 items) — pub struct embedded in `Corpus`, zero named use.
6. `metrics::calculate_psnr` (~20 items) — free fn, zero external callers; PSNR surface already covered by `MetricConfig`/`MetricResult`.
7. `rd_knee::defaults` (6 items) — calibration constant constructors, zero external use.
8. `corpus::compute_checksum` (5 items) — file-hash helper, zero external callers.
9. `metrics::prelude::SsimMap` — already `#[doc(hidden)]` in source; confirming no change needed.
10. `viewing::presets::all` / `viewing::presets::key` — list/enum helpers only used in tests.

---

## Items confirmed KEEP (selected)

These have verified external consumers and must not be touched:

- `EvalConfig EvalSession EvalConfig::builder() ImageData CodecResult ImageReport CorpusReport` — core eval session API
- `eval::session::{EncodeFn DecodeFn EncodeRequest}` — used by codec-compare encoders
- `corpus::{Corpus CorpusImage ImageCategory}` — corpus management
- `corpus::sparse::{SparseCheckout SparseFilter SparseStatus preview_patterns}` — sparse corpus
- `metrics::{MetricConfig MetricResult PerceptionLevel ColorProfile}` — metric surface
- `metrics::dssim::{calculate_dssim rgb8_to_dssim_image rgba8_to_dssim_image}` — used by codec-compare::full_comparison
- `metrics::ssimulacra2::calculate_ssimulacra2` — used by codec-compare::full_comparison
- `metrics::butteraugli::calculate_butteraugli` — used by retired code; prelude may wrap this
- `stats::{ParetoFront RDPoint Summary bd_rate}` — stats surface
- `stats::rd_knee::{FixedFrame BinScheme CorpusAggregate RDKnee RDCalibration NormalizationContext AxisRange QualityDirection AngleBin DualAngleBin ConfiguredParetoFront ConfiguredRDPoint CodecConfig EncodeResult RDPosition ParamValue plot_rd_svg}` — rd_knee core, consumed by zenjpeg and codec-compare
- `stats::chart::{ChartConfig ChartPoint ChartSeries generate_svg}` — used by codec-compare::report
- `import::{CsvImporter CsvSchema ExternalResult}` — import surface
- `viewing::{ViewingCondition REFERENCE_PPD SimulationMode SimulationParams}` — viewing condition
- `error::{Error Result}` — error types
- `decode::{decode_jpeg_with_icc jpeg_decode_callback JpegDecodeCallback}` — used by mozjpeg-rs examples
- `interpolation::{GapPolynomial InterpolationConfig InterpolationTable fit_power_law fit_gap_polynomial compute_gap_polynomials linear_interpolate}` — feature-gated; no current external callers found but deliberately consumable
- `stats::{mean median std_dev percentile percentile_u32 trimmed_mean iqr}` — stat functions, re-exported from lib.rs

---

## Notes

- **`metrics::prelude`** re-exports types from upstream crates (`butteraugli::ButteraugliResult`, `fast_ssim2::Ssimulacra2Config`, etc.). Marking `#[doc(hidden)]` does not affect compilation — downstream code that already depends on these types directly from the upstream crates is unaffected.
- **`viewing::presets`** is used only in internal `#[cfg(test)]` blocks. `ViewingCondition::desktop()` etc. on the struct are separate and unaffected.
- **No B-class proposals** are made. All zero-hit items are docstring/visibility tweaks (A). The crate is 0.x so B would be a minor bump; the conservative threshold warrants holding off until a batch of confirmed breaks justifies a 0.4.0.
- **`rd_knee::defaults`** has zero hits but the calibration data it encodes is documentation of the reference codec knee — `#[doc(hidden)]` lets tooling keep emitting it without advertising it as stable API.
