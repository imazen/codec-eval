//! CSV import for third-party encoder results.
//!
//! This module allows importing benchmark results from external sources,
//! enabling cross-codec comparisons without re-running encoders.
//!
//! ## Supported Formats
//!
//! The importer is flexible and can handle various CSV schemas. At minimum,
//! it expects columns for:
//! - Image identifier
//! - Codec name
//! - Quality setting or file size
//! - At least one quality metric
//!
//! ## Example
//!
//! ```rust,ignore
//! use codec_eval::import::{CsvImporter, CsvSchema};
//!
//! let schema = CsvSchema::builder()
//!     .image_column("filename")
//!     .codec_column("encoder")
//!     .quality_column("q")
//!     .size_column("bytes")
//!     .dssim_column("dssim")
//!     .build();
//!
//! let results = CsvImporter::new(schema).import("results.csv")?;
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// An imported result from an external encoder benchmark.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalResult {
    /// Image name or identifier.
    pub image_name: String,

    /// Codec identifier.
    pub codec: String,

    /// Codec version (if available).
    pub codec_version: Option<String>,

    /// Quality setting used.
    pub quality_setting: Option<f64>,

    /// Encoded file size in bytes.
    pub file_size: Option<usize>,

    /// Bits per pixel.
    pub bits_per_pixel: Option<f64>,

    /// SSIMULACRA2 score (if available).
    pub ssimulacra2: Option<f64>,

    /// DSSIM value (if available).
    pub dssim: Option<f64>,

    /// PSNR value (if available).
    pub psnr: Option<f64>,

    /// Butteraugli distance (if available).
    pub butteraugli: Option<f64>,

    /// Encoding time in milliseconds (if available).
    pub encode_time_ms: Option<f64>,

    /// Additional fields.
    #[serde(default)]
    pub extra: HashMap<String, String>,
}

/// Schema for CSV import.
#[derive(Debug, Clone, Default)]
pub struct CsvSchema {
    /// Column name for image identifier.
    pub image_column: Option<String>,
    /// Column name for codec name.
    pub codec_column: Option<String>,
    /// Column name for codec version.
    pub codec_version_column: Option<String>,
    /// Column name for quality setting.
    pub quality_column: Option<String>,
    /// Column name for file size.
    pub size_column: Option<String>,
    /// Column name for bits per pixel.
    pub bpp_column: Option<String>,
    /// Column name for SSIMULACRA2.
    pub ssimulacra2_column: Option<String>,
    /// Column name for DSSIM.
    pub dssim_column: Option<String>,
    /// Column name for PSNR.
    pub psnr_column: Option<String>,
    /// Column name for Butteraugli.
    pub butteraugli_column: Option<String>,
    /// Column name for encode time (ms).
    pub encode_time_column: Option<String>,
}

impl CsvSchema {
    /// Create a schema builder.
    #[must_use]
    pub fn builder() -> CsvSchemaBuilder {
        CsvSchemaBuilder::default()
    }

    /// Create a schema that auto-detects columns from common names.
    #[must_use]
    pub fn auto_detect() -> Self {
        Self::default()
    }

    /// Try to find a column index by name (case-insensitive, with aliases).
    fn find_column(
        &self,
        headers: &[&str],
        primary: Option<&str>,
        aliases: &[&str],
    ) -> Option<usize> {
        // First try the configured column name
        if let Some(name) = primary {
            if let Some(idx) = find_header_index(headers, name) {
                return Some(idx);
            }
        }

        // Then try aliases
        for alias in aliases {
            if let Some(idx) = find_header_index(headers, alias) {
                return Some(idx);
            }
        }

        None
    }
}

/// Builder for CSV schema.
#[derive(Debug, Default)]
pub struct CsvSchemaBuilder {
    schema: CsvSchema,
}

impl CsvSchemaBuilder {
    /// Set the image column name.
    #[must_use]
    pub fn image_column(mut self, name: impl Into<String>) -> Self {
        self.schema.image_column = Some(name.into());
        self
    }

    /// Set the codec column name.
    #[must_use]
    pub fn codec_column(mut self, name: impl Into<String>) -> Self {
        self.schema.codec_column = Some(name.into());
        self
    }

    /// Set the codec version column name.
    #[must_use]
    pub fn codec_version_column(mut self, name: impl Into<String>) -> Self {
        self.schema.codec_version_column = Some(name.into());
        self
    }

    /// Set the quality column name.
    #[must_use]
    pub fn quality_column(mut self, name: impl Into<String>) -> Self {
        self.schema.quality_column = Some(name.into());
        self
    }

    /// Set the file size column name.
    #[must_use]
    pub fn size_column(mut self, name: impl Into<String>) -> Self {
        self.schema.size_column = Some(name.into());
        self
    }

    /// Set the bits per pixel column name.
    #[must_use]
    pub fn bpp_column(mut self, name: impl Into<String>) -> Self {
        self.schema.bpp_column = Some(name.into());
        self
    }

    /// Set the SSIMULACRA2 column name.
    #[must_use]
    pub fn ssimulacra2_column(mut self, name: impl Into<String>) -> Self {
        self.schema.ssimulacra2_column = Some(name.into());
        self
    }

    /// Set the DSSIM column name.
    #[must_use]
    pub fn dssim_column(mut self, name: impl Into<String>) -> Self {
        self.schema.dssim_column = Some(name.into());
        self
    }

    /// Set the PSNR column name.
    #[must_use]
    pub fn psnr_column(mut self, name: impl Into<String>) -> Self {
        self.schema.psnr_column = Some(name.into());
        self
    }

    /// Set the Butteraugli column name.
    #[must_use]
    pub fn butteraugli_column(mut self, name: impl Into<String>) -> Self {
        self.schema.butteraugli_column = Some(name.into());
        self
    }

    /// Set the encode time column name.
    #[must_use]
    pub fn encode_time_column(mut self, name: impl Into<String>) -> Self {
        self.schema.encode_time_column = Some(name.into());
        self
    }

    /// Build the schema.
    #[must_use]
    pub fn build(self) -> CsvSchema {
        self.schema
    }
}

/// CSV importer for external results.
pub struct CsvImporter {
    schema: CsvSchema,
}

impl CsvImporter {
    /// Create a new importer with the given schema.
    #[must_use]
    pub fn new(schema: CsvSchema) -> Self {
        Self { schema }
    }

    /// Create an importer that auto-detects columns.
    #[must_use]
    pub fn auto_detect() -> Self {
        Self::new(CsvSchema::auto_detect())
    }

    /// Import results from a CSV file.
    pub fn import(&self, path: impl AsRef<Path>) -> Result<Vec<ExternalResult>> {
        let path = path.as_ref();
        let mut reader = csv::Reader::from_path(path)?;

        let headers: Vec<String> = reader.headers()?.iter().map(String::from).collect();
        let header_refs: Vec<&str> = headers.iter().map(String::as_str).collect();

        // Find column indices
        let image_idx = self.schema.find_column(
            &header_refs,
            self.schema.image_column.as_deref(),
            &["image", "filename", "file", "name", "source", "input"],
        );

        let codec_idx = self.schema.find_column(
            &header_refs,
            self.schema.codec_column.as_deref(),
            &["codec", "encoder", "format", "method"],
        );

        let version_idx = self.schema.find_column(
            &header_refs,
            self.schema.codec_version_column.as_deref(),
            &["version", "codec_version", "encoder_version"],
        );

        let quality_idx = self.schema.find_column(
            &header_refs,
            self.schema.quality_column.as_deref(),
            &["quality", "q", "qp", "crf", "effort"],
        );

        let size_idx = self.schema.find_column(
            &header_refs,
            self.schema.size_column.as_deref(),
            &["size", "file_size", "bytes", "filesize"],
        );

        let bpp_idx = self.schema.find_column(
            &header_refs,
            self.schema.bpp_column.as_deref(),
            &["bpp", "bits_per_pixel", "bitrate"],
        );

        let ssimulacra2_idx = self.schema.find_column(
            &header_refs,
            self.schema.ssimulacra2_column.as_deref(),
            &["ssimulacra2", "ssim2", "ssimulacra_2"],
        );

        let dssim_idx = self.schema.find_column(
            &header_refs,
            self.schema.dssim_column.as_deref(),
            &["dssim", "ssim", "ms_ssim", "ms-ssim"],
        );

        let psnr_idx = self.schema.find_column(
            &header_refs,
            self.schema.psnr_column.as_deref(),
            &["psnr", "psnr_db", "psnr-hvs"],
        );

        let butteraugli_idx = self.schema.find_column(
            &header_refs,
            self.schema.butteraugli_column.as_deref(),
            &["butteraugli", "butter", "ba"],
        );

        let encode_time_idx = self.schema.find_column(
            &header_refs,
            self.schema.encode_time_column.as_deref(),
            &["encode_time", "encode_ms", "time_ms", "encoding_time"],
        );

        // Check we have at least image and codec columns
        let image_idx = image_idx.ok_or_else(|| Error::CsvImport {
            line: 0,
            reason: "Could not find image/filename column".to_string(),
        })?;

        let codec_idx = codec_idx.ok_or_else(|| Error::CsvImport {
            line: 0,
            reason: "Could not find codec/encoder column".to_string(),
        })?;

        let mut results = Vec::new();

        for (line_num, record) in reader.records().enumerate() {
            let record = record.map_err(|e| Error::CsvImport {
                line: line_num + 2, // +2 for 1-based and header
                reason: e.to_string(),
            })?;

            let image_name = record.get(image_idx).unwrap_or("").to_string();
            let codec = record.get(codec_idx).unwrap_or("").to_string();

            if image_name.is_empty() || codec.is_empty() {
                continue;
            }

            let result = ExternalResult {
                image_name,
                codec,
                codec_version: version_idx.and_then(|i| record.get(i)).map(String::from),
                quality_setting: quality_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                file_size: size_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                bits_per_pixel: bpp_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                ssimulacra2: ssimulacra2_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                dssim: dssim_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                psnr: psnr_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                butteraugli: butteraugli_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                encode_time_ms: encode_time_idx
                    .and_then(|i| record.get(i))
                    .and_then(|s| s.parse().ok()),
                extra: HashMap::new(),
            };

            results.push(result);
        }

        Ok(results)
    }
}

/// Find a header index by name (case-insensitive).
fn find_header_index(headers: &[&str], name: &str) -> Option<usize> {
    let name_lower = name.to_lowercase();
    headers.iter().position(|h| h.to_lowercase() == name_lower)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_builder() {
        let schema = CsvSchema::builder()
            .image_column("img")
            .codec_column("enc")
            .quality_column("q")
            .build();

        assert_eq!(schema.image_column, Some("img".to_string()));
        assert_eq!(schema.codec_column, Some("enc".to_string()));
        assert_eq!(schema.quality_column, Some("q".to_string()));
    }

    #[test]
    fn test_find_header_index() {
        let headers = ["Image", "Codec", "Quality", "DSSIM"];
        assert_eq!(find_header_index(&headers, "image"), Some(0));
        assert_eq!(find_header_index(&headers, "QUALITY"), Some(2));
        assert_eq!(find_header_index(&headers, "unknown"), None);
    }
}
