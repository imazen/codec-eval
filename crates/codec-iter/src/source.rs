use std::fs;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use imgref::ImgVec;
use rgb::RGB;

pub type Rgb8 = RGB<u8>;

pub struct SourceImage {
    pub name: String,
    pub width: usize,
    pub height: usize,
    pub pixels: ImgVec<Rgb8>,
}

// CID22-512 representative image tiers (from glassa clustering)
const TINY: &[&str] = &["pexels-photo-951408.png", "53435.png", "1963557.png"];

const SMALL: &[&str] = &[
    "pexels-photo-951408.png",
    "53435.png",
    "1963557.png",
    "160577.png",
    "2866385.png",
];

const MEDIUM: &[&str] = &[
    "pexels-photo-951408.png",
    "pexels-photo-3193731.png",
    "pexels-photo-7438498.png",
    "53435.png",
    "pexels-photo-1130297.png",
    "1963557.png",
    "Temperament-pie-chart-according-to-Eysenck.png",
    "160577.png",
    "1277396.png",
    "2866385.png",
    "1583339.png",
    "144200.png",
    "pexels-photo-2908983.png",
    "1183021.png",
    "162511.png",
];

pub fn load_sources(corpus: &Path, limit: usize) -> Result<Vec<SourceImage>> {
    let tier_names: &[&str] = match limit {
        0..=3 => &TINY[..limit.min(TINY.len())],
        4..=5 => &SMALL[..limit.min(SMALL.len())],
        6..=15 => &MEDIUM[..limit.min(MEDIUM.len())],
        _ => &[],
    };

    if tier_names.is_empty() {
        load_all_from_dir(corpus, limit)
    } else {
        load_by_names(corpus, tier_names)
    }
}

fn load_by_names(corpus: &Path, names: &[&str]) -> Result<Vec<SourceImage>> {
    let cache_dir = corpus.join(".codec-iter-cache");
    let mut images = Vec::with_capacity(names.len());

    for name in names {
        let ppm_path = cache_dir
            .join(Path::new(name).file_stem().unwrap())
            .with_extension("ppm");
        let png_path = corpus.join(name);

        let img = if ppm_path.exists() {
            load_ppm(&ppm_path, name)?
        } else if png_path.exists() {
            let img = load_png(&png_path, name)?;
            if let Err(e) = cache_as_ppm(&img, &cache_dir, name) {
                eprintln!("warning: failed to cache PPM for {name}: {e}");
            }
            img
        } else {
            anyhow::bail!(
                "Image not found: {name} (looked in {} and {})",
                png_path.display(),
                ppm_path.display()
            );
        };

        images.push(img);
    }

    Ok(images)
}

fn load_all_from_dir(corpus: &Path, limit: usize) -> Result<Vec<SourceImage>> {
    let mut entries: Vec<_> = fs::read_dir(corpus)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            matches!(
                path.extension().and_then(|s| s.to_str()),
                Some("png" | "ppm")
            )
        })
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if limit > 0 && entries.len() > limit {
        entries.truncate(limit);
    }

    let mut images = Vec::with_capacity(entries.len());
    for entry in &entries {
        let path = entry.path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        let img = match path.extension().and_then(|s| s.to_str()) {
            Some("ppm") => load_ppm(&path, &name)?,
            Some("png") => load_png(&path, &name)?,
            _ => continue,
        };
        images.push(img);
    }

    Ok(images)
}

fn load_png(path: &Path, name: &str) -> Result<SourceImage> {
    let file = fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let decoder = png::Decoder::new(BufReader::new(file));
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("reading PNG header: {}", path.display()))?;

    let info = reader.info();
    let width = info.width as usize;
    let height = info.height as usize;
    let color_type = info.color_type;

    let buf_size = reader
        .output_buffer_size()
        .context("PNG output buffer size unavailable")?;
    let mut buf = vec![0u8; buf_size];
    reader.next_frame(&mut buf)?;

    let pixels: Vec<Rgb8> = match color_type {
        png::ColorType::Rgb => buf[..width * height * 3]
            .chunks_exact(3)
            .map(|c| Rgb8 {
                r: c[0],
                g: c[1],
                b: c[2],
            })
            .collect(),
        png::ColorType::Rgba => buf[..width * height * 4]
            .chunks_exact(4)
            .map(|c| Rgb8 {
                r: c[0],
                g: c[1],
                b: c[2],
            })
            .collect(),
        png::ColorType::Grayscale => buf[..width * height]
            .iter()
            .map(|&g| Rgb8 { r: g, g, b: g })
            .collect(),
        other => anyhow::bail!("Unsupported PNG color type: {other:?} in {name}"),
    };

    Ok(SourceImage {
        name: name.to_string(),
        width,
        height,
        pixels: ImgVec::new(pixels, width, height),
    })
}

fn load_ppm(path: &Path, name: &str) -> Result<SourceImage> {
    let data = fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let img: ImgVec<Rgb8> = zenpnm::decode_img(&data, zenpnm::Unstoppable)
        .map_err(|e| anyhow::anyhow!("PPM decode error for {name}: {e}"))?;

    let width = img.width();
    let height = img.height();

    Ok(SourceImage {
        name: name.to_string(),
        width,
        height,
        pixels: img,
    })
}

fn cache_as_ppm(img: &SourceImage, cache_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(cache_dir)?;
    let ppm_path = cache_dir
        .join(Path::new(name).file_stem().unwrap())
        .with_extension("ppm");
    let ppm_data = zenpnm::encode_ppm_img(img.pixels.as_ref(), zenpnm::Unstoppable)
        .map_err(|e| anyhow::anyhow!("PPM encode error: {e}"))?;
    fs::write(&ppm_path, &ppm_data)?;
    Ok(())
}
