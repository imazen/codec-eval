# Codec Integration Guide

Practical guide for integrating codec-eval into your image codec project (mozjpeg-rs, jpegli-rs, libavif, etc.).

## Quick Start

Add codec-eval to your dev-dependencies:

```toml
[dev-dependencies]
codec-eval = { git = "https://github.com/imazen/codec-eval" }
```

## Wiring Up Your Codec

codec-eval uses a callback-based design. You provide encode/decode functions, and the library handles metrics, reports, and analysis.

### Basic Pattern

```rust
use codec_eval::{EvalSession, EvalConfig, ImageData, ViewingCondition};
use std::path::PathBuf;

fn setup_session() -> EvalSession {
    let config = EvalConfig::builder()
        .report_dir(PathBuf::from("./benchmark-reports"))
        .viewing(ViewingCondition::desktop())
        .quality_levels(vec![20.0, 40.0, 60.0, 80.0, 95.0])
        .build();

    let mut session = EvalSession::new(config);

    // Register your codec with encode callback
    session.add_codec(
        "my-codec",
        env!("CARGO_PKG_VERSION"),
        Box::new(|image, request| {
            // Your encoding logic here
            // image: &ImageData - the source image
            // request: &EncodeRequest - contains quality level and params

            let quality = request.quality as u8;
            let encoded_bytes = my_codec::encode(image, quality)?;
            Ok(encoded_bytes)
        }),
    );

    session
}
```

### MozJPEG Example

```rust
use codec_eval::{EvalSession, EvalConfig, ImageData, EncodeRequest, ViewingCondition};
use mozjpeg::{Compress, ColorSpace};

fn register_mozjpeg(session: &mut EvalSession) {
    session.add_codec(
        "mozjpeg",
        "4.1.1",  // or pull from mozjpeg-sys version
        Box::new(|image, request| {
            let (width, height) = image.dimensions();
            let rgb_data = image.as_rgb_slice()?;

            let mut compress = Compress::new(ColorSpace::JCS_RGB);
            compress.set_size(width, height);
            compress.set_quality(request.quality as f32);
            compress.set_mem_dest();
            compress.start_compress();

            // Write scanlines
            let row_stride = width * 3;
            for y in 0..height {
                let row_start = y * row_stride;
                let row = &rgb_data[row_start..row_start + row_stride];
                compress.write_scanlines(&[row]);
            }

            compress.finish_compress();
            Ok(compress.data_to_vec()?)
        }),
    );
}
```

### Jpegli Example

```rust
use codec_eval::{EvalSession, ImageData, EncodeRequest};
use jpegli::{Encoder, ColorType};

fn register_jpegli(session: &mut EvalSession) {
    session.add_codec(
        "jpegli",
        jpegli::version(),
        Box::new(|image, request| {
            let (width, height) = image.dimensions();
            let rgb_data = image.as_rgb_slice()?;

            let encoder = Encoder::new_mem()?;
            encoder.set_quality(request.quality as f32)?;

            let encoded = encoder.encode(
                rgb_data,
                width as u32,
                height as u32,
                ColorType::Rgb,
            )?;

            Ok(encoded)
        }),
    );
}
```

### AVIF Example

```rust
use codec_eval::{EvalSession, ImageData, EncodeRequest};
use libavif::{AvifEncoder, AvifImage};

fn register_avif(session: &mut EvalSession) {
    session.add_codec_with_params(
        "avif",
        "1.0.0",
        Box::new(|image, request| {
            let (width, height) = image.dimensions();
            let rgba_data = image.as_rgba_slice()?;

            let avif_image = AvifImage::from_rgba(
                width as u32,
                height as u32,
                rgba_data,
            )?;

            let mut encoder = AvifEncoder::new();
            // AVIF quality is 0-63, lower = better
            // Map 0-100 scale to 63-0
            let avif_quality = ((100.0 - request.quality) * 0.63) as i32;
            encoder.set_quality(avif_quality);

            // Check for speed param
            if let Some(speed) = request.params.get("speed") {
                encoder.set_speed(speed.parse().unwrap_or(6));
            }

            Ok(encoder.encode(&avif_image)?)
        }),
    );
}
```

## Running Evaluations

### Single Image

```rust
use codec_eval::{ImageData, ViewingCondition};
use imgref::ImgVec;
use rgb::RGB8;

fn evaluate_single_image() -> anyhow::Result<()> {
    let session = setup_session();

    // Load your test image
    let img: ImgVec<RGB8> = load_png("test_images/photo.png")?;
    let image_data = ImageData::from_imgvec(&img);

    // Run evaluation across all quality levels
    let report = session.evaluate_image("photo.png", image_data)?;

    // Check results
    for result in &report.results {
        println!(
            "{} q={}: {} bytes, DSSIM={:.6}",
            result.codec_id,
            result.quality,
            result.encoded_size,
            result.metrics.dssim.unwrap_or(0.0)
        );
    }

    Ok(())
}
```

### Corpus Evaluation

```rust
fn evaluate_corpus() -> anyhow::Result<()> {
    let session = setup_session();

    // Evaluate all images in a directory
    let report = session.evaluate_corpus("./test_images")?;

    // Write reports
    session.write_reports(&report)?;
    // Creates:
    //   benchmark-reports/results.csv
    //   benchmark-reports/results.json
    //   benchmark-reports/summary.json

    Ok(())
}
```

### Using Sparse Checkout for Test Corpora

Download only the images you need from large corpus repositories:

```rust
use codec_eval::corpus::sparse::{SparseCheckout, SparseFilter};

fn setup_test_corpus() -> anyhow::Result<()> {
    // Clone with only PNG photos
    let checkout = SparseCheckout::clone_shallow(
        "https://github.com/imazen/codec-corpus",
        "./codec-corpus",
        1,  // depth=1 for speed
    )?;

    // Download only what you need
    checkout.set_filters(&[
        SparseFilter::Format("png".to_string()),
        SparseFilter::Category("photos".to_string()),
    ])?;

    checkout.checkout()?;

    let status = checkout.status()?;
    println!("Downloaded {} files", status.checked_out_files);

    Ok(())
}
```

## Quality Assertions for CI

### Threshold-Based Testing

```rust
use codec_eval::metrics::PerceptionLevel;

#[test]
fn test_quality_at_q80() {
    let session = setup_session();
    let img = load_test_image();

    let report = session.evaluate_image("test.png", img).unwrap();

    for result in report.results.iter().filter(|r| r.quality == 80.0) {
        let dssim = result.metrics.dssim.unwrap();

        // Assert quality is at least "subtle" (imperceptible to most viewers)
        assert!(
            dssim < PerceptionLevel::Subtle.threshold(),
            "{} at q80 has DSSIM {:.6}, expected < {:.6}",
            result.codec_id,
            dssim,
            PerceptionLevel::Subtle.threshold()
        );
    }
}
```

### Regression Testing Against Baseline

```rust
#[test]
fn test_no_quality_regression() {
    let session = setup_session();
    let report = session.evaluate_corpus("./test_images").unwrap();

    // Load previous baseline
    let baseline: CorpusReport = serde_json::from_str(
        &std::fs::read_to_string("baseline.json").unwrap()
    ).unwrap();

    // Compare each image/quality combination
    for result in &report.results {
        if let Some(base) = baseline.find_result(&result.image_name, result.quality) {
            let dssim = result.metrics.dssim.unwrap();
            let base_dssim = base.metrics.dssim.unwrap();

            // Allow 5% regression tolerance
            let tolerance = base_dssim * 1.05;
            assert!(
                dssim <= tolerance,
                "Quality regression: {} at q{} DSSIM {:.6} > baseline {:.6}",
                result.image_name,
                result.quality,
                dssim,
                base_dssim
            );
        }
    }
}
```

### Size Regression Testing

```rust
#[test]
fn test_no_size_regression() {
    let session = setup_session();
    let report = session.evaluate_corpus("./test_images").unwrap();
    let baseline = load_baseline();

    for result in &report.results {
        if let Some(base) = baseline.find_result(&result.image_name, result.quality) {
            // Allow 2% size increase
            let max_size = (base.encoded_size as f64 * 1.02) as usize;
            assert!(
                result.encoded_size <= max_size,
                "Size regression: {} at q{} is {} bytes, baseline {} bytes",
                result.image_name,
                result.quality,
                result.encoded_size,
                base.encoded_size
            );
        }
    }
}
```

## GitHub Actions Integration

Add this to your codec's `.github/workflows/quality.yml`:

```yaml
name: Quality Benchmarks

on:
  pull_request:
  push:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Download test corpus
        run: |
          cargo run -p codec-eval-cli -- sparse clone \
            https://github.com/imazen/codec-corpus \
            ./corpus \
            --depth 1 \
            --format png \
            --category photos

      - name: Run benchmarks
        run: cargo test --release quality_benchmarks

      - name: Compare to baseline
        run: |
          cargo run -p codec-eval-cli -- stats \
            -i benchmark-reports/results.json \
            --compare baseline.json

      - name: Upload results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: benchmark-reports/

      # Optional: Update baseline on main branch
      - name: Update baseline
        if: github.ref == 'refs/heads/main'
        run: cp benchmark-reports/results.json baseline.json
```

## Interpreting Results

### DSSIM Thresholds

| DSSIM | PerceptionLevel | Meaning |
|-------|-----------------|---------|
| < 0.0003 | Imperceptible | Mathematically different, visually identical |
| < 0.0007 | Marginal | Requires careful inspection to notice |
| < 0.0015 | Subtle | Visible if you know where to look |
| < 0.003 | Noticeable | Casual viewers may notice |
| >= 0.003 | Degraded | Obviously degraded |

### Quality Targets by Use Case

| Use Case | Target DSSIM | Typical Quality Setting |
|----------|--------------|------------------------|
| Archival | < 0.0003 | q95+ |
| Photography site | < 0.001 | q85-92 |
| General web | < 0.002 | q75-85 |
| Thumbnails | < 0.01 | q60-75 |
| Aggressive compression | < 0.05 | q40-60 |

### Viewing Conditions

Choose based on your target audience:

```rust
// Desktop/laptop users at normal distance
ViewingCondition::desktop()  // 40 PPD

// Mobile-first or retina displays
ViewingCondition::smartphone()  // 90 PPD

// Mixed audience (conservative)
ViewingCondition::laptop()  // 60 PPD

// Custom: high-end photo site on retina
ViewingCondition::new(60.0)
    .with_browser_dppx(2.0)
    .with_image_intrinsic_dppx(2.0)
```

Higher PPD = more demanding quality threshold. Mobile users on retina displays will notice artifacts that desktop users miss.

## Pareto Analysis

Find the best codec at each quality/size tradeoff:

```rust
use codec_eval::stats::{ParetoFront, RDPoint};

fn analyze_pareto() -> anyhow::Result<()> {
    let report = load_benchmark_results()?;

    // Convert to rate-distortion points
    let points: Vec<RDPoint> = report.results.iter().map(|r| {
        RDPoint {
            codec: r.codec_id.clone(),
            bpp: r.bits_per_pixel(),
            quality: 1.0 - r.metrics.dssim.unwrap(),  // Convert to quality (higher = better)
            quality_metric: "dssim".to_string(),
        }
    }).collect();

    let front = ParetoFront::compute(&points);

    println!("Pareto-optimal points:");
    for point in front.points() {
        println!("  {} @ {:.3} bpp: quality {:.4}",
            point.codec, point.bpp, point.quality);
    }

    // Find best codec at specific bit rate
    if let Some(best) = front.best_at_bpp(0.5) {
        println!("Best codec at 0.5 bpp: {}", best.codec);
    }

    Ok(())
}
```

## CLI Usage

The codec-eval CLI provides quick benchmarking without writing code:

```bash
# Discover images in a directory
codec-eval corpus discover ./test_images -o corpus.json

# Import results from another tool's CSV
codec-eval import -i results.csv -o results.json

# Calculate Pareto front
codec-eval pareto -i results.json -o pareto.json --metric dssim

# Show statistics
codec-eval stats -i results.json --by-codec

# Sparse checkout of test corpus
codec-eval sparse clone https://github.com/imazen/codec-corpus ./corpus \
    --depth 1 --format png --category photos
```

## Recommended Test Corpora

| Corpus | Images | Best For | Sparse Checkout |
|--------|--------|----------|-----------------|
| Kodak | 24 | Quick validation | `--category kodak` |
| CLIC 2024 | 62 | High-res photos | `--category clic` |
| Tecnick | 100 | Diverse content | `--category tecnick` |
| CID22 | 250 | Research validation | Full checkout recommended |

## Troubleshooting

### "Dimension mismatch" errors

Ensure your decode function returns the same dimensions as input:

```rust
// Bad: decoder may return different size
let decoded = my_codec::decode(&encoded)?;

// Good: verify dimensions match
let decoded = my_codec::decode(&encoded)?;
assert_eq!(decoded.width(), original.width());
assert_eq!(decoded.height(), original.height());
```

### Inconsistent DSSIM results

DSSIM is sensitive to color space. Ensure you're using the same color space throughout:

```rust
// Ensure sRGB throughout the pipeline
let img = image::open(path)?.to_rgb8();
```

### High DSSIM on certain images

Some image types naturally compress worse:
- Fine text/diagrams: Use higher quality or lossless
- Film grain: Consider denoising before compression
- Gradients: Check for banding artifacts

## Contributing Improvements

We welcome contributions from codec developers! See [CONTRIBUTING.md](CONTRIBUTING.md) for:

- Adding new metrics
- Improving viewing condition models
- Adding codec-specific optimizations
- Sharing benchmark results
