//! Test ICC profile extraction and transformation with jpegli XYB JPEG

use codec_eval::ImageData;
use codec_eval::decode::decode_jpeg_with_icc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a simple test image
    let width = 64;
    let height = 64;
    let mut rgb = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        for x in 0..width {
            rgb.push((x * 4) as u8); // R gradient
            rgb.push((y * 4) as u8); // G gradient
            rgb.push(128u8); // B constant
        }
    }

    println!("Test image: {}x{}", width, height);

    // Encode with jpegli XYB mode (produces XYB JPEG with ICC profile)
    let encoder = jpegli::Encoder::new()
        .width(width as u32)
        .height(height as u32)
        .pixel_format(jpegli::PixelFormat::Rgb)
        .quality(jpegli::Quality::from_quality(90.0))
        .use_xyb(true); // Enable XYB mode which embeds ICC profile

    let jpeg_data = encoder.encode(&rgb)?;
    println!("Encoded JPEG size: {} bytes", jpeg_data.len());

    // Decode with ICC extraction
    let decoded = decode_jpeg_with_icc(&jpeg_data)?;

    match &decoded {
        ImageData::RgbSliceWithIcc {
            width,
            height,
            icc_profile,
            ..
        } => {
            println!("Decoded with ICC profile!");
            println!("  Dimensions: {}x{}", width, height);
            println!("  ICC profile size: {} bytes", icc_profile.len());

            // Check ICC profile header
            if icc_profile.len() > 128 {
                let profile_class = &icc_profile[12..16];
                let color_space = &icc_profile[16..20];
                println!(
                    "  Profile class: {:?}",
                    String::from_utf8_lossy(profile_class)
                );
                println!("  Color space: {:?}", String::from_utf8_lossy(color_space));
            }
        }
        ImageData::RgbSlice { width, height, .. } => {
            println!("Decoded WITHOUT ICC profile (assumed sRGB)");
            println!("  Dimensions: {}x{}", width, height);
        }
        _ => println!("Unexpected ImageData variant"),
    }

    // Test the sRGB conversion
    println!("\nTesting sRGB conversion...");
    let srgb_pixels = decoded.to_rgb8_srgb()?;
    println!("Converted to sRGB: {} bytes", srgb_pixels.len());

    // Compare: raw XYB values vs ICC-transformed sRGB vs original input
    let xyb_raw = decoded.to_rgb8_vec();
    println!("\nPixel comparison (first 3 pixels):");
    println!("  Showing: original_input → XYB_stored → ICC_transformed_sRGB");
    for i in 0..3 {
        let input_r = (i * 4) as u8;
        let input_g = 0u8; // y=0 for first row
        let input_b = 128u8;

        let xyb = &xyb_raw[i * 3..i * 3 + 3];
        let srgb = &srgb_pixels[i * 3..i * 3 + 3];

        let error_r = (input_r as i16 - srgb[0] as i16).abs();
        let error_g = (input_g as i16 - srgb[1] as i16).abs();
        let error_b = (input_b as i16 - srgb[2] as i16).abs();
        let avg_error = (error_r + error_g + error_b) as f32 / 3.0;

        println!(
            "  Pixel {}: [{:3},{:3},{:3}] → [{:3},{:3},{:3}] → [{:3},{:3},{:3}]  error={:.1}",
            i,
            input_r,
            input_g,
            input_b,
            xyb[0],
            xyb[1],
            xyb[2],
            srgb[0],
            srgb[1],
            srgb[2],
            avg_error
        );
    }

    // Calculate per-channel and overall roundtrip error
    println!("\nCalculating overall roundtrip error...");
    let mut total_r = 0u64;
    let mut total_g = 0u64;
    let mut total_b = 0u64;
    let mut max_error = 0u64;
    let mut max_error_pos = (0, 0);

    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            let input_r = (x * 4) as u8;
            let input_g = (y * 4) as u8;
            let input_b = 128u8;

            let srgb = &srgb_pixels[i * 3..i * 3 + 3];
            let err_r = (input_r as i16 - srgb[0] as i16).unsigned_abs() as u64;
            let err_g = (input_g as i16 - srgb[1] as i16).unsigned_abs() as u64;
            let err_b = (input_b as i16 - srgb[2] as i16).unsigned_abs() as u64;

            total_r += err_r;
            total_g += err_g;
            total_b += err_b;

            let pixel_error = err_r + err_g + err_b;
            if pixel_error > max_error {
                max_error = pixel_error;
                max_error_pos = (x, y);
            }
        }
    }
    let num_pixels = (width * height) as u64;
    println!(
        "  Per-channel average error: R={:.2}, G={:.2}, B={:.2}",
        total_r as f64 / num_pixels as f64,
        total_g as f64 / num_pixels as f64,
        total_b as f64 / num_pixels as f64
    );

    let (wx, wy) = max_error_pos;
    let wi = wy * width + wx;
    let worst_input = [(wx * 4) as u8, (wy * 4) as u8, 128u8];
    let worst_xyb = &xyb_raw[wi * 3..wi * 3 + 3];
    let worst_srgb = &srgb_pixels[wi * 3..wi * 3 + 3];
    println!(
        "  Worst pixel at ({},{}): [{:3},{:3},{:3}] → [{:3},{:3},{:3}] → [{:3},{:3},{:3}]",
        wx,
        wy,
        worst_input[0],
        worst_input[1],
        worst_input[2],
        worst_xyb[0],
        worst_xyb[1],
        worst_xyb[2],
        worst_srgb[0],
        worst_srgb[1],
        worst_srgb[2]
    );

    let avg_error = (total_r + total_g + total_b) as f64 / (num_pixels as f64 * 3.0);
    println!("  Overall average per-channel error: {:.2}", avg_error);

    // Note: XYB JPEGs have higher expected error due to B-channel subsampling
    // and the perceptual color space. Acceptable thresholds are higher than sRGB JPEG.
    if avg_error < 20.0 {
        println!("  ICC roundtrip is within expected range for XYB JPEG.");
    } else {
        println!("  Note: Higher error may be due to XYB B-channel 2x2 subsampling.");
    }

    Ok(())
}
