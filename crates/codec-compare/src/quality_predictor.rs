//! Unified quality interpretation for encoder selection
//!
//! Maps between quality values, perceptual targets, and encoder selection.

/// Quality equivalence between encoders (based on butteraugli)
///
/// At the same perceptual quality (butteraugli score):
/// - mozjpeg Q90 ≈ jpegli Q80
/// - mozjpeg Q85 ≈ jpegli Q70
/// - mozjpeg Q75 ≈ jpegli Q55
/// - mozjpeg Q60 ≈ jpegli Q35
pub fn mozjpeg_to_jpegli_quality(moz_quality: u8) -> u8 {
    // Approximate mapping based on butteraugli equivalence
    match moz_quality {
        90..=100 => ((moz_quality as i32 - 10).max(75) as u8), // Q90→Q80
        85..=89 => ((moz_quality as i32 - 15).max(70) as u8),  // Q85→Q70
        75..=84 => ((moz_quality as i32 - 20).max(55) as u8),  // Q75→Q55
        60..=74 => ((moz_quality as i32 - 25).max(35) as u8),  // Q60→Q35
        _ => 25, // Below Q60, jpegli Q25 is still better
    }
}

/// Convert jpegli quality to equivalent mozjpeg quality
pub fn jpegli_to_mozjpeg_quality(jpegli_quality: u8) -> u8 {
    match jpegli_quality {
        80..=100 => (jpegli_quality + 10).min(100),
        70..=79 => jpegli_quality + 15,
        55..=69 => jpegli_quality + 20,
        35..=54 => jpegli_quality + 25,
        _ => 100, // jpegli Q25-34 has no mozjpeg equivalent
    }
}

/// Estimate butteraugli score for a given quality and encoder
pub fn estimate_butteraugli(quality: u8, encoder: &str) -> f64 {
    // Empirical fit from combined corpus analysis
    let q = quality as f64;

    if encoder == "jpegli" {
        // jpegli: butteraugli ≈ 7.5 - 0.065*Q
        (7.5 - 0.065 * q).max(0.5)
    } else {
        // mozjpeg: butteraugli ≈ 9.5 - 0.078*Q
        (9.5 - 0.078 * q).max(1.0)
    }
}

/// Find quality setting to achieve target butteraugli
pub fn quality_for_butteraugli(target: f64, encoder: &str) -> u8 {
    if encoder == "jpegli" {
        // Invert: Q = (7.5 - target) / 0.065
        ((7.5 - target) / 0.065).clamp(25.0, 100.0) as u8
    } else {
        // Invert: Q = (9.5 - target) / 0.078
        ((9.5 - target) / 0.078).clamp(25.0, 100.0) as u8
    }
}

/// Predict which encoder produces smaller files at target perceptual quality
///
/// Returns ("encoder_name", estimated_bpp)
pub fn predict_encoder_for_quality(
    target_butteraugli: f64,
    flat_block_pct: f64,
    edge_strength: f64,
    local_contrast: f64,
) -> (&'static str, f64) {
    let complexity = edge_strength + local_contrast;

    // Crossover point depends on image characteristics
    let crossover = if flat_block_pct > 75.0 && complexity < 20.0 {
        // Very flat images: mozjpeg wins up to higher quality
        3.0 // butteraugli threshold
    } else if flat_block_pct > 60.0 {
        // Flat images
        3.5
    } else {
        // Complex images: jpegli almost always wins
        4.5
    };

    if target_butteraugli > crossover {
        // Low quality target → mozjpeg
        let q = quality_for_butteraugli(target_butteraugli, "mozjpeg");
        let bpp = estimate_bpp_mozjpeg(q, flat_block_pct);
        ("mozjpeg", bpp)
    } else {
        // High quality target → jpegli
        let q = quality_for_butteraugli(target_butteraugli, "jpegli");
        let bpp = estimate_bpp_jpegli(q, flat_block_pct);
        ("jpegli", bpp)
    }
}

/// Estimate BPP for mozjpeg at given quality and image flatness
fn estimate_bpp_mozjpeg(quality: u8, flat_pct: f64) -> f64 {
    // Base BPP from quality
    let base = 0.1 + 0.016 * quality as f64;
    // Adjust for content (flat images compress better)
    let content_factor = 0.3 + 0.7 * (100.0 - flat_pct) / 100.0;
    base * content_factor
}

/// Estimate BPP for jpegli at given quality and image flatness
fn estimate_bpp_jpegli(quality: u8, flat_pct: f64) -> f64 {
    // jpegli produces ~30% larger files at same quality
    let base = 0.4 + 0.017 * quality as f64;
    let content_factor = 0.3 + 0.7 * (100.0 - flat_pct) / 100.0;
    base * content_factor
}

/// Unified quality scale: 0-100 where 100 = lossless
/// Maps to butteraugli: UQ100 = 0.0, UQ0 = ~8.0
pub fn unified_quality_to_butteraugli(unified_quality: u8) -> f64 {
    // UQ100 → butteraugli 0.0
    // UQ75  → butteraugli 2.0 (very good)
    // UQ50  → butteraugli 4.0 (acceptable)
    // UQ25  → butteraugli 6.0 (noticeable)
    // UQ0   → butteraugli 8.0 (degraded)
    8.0 * (1.0 - unified_quality as f64 / 100.0)
}

/// Get encoder quality setting for unified quality target
pub fn unified_to_encoder_quality(unified_quality: u8, encoder: &str) -> u8 {
    let target_ba = unified_quality_to_butteraugli(unified_quality);
    quality_for_butteraugli(target_ba, encoder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quality_equivalence() {
        // mozjpeg Q90 should map to roughly jpegli Q80
        assert!((mozjpeg_to_jpegli_quality(90) as i32 - 80).abs() <= 5);

        // mozjpeg Q85 should map to roughly jpegli Q70
        assert!((mozjpeg_to_jpegli_quality(85) as i32 - 70).abs() <= 5);
    }

    #[test]
    fn test_butteraugli_estimation() {
        // jpegli should have lower butteraugli at same Q
        let jpegli_ba = estimate_butteraugli(75, "jpegli");
        let moz_ba = estimate_butteraugli(75, "mozjpeg");
        assert!(jpegli_ba < moz_ba);
    }

    #[test]
    fn test_unified_quality() {
        // UQ75 should be around butteraugli 2.0
        let ba = unified_quality_to_butteraugli(75);
        assert!((ba - 2.0).abs() < 0.5);

        // UQ50 should be around butteraugli 4.0
        let ba = unified_quality_to_butteraugli(50);
        assert!((ba - 4.0).abs() < 0.5);
    }

    #[test]
    fn test_encoder_selection() {
        // High quality (low butteraugli) → jpegli
        let (enc, _) = predict_encoder_for_quality(2.0, 50.0, 15.0, 15.0);
        assert_eq!(enc, "jpegli");

        // Low quality, flat image → mozjpeg
        let (enc, _) = predict_encoder_for_quality(5.0, 85.0, 5.0, 5.0);
        assert_eq!(enc, "mozjpeg");
    }
}
