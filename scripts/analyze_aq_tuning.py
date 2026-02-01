#!/usr/bin/env python3
"""
Analyze AQ tuning results for sharpened images.

Plots bpp vs quality metrics for different AQ scales.
"""

import pandas as pd
import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path
import sys

def load_results(csv_path):
    """Load and clean results CSV."""
    df = pd.read_csv(csv_path)
    # Ensure numeric columns
    for col in ['distance', 'aq_scale', 'aq_mean', 'file_size', 'bpp', 'dssim', 'ssimulacra2']:
        if col in df.columns:
            df[col] = pd.to_numeric(df[col], errors='coerce')
    return df

def plot_bpp_vs_dssim(df, output_dir):
    """Plot bpp vs DSSIM for each AQ scale."""
    fig, axes = plt.subplots(1, 2, figsize=(14, 6))

    # Get unique AQ scales
    aq_scales = sorted(df['aq_scale'].unique())
    colors = plt.cm.viridis(np.linspace(0, 1, len(aq_scales)))

    # Plot 1: All data points
    ax = axes[0]
    for aq_scale, color in zip(aq_scales, colors):
        subset = df[df['aq_scale'] == aq_scale]
        ax.scatter(subset['bpp'], subset['dssim'],
                  alpha=0.5, s=20, c=[color], label=f'AQ={aq_scale:.2f}')
    ax.set_xlabel('Bits per pixel (bpp)')
    ax.set_ylabel('DSSIM (lower is better)')
    ax.set_title('BPP vs DSSIM for all images')
    ax.legend(loc='upper right', fontsize=8)
    ax.set_yscale('log')
    ax.grid(True, alpha=0.3)

    # Plot 2: Average per distance
    ax = axes[1]
    for aq_scale, color in zip(aq_scales, colors):
        subset = df[df['aq_scale'] == aq_scale]
        avg = subset.groupby('distance').agg({'bpp': 'mean', 'dssim': 'mean'}).reset_index()
        ax.plot(avg['bpp'], avg['dssim'], 'o-', color=color,
                label=f'AQ={aq_scale:.2f}', markersize=8)
    ax.set_xlabel('Average BPP')
    ax.set_ylabel('Average DSSIM (lower is better)')
    ax.set_title('Average BPP vs DSSIM by AQ scale')
    ax.legend(loc='upper right', fontsize=8)
    ax.set_yscale('log')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_dir / 'bpp_vs_dssim.png', dpi=150)
    plt.close()

def plot_bpp_vs_ssim2(df, output_dir):
    """Plot bpp vs SSIMULACRA2 for each AQ scale."""
    fig, axes = plt.subplots(1, 2, figsize=(14, 6))

    aq_scales = sorted(df['aq_scale'].unique())
    colors = plt.cm.viridis(np.linspace(0, 1, len(aq_scales)))

    # Plot 1: All data points
    ax = axes[0]
    for aq_scale, color in zip(aq_scales, colors):
        subset = df[df['aq_scale'] == aq_scale]
        ax.scatter(subset['bpp'], subset['ssimulacra2'],
                  alpha=0.5, s=20, c=[color], label=f'AQ={aq_scale:.2f}')
    ax.set_xlabel('Bits per pixel (bpp)')
    ax.set_ylabel('SSIMULACRA2 (higher is better)')
    ax.set_title('BPP vs SSIMULACRA2 for all images')
    ax.legend(loc='lower right', fontsize=8)
    ax.grid(True, alpha=0.3)

    # Plot 2: Average per distance
    ax = axes[1]
    for aq_scale, color in zip(aq_scales, colors):
        subset = df[df['aq_scale'] == aq_scale]
        avg = subset.groupby('distance').agg({'bpp': 'mean', 'ssimulacra2': 'mean'}).reset_index()
        ax.plot(avg['bpp'], avg['ssimulacra2'], 'o-', color=color,
                label=f'AQ={aq_scale:.2f}', markersize=8)
    ax.set_xlabel('Average BPP')
    ax.set_ylabel('Average SSIMULACRA2 (higher is better)')
    ax.set_title('Average BPP vs SSIMULACRA2 by AQ scale')
    ax.legend(loc='lower right', fontsize=8)
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_dir / 'bpp_vs_ssim2.png', dpi=150)
    plt.close()

def plot_rd_efficiency(df, output_dir):
    """Plot rate-distortion efficiency (DSSIM * bpp) by AQ scale."""
    fig, ax = plt.subplots(figsize=(10, 6))

    # Compute RD efficiency for each row
    df['rd_efficiency'] = df['dssim'] * df['bpp']

    # Box plot by AQ scale
    aq_scales = sorted(df['aq_scale'].unique())
    data = [df[df['aq_scale'] == s]['rd_efficiency'].values for s in aq_scales]

    bp = ax.boxplot(data, labels=[f'{s:.2f}' for s in aq_scales], patch_artist=True)
    colors = plt.cm.viridis(np.linspace(0, 1, len(aq_scales)))
    for patch, color in zip(bp['boxes'], colors):
        patch.set_facecolor(color)
        patch.set_alpha(0.7)

    ax.set_xlabel('AQ Scale')
    ax.set_ylabel('Rate-Distortion (DSSIM * bpp, lower is better)')
    ax.set_title('Rate-Distortion Efficiency by AQ Scale')
    ax.grid(True, alpha=0.3, axis='y')

    # Add mean line
    means = [np.mean(d) for d in data]
    ax.plot(range(1, len(aq_scales) + 1), means, 'r--', marker='D', label='Mean')
    ax.legend()

    plt.tight_layout()
    plt.savefig(output_dir / 'rd_efficiency.png', dpi=150)
    plt.close()

def compute_pareto_front(df, aq_scale):
    """Find Pareto-optimal points (minimize bpp and dssim)."""
    subset = df[df['aq_scale'] == aq_scale].copy()
    subset = subset.sort_values('bpp')

    pareto = []
    min_dssim = float('inf')

    for _, row in subset.iterrows():
        if row['dssim'] < min_dssim:
            pareto.append(row)
            min_dssim = row['dssim']

    return pd.DataFrame(pareto)

def plot_pareto_comparison(df, output_dir):
    """Compare Pareto fronts for different AQ scales."""
    fig, ax = plt.subplots(figsize=(10, 8))

    aq_scales = [0.25, 0.5, 1.0, 1.5, 2.0]
    colors = plt.cm.tab10(range(len(aq_scales)))

    for aq_scale, color in zip(aq_scales, colors):
        pareto = compute_pareto_front(df, aq_scale)
        if not pareto.empty:
            ax.plot(pareto['bpp'], pareto['dssim'], 'o-', color=color,
                   label=f'AQ={aq_scale:.2f}', markersize=6, linewidth=2)

    ax.set_xlabel('Bits per pixel (bpp)')
    ax.set_ylabel('DSSIM (lower is better)')
    ax.set_title('Pareto Fronts by AQ Scale\n(Lower-left is better)')
    ax.legend(loc='upper right')
    ax.set_yscale('log')
    ax.grid(True, alpha=0.3)

    plt.tight_layout()
    plt.savefig(output_dir / 'pareto_comparison.png', dpi=150)
    plt.close()

def print_summary(df):
    """Print summary statistics."""
    print("\n=== AQ Tuning Results Summary ===\n")

    # Group by AQ scale
    summary = df.groupby('aq_scale').agg({
        'bpp': 'mean',
        'dssim': 'mean',
        'ssimulacra2': 'mean',
        'file_size': 'mean'
    }).round(4)

    # Add RD efficiency
    summary['rd_efficiency'] = (summary['dssim'] * summary['bpp']).round(6)

    print("Average metrics by AQ scale:")
    print(summary.to_string())

    # Find optimal AQ scale
    optimal_rd = summary['rd_efficiency'].idxmin()
    print(f"\nOptimal AQ scale (min RD): {optimal_rd}")

    # By distance
    print("\nOptimal AQ scale by distance:")
    for dist in sorted(df['distance'].unique()):
        dist_df = df[df['distance'] == dist]
        best = dist_df.groupby('aq_scale').apply(
            lambda x: (x['dssim'] * x['bpp']).mean()
        ).idxmin()
        print(f"  distance={dist}: AQ={best}")

def main():
    if len(sys.argv) < 2:
        print("Usage: python analyze_aq_tuning.py <results.csv> [output_dir]")
        sys.exit(1)

    csv_path = Path(sys.argv[1])
    output_dir = Path(sys.argv[2]) if len(sys.argv) > 2 else csv_path.parent

    print(f"Loading results from: {csv_path}")
    df = load_results(csv_path)
    print(f"Loaded {len(df)} data points")
    print(f"Images: {df['image'].nunique()}")
    print(f"Distances: {sorted(df['distance'].unique())}")
    print(f"AQ scales: {sorted(df['aq_scale'].unique())}")

    # Print summary
    print_summary(df)

    # Create plots
    print(f"\nGenerating plots in: {output_dir}")
    plot_bpp_vs_dssim(df, output_dir)
    plot_bpp_vs_ssim2(df, output_dir)
    plot_rd_efficiency(df, output_dir)
    plot_pareto_comparison(df, output_dir)

    print("\nPlots saved:")
    print("  - bpp_vs_dssim.png")
    print("  - bpp_vs_ssim2.png")
    print("  - rd_efficiency.png")
    print("  - pareto_comparison.png")

if __name__ == '__main__':
    main()
