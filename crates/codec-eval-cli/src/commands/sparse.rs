//! Sparse checkout commands.

use anyhow::{Context, Result};
use codec_eval::corpus::sparse::{SparseCheckout, SparseFilter, preview_patterns};

use crate::SparseAction;

pub fn run(action: SparseAction, verbose: bool) -> Result<()> {
    match action {
        SparseAction::Clone {
            url,
            target,
            depth,
            include,
            format,
            category,
        } => clone_repo(&url, &target, depth, &include, &format, &category, verbose),

        SparseAction::Init { path } => init(&path, verbose),

        SparseAction::Add {
            path,
            patterns,
            format,
            category,
        } => add(&path, &patterns, &format, &category, verbose),

        SparseAction::Set {
            path,
            patterns,
            format,
            category,
        } => set(&path, &patterns, &format, &category, verbose),

        SparseAction::Status { path } => status(&path, verbose),

        SparseAction::List { path } => list(&path, verbose),

        SparseAction::Preview {
            path,
            patterns,
            format,
            category,
        } => preview(&path, &patterns, &format, &category, verbose),

        SparseAction::Disable { path } => disable(&path, verbose),

        SparseAction::Fetch { path } => fetch(&path, verbose),
    }
}

fn clone_repo(
    url: &str,
    target: &std::path::Path,
    depth: Option<u32>,
    include: &[String],
    formats: &[String],
    categories: &[String],
    verbose: bool,
) -> Result<()> {
    if verbose {
        eprintln!("Cloning {} to {}", url, target.display());
    }

    let sparse = if let Some(d) = depth {
        if verbose {
            eprintln!("Using shallow clone with depth {}", d);
        }
        SparseCheckout::clone_shallow(url, target, d)
    } else {
        SparseCheckout::clone(url, target)
    }
    .with_context(|| format!("Failed to clone {}", url))?;

    // Build filters
    let mut filters: Vec<SparseFilter> = Vec::new();

    for path in include {
        filters.push(SparseFilter::Pattern(path.clone()));
    }

    for fmt in formats {
        filters.push(SparseFilter::Format(fmt.clone()));
    }

    for cat in categories {
        filters.push(SparseFilter::Category(cat.clone()));
    }

    if !filters.is_empty() {
        if verbose {
            eprintln!("Setting {} filters", filters.len());
        }
        sparse
            .set_filters(&filters)
            .context("Failed to set filters")?;
    }

    // Checkout
    if verbose {
        eprintln!("Checking out files...");
    }
    sparse.checkout().context("Failed to checkout")?;

    let status = sparse.status()?;
    println!("Cloned to: {}", target.display());
    println!("Files checked out: {}", status.checked_out_files);
    if let Some(total) = status.total_files {
        println!(
            "Total files in repo: {} ({:.1}% checked out)",
            total,
            status.percentage().unwrap_or(100.0)
        );
    }

    Ok(())
}

fn init(path: &std::path::Path, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("Initializing sparse checkout in {}", path.display());
    }

    SparseCheckout::init(path).context("Failed to initialize sparse checkout")?;

    println!("Sparse checkout initialized in {}", path.display());
    Ok(())
}

fn add(
    path: &std::path::Path,
    patterns: &[String],
    formats: &[String],
    categories: &[String],
    verbose: bool,
) -> Result<()> {
    let sparse = SparseCheckout::open(path).context("Failed to open repository")?;

    let mut added = 0;

    // Add patterns
    if !patterns.is_empty() {
        let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
        sparse.add_paths(&refs).context("Failed to add patterns")?;
        added += patterns.len();
        if verbose {
            for p in patterns {
                eprintln!("Added pattern: {}", p);
            }
        }
    }

    // Add formats
    for fmt in formats {
        sparse
            .add_filter(&SparseFilter::Format(fmt.clone()))
            .context("Failed to add format filter")?;
        added += 1;
        if verbose {
            eprintln!("Added format: {}", fmt);
        }
    }

    // Add categories
    for cat in categories {
        sparse
            .add_filter(&SparseFilter::Category(cat.clone()))
            .context("Failed to add category filter")?;
        added += 1;
        if verbose {
            eprintln!("Added category: {}", cat);
        }
    }

    println!("Added {} patterns", added);

    // Reapply
    sparse
        .reapply()
        .context("Failed to reapply sparse checkout")?;

    let status = sparse.status()?;
    println!("Files now checked out: {}", status.checked_out_files);

    Ok(())
}

fn set(
    path: &std::path::Path,
    patterns: &[String],
    formats: &[String],
    categories: &[String],
    verbose: bool,
) -> Result<()> {
    let sparse = SparseCheckout::open(path).context("Failed to open repository")?;

    // Build all filters
    let mut filters: Vec<SparseFilter> = Vec::new();

    for p in patterns {
        filters.push(SparseFilter::Pattern(p.clone()));
    }

    for fmt in formats {
        filters.push(SparseFilter::Format(fmt.clone()));
    }

    for cat in categories {
        filters.push(SparseFilter::Category(cat.clone()));
    }

    if filters.is_empty() {
        println!("No patterns specified. Use patterns, --format, or --category.");
        return Ok(());
    }

    if verbose {
        eprintln!("Setting {} filters", filters.len());
    }

    sparse
        .set_filters(&filters)
        .context("Failed to set filters")?;

    let status = sparse.status()?;
    println!("Sparse checkout updated");
    println!("Patterns: {}", status.patterns.len());
    println!("Files checked out: {}", status.checked_out_files);

    Ok(())
}

fn status(path: &std::path::Path, _verbose: bool) -> Result<()> {
    let sparse = SparseCheckout::open(path).context("Failed to open repository")?;
    let status = sparse.status()?;

    println!("Repository: {}", path.display());
    println!(
        "Sparse checkout: {}",
        if status.enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("Patterns: {}", status.patterns.len());
    println!("Files checked out: {}", status.checked_out_files);

    if let Some(total) = status.total_files {
        let pct = status.percentage().unwrap_or(100.0);
        println!("Total files in repo: {}", total);
        println!("Checkout percentage: {:.1}%", pct);
    }

    if let Some(url) = sparse.remote_url() {
        println!("Remote: {}", url);
    }

    Ok(())
}

fn list(path: &std::path::Path, _verbose: bool) -> Result<()> {
    let sparse = SparseCheckout::open(path).context("Failed to open repository")?;
    let patterns = sparse.list_patterns().context("Failed to list patterns")?;

    if patterns.is_empty() {
        println!("No sparse checkout patterns configured.");
    } else {
        println!("Sparse checkout patterns:");
        for pattern in &patterns {
            println!("  {}", pattern);
        }
    }

    Ok(())
}

fn preview(
    path: &std::path::Path,
    patterns: &[String],
    formats: &[String],
    categories: &[String],
    verbose: bool,
) -> Result<()> {
    // Build all patterns
    let mut all_patterns: Vec<String> = patterns.to_vec();

    for fmt in formats {
        all_patterns.extend(SparseFilter::Format(fmt.clone()).to_patterns());
    }

    for cat in categories {
        all_patterns.extend(SparseFilter::Category(cat.clone()).to_patterns());
    }

    if all_patterns.is_empty() {
        println!("No patterns specified. Use patterns, --format, or --category.");
        return Ok(());
    }

    if verbose {
        eprintln!("Previewing {} patterns:", all_patterns.len());
        for p in &all_patterns {
            eprintln!("  {}", p);
        }
    }

    let refs: Vec<&str> = all_patterns.iter().map(String::as_str).collect();
    let matched = preview_patterns(path, &refs).context("Failed to preview patterns")?;

    println!("Files that would be checked out ({}):", matched.len());
    for file in matched.iter().take(50) {
        println!("  {}", file);
    }

    if matched.len() > 50 {
        println!("  ... and {} more files", matched.len() - 50);
    }

    Ok(())
}

fn disable(path: &std::path::Path, verbose: bool) -> Result<()> {
    if verbose {
        eprintln!("Disabling sparse checkout in {}", path.display());
    }

    let sparse = SparseCheckout::open(path).context("Failed to open repository")?;
    sparse
        .disable()
        .context("Failed to disable sparse checkout")?;

    println!("Sparse checkout disabled. All files will be checked out.");

    Ok(())
}

fn fetch(path: &std::path::Path, verbose: bool) -> Result<()> {
    let sparse = SparseCheckout::open(path).context("Failed to open repository")?;

    if verbose {
        eprintln!("Fetching from remote...");
    }

    sparse.fetch().context("Failed to fetch")?;

    if verbose {
        eprintln!("Checking out...");
    }

    sparse.checkout().context("Failed to checkout")?;

    let status = sparse.status()?;
    println!("Fetch complete");
    println!("Files checked out: {}", status.checked_out_files);

    Ok(())
}
