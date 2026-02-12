use std::time::Instant;

use anyhow::Result;
use imgref::{ImgRef, ImgVec};

use crate::source::{Rgb8, SourceImage};

/// Generic codec abstraction — encode/decode callbacks + description.
///
/// The caller provides format-specific encode/decode logic (JPEG, AVIF, etc.)
/// and eval.rs handles metrics, timing, and aggregation.
pub struct Codec {
    /// Encode an RGB8 image at the given quality level (0-100).
    pub encode: Box<dyn Fn(ImgRef<'_, Rgb8>, u8) -> Result<Vec<u8>>>,
    /// Decode compressed bytes back to RGB8.
    pub decode: Box<dyn Fn(&[u8]) -> Result<ImgVec<Rgb8>>>,
    /// Human-readable summary of the codec configuration.
    pub summary: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EvalPoint {
    pub image: String,
    pub quality: u8,
    pub bpp: f64,
    pub ssim2: f64,
    pub size_bytes: usize,
    pub encode_ms: u64,
}

pub struct EvalResult {
    pub config_summary: String,
    pub points: Vec<EvalPoint>,
    pub total_ms: u64,
}

/// Convert `ImgRef<Rgb<u8>>` to packed RGB bytes for GPU path.
#[cfg(feature = "gpu")]
fn to_packed_rgb(img: ImgRef<'_, Rgb8>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(img.width() * img.height() * 3);
    for p in img.pixels() {
        bytes.push(p.r);
        bytes.push(p.g);
        bytes.push(p.b);
    }
    bytes
}

/// Convert `ImgRef<Rgb<u8>>` to `ImgVec<[u8; 3]>` for fast-ssim2's imgref API.
fn to_u8x3(img: ImgRef<'_, Rgb8>) -> ImgVec<[u8; 3]> {
    let pixels: Vec<[u8; 3]> = img.pixels().map(|p| [p.r, p.g, p.b]).collect();
    ImgVec::new(pixels, img.width(), img.height())
}

/// SSIM2 backend abstraction — GPU or CPU.
enum Ssim2Backend {
    #[cfg(feature = "gpu")]
    Gpu(Box<crate::gpu::GpuSsim2>),
    Cpu,
}

impl Ssim2Backend {
    /// Compute SSIM2 for a single image pair.
    ///
    /// For the CPU path, `source_ref` is used to build a `Ssimulacra2Reference`.
    /// For the GPU path, packed RGB bytes are uploaded directly.
    #[cfg_attr(not(feature = "gpu"), allow(unused_variables))]
    fn compare_with_precomputed(
        &mut self,
        source_ref: ImgRef<'_, Rgb8>,
        decoded_ref: ImgRef<'_, Rgb8>,
        cpu_reference: Option<&fast_ssim2::Ssimulacra2Reference>,
        image_name: &str,
        quality: u8,
    ) -> Result<f64> {
        match self {
            #[cfg(feature = "gpu")]
            Ssim2Backend::Gpu(gpu) => {
                let source_bytes = to_packed_rgb(source_ref);
                let decoded_bytes = to_packed_rgb(decoded_ref);
                gpu.compute(&source_bytes, &decoded_bytes)
            }
            Ssim2Backend::Cpu => {
                let decoded_arr = to_u8x3(decoded_ref);
                cpu_reference
                    .expect("CPU reference must be precomputed")
                    .compare(decoded_arr.as_ref())
                    .map_err(|e| anyhow::anyhow!("SSIM2 error for {image_name} q{quality}: {e}"))
            }
        }
    }
}

pub fn run_eval(
    images: &[SourceImage],
    codec: &Codec,
    quality_levels: &[u8],
    use_gpu: bool,
) -> Result<EvalResult> {
    let config_summary = codec.summary.clone();
    let total_start = Instant::now();
    let mut points = Vec::new();

    let mut backend = if use_gpu {
        #[cfg(feature = "gpu")]
        {
            // All CID22-512 images are 512x512. Use first image dimensions.
            let (w, h) = if let Some(img) = images.first() {
                (img.width as u32, img.height as u32)
            } else {
                return Ok(EvalResult {
                    config_summary,
                    points,
                    total_ms: 0,
                });
            };
            eprintln!("Initializing GPU SSIM2 ({w}x{h})...");
            crate::gpu::init_cuda()?;
            let gpu = crate::gpu::GpuSsim2::new(w, h)?;
            eprintln!("GPU SSIM2 ready");
            Ssim2Backend::Gpu(Box::new(gpu))
        }
        #[cfg(not(feature = "gpu"))]
        {
            anyhow::bail!(
                "GPU support requires the 'gpu' feature. Build with: cargo build --features gpu"
            );
        }
    } else {
        Ssim2Backend::Cpu
    };

    for image in images {
        let source_ref = image.pixels.as_ref();
        let total_pixels = (image.width * image.height) as f64;

        // Precompute CPU SSIM2 reference (only needed for CPU path)
        let cpu_reference = match &backend {
            Ssim2Backend::Cpu => {
                let source_arr = to_u8x3(source_ref);
                Some(
                    fast_ssim2::Ssimulacra2Reference::new(source_arr.as_ref()).map_err(|e| {
                        anyhow::anyhow!("SSIM2 reference error for {}: {e}", image.name)
                    })?,
                )
            }
            #[cfg(feature = "gpu")]
            Ssim2Backend::Gpu(_) => None,
        };

        for &quality in quality_levels {
            // Encode
            let enc_start = Instant::now();
            let encoded = (codec.encode)(source_ref, quality)
                .map_err(|e| anyhow::anyhow!("Encode error for {} q{quality}: {e}", image.name))?;
            let encode_ms = enc_start.elapsed().as_millis() as u64;

            let size_bytes = encoded.len();
            let bpp = (size_bytes as f64 * 8.0) / total_pixels;

            // Decode
            let decoded_rgb = (codec.decode)(&encoded)
                .map_err(|e| anyhow::anyhow!("Decode error for {} q{quality}: {e}", image.name))?;

            // SSIM2
            let ssim2 = backend.compare_with_precomputed(
                source_ref,
                decoded_rgb.as_ref(),
                cpu_reference.as_ref(),
                &image.name,
                quality,
            )?;

            points.push(EvalPoint {
                image: image.name.clone(),
                quality,
                bpp,
                ssim2,
                size_bytes,
                encode_ms,
            });
        }
    }

    let total_ms = total_start.elapsed().as_millis() as u64;

    Ok(EvalResult {
        config_summary,
        points,
        total_ms,
    })
}
