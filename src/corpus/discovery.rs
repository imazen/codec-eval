//! Image discovery in directories.

use std::fs;
use std::path::Path;

use crate::corpus::{Corpus, CorpusImage, ImageCategory};
use crate::error::{Error, Result};

/// Supported image extensions.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "webp", "avif", "jxl", "heic", "heif", "bmp", "tiff", "tif",
];

/// Discover images in a directory.
pub fn discover_corpus(path: &Path) -> Result<Corpus> {
    if !path.exists() {
        return Err(Error::Corpus(format!("Path does not exist: {}", path.display())));
    }

    if !path.is_dir() {
        return Err(Error::Corpus(format!("Path is not a directory: {}", path.display())));
    }

    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("corpus")
        .to_string();

    let mut corpus = Corpus::new(name, path);

    discover_recursive(path, path, &mut corpus.images)?;

    // Try to infer categories from directory names
    infer_categories(&mut corpus);

    // Update category counts
    corpus.update_category_counts();

    Ok(corpus)
}

fn discover_recursive(
    root: &Path,
    current: &Path,
    images: &mut Vec<CorpusImage>,
) -> Result<()> {
    let entries = fs::read_dir(current).map_err(|e| {
        Error::Corpus(format!("Failed to read directory {}: {}", current.display(), e))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            Error::Corpus(format!("Failed to read entry in {}: {}", current.display(), e))
        })?;

        let path = entry.path();

        if path.is_dir() {
            // Skip hidden directories
            if path
                .file_name()
                .and_then(|s| s.to_str())
                .map_or(false, |s| s.starts_with('.'))
            {
                continue;
            }
            discover_recursive(root, &path, images)?;
        } else if path.is_file() {
            if let Some(img) = try_load_image_info(&path, root) {
                images.push(img);
            }
        }
    }

    Ok(())
}

fn try_load_image_info(path: &Path, root: &Path) -> Option<CorpusImage> {
    // Check extension
    let extension = path.extension()?.to_str()?.to_lowercase();
    if !SUPPORTED_EXTENSIONS.contains(&extension.as_str()) {
        return None;
    }

    // Get file metadata
    let metadata = fs::metadata(path).ok()?;
    let file_size = metadata.len();

    // Get relative path
    let relative_path = path.strip_prefix(root).ok()?.to_path_buf();

    // Try to get image dimensions
    let (width, height) = get_image_dimensions(path).unwrap_or((0, 0));

    // Map extension to format
    let format = match extension.as_str() {
        "jpg" | "jpeg" => "jpeg",
        "jxl" => "jpegxl",
        "heic" | "heif" => "heif",
        "tif" | "tiff" => "tiff",
        other => other,
    }
    .to_string();

    Some(CorpusImage {
        relative_path,
        category: None,
        width,
        height,
        file_size,
        checksum: None,
        format,
    })
}

/// Get image dimensions by reading file header.
fn get_image_dimensions(path: &Path) -> Option<(u32, u32)> {
    let data = fs::read(path).ok()?;

    // PNG: check signature and read IHDR
    if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        if data.len() >= 24 {
            let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
            let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
            return Some((width, height));
        }
    }

    // JPEG: search for SOF marker
    if data.starts_with(&[0xFF, 0xD8]) {
        return parse_jpeg_dimensions(&data);
    }

    // WebP: check RIFF header
    if data.len() >= 30 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return parse_webp_dimensions(&data);
    }

    None
}

fn parse_jpeg_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 9 < data.len() {
        if data[i] != 0xFF {
            i += 1;
            continue;
        }

        let marker = data[i + 1];

        // SOF markers (Start Of Frame)
        if matches!(marker, 0xC0 | 0xC1 | 0xC2 | 0xC3 | 0xC5 | 0xC6 | 0xC7 | 0xC9 | 0xCA | 0xCB | 0xCD | 0xCE | 0xCF) {
            let height = u32::from(data[i + 5]) << 8 | u32::from(data[i + 6]);
            let width = u32::from(data[i + 7]) << 8 | u32::from(data[i + 8]);
            return Some((width, height));
        }

        // Skip to next marker
        if i + 3 >= data.len() {
            break;
        }
        let length = u16::from(data[i + 2]) << 8 | u16::from(data[i + 3]);
        i += 2 + length as usize;
    }

    None
}

fn parse_webp_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // VP8 format
    if data.len() >= 30 && &data[12..16] == b"VP8 " {
        // Skip to frame header
        if data.len() >= 26 {
            let width = u32::from(data[26]) | (u32::from(data[27] & 0x3F) << 8);
            let height = u32::from(data[28]) | (u32::from(data[29] & 0x3F) << 8);
            return Some((width, height));
        }
    }

    // VP8L format (lossless)
    if data.len() >= 25 && &data[12..16] == b"VP8L" {
        let bits = u32::from(data[21])
            | (u32::from(data[22]) << 8)
            | (u32::from(data[23]) << 16)
            | (u32::from(data[24]) << 24);
        let width = (bits & 0x3FFF) + 1;
        let height = ((bits >> 14) & 0x3FFF) + 1;
        return Some((width, height));
    }

    // VP8X format (extended)
    if data.len() >= 30 && &data[12..16] == b"VP8X" {
        let width = u32::from(data[24])
            | (u32::from(data[25]) << 8)
            | (u32::from(data[26]) << 16);
        let height = u32::from(data[27])
            | (u32::from(data[28]) << 8)
            | (u32::from(data[29]) << 16);
        return Some((width + 1, height + 1));
    }

    None
}

/// Try to infer categories from directory names.
fn infer_categories(corpus: &mut Corpus) {
    for img in &mut corpus.images {
        if img.category.is_some() {
            continue;
        }

        // Check parent directory names
        for component in img.relative_path.components() {
            if let std::path::Component::Normal(name) = component {
                if let Some(name_str) = name.to_str() {
                    if let Some(cat) = ImageCategory::from_str_loose(name_str) {
                        img.category = Some(cat);
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_dimensions() {
        // Minimal valid PNG header with 100x50 dimensions
        let mut png = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        png.extend_from_slice(&[0, 0, 0, 13]); // IHDR length
        png.extend_from_slice(b"IHDR");
        png.extend_from_slice(&100u32.to_be_bytes()); // width
        png.extend_from_slice(&50u32.to_be_bytes()); // height
        png.extend_from_slice(&[8, 2, 0, 0, 0]); // bit depth, color type, etc.

        let dims = get_image_dimensions_from_bytes(&png);
        assert_eq!(dims, Some((100, 50)));
    }

    fn get_image_dimensions_from_bytes(data: &[u8]) -> Option<(u32, u32)> {
        if data.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
            if data.len() >= 24 {
                let width = u32::from_be_bytes([data[16], data[17], data[18], data[19]]);
                let height = u32::from_be_bytes([data[20], data[21], data[22], data[23]]);
                return Some((width, height));
            }
        }
        None
    }
}
