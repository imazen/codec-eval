# Contributing to codec-eval

This project is designed to be **community-driven**. Codec developers (mozjpeg, jpegli, libavif, webp, etc.) are the experts on their domains—we want your input to make this tool actually useful.

## How to Contribute

### 1. Fork and Experiment

```bash
git clone https://github.com/imazen/codec-eval
cd codec-eval
cargo test
```

Make changes, break things, try ideas. No permission needed.

### 2. Share What You Learn

Found that a metric doesn't work well for your codec? Discovered a better viewing condition model? **Open an issue or PR.**

Even rough findings are valuable:
- "DSSIM underestimates quality loss on gradients"
- "Our encoder benefits from these specific test images"
- "Here's how we wire up our encode callback"

### 3. Add Your Codec's Perspective

Each codec has unique characteristics. Help us capture them:

```rust
// Example: Add codec-specific quality mapping
impl QualityMapping for MyCodec {
    fn quality_to_internal(&self, q: f64) -> u32 {
        // Your codec's quality scale is 0-63, inverted
        ((100.0 - q) * 0.63) as u32
    }
}
```

## What We Need

### From Codec Developers

| Contribution | Why It Matters |
|--------------|----------------|
| **Integration examples** | Show others how to wire up callbacks correctly |
| **Quality scale documentation** | "q80 in our codec means X" |
| **Known edge cases** | Images that your codec handles differently |
| **Benchmark results** | Real-world data on your corpus |
| **Bug reports** | Metrics that don't match your expectations |

### From Metrics Researchers

| Contribution | Why It Matters |
|--------------|----------------|
| **New metrics** | SSIMULACRA2, Butteraugli, VMAF integration |
| **Calibration data** | Human study correlations |
| **Viewing condition models** | Better PPD calculations |
| **Perception thresholds** | When do artifacts become visible? |

### From Anyone

| Contribution | Why It Matters |
|--------------|----------------|
| **Documentation fixes** | Typos, unclear explanations |
| **Test images** | Diverse, well-licensed corpus additions |
| **CI improvements** | Faster, more reliable testing |
| **API ergonomics** | Make it easier to use |

## Contribution Ideas

### Easy (Good First Issues)

- [ ] Add doc examples to public functions
- [ ] Fix clippy warnings
- [ ] Add tests for edge cases
- [ ] Improve error messages

### Medium

- [ ] Add SSIMULACRA2 integration (crate exists, needs wiring)
- [ ] Add Butteraugli metric
- [ ] Create example integrations for popular codecs
- [ ] Add CSV export with customizable columns
- [ ] Implement BD-Rate calculation display in CLI

### Advanced

- [ ] Docker-based codec runners for reproducibility
- [ ] TUI dashboard with ratatui
- [ ] Parallel corpus evaluation with rayon
- [ ] Quality interpolation (find quality setting for target DSSIM)
- [ ] Cross-codec quality calibration

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy --all-targets` and fix warnings
- Add tests for new functionality
- Document public APIs with examples

## Pull Request Process

1. **Fork** the repository
2. **Create a branch** for your feature (`git checkout -b feature/my-improvement`)
3. **Make changes** with tests
4. **Run checks**: `cargo fmt && cargo clippy && cargo test`
5. **Push** and open a PR

### PR Guidelines

- **Small PRs** are easier to review (< 500 lines ideal)
- **Describe the why**, not just the what
- **Link related issues** if applicable
- **Include test output** for benchmark changes

## Sharing Benchmark Results

If you've run benchmarks with this tool, consider sharing:

1. **Raw results** (JSON/CSV) - helps validate metrics
2. **Pareto analysis** - which codecs win at which tradeoffs
3. **Surprising findings** - things that don't match expectations

You can:
- Open an issue with findings
- Add to the `benchmarks/` directory via PR
- Link to your own published results

## Codec-Specific Modules

Want to add first-class support for your codec? Create a module:

```
src/codecs/
├── mod.rs
├── mozjpeg.rs    # MozJPEG-specific helpers
├── jpegli.rs     # Jpegli-specific helpers
└── avif.rs       # AVIF-specific helpers
```

Each module can provide:
- Quality scale mapping
- Recommended settings
- Known limitations
- Optimal test images

## Questions?

- **Issues**: Bug reports, feature requests, questions
- **Discussions**: Ideas, show-and-tell, general chat

## License

Contributions are licensed under the same terms as the project (see LICENSE).

By submitting a PR, you agree to license your contribution under these terms.

---

## Why Contribute Here?

### For Codec Developers

- **Better benchmarks** for your codec
- **Fair comparisons** against competitors
- **Shared tooling** instead of each project building their own
- **Visibility** when others use this tool

### For the Community

- **Standardized methodology** across projects
- **Reproducible results** anyone can verify
- **Living documentation** of best practices
- **Collective knowledge** from domain experts

---

*This project exists because codec comparison is hard, and doing it well requires expertise from many domains. Your contribution—however small—makes it better for everyone.*
