//! Build and evaluate encoder prediction models
//!
//! Reads comparison results and heuristics, determines which encoder wins
//! at each quality/bpp level, and evaluates prediction heuristics.

use anyhow::Result;
use clap::Parser;
use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "build-predictor")]
#[command(about = "Build and evaluate encoder prediction models")]
struct Args {
    /// Comparison CSV file
    #[arg(short, long)]
    comparison: PathBuf,

    /// Heuristics CSV file
    #[arg(short = 'u', long)]
    heuristics: PathBuf,

    /// Output predictions CSV
    #[arg(short, long, default_value = "predictions.csv")]
    output: PathBuf,
}

#[derive(Debug, Clone)]
struct ComparisonRow {
    image: String,
    encoder: String,
    quality: u8,
    bpp: f64,
    butteraugli: f64,
    dssim: f64,
    ssimulacra2: f64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct HeuristicRow {
    image: String,
    flat_block_pct: f64,
    edge_strength_mean: f64,
    edge_density: f64,
    detail_block_pct: f64,
    block_variance_mean: f64,
    high_freq_energy: f64,
    freq_ratio: f64,
    local_contrast_mean: f64,
    saturation_mean: f64,
    luminance_std: f64,
}

fn parse_comparison_csv(path: &PathBuf) -> Result<Vec<ComparisonRow>> {
    let mut rows = Vec::new();
    let mut rdr = csv::Reader::from_path(path)?;

    for result in rdr.records() {
        let record = result?;
        rows.push(ComparisonRow {
            image: record[0].to_string(),
            encoder: record[1].to_string(),
            quality: record[2].parse()?,
            bpp: record[6].parse()?,
            butteraugli: record[7].parse().unwrap_or(f64::NAN),
            dssim: record[8].parse().unwrap_or(f64::NAN),
            ssimulacra2: record[9].parse().unwrap_or(f64::NAN),
        });
    }
    Ok(rows)
}

fn parse_heuristics_csv(path: &PathBuf) -> Result<HashMap<String, HeuristicRow>> {
    let mut map = HashMap::new();
    let mut rdr = csv::Reader::from_path(path)?;

    for result in rdr.records() {
        let record = result?;
        let image = record[0].to_string();
        map.insert(
            image.clone(),
            HeuristicRow {
                image,
                flat_block_pct: record[10].parse().unwrap_or(0.0),
                edge_strength_mean: record[7].parse().unwrap_or(0.0),
                edge_density: record[9].parse().unwrap_or(0.0),
                detail_block_pct: record[14].parse().unwrap_or(0.0),
                block_variance_mean: record[15].parse().unwrap_or(0.0),
                high_freq_energy: record[20].parse().unwrap_or(0.0),
                freq_ratio: record[22].parse().unwrap_or(0.0),
                local_contrast_mean: record[23].parse().unwrap_or(0.0),
                saturation_mean: record[18].parse().unwrap_or(0.0),
                luminance_std: record[6].parse().unwrap_or(0.0),
            },
        );
    }
    Ok(map)
}

/// For each image, compute RD curves and find which encoder is better at each BPP level
/// Returns: (image, bpp_bucket) -> (winner, margin)
fn determine_winners_bpp_based(
    comparisons: &[ComparisonRow],
) -> HashMap<(String, u8), (String, f64)> {
    let mut winners = HashMap::new();

    // Group by image
    let mut by_image: HashMap<String, Vec<&ComparisonRow>> = HashMap::new();
    for row in comparisons {
        by_image.entry(row.image.clone()).or_default().push(row);
    }

    // BPP buckets: 0.2, 0.4, 0.6, 0.8, 1.0, 1.5, 2.0, 3.0
    let bpp_targets = [0.2, 0.4, 0.6, 0.8, 1.0, 1.5, 2.0, 3.0];

    for (image, rows) in by_image {
        // Separate by encoder and sort by bpp
        let mut moz: Vec<_> = rows.iter().filter(|r| r.encoder == "mozjpeg").collect();
        let mut jpegli: Vec<_> = rows.iter().filter(|r| r.encoder == "jpegli").collect();

        moz.sort_by(|a, b| a.bpp.partial_cmp(&b.bpp).unwrap());
        jpegli.sort_by(|a, b| a.bpp.partial_cmp(&b.bpp).unwrap());

        for (i, &target_bpp) in bpp_targets.iter().enumerate() {
            // Find butteraugli at target BPP via linear interpolation
            let moz_butteraugli = interpolate_butteraugli(&moz, target_bpp);
            let jpegli_butteraugli = interpolate_butteraugli(&jpegli, target_bpp);

            if let (Some(m_ba), Some(j_ba)) = (moz_butteraugli, jpegli_butteraugli) {
                // Lower butteraugli is better
                let (winner, margin) = if m_ba < j_ba {
                    ("mozjpeg".to_string(), (j_ba - m_ba) / j_ba)
                } else {
                    ("jpegli".to_string(), (m_ba - j_ba) / m_ba)
                };
                // Only count as a win if margin is significant (>5%)
                if margin > 0.05 {
                    winners.insert((image.clone(), i as u8), (winner, margin));
                }
            }
        }
    }

    winners
}

/// Interpolate butteraugli score at target BPP
fn interpolate_butteraugli(rows: &[&&ComparisonRow], target_bpp: f64) -> Option<f64> {
    if rows.is_empty() {
        return None;
    }

    // Find the two points bracketing target_bpp
    let mut below: Option<&&ComparisonRow> = None;
    let mut above: Option<&&ComparisonRow> = None;

    for row in rows {
        if row.bpp <= target_bpp {
            below = Some(row);
        }
        if row.bpp >= target_bpp && above.is_none() {
            above = Some(row);
        }
    }

    match (below, above) {
        (Some(b), Some(a)) if b.bpp == a.bpp => Some(b.butteraugli),
        (Some(b), Some(a)) => {
            // Linear interpolation
            let t = (target_bpp - b.bpp) / (a.bpp - b.bpp);
            Some(b.butteraugli + t * (a.butteraugli - b.butteraugli))
        }
        (Some(b), None) => Some(b.butteraugli), // Extrapolate from below
        (None, Some(a)) => Some(a.butteraugli), // Extrapolate from above
        (None, None) => None,
    }
}

/// BPP-bucket index to approximate BPP
fn bpp_bucket_to_value(bucket: u8) -> f64 {
    let targets = [0.2, 0.4, 0.6, 0.8, 1.0, 1.5, 2.0, 3.0];
    targets.get(bucket as usize).copied().unwrap_or(1.0)
}

/// Prediction rule type - now takes BPP instead of quality
#[derive(Clone)]
struct PredictionRule {
    name: String,
    predict: fn(&HeuristicRow, f64) -> &'static str,
}

fn rule_flat_based(h: &HeuristicRow, bpp: f64) -> &'static str {
    if h.flat_block_pct > 70.0 && bpp < 0.8 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_edge_based(h: &HeuristicRow, bpp: f64) -> &'static str {
    if h.edge_strength_mean > 15.0 {
        "jpegli"
    } else if bpp < 0.6 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_detail_based(h: &HeuristicRow, bpp: f64) -> &'static str {
    if h.detail_block_pct > 5.0 {
        "jpegli"
    } else if h.flat_block_pct > 60.0 && bpp < 0.8 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_freq_based(h: &HeuristicRow, bpp: f64) -> &'static str {
    if h.freq_ratio > 0.1 {
        "jpegli"
    } else if bpp < 0.6 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v1(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Compute a score: higher = more likely jpegli wins
    let jpegli_score = h.edge_strength_mean / 10.0
        + h.detail_block_pct / 5.0
        + h.freq_ratio * 5.0
        + h.local_contrast_mean / 10.0
        - h.flat_block_pct / 40.0;

    // BPP-dependent threshold
    let threshold = if bpp < 0.4 {
        3.0
    } else if bpp < 0.8 {
        1.5
    } else {
        0.0
    };

    if jpegli_score > threshold {
        "jpegli"
    } else {
        "mozjpeg"
    }
}

fn rule_combined_v2(h: &HeuristicRow, bpp: f64) -> &'static str {
    // More sophisticated combination
    let complexity = h.edge_strength_mean + h.local_contrast_mean;
    let uniformity = h.flat_block_pct;

    if complexity > 35.0 {
        // High complexity images favor jpegli
        "jpegli"
    } else if uniformity > 75.0 && complexity < 25.0 && bpp < 0.8 {
        // Flat images with low-moderate complexity favor mozjpeg up to 0.8 bpp
        "mozjpeg"
    } else if uniformity > 65.0 && complexity < 20.0 && bpp < 0.5 {
        // Moderately flat at very low bpp favor mozjpeg
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v12(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Fine-tuned thresholds based on error analysis
    let complexity = h.edge_strength_mean + h.local_contrast_mean;
    let uniformity = h.flat_block_pct;

    // At 0.2 bpp, even flat images often favor jpegli
    if bpp < 0.3 {
        if uniformity > 85.0 && complexity < 15.0 {
            "mozjpeg"
        } else {
            "jpegli"
        }
    } else if bpp < 0.5 {
        // 0.3-0.5 bpp: mozjpeg for flat, low-complexity
        if uniformity > 70.0 && complexity < 25.0 {
            "mozjpeg"
        } else {
            "jpegli"
        }
    } else if bpp < 0.8 {
        // 0.5-0.8 bpp: mozjpeg only for very flat, very low complexity
        if uniformity > 75.0 && complexity < 20.0 {
            "mozjpeg"
        } else {
            "jpegli"
        }
    } else {
        // Above 0.8 bpp: usually jpegli wins
        "jpegli"
    }
}

fn rule_combined_v5(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Focused on the very_flat_low_bpp category
    let uniformity = h.flat_block_pct;

    // Very flat images at low BPP - this is where mozjpeg wins
    if uniformity > 80.0 && bpp < 0.5 {
        "mozjpeg"
    } else if uniformity > 90.0 && bpp < 0.6 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v6(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Use edge strength as primary discriminator
    // Low edge images at low bpp favor mozjpeg
    if h.edge_strength_mean < 5.0 && bpp < 0.5 {
        "mozjpeg"
    } else if h.edge_strength_mean < 8.0 && bpp < 0.4 && h.flat_block_pct > 75.0 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v7(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Local contrast based
    if h.local_contrast_mean < 8.0 && bpp < 0.5 && h.flat_block_pct > 80.0 {
        "mozjpeg"
    } else if h.local_contrast_mean < 12.0 && bpp < 0.4 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v8(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Combination of local contrast and edge strength
    let texture_score = h.edge_strength_mean + h.local_contrast_mean;

    if texture_score < 15.0 && bpp < 0.5 {
        "mozjpeg"
    } else if texture_score < 20.0 && bpp < 0.4 && h.flat_block_pct > 75.0 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v9(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Based on error analysis: flat images with low edge/contrast favor mozjpeg
    // at various BPP levels depending on flatness
    let is_flat = h.flat_block_pct > 60.0;
    let is_very_flat = h.flat_block_pct > 80.0;
    let low_texture = h.edge_strength_mean < 15.0 && h.local_contrast_mean < 20.0;
    let very_low_texture = h.edge_strength_mean < 10.0 && h.local_contrast_mean < 15.0;

    if is_very_flat && very_low_texture {
        // Very flat images: mozjpeg wins up to moderate bpp
        if bpp < 0.8 {
            "mozjpeg"
        } else {
            "jpegli"
        }
    } else if is_flat && low_texture && bpp < 0.5 {
        // Flat images: mozjpeg wins only at low bpp
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v10(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Refined approach: use edge+contrast sum as texture score
    let texture = h.edge_strength_mean + h.local_contrast_mean;

    // BPP threshold depends on texture level
    let mozjpeg_bpp_threshold = if texture < 15.0 {
        1.0  // Very low texture: mozjpeg good up to 1.0 bpp
    } else if texture < 25.0 {
        0.6  // Low texture: mozjpeg good up to 0.6 bpp
    } else if texture < 35.0 {
        0.4  // Medium texture: mozjpeg only at 0.4 bpp
    } else {
        0.0  // High texture: always jpegli
    };

    if bpp < mozjpeg_bpp_threshold && h.flat_block_pct > 50.0 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v11(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Score-based with continuous variables
    // Higher score = prefer mozjpeg
    let mut moz_preference = 0.0;

    // Flatness contributes positively
    moz_preference += (h.flat_block_pct - 50.0).max(0.0) / 50.0;

    // Low edge contributes positively
    moz_preference += (20.0 - h.edge_strength_mean).max(0.0) / 20.0;

    // Low contrast contributes positively
    moz_preference += (25.0 - h.local_contrast_mean).max(0.0) / 25.0;

    // Low BPP contributes positively
    moz_preference += (0.8 - bpp).max(0.0) / 0.8;

    // High freq ratio penalizes mozjpeg
    moz_preference -= h.freq_ratio.min(0.5) * 2.0;

    if moz_preference > 1.5 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_combined_v3(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Use all available heuristics
    let complexity = h.edge_strength_mean + h.local_contrast_mean + h.luminance_std;
    let uniformity = h.flat_block_pct;
    let texture = h.detail_block_pct + h.high_freq_energy * 100.0;

    // Strong complexity indicators favor jpegli
    if complexity > 40.0 || texture > 10.0 {
        return "jpegli";
    }

    // Very flat images with low frequency content favor mozjpeg at low bpp
    if uniformity > 85.0 && h.freq_ratio < 0.02 && bpp < 0.5 {
        return "mozjpeg";
    }

    // Flat images at low bpp favor mozjpeg
    if uniformity > 75.0 && bpp < 0.4 {
        return "mozjpeg";
    }

    "jpegli"
}

fn rule_combined_v4(h: &HeuristicRow, bpp: f64) -> &'static str {
    // Weighted score approach
    let mut moz_score = 0.0;
    let mut jpegli_score = 0.0;

    // Flat blocks favor mozjpeg at low bpp
    if h.flat_block_pct > 70.0 {
        moz_score += (h.flat_block_pct - 70.0) / 30.0;
    }

    // Detail favors jpegli
    if h.detail_block_pct > 1.0 {
        jpegli_score += h.detail_block_pct / 5.0;
    }

    // High edge strength favors jpegli
    if h.edge_strength_mean > 10.0 {
        jpegli_score += (h.edge_strength_mean - 10.0) / 20.0;
    }

    // High local contrast favors jpegli
    if h.local_contrast_mean > 10.0 {
        jpegli_score += (h.local_contrast_mean - 10.0) / 20.0;
    }

    // High freq ratio favors jpegli
    if h.freq_ratio > 0.05 {
        jpegli_score += h.freq_ratio * 5.0;
    }

    // Low bpp favors mozjpeg
    if bpp < 0.6 {
        moz_score += (0.6 - bpp) * 2.0;
    }

    if moz_score > jpegli_score {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn rule_always_jpegli(_h: &HeuristicRow, _bpp: f64) -> &'static str {
    "jpegli"
}

fn rule_bpp_only(_h: &HeuristicRow, bpp: f64) -> &'static str {
    if bpp < 0.5 {
        "mozjpeg"
    } else {
        "jpegli"
    }
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("=== Encoder Prediction Model Builder (BPP-based) ===\n");

    // Load data
    let comparisons = parse_comparison_csv(&args.comparison)?;
    println!("Loaded {} comparison rows", comparisons.len());

    let heuristics = parse_heuristics_csv(&args.heuristics)?;
    println!("Loaded {} heuristic rows", heuristics.len());

    // Determine actual winners at each BPP level
    let winners = determine_winners_bpp_based(&comparisons);
    println!("Determined {} winner decisions\n", winners.len());

    // Count overall wins
    let moz_total = winners.values().filter(|(w, _)| w == "mozjpeg").count();
    let jpegli_total = winners.values().filter(|(w, _)| w == "jpegli").count();
    println!(
        "Overall wins: mozjpeg={}, jpegli={} ({:.1}% jpegli)\n",
        moz_total,
        jpegli_total,
        100.0 * jpegli_total as f64 / (moz_total + jpegli_total).max(1) as f64
    );

    // Define prediction rules
    let rules: Vec<PredictionRule> = vec![
        PredictionRule { name: "always_jpegli".to_string(), predict: rule_always_jpegli },
        PredictionRule { name: "bpp_only".to_string(), predict: rule_bpp_only },
        PredictionRule { name: "flat_based".to_string(), predict: rule_flat_based },
        PredictionRule { name: "edge_based".to_string(), predict: rule_edge_based },
        PredictionRule { name: "detail_based".to_string(), predict: rule_detail_based },
        PredictionRule { name: "freq_based".to_string(), predict: rule_freq_based },
        PredictionRule { name: "combined_v1".to_string(), predict: rule_combined_v1 },
        PredictionRule { name: "combined_v2".to_string(), predict: rule_combined_v2 },
        PredictionRule { name: "combined_v3".to_string(), predict: rule_combined_v3 },
        PredictionRule { name: "combined_v4".to_string(), predict: rule_combined_v4 },
        PredictionRule { name: "combined_v5".to_string(), predict: rule_combined_v5 },
        PredictionRule { name: "combined_v6".to_string(), predict: rule_combined_v6 },
        PredictionRule { name: "combined_v7".to_string(), predict: rule_combined_v7 },
        PredictionRule { name: "combined_v8".to_string(), predict: rule_combined_v8 },
        PredictionRule { name: "combined_v9".to_string(), predict: rule_combined_v9 },
        PredictionRule { name: "combined_v10".to_string(), predict: rule_combined_v10 },
        PredictionRule { name: "combined_v11".to_string(), predict: rule_combined_v11 },
        PredictionRule { name: "combined_v12".to_string(), predict: rule_combined_v12 },
    ];

    // Evaluate each rule
    println!("{:>20} | {:>10} | {:>10} | {:>10}", "Rule", "Correct", "Total", "Accuracy");
    println!("{}", "-".repeat(60));

    let mut best_rule = String::new();
    let mut best_accuracy = 0.0;

    for rule in &rules {
        let mut correct = 0;
        let mut total = 0;

        for ((image, bpp_bucket), (actual_winner, _margin)) in &winners {
            if let Some(h) = heuristics.get(image) {
                let bpp = bpp_bucket_to_value(*bpp_bucket);
                let predicted = (rule.predict)(h, bpp);
                if predicted == actual_winner {
                    correct += 1;
                }
                total += 1;
            }
        }

        let accuracy = if total > 0 {
            100.0 * correct as f64 / total as f64
        } else {
            0.0
        };

        println!(
            "{:>20} | {:>10} | {:>10} | {:>9.1}%",
            rule.name, correct, total, accuracy
        );

        if accuracy > best_accuracy {
            best_accuracy = accuracy;
            best_rule = rule.name.clone();
        }
    }

    println!("\nBest rule: {} ({:.1}% accuracy)", best_rule, best_accuracy);

    // Analyze by BPP level
    println!("\n=== Winners by BPP Level ===\n");
    println!("{:>8} | {:>12} | {:>12} | {:>12}", "BPP", "mozjpeg wins", "jpegli wins", "% jpegli");

    let bpp_targets = [0.2, 0.4, 0.6, 0.8, 1.0, 1.5, 2.0, 3.0];
    for (i, bpp) in bpp_targets.iter().enumerate() {
        let bucket_winners: Vec<_> = winners
            .iter()
            .filter(|((_, b), _)| *b == i as u8)
            .collect();

        let moz_wins = bucket_winners.iter().filter(|(_, (w, _))| w == "mozjpeg").count();
        let jpegli_wins = bucket_winners.iter().filter(|(_, (w, _))| w == "jpegli").count();
        let total = moz_wins + jpegli_wins;
        let pct_jpegli = if total > 0 {
            100.0 * jpegli_wins as f64 / total as f64
        } else {
            0.0
        };

        println!(
            "{:>8.1} | {:>12} | {:>12} | {:>11.1}%",
            bpp, moz_wins, jpegli_wins, pct_jpegli
        );
    }

    // Analyze by image characteristics
    println!("\n=== Winner Analysis by Image Type ===\n");

    // Group images by flat_block_pct
    let mut flat_analysis: HashMap<&str, (usize, usize)> = HashMap::new();
    for ((image, bpp_bucket), (winner, _)) in &winners {
        if let Some(h) = heuristics.get(image) {
            let bpp = bpp_bucket_to_value(*bpp_bucket);
            let category = if h.flat_block_pct > 80.0 {
                "very_flat"
            } else if h.flat_block_pct > 60.0 {
                "flat"
            } else if h.flat_block_pct > 40.0 {
                "mixed"
            } else {
                "complex"
            };

            let key = if bpp < 0.6 {
                match category {
                    "very_flat" => "very_flat_low_bpp",
                    "flat" => "flat_low_bpp",
                    "mixed" => "mixed_low_bpp",
                    _ => "complex_low_bpp",
                }
            } else {
                match category {
                    "very_flat" => "very_flat_high_bpp",
                    "flat" => "flat_high_bpp",
                    "mixed" => "mixed_high_bpp",
                    _ => "complex_high_bpp",
                }
            };

            let entry = flat_analysis.entry(key).or_insert((0, 0));
            if winner == "mozjpeg" {
                entry.0 += 1;
            } else {
                entry.1 += 1;
            }
        }
    }

    println!("{:>25} | {:>8} | {:>8} | {:>10}", "Category", "mozjpeg", "jpegli", "% jpegli");
    println!("{}", "-".repeat(60));
    let mut categories: Vec<_> = flat_analysis.keys().collect();
    categories.sort();
    for cat in categories {
        let (moz, jpegli) = flat_analysis[cat];
        let total = moz + jpegli;
        let pct = if total > 0 { 100.0 * jpegli as f64 / total as f64 } else { 0.0 };
        println!("{:>25} | {:>8} | {:>8} | {:>9.1}%", cat, moz, jpegli, pct);
    }

    // Write detailed predictions using best rule
    let best_predict = rules.iter().find(|r| r.name == best_rule).map(|r| r.predict);

    let mut file = std::fs::File::create(&args.output)?;
    writeln!(
        file,
        "image,bpp_bucket,target_bpp,actual_winner,margin,predicted,correct"
    )?;

    for ((image, bpp_bucket), (actual_winner, margin)) in &winners {
        if let Some(h) = heuristics.get(image) {
            let bpp = bpp_bucket_to_value(*bpp_bucket);
            let predicted = if let Some(pred_fn) = best_predict {
                pred_fn(h, bpp)
            } else {
                "jpegli"
            };
            let correct = if predicted == actual_winner { 1 } else { 0 };
            writeln!(
                file,
                "{},{},{:.1},{},{:.4},{},{}",
                image, bpp_bucket, bpp, actual_winner, margin, predicted, correct
            )?;
        }
    }

    println!("\nWrote predictions to {}", args.output.display());

    Ok(())
}
