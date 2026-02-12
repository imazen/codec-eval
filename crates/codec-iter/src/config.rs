use zencodecs::config::jpeg::{ChromaSubsampling, EncoderConfig};
use zencodecs::config::CodecConfig;
use zenjpeg::encoder::XybSubsampling;

pub struct JpegConfig {
    pub subsampling: ChromaSubsampling,
    pub xyb: bool,
    pub xyb_subsampling: XybSubsampling,
    pub progressive: bool,
}

impl Default for JpegConfig {
    fn default() -> Self {
        Self {
            subsampling: ChromaSubsampling::Quarter,
            xyb: false,
            xyb_subsampling: XybSubsampling::BQuarter,
            progressive: true,
        }
    }
}

pub fn build_encoder_config(config: &JpegConfig, quality: u8) -> EncoderConfig {
    let enc = if config.xyb {
        EncoderConfig::xyb(quality, config.xyb_subsampling)
    } else {
        EncoderConfig::ycbcr(quality, config.subsampling)
    };

    enc.progressive(config.progressive)
}

pub fn build_codec_config(config: &JpegConfig, quality: u8) -> CodecConfig {
    let encoder = build_encoder_config(config, quality);
    CodecConfig::default().with_jpeg_encoder(encoder)
}

pub fn config_summary(config: &JpegConfig) -> String {
    let color = if config.xyb { "xyb" } else { "ycbcr" };
    let sub = if config.xyb {
        match config.xyb_subsampling {
            XybSubsampling::Full => "full",
            XybSubsampling::BQuarter => "bq",
            _ => "?",
        }
    } else {
        match config.subsampling {
            ChromaSubsampling::None => "444",
            ChromaSubsampling::Quarter => "420",
            ChromaSubsampling::HalfHorizontal => "422",
            ChromaSubsampling::HalfVertical => "440",
            _ => "?",
        }
    };
    let prog = if config.progressive { "prog" } else { "base" };
    format!("zenjpeg-{sub}-{color}-{prog}")
}

pub fn parse_subsampling(s: &str) -> Option<ChromaSubsampling> {
    match s {
        "420" => Some(ChromaSubsampling::Quarter),
        "444" => Some(ChromaSubsampling::None),
        "422" => Some(ChromaSubsampling::HalfHorizontal),
        "440" => Some(ChromaSubsampling::HalfVertical),
        _ => None,
    }
}
