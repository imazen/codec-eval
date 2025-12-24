//! Image category classification.

use serde::{Deserialize, Serialize};

/// Category of an image for per-category analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageCategory {
    /// Photographic content.
    Photo,
    /// Digital illustrations, drawings, artwork.
    Illustration,
    /// Text-heavy images, documents.
    Text,
    /// Screenshots, UI captures.
    Screenshot,
    /// High-frequency detail (textures, foliage).
    HighFrequency,
    /// Low-frequency content (sky, gradients).
    LowFrequency,
    /// Smooth gradients.
    Gradient,
    /// Repeating patterns.
    Pattern,
    /// Computer-generated imagery.
    Cgi,
    /// Medical or scientific imagery.
    Scientific,
    /// Uncategorized.
    Other,
}

impl ImageCategory {
    /// Get all category variants.
    #[must_use]
    pub fn all() -> &'static [Self] {
        &[
            Self::Photo,
            Self::Illustration,
            Self::Text,
            Self::Screenshot,
            Self::HighFrequency,
            Self::LowFrequency,
            Self::Gradient,
            Self::Pattern,
            Self::Cgi,
            Self::Scientific,
            Self::Other,
        ]
    }

    /// Parse from string (case-insensitive).
    #[must_use]
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "photo" | "photograph" | "photos" => Some(Self::Photo),
            "illustration" | "drawing" | "art" | "artwork" => Some(Self::Illustration),
            "text" | "document" | "docs" => Some(Self::Text),
            "screenshot" | "screenshots" | "ui" => Some(Self::Screenshot),
            "high_frequency" | "highfreq" | "texture" | "textures" => Some(Self::HighFrequency),
            "low_frequency" | "lowfreq" | "smooth" => Some(Self::LowFrequency),
            "gradient" | "gradients" => Some(Self::Gradient),
            "pattern" | "patterns" => Some(Self::Pattern),
            "cgi" | "render" | "3d" => Some(Self::Cgi),
            "scientific" | "medical" | "science" => Some(Self::Scientific),
            "other" | "misc" | "unknown" => Some(Self::Other),
            _ => None,
        }
    }

    /// Get a description of this category.
    #[must_use]
    pub fn description(self) -> &'static str {
        match self {
            Self::Photo => "Photographic content",
            Self::Illustration => "Digital illustrations and artwork",
            Self::Text => "Text-heavy images and documents",
            Self::Screenshot => "Screenshots and UI captures",
            Self::HighFrequency => "High-frequency detail (textures, foliage)",
            Self::LowFrequency => "Low-frequency content (sky, gradients)",
            Self::Gradient => "Smooth gradients",
            Self::Pattern => "Repeating patterns",
            Self::Cgi => "Computer-generated imagery",
            Self::Scientific => "Medical or scientific imagery",
            Self::Other => "Uncategorized",
        }
    }
}

impl std::fmt::Display for ImageCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Photo => write!(f, "photo"),
            Self::Illustration => write!(f, "illustration"),
            Self::Text => write!(f, "text"),
            Self::Screenshot => write!(f, "screenshot"),
            Self::HighFrequency => write!(f, "high_frequency"),
            Self::LowFrequency => write!(f, "low_frequency"),
            Self::Gradient => write!(f, "gradient"),
            Self::Pattern => write!(f, "pattern"),
            Self::Cgi => write!(f, "cgi"),
            Self::Scientific => write!(f, "scientific"),
            Self::Other => write!(f, "other"),
        }
    }
}

impl std::str::FromStr for ImageCategory {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_str_loose(s).ok_or_else(|| format!("Unknown category: {s}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_category_roundtrip() {
        for cat in ImageCategory::all() {
            let s = cat.to_string();
            let parsed: ImageCategory = s.parse().unwrap();
            assert_eq!(*cat, parsed);
        }
    }

    #[test]
    fn test_category_from_str_loose() {
        assert_eq!(ImageCategory::from_str_loose("PHOTO"), Some(ImageCategory::Photo));
        assert_eq!(ImageCategory::from_str_loose("Photos"), Some(ImageCategory::Photo));
        assert_eq!(ImageCategory::from_str_loose("artwork"), Some(ImageCategory::Illustration));
        assert_eq!(ImageCategory::from_str_loose("invalid"), None);
    }
}
