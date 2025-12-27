//! Corpus management for test image collections.
//!
//! This module provides tools for managing collections of test images,
//! including discovery, categorization, and checksum-based deduplication.
//!
//! ## Example
//!
//! ```rust,ignore
//! use codec_eval::corpus::Corpus;
//!
//! // Discover images in a directory
//! let corpus = Corpus::discover("./test_images")?;
//!
//! // Filter by category
//! let photos = corpus.filter_category(ImageCategory::Photo);
//!
//! // Get training/validation split
//! let (train, val) = corpus.split(0.8);
//! ```

mod category;
mod checksum;
mod discovery;
pub mod sparse;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use category::ImageCategory;
pub use checksum::compute_checksum;
pub use sparse::{SparseCheckout, SparseFilter, SparseStatus};

use crate::error::Result;

/// A corpus of test images.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Corpus {
    /// Name of the corpus.
    pub name: String,

    /// Root path of the corpus.
    pub root_path: PathBuf,

    /// Images in the corpus.
    pub images: Vec<CorpusImage>,

    /// Metadata about the corpus.
    #[serde(default)]
    pub metadata: CorpusMetadata,
}

/// Metadata about a corpus.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CorpusMetadata {
    /// Description of the corpus.
    pub description: Option<String>,

    /// License information.
    pub license: Option<String>,

    /// Source URL.
    pub source_url: Option<String>,

    /// Number of images by category.
    #[serde(default)]
    pub category_counts: std::collections::HashMap<String, usize>,
}

/// An image in the corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusImage {
    /// Relative path from corpus root.
    pub relative_path: PathBuf,

    /// Image category (if classified).
    pub category: Option<ImageCategory>,

    /// Image dimensions.
    pub width: u32,
    pub height: u32,

    /// File size in bytes.
    pub file_size: u64,

    /// Content checksum (for deduplication).
    pub checksum: Option<String>,

    /// Format detected from file extension.
    pub format: String,
}

impl CorpusImage {
    /// Get the full path to the image.
    #[must_use]
    pub fn full_path(&self, root: &Path) -> PathBuf {
        root.join(&self.relative_path)
    }

    /// Get the image name (filename without path).
    #[must_use]
    pub fn name(&self) -> &str {
        self.relative_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
    }

    /// Get pixel count.
    #[must_use]
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }
}

impl Corpus {
    /// Create a new empty corpus.
    #[must_use]
    pub fn new(name: impl Into<String>, root_path: impl Into<PathBuf>) -> Self {
        Self {
            name: name.into(),
            root_path: root_path.into(),
            images: Vec::new(),
            metadata: CorpusMetadata::default(),
        }
    }

    /// Discover images in a directory.
    ///
    /// Recursively scans the directory for supported image formats
    /// (PNG, JPEG, WebP, AVIF).
    pub fn discover(path: impl AsRef<Path>) -> Result<Self> {
        discovery::discover_corpus(path.as_ref())
    }

    /// Default corpus repository URL.
    pub const DEFAULT_CORPUS_URL: &'static str =
        "https://github.com/AcrossTheCloud/codec-corpus.git";

    /// Discover or download a corpus on demand.
    ///
    /// If the path exists, discovers images. If not, clones from the given URL
    /// (or the default codec-corpus repository) and then discovers.
    ///
    /// # Arguments
    /// * `path` - Local path for the corpus
    /// * `url` - Optional remote URL (defaults to codec-corpus)
    /// * `subsets` - Optional list of subsets to download (e.g., ["kodak", "clic2025"])
    ///
    /// # Example
    /// ```rust,ignore
    /// // Download just the kodak test set
    /// let corpus = Corpus::discover_or_download(
    ///     "./corpus",
    ///     None,
    ///     Some(&["kodak"]),
    /// )?;
    /// ```
    pub fn discover_or_download(
        path: impl AsRef<Path>,
        url: Option<&str>,
        subsets: Option<&[&str]>,
    ) -> Result<Self> {
        let path = path.as_ref();
        let url = url.unwrap_or(Self::DEFAULT_CORPUS_URL);

        // If path exists and has images, just discover
        if path.exists() && path.is_dir() {
            // Check if it has any image files
            if has_image_files(path) {
                return Self::discover(path);
            }
        }

        // Need to download
        eprintln!("Corpus not found at {}, downloading from {}", path.display(), url);

        // Use sparse checkout for efficiency
        let sparse = if let Some(subsets) = subsets {
            let checkout = SparseCheckout::clone_shallow(url, path, 1)?;
            // Add subset directories
            let paths: Vec<&str> = subsets.iter().copied().collect();
            checkout.add_paths(&paths)?;
            checkout.checkout()?;
            checkout
        } else {
            // Clone everything (but still use sparse for efficiency)
            let checkout = SparseCheckout::clone_shallow(url, path, 1)?;
            checkout.set_paths(&["*"])?;
            checkout.checkout()?;
            checkout
        };

        eprintln!("Downloaded corpus to {}", sparse.path().display());

        // Now discover
        Self::discover(path)
    }

    /// Download a specific subset of the corpus.
    ///
    /// # Example
    /// ```rust,ignore
    /// let corpus = Corpus::download_subset("./corpus", "kodak")?;
    /// ```
    pub fn download_subset(path: impl AsRef<Path>, subset: &str) -> Result<Self> {
        Self::discover_or_download(path, None, Some(&[subset]))
    }

    /// Get corpus, downloading if necessary. Returns cached corpus if available.
    ///
    /// Checks common locations for existing corpus before downloading:
    /// 1. The specified path
    /// 2. ./codec-corpus
    /// 3. ../codec-corpus
    /// 4. ../codec-comparison/codec-corpus
    pub fn get_or_download(preferred_path: impl AsRef<Path>) -> Result<Self> {
        let preferred = preferred_path.as_ref();

        // Check common locations
        let candidates = [
            preferred.to_path_buf(),
            PathBuf::from("./codec-corpus"),
            PathBuf::from("../codec-corpus"),
            PathBuf::from("../codec-comparison/codec-corpus"),
        ];

        for path in &candidates {
            if path.exists() && has_image_files(path) {
                eprintln!("Found corpus at {}", path.display());
                return Self::discover(path);
            }
        }

        // Not found, download to preferred path
        Self::discover_or_download(preferred, None, None)
    }

    /// Load a corpus from a JSON manifest file.
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())?;
        let corpus: Corpus = serde_json::from_str(&content)?;
        Ok(corpus)
    }

    /// Save the corpus to a JSON manifest file.
    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path.as_ref(), content)?;
        Ok(())
    }

    /// Get the number of images in the corpus.
    #[must_use]
    pub fn len(&self) -> usize {
        self.images.len()
    }

    /// Check if the corpus is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.images.is_empty()
    }

    /// Filter images by category.
    #[must_use]
    pub fn filter_category(&self, category: ImageCategory) -> Vec<&CorpusImage> {
        self.images
            .iter()
            .filter(|img| img.category == Some(category))
            .collect()
    }

    /// Filter images by format.
    #[must_use]
    pub fn filter_format(&self, format: &str) -> Vec<&CorpusImage> {
        let format_lower = format.to_lowercase();
        self.images
            .iter()
            .filter(|img| img.format.to_lowercase() == format_lower)
            .collect()
    }

    /// Filter images by minimum dimensions.
    #[must_use]
    pub fn filter_min_size(&self, min_width: u32, min_height: u32) -> Vec<&CorpusImage> {
        self.images
            .iter()
            .filter(|img| img.width >= min_width && img.height >= min_height)
            .collect()
    }

    /// Split the corpus into training and validation sets.
    ///
    /// Uses a deterministic split based on checksum to ensure reproducibility.
    ///
    /// # Arguments
    ///
    /// * `train_ratio` - Fraction of images to include in training set (0.0-1.0).
    #[must_use]
    pub fn split(&self, train_ratio: f64) -> (Vec<&CorpusImage>, Vec<&CorpusImage>) {
        let train_ratio = train_ratio.clamp(0.0, 1.0);
        let mut train = Vec::new();
        let mut val = Vec::new();

        for (i, img) in self.images.iter().enumerate() {
            // Use checksum if available, otherwise use index
            let hash = img.checksum.as_ref().map_or(i, |s| {
                s.bytes()
                    .fold(0usize, |acc, b| acc.wrapping_add(b as usize))
            });

            if (hash % 1000) < (train_ratio * 1000.0) as usize {
                train.push(img);
            } else {
                val.push(img);
            }
        }

        (train, val)
    }

    /// Compute checksums for all images that don't have them.
    pub fn compute_checksums(&mut self) -> Result<usize> {
        let mut computed = 0;

        for img in &mut self.images {
            if img.checksum.is_none() {
                let path = self.root_path.join(&img.relative_path);
                if path.exists() {
                    img.checksum = Some(compute_checksum(&path)?);
                    computed += 1;
                }
            }
        }

        Ok(computed)
    }

    /// Find duplicate images by checksum.
    #[must_use]
    pub fn find_duplicates(&self) -> Vec<Vec<&CorpusImage>> {
        use std::collections::HashMap;

        let mut by_checksum: HashMap<&str, Vec<&CorpusImage>> = HashMap::new();

        for img in &self.images {
            if let Some(ref checksum) = img.checksum {
                by_checksum.entry(checksum).or_default().push(img);
            }
        }

        by_checksum.into_values().filter(|v| v.len() > 1).collect()
    }

    /// Update category counts in metadata.
    pub fn update_category_counts(&mut self) {
        self.metadata.category_counts.clear();

        for img in &self.images {
            if let Some(cat) = img.category {
                *self
                    .metadata
                    .category_counts
                    .entry(cat.to_string())
                    .or_insert(0) += 1;
            }
        }
    }

    /// Get statistics about the corpus.
    #[must_use]
    pub fn stats(&self) -> CorpusStats {
        let total_pixels: u64 = self.images.iter().map(|img| img.pixel_count()).sum();
        let total_bytes: u64 = self.images.iter().map(|img| img.file_size).sum();

        let widths: Vec<u32> = self.images.iter().map(|img| img.width).collect();
        let heights: Vec<u32> = self.images.iter().map(|img| img.height).collect();

        CorpusStats {
            image_count: self.images.len(),
            total_pixels,
            total_bytes,
            min_width: widths.iter().copied().min().unwrap_or(0),
            max_width: widths.iter().copied().max().unwrap_or(0),
            min_height: heights.iter().copied().min().unwrap_or(0),
            max_height: heights.iter().copied().max().unwrap_or(0),
        }
    }
}

/// Statistics about a corpus.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusStats {
    /// Number of images.
    pub image_count: usize,
    /// Total pixels across all images.
    pub total_pixels: u64,
    /// Total file size in bytes.
    pub total_bytes: u64,
    /// Minimum image width.
    pub min_width: u32,
    /// Maximum image width.
    pub max_width: u32,
    /// Minimum image height.
    pub min_height: u32,
    /// Maximum image height.
    pub max_height: u32,
}

/// Check if a directory contains any image files.
fn has_image_files(path: &Path) -> bool {
    const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "avif", "jxl"];

    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(ext) = entry_path.extension().and_then(|e| e.to_str()) {
                    if IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                        return true;
                    }
                }
            } else if entry_path.is_dir() {
                // Check subdirectories recursively (but only one level deep for performance)
                if let Ok(sub_entries) = std::fs::read_dir(&entry_path) {
                    for sub_entry in sub_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.is_file() {
                            if let Some(ext) = sub_path.extension().and_then(|e| e.to_str()) {
                                if IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_corpus_new() {
        let corpus = Corpus::new("test", "/tmp/images");
        assert_eq!(corpus.name, "test");
        assert!(corpus.is_empty());
    }

    #[test]
    fn test_corpus_image_name() {
        let img = CorpusImage {
            relative_path: PathBuf::from("subdir/image.png"),
            category: None,
            width: 100,
            height: 100,
            file_size: 1000,
            checksum: None,
            format: "png".to_string(),
        };
        assert_eq!(img.name(), "image.png");
    }

    #[test]
    fn test_corpus_split() {
        let mut corpus = Corpus::new("test", "/tmp");
        for i in 0..100 {
            corpus.images.push(CorpusImage {
                relative_path: PathBuf::from(format!("img{i}.png")),
                category: None,
                width: 100,
                height: 100,
                file_size: 1000,
                // Use varied checksums to get good distribution
                checksum: Some(format!("{i:016x}")),
                format: "png".to_string(),
            });
        }

        let (train, val) = corpus.split(0.8);
        // Should split all images
        assert_eq!(train.len() + val.len(), 100);
    }
}
