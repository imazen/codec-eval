//! SVG chart generation for rate-distortion analysis.
//!
//! Generates Pareto front plots comparing codecs across quality metrics.
//! All charts support light and dark mode via CSS media queries.

use std::fmt::Write as _;

/// Data point for a chart series.
#[derive(Debug, Clone)]
pub struct ChartPoint {
    /// X-axis value (typically BPP or file size).
    pub x: f64,
    /// Y-axis value (typically quality metric).
    pub y: f64,
    /// Optional label (e.g., quality level).
    pub label: Option<String>,
}

/// A series of data points with styling.
#[derive(Debug, Clone)]
pub struct ChartSeries {
    /// Series identifier (used in legend).
    pub name: String,
    /// CSS color for the series.
    pub color: String,
    /// Data points sorted by X.
    pub points: Vec<ChartPoint>,
}

/// Chart configuration.
#[derive(Debug, Clone)]
pub struct ChartConfig {
    /// Chart title.
    pub title: String,
    /// X-axis label.
    pub x_label: String,
    /// Y-axis label.
    pub y_label: String,
    /// Whether lower Y values are better (affects axis direction).
    pub lower_is_better: bool,
    /// Chart width in pixels.
    pub width: u32,
    /// Chart height in pixels.
    pub height: u32,
}

impl Default for ChartConfig {
    fn default() -> Self {
        Self {
            title: "Quality vs Size".to_string(),
            x_label: "Bits per Pixel (BPP) →".to_string(),
            y_label: "Quality Score".to_string(),
            lower_is_better: false,
            width: 700,
            height: 450,
        }
    }
}

impl ChartConfig {
    /// Creates a new chart configuration with the given title.
    #[must_use]
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            ..Default::default()
        }
    }

    /// Sets the X-axis label.
    #[must_use]
    pub fn with_x_label(mut self, label: impl Into<String>) -> Self {
        self.x_label = label.into();
        self
    }

    /// Sets the Y-axis label.
    #[must_use]
    pub fn with_y_label(mut self, label: impl Into<String>) -> Self {
        self.y_label = label.into();
        self
    }

    /// Sets whether lower Y values are better.
    #[must_use]
    pub fn with_lower_is_better(mut self, lower_is_better: bool) -> Self {
        self.lower_is_better = lower_is_better;
        self
    }

    /// Sets the chart dimensions.
    #[must_use]
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.width = width;
        self.height = height;
        self
    }
}

/// Generates an SVG chart from the given series.
///
/// # Example
///
/// ```rust
/// use codec_eval::stats::chart::{generate_svg, ChartConfig, ChartSeries, ChartPoint};
///
/// let series = vec![
///     ChartSeries {
///         name: "Codec A".to_string(),
///         color: "#e74c3c".to_string(),
///         points: vec![
///             ChartPoint { x: 0.5, y: 80.0, label: None },
///             ChartPoint { x: 1.0, y: 90.0, label: None },
///         ],
///     },
/// ];
///
/// let config = ChartConfig::new("Quality vs Size")
///     .with_x_label("BPP →")
///     .with_y_label("← SSIMULACRA2");
///
/// let svg = generate_svg(&series, &config);
/// // svg contains valid SVG content
/// ```
#[must_use]
pub fn generate_svg(series: &[ChartSeries], config: &ChartConfig) -> String {
    let mut svg = String::with_capacity(8192);

    // Filter empty series and find bounds
    let non_empty: Vec<_> = series.iter().filter(|s| !s.points.is_empty()).collect();
    if non_empty.is_empty() {
        return String::new();
    }

    let all_x: Vec<f64> = non_empty
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p.x))
        .collect();
    let all_y: Vec<f64> = non_empty
        .iter()
        .flat_map(|s| s.points.iter().map(|p| p.y))
        .collect();

    let (min_x, max_x) = bounds_with_padding(&all_x, 0.05);
    let (min_y, max_y) = bounds_with_padding(&all_y, 0.05);

    let width = config.width;
    let height = config.height;
    let margin_top = 50;
    let margin_right = 140;
    let margin_bottom = 70;
    let margin_left = 90;
    let plot_width = width - margin_left - margin_right;
    let plot_height = height - margin_top - margin_bottom;

    let scale_x = |v: f64| -> f64 {
        f64::from(margin_left) + (v - min_x) / (max_x - min_x) * f64::from(plot_width)
    };

    let scale_y = |v: f64| -> f64 {
        if config.lower_is_better {
            f64::from(margin_top) + (v - min_y) / (max_y - min_y) * f64::from(plot_height)
        } else {
            f64::from(margin_top) + (1.0 - (v - min_y) / (max_y - min_y)) * f64::from(plot_height)
        }
    };

    // SVG header
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {} {}">"#,
        width, height
    );

    // CSS with dark mode support
    svg.push_str(
        r#"<style>
  :root {
    --bg-color: #ffffff;
    --text-color: #1a1a1a;
    --grid-color: #e0e0e0;
    --axis-color: #333333;
    --legend-bg: #ffffff;
    --legend-border: #cccccc;
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg-color: #1a1a1a;
      --text-color: #e0e0e0;
      --grid-color: #404040;
      --axis-color: #b0b0b0;
      --legend-bg: #2a2a2a;
      --legend-border: #505050;
    }
  }
  .background { fill: var(--bg-color); }
  .title { font: bold 18px system-ui, sans-serif; fill: var(--text-color); }
  .axis-label { font: 13px system-ui, sans-serif; fill: var(--text-color); }
  .tick-label { font: 11px system-ui, sans-serif; fill: var(--text-color); }
  .legend { font: 13px system-ui, sans-serif; fill: var(--text-color); }
  .grid { stroke: var(--grid-color); stroke-width: 1; }
  .axis { stroke: var(--axis-color); stroke-width: 1.5; }
  .legend-bg { fill: var(--legend-bg); stroke: var(--legend-border); }
</style>
"#,
    );

    // Background
    let _ = writeln!(
        svg,
        r#"<rect class="background" width="{}" height="{}"/>"#,
        width, height
    );

    // Title
    let _ = writeln!(
        svg,
        r#"<text x="{}" y="30" text-anchor="middle" class="title">{}</text>"#,
        f64::from(width) / 2.0,
        config.title
    );

    // Grid lines
    for i in 0..=5 {
        let frac = f64::from(i) / 5.0;
        let x = scale_x(min_x + frac * (max_x - min_x));
        let y = scale_y(min_y + frac * (max_y - min_y));

        let _ = writeln!(
            svg,
            r#"<line x1="{:.2}" y1="{}" x2="{:.2}" y2="{}" class="grid"/>"#,
            x,
            margin_top,
            x,
            height - margin_bottom
        );
        let _ = writeln!(
            svg,
            r#"<line x1="{}" y1="{:.2}" x2="{}" y2="{:.2}" class="grid"/>"#,
            margin_left,
            y,
            width - margin_right,
            y
        );
    }

    // Axes
    let _ = writeln!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" class="axis"/>"#,
        margin_left,
        height - margin_bottom,
        width - margin_right,
        height - margin_bottom
    );
    let _ = writeln!(
        svg,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" class="axis"/>"#,
        margin_left,
        margin_top,
        margin_left,
        height - margin_bottom
    );

    // Tick labels
    for i in 0..=5 {
        let frac = f64::from(i) / 5.0;
        let x_val = min_x + frac * (max_x - min_x);
        let y_val = min_y + frac * (max_y - min_y);
        let x = scale_x(x_val);
        let y = scale_y(y_val);

        let _ = writeln!(
            svg,
            r#"<text x="{:.2}" y="{}" text-anchor="middle" class="tick-label">{:.2}</text>"#,
            x,
            height - margin_bottom + 20,
            x_val
        );

        // Format Y label based on magnitude
        let y_label = if y_val.abs() < 0.0001 {
            format!("{:.6}", y_val)
        } else if y_val.abs() < 0.1 {
            format!("{:.4}", y_val)
        } else {
            format!("{:.2}", y_val)
        };
        let _ = writeln!(
            svg,
            r#"<text x="{}" y="{:.2}" text-anchor="end" class="tick-label">{}</text>"#,
            margin_left - 10,
            y + 4.0,
            y_label
        );
    }

    // X axis label
    let _ = writeln!(
        svg,
        r#"<text x="{}" y="{}" text-anchor="middle" class="axis-label">{}</text>"#,
        f64::from(width) / 2.0,
        height - 20,
        config.x_label
    );

    // Y axis label (rotated)
    let _ = writeln!(
        svg,
        r#"<text x="25" y="{}" text-anchor="middle" class="axis-label" transform="rotate(-90 25 {})">{}</text>"#,
        f64::from(height) / 2.0,
        f64::from(height) / 2.0,
        config.y_label
    );

    // Plot series
    for s in &non_empty {
        if s.points.is_empty() {
            continue;
        }

        // Line
        let mut path = String::new();
        for (i, p) in s.points.iter().enumerate() {
            let prefix = if i == 0 { "M" } else { " L" };
            let _ = write!(path, "{} {:.2},{:.2}", prefix, scale_x(p.x), scale_y(p.y));
        }
        let _ = writeln!(
            svg,
            r#"<path d="{}" stroke="{}" stroke-width="2.5" fill="none"/>"#,
            path, s.color
        );

        // Points
        for p in &s.points {
            let _ = writeln!(
                svg,
                r#"<circle cx="{:.2}" cy="{:.2}" r="5" fill="{}"/>"#,
                scale_x(p.x),
                scale_y(p.y),
                s.color
            );
        }
    }

    // Legend
    let legend_x = width - margin_right + 15;
    let legend_y = margin_top + 20;
    let legend_height = 20 + non_empty.len() as u32 * 25;

    let _ = writeln!(
        svg,
        r#"<rect x="{}" y="{}" width="115" height="{}" rx="4" class="legend-bg"/>"#,
        legend_x,
        legend_y - 15,
        legend_height
    );

    for (i, s) in non_empty.iter().enumerate() {
        let y_offset = legend_y + i as u32 * 25;
        let _ = writeln!(
            svg,
            r#"<circle cx="{}" cy="{}" r="5" fill="{}"/>"#,
            legend_x + 15,
            y_offset + 5,
            s.color
        );
        let _ = writeln!(
            svg,
            r#"<text x="{}" y="{}" class="legend">{}</text>"#,
            legend_x + 28,
            y_offset + 9,
            s.name
        );
    }

    svg.push_str("</svg>\n");
    svg
}

/// Calculates min/max bounds with padding.
fn bounds_with_padding(values: &[f64], padding: f64) -> (f64, f64) {
    let min = values.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let range = max - min;
    (min - range * padding, max + range * padding)
}

/// Standard color palette for codec comparison.
pub mod colors {
    /// Red - typically for the primary/new implementation.
    pub const RED: &str = "#e74c3c";
    /// Blue - typically for the reference implementation.
    pub const BLUE: &str = "#3498db";
    /// Green - for a third codec.
    pub const GREEN: &str = "#27ae60";
    /// Orange - for a fourth codec.
    pub const ORANGE: &str = "#e67e22";
    /// Purple - for a fifth codec.
    pub const PURPLE: &str = "#9b59b6";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_svg_basic() {
        let series = vec![ChartSeries {
            name: "Test".to_string(),
            color: colors::RED.to_string(),
            points: vec![
                ChartPoint {
                    x: 0.5,
                    y: 80.0,
                    label: None,
                },
                ChartPoint {
                    x: 1.0,
                    y: 90.0,
                    label: None,
                },
            ],
        }];

        let config = ChartConfig::new("Test Chart");
        let svg = generate_svg(&series, &config);

        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
        assert!(svg.contains("Test Chart"));
        assert!(svg.contains("Test")); // legend
    }

    #[test]
    fn test_empty_series() {
        let series: Vec<ChartSeries> = vec![];
        let config = ChartConfig::default();
        let svg = generate_svg(&series, &config);
        assert!(svg.is_empty());
    }

    #[test]
    fn test_lower_is_better() {
        let series = vec![ChartSeries {
            name: "Test".to_string(),
            color: colors::BLUE.to_string(),
            points: vec![
                ChartPoint {
                    x: 0.5,
                    y: 0.01,
                    label: None,
                },
                ChartPoint {
                    x: 1.0,
                    y: 0.005,
                    label: None,
                },
            ],
        }];

        let config = ChartConfig::new("DSSIM Chart").with_lower_is_better(true);
        let svg = generate_svg(&series, &config);

        assert!(svg.contains("<svg"));
    }
}
