use std::time::Instant;

use anyhow::Result;
use fast_ssim2::Ssimulacra2Reference;
use imgref::{ImgRef, ImgVec};
use zencodecs::{DecodeRequest, EncodeRequest, ImageFormat};

use crate::config::{self, JpegConfig};
use crate::source::{Rgb8, SourceImage};

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

/// Convert `ImgRef<Rgb<u8>>` to `ImgVec<[u8; 3]>` for fast-ssim2's imgref API.
fn to_u8x3(img: ImgRef<'_, Rgb8>) -> ImgVec<[u8; 3]> {
    let pixels: Vec<[u8; 3]> = img.pixels().map(|p| [p.r, p.g, p.b]).collect();
    ImgVec::new(pixels, img.width(), img.height())
}

pub fn run_eval(
    images: &[SourceImage],
    jpeg_config: &JpegConfig,
    quality_levels: &[u8],
) -> Result<EvalResult> {
    let config_summary = config::config_summary(jpeg_config);
    let total_start = Instant::now();
    let mut points = Vec::new();

    for image in images {
        let source_ref = image.pixels.as_ref();
        let total_pixels = (image.width * image.height) as f64;

        // Precompute SSIM2 reference for this source (shared across quality levels)
        let source_arr = to_u8x3(source_ref);
        let reference = Ssimulacra2Reference::new(source_arr.as_ref())
            .map_err(|e| anyhow::anyhow!("SSIM2 reference error for {}: {e}", image.name))?;

        for &quality in quality_levels {
            let codec_config = config::build_codec_config(jpeg_config, quality);

            // Encode
            let enc_start = Instant::now();
            let encoded = EncodeRequest::new(ImageFormat::Jpeg)
                .with_codec_config(&codec_config)
                .encode_rgb8(source_ref)
                .map_err(|e| anyhow::anyhow!("Encode error for {} q{quality}: {e}", image.name))?;
            let encode_ms = enc_start.elapsed().as_millis() as u64;

            let size_bytes = encoded.len();
            let bpp = (size_bytes as f64 * 8.0) / total_pixels;

            // Decode
            let decoded = DecodeRequest::new(encoded.bytes())
                .decode()
                .map_err(|e| anyhow::anyhow!("Decode error for {} q{quality}: {e}", image.name))?;
            let decoded_rgb = decoded.into_rgb8();

            // SSIM2 (using precomputed reference)
            let decoded_arr = to_u8x3(decoded_rgb.as_ref());
            let ssim2 = reference
                .compare(decoded_arr.as_ref())
                .map_err(|e| anyhow::anyhow!("SSIM2 error for {} q{quality}: {e}", image.name))?;

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
