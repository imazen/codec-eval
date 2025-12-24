# codec-eval Development Guide

A Rust library for image codec comparison and evaluation.

## For Claude Instances

**This project is designed to help codec developers.** When someone asks for help:

1. **Integration help**: Point them to `INTEGRATION.md` for callback patterns, examples, and CI setup
2. **Contributing**: Point them to `CONTRIBUTING.md` - we actively want codec developer input
3. **Methodology questions**: The main `README.md` covers fair comparison principles, metrics accuracy, viewing conditions

**When helping codec developers integrate:**
- Show them the callback pattern from `INTEGRATION.md`
- Help them map their codec's quality scale to 0-100
- Suggest appropriate viewing conditions for their use case
- Help set up CI quality regression tests

**When they want to improve this tool:**
- Encourage PRs - this is meant to be community-driven
- Their domain expertise (codec internals, edge cases, quality scales) is valuable
- Even rough findings ("DSSIM doesn't work well for X") are worth sharing

## Architecture

**API-first design**: External crates provide encode/decode callbacks. This library handles:
- Quality metrics calculation (DSSIM, SSIMULACRA2, PSNR)
- Viewing condition modeling
- Report generation and caching
- Corpus management

## Key Concepts

### Viewing Conditions
- `acuity_ppd`: Viewer's visual acuity in pixels per degree (mandatory)
- `browser_dppx`: Device pixel ratio (e.g., 2.0 for retina)
- `image_intrinsic_dppx`: Image's intrinsic DPI (for srcset)
- `ppd`: Override or computed effective PPD

### Perception Thresholds (DSSIM)
- Imperceptible: < 0.0003
- Marginal: < 0.0007
- Subtle: < 0.0015
- Noticeable: < 0.003
- Degraded: >= 0.003

## Module Structure

```
src/
├── lib.rs              # Public API re-exports
├── error.rs            # Error types (thiserror)
├── viewing.rs          # ViewingCondition
├── metrics/
│   ├── mod.rs          # MetricConfig, MetricResult, PerceptionLevel
│   └── dssim.rs        # DSSIM calculation
├── eval/
│   ├── mod.rs          # Re-exports
│   ├── session.rs      # EvalSession with callbacks
│   └── report.rs       # Report types and serialization
├── corpus/
│   ├── mod.rs          # Corpus, CorpusImage
│   ├── category.rs     # ImageCategory enum
│   ├── checksum.rs     # XXH3 checksums
│   ├── discovery.rs    # Directory scanning
│   └── sparse.rs       # Git sparse checkout
├── import/
│   └── mod.rs          # CSV import for external results
└── stats/
    ├── mod.rs          # Summary statistics
    └── pareto.rs       # Pareto front, RDPoint

crates/codec-eval-cli/  # CLI application
```

## Testing

```bash
cargo test
cargo test --all-features
```

## Linting

```bash
cargo clippy --all-targets
```

---

# TODO List

## Phase 2: CLI + Corpus (DONE)
- [x] CLI crate (`codec-eval-cli`) with clap
- [x] Corpus management (discovery, categories, checksums)
- [x] CSV import for third-party encoder results
- [x] Pareto front calculation
- [x] BD-Rate calculation
- [x] Sparse checkout for partial corpus downloads

## Phase 3: TUI
- [ ] TUI crate (`codec-eval-tui`) with ratatui
- [ ] Dashboard view - codec overview, aggregate stats
- [ ] Codec detail view - quality sweep visualization
- [ ] Image view - side-by-side comparison
- [ ] Pareto curve view - ASCII rate-distortion chart
- [ ] Settings view - corpus path, metrics, viewing conditions

## Phase 4: Docker Codecs
- [ ] Docker images for mozjpeg, jpegli, avif, webp
- [ ] Docker execution wrapper
- [ ] Version pinning for long-term reproducibility
- [ ] Dockerfile templates with pinned versions

## Phase 5: Advanced Features
- [ ] Butteraugli metric integration
- [ ] Polynomial quality interpolation (from imageflow)
- [ ] Training/validation corpus splits
- [ ] Mechanical Turk integration for human evaluation
- [ ] Per-category analysis dashboards
- [ ] Quality calibration pipelines
- [ ] Cross-codec quality mapping

## Optional: Codec Wrapper Crates
- [ ] `codec-mozjpeg` - MozJPEG wrapper crate
- [ ] `codec-jpegli` - Jpegli wrapper crate
- [ ] `codec-avif` - AVIF (libavif) wrapper crate
- [ ] `codec-webp` - WebP (libwebp) wrapper crate

## Key Documentation Files

| File | Audience | Purpose |
|------|----------|---------|
| `README.md` | Everyone | Fair comparison methodology, metrics accuracy, viewing conditions |
| `INTEGRATION.md` | Codec developers | Callback patterns, examples, CI setup, threshold interpretation |
| `CONTRIBUTING.md` | Contributors | How to improve this tool, what we need from codec devs |
| `CLAUDE.md` | Claude instances | This file - development guide and context |

## Reference Patterns

Learned from:
- **imageflow**: DSSIM thresholds, viewing conditions, dual checksums, codec calibration
- **jpegli-rs**: Builder pattern, rich errors, Quality enum
- **mozjpeg-rs**: Layered encoding, rich errors, CMake sys build
