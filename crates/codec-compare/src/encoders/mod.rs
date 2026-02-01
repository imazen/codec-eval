//! Encoder implementations for various image codecs.
//!
//! Each encoder provides:
//! - `EncodeFn` callback for codec-eval integration
//! - `DecodeFn` callback for quality metrics
//! - Version detection
//! - Quality mapping to 0-100 scale

pub mod avif;
pub mod jpeg;
pub mod jpegxl;
pub mod webp;
pub mod zenjpeg;

// Re-export JPEG config types
pub use jpeg::{JpegMode, JpegSubsampling};

use codec_eval::eval::session::{DecodeFn, EncodeFn};

/// Trait for codec implementations that can register with codec-eval.
pub trait CodecImpl: Send + Sync {
    /// Unique identifier for this codec (e.g., "mozjpeg", "jpegli").
    fn id(&self) -> &str;

    /// Version string (e.g., "4.1.1").
    fn version(&self) -> &str;

    /// Output format extension (e.g., "jpg", "webp", "avif").
    fn format(&self) -> &str;

    /// Create the encode function.
    fn encode_fn(&self) -> EncodeFn;

    /// Create the decode function.
    fn decode_fn(&self) -> DecodeFn;

    /// Whether this codec is available (dependencies present).
    fn is_available(&self) -> bool {
        true
    }
}

/// Get a color for a codec (for charts).
pub fn codec_color(id: &str) -> &'static str {
    match id {
        // JPEG variants
        "mozjpeg" => "#e74c3c",       // red
        "jpegli" => "#3498db",        // blue
        "libjpeg-turbo" => "#95a5a6", // gray

        // Zenjpeg (hybrid encoder)
        "zenjpeg" => "#2ecc71", // emerald green

        // WebP
        "webp" => "#27ae60", // green

        // AVIF variants
        "avif-aom" => "#9b59b6",   // purple
        "avif-rav1e" => "#e67e22", // orange
        "avif-svt" => "#1abc9c",   // teal

        // JPEG XL
        "jpegxl" => "#f39c12", // golden yellow

        // Default
        _ => "#34495e", // dark gray
    }
}

/// Standard quality levels for codec comparison.
///
/// These are chosen to provide good coverage of the rate-distortion curve:
/// - 50-70: Low-medium quality (high compression)
/// - 75-85: Medium-high quality (typical use)
/// - 90-95: High quality (minimal artifacts)
pub const STANDARD_QUALITY_LEVELS: &[f64] = &[50.0, 60.0, 70.0, 75.0, 80.0, 85.0, 90.0, 95.0];
