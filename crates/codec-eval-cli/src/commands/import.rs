//! CSV import command.

use std::path::PathBuf;

use anyhow::{Context, Result};
use codec_eval::import::{CsvImporter, CsvSchema};

pub fn run(
    input: PathBuf,
    output: Option<PathBuf>,
    image_col: Option<String>,
    codec_col: Option<String>,
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Importing from: {}", input.display());
    }

    // Build schema
    let mut schema = CsvSchema::builder();
    if let Some(col) = image_col {
        schema = schema.image_column(col);
    }
    if let Some(col) = codec_col {
        schema = schema.codec_column(col);
    }

    let importer = CsvImporter::new(schema.build());
    let results = importer
        .import(&input)
        .with_context(|| format!("Failed to import CSV from {}", input.display()))?;

    println!("Imported {} results", results.len());

    // Count by codec
    let mut by_codec: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for result in &results {
        *by_codec.entry(&result.codec).or_default() += 1;
    }

    println!("Codecs:");
    let mut sorted: Vec<_> = by_codec.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1));
    for (codec, count) in sorted {
        println!("  {}: {}", codec, count);
    }

    // Count by image
    let mut images: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for result in &results {
        images.insert(&result.image_name);
    }
    println!("Unique images: {}", images.len());

    // Count metrics available
    let has_dssim = results.iter().filter(|r| r.dssim.is_some()).count();
    let has_ssimulacra2 = results.iter().filter(|r| r.ssimulacra2.is_some()).count();
    let has_psnr = results.iter().filter(|r| r.psnr.is_some()).count();

    println!("Metrics:");
    if has_dssim > 0 {
        println!("  DSSIM: {} results", has_dssim);
    }
    if has_ssimulacra2 > 0 {
        println!("  SSIMULACRA2: {} results", has_ssimulacra2);
    }
    if has_psnr > 0 {
        println!("  PSNR: {} results", has_psnr);
    }

    if let Some(output_path) = output {
        let json = serde_json::to_string_pretty(&results)?;
        std::fs::write(&output_path, json)
            .with_context(|| format!("Failed to write to {}", output_path.display()))?;
        println!("Saved to: {}", output_path.display());
    }

    Ok(())
}
