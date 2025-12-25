//! Sparse checkout utilities for partial corpus downloads.
//!
//! This module provides tools for working with git sparse checkout,
//! allowing you to download only specific files from a corpus repository.
//!
//! ## Example
//!
//! ```rust,ignore
//! use codec_eval::corpus::sparse::{SparseCheckout, SparseFilter};
//!
//! // Clone with sparse checkout
//! let sparse = SparseCheckout::clone(
//!     "https://github.com/example/codec-corpus.git",
//!     "./corpus",
//! )?;
//!
//! // Add specific paths
//! sparse.add_paths(&["images/photos/*.png", "images/screenshots/"])?;
//!
//! // Or use filters
//! sparse.add_filter(SparseFilter::Category("photos"))?;
//! sparse.add_filter(SparseFilter::Format("png"))?;
//!
//! // Fetch the files
//! sparse.fetch()?;
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::error::{Error, Result};

/// Sparse checkout manager for git repositories.
#[derive(Debug)]
pub struct SparseCheckout {
    /// Local repository path.
    repo_path: PathBuf,
    /// Remote URL (if cloned).
    remote_url: Option<String>,
}

/// Filter for selecting files in sparse checkout.
#[derive(Debug, Clone)]
pub enum SparseFilter {
    /// Match files by glob pattern.
    Pattern(String),
    /// Match by directory path.
    Directory(String),
    /// Match by file extension/format.
    Format(String),
    /// Match by category directory name.
    Category(String),
    /// Match files by minimum dimensions (requires manifest).
    MinSize { width: u32, height: u32 },
    /// Match specific file paths.
    Paths(Vec<String>),
}

impl SparseFilter {
    /// Convert filter to sparse-checkout patterns.
    pub fn to_patterns(&self) -> Vec<String> {
        match self {
            Self::Pattern(p) => vec![p.clone()],
            Self::Directory(d) => {
                let d = d.trim_end_matches('/');
                vec![format!("{d}/"), format!("{d}/**")]
            }
            Self::Format(ext) => {
                let ext = ext.trim_start_matches('.');
                vec![format!("**/*.{ext}")]
            }
            Self::Category(cat) => {
                vec![
                    format!("**/{cat}/"),
                    format!("**/{cat}/**"),
                    format!("{cat}/"),
                    format!("{cat}/**"),
                ]
            }
            Self::MinSize { .. } => {
                // MinSize requires manifest lookup, return all and filter later
                vec!["**/*".to_string()]
            }
            Self::Paths(paths) => paths.clone(),
        }
    }
}

impl SparseCheckout {
    /// Initialize sparse checkout in an existing repository.
    pub fn init(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();

        // Enable sparse checkout
        run_git(&repo_path, &["sparse-checkout", "init", "--cone"])?;

        Ok(Self {
            repo_path,
            remote_url: None,
        })
    }

    /// Clone a repository with sparse checkout enabled.
    pub fn clone(url: &str, target: impl AsRef<Path>) -> Result<Self> {
        let target = target.as_ref();

        // Create parent directory if needed
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Clone with sparse checkout and no checkout initially
        run_git_cwd(
            target.parent().unwrap_or(Path::new(".")),
            &[
                "clone",
                "--filter=blob:none",
                "--sparse",
                "--no-checkout",
                url,
                &target.file_name().unwrap().to_string_lossy(),
            ],
        )?;

        // Initialize sparse checkout
        run_git(target, &["sparse-checkout", "init", "--cone"])?;

        Ok(Self {
            repo_path: target.to_path_buf(),
            remote_url: Some(url.to_string()),
        })
    }

    /// Clone with depth limit for faster initial clone.
    pub fn clone_shallow(url: &str, target: impl AsRef<Path>, depth: u32) -> Result<Self> {
        let target = target.as_ref();

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }

        run_git_cwd(
            target.parent().unwrap_or(Path::new(".")),
            &[
                "clone",
                "--filter=blob:none",
                "--sparse",
                "--no-checkout",
                "--depth",
                &depth.to_string(),
                url,
                &target.file_name().unwrap().to_string_lossy(),
            ],
        )?;

        run_git(target, &["sparse-checkout", "init", "--cone"])?;

        Ok(Self {
            repo_path: target.to_path_buf(),
            remote_url: Some(url.to_string()),
        })
    }

    /// Open an existing sparse checkout repository.
    pub fn open(repo_path: impl AsRef<Path>) -> Result<Self> {
        let repo_path = repo_path.as_ref().to_path_buf();

        if !repo_path.join(".git").exists() {
            return Err(Error::Corpus(format!(
                "Not a git repository: {}",
                repo_path.display()
            )));
        }

        // Get remote URL if available
        let remote_url = run_git(&repo_path, &["remote", "get-url", "origin"]).ok();

        Ok(Self {
            repo_path,
            remote_url,
        })
    }

    /// Get the repository path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.repo_path
    }

    /// Get the remote URL.
    #[must_use]
    pub fn remote_url(&self) -> Option<&str> {
        self.remote_url.as_deref()
    }

    /// Add paths to the sparse checkout.
    pub fn add_paths(&self, paths: &[&str]) -> Result<()> {
        let mut args = vec!["sparse-checkout", "add"];
        args.extend(paths);
        run_git(&self.repo_path, &args)?;
        Ok(())
    }

    /// Set the sparse checkout paths (replaces existing).
    pub fn set_paths(&self, paths: &[&str]) -> Result<()> {
        let mut args = vec!["sparse-checkout", "set"];
        args.extend(paths);
        run_git(&self.repo_path, &args)?;
        Ok(())
    }

    /// Add a filter to the sparse checkout.
    pub fn add_filter(&self, filter: &SparseFilter) -> Result<()> {
        let patterns = filter.to_patterns();
        let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
        self.add_paths(&refs)
    }

    /// Set filters for the sparse checkout (replaces existing).
    pub fn set_filters(&self, filters: &[SparseFilter]) -> Result<()> {
        let patterns: Vec<String> = filters.iter().flat_map(|f| f.to_patterns()).collect();
        let refs: Vec<&str> = patterns.iter().map(String::as_str).collect();
        self.set_paths(&refs)
    }

    /// List current sparse checkout patterns.
    pub fn list_patterns(&self) -> Result<Vec<String>> {
        let output = run_git(&self.repo_path, &["sparse-checkout", "list"])?;
        Ok(output.lines().map(String::from).collect())
    }

    /// Checkout the sparse files.
    pub fn checkout(&self) -> Result<()> {
        run_git(&self.repo_path, &["checkout"])?;
        Ok(())
    }

    /// Checkout a specific branch or tag.
    pub fn checkout_ref(&self, reference: &str) -> Result<()> {
        run_git(&self.repo_path, &["checkout", reference])?;
        Ok(())
    }

    /// Fetch updates from remote.
    pub fn fetch(&self) -> Result<()> {
        run_git(&self.repo_path, &["fetch", "--filter=blob:none"])?;
        Ok(())
    }

    /// Pull updates (fetch + checkout).
    pub fn pull(&self) -> Result<()> {
        self.fetch()?;
        run_git(&self.repo_path, &["pull"])?;
        Ok(())
    }

    /// Disable sparse checkout (get all files).
    pub fn disable(&self) -> Result<()> {
        run_git(&self.repo_path, &["sparse-checkout", "disable"])?;
        Ok(())
    }

    /// Re-enable sparse checkout.
    pub fn reapply(&self) -> Result<()> {
        run_git(&self.repo_path, &["sparse-checkout", "reapply"])?;
        Ok(())
    }

    /// Get status of sparse checkout.
    pub fn status(&self) -> Result<SparseStatus> {
        // Check if sparse checkout is enabled
        let config =
            run_git(&self.repo_path, &["config", "core.sparseCheckout"]).unwrap_or_default();
        let enabled = config.trim() == "true";

        // Get patterns
        let patterns = if enabled {
            self.list_patterns().unwrap_or_default()
        } else {
            Vec::new()
        };

        // Count checked out files
        let files_output = run_git(&self.repo_path, &["ls-files"])?;
        let checked_out_files = files_output.lines().count();

        // Count total files in repo (if available)
        let total_files = run_git(&self.repo_path, &["ls-tree", "-r", "--name-only", "HEAD"])
            .map(|o| o.lines().count())
            .ok();

        Ok(SparseStatus {
            enabled,
            patterns,
            checked_out_files,
            total_files,
        })
    }
}

/// Status of a sparse checkout repository.
#[derive(Debug, Clone)]
pub struct SparseStatus {
    /// Whether sparse checkout is enabled.
    pub enabled: bool,
    /// Current sparse checkout patterns.
    pub patterns: Vec<String>,
    /// Number of files currently checked out.
    pub checked_out_files: usize,
    /// Total files in repository (if known).
    pub total_files: Option<usize>,
}

impl SparseStatus {
    /// Get the percentage of files checked out.
    #[must_use]
    pub fn percentage(&self) -> Option<f64> {
        self.total_files.map(|total| {
            if total == 0 {
                100.0
            } else {
                (self.checked_out_files as f64 / total as f64) * 100.0
            }
        })
    }
}

/// Run a git command in a repository.
fn run_git(repo_path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(["-C", &repo_path.to_string_lossy()])
        .args(args)
        .output()
        .map_err(|e| Error::Corpus(format!("Failed to run git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Corpus(format!(
            "git {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a git command in a specific directory.
fn run_git_cwd(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| Error::Corpus(format!("Failed to run git: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Corpus(format!(
            "git {} failed: {}",
            args.join(" "),
            stderr.trim()
        )));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// List files that would be matched by patterns (without checking out).
pub fn preview_patterns(repo_path: &Path, patterns: &[&str]) -> Result<Vec<String>> {
    // Get all files in repo
    let all_files = run_git(repo_path, &["ls-tree", "-r", "--name-only", "HEAD"])?;

    let mut matched = Vec::new();

    for file in all_files.lines() {
        for pattern in patterns {
            if matches_pattern(file, pattern) {
                matched.push(file.to_string());
                break;
            }
        }
    }

    Ok(matched)
}

/// Simple glob pattern matching.
fn matches_pattern(path: &str, pattern: &str) -> bool {
    // Handle common patterns
    if pattern == "**/*" {
        return true;
    }

    // Handle **/dir/** - match dir anywhere in path
    if pattern.starts_with("**/") && pattern.ends_with("/**") {
        let middle = pattern.trim_start_matches("**/").trim_end_matches("/**");
        // Match as a directory component anywhere
        return path.starts_with(&format!("{middle}/")) || path.contains(&format!("/{middle}/"));
    }

    if pattern.ends_with("/**") {
        let prefix = pattern.trim_end_matches("/**");
        return path.starts_with(prefix) || path.starts_with(&format!("{prefix}/"));
    }

    if pattern.ends_with('/') {
        let dir = pattern.trim_end_matches('/');
        return path.starts_with(dir) || path.contains(&format!("/{dir}/"));
    }

    if pattern.starts_with("**/") {
        let suffix = pattern.trim_start_matches("**/");
        if suffix.contains('*') {
            // Handle **/*.ext
            if let Some(ext) = suffix.strip_prefix("*.") {
                return path.ends_with(&format!(".{ext}"));
            }
        }
        return path.ends_with(suffix) || path.contains(&format!("/{suffix}"));
    }

    // Direct match or prefix match
    path == pattern || path.starts_with(&format!("{pattern}/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_to_patterns() {
        let filter = SparseFilter::Format("png".to_string());
        assert_eq!(filter.to_patterns(), vec!["**/*.png"]);

        let filter = SparseFilter::Category("photos".to_string());
        let patterns = filter.to_patterns();
        assert!(patterns.contains(&"**/photos/".to_string()));
        assert!(patterns.contains(&"**/photos/**".to_string()));

        let filter = SparseFilter::Directory("images/test".to_string());
        let patterns = filter.to_patterns();
        assert!(patterns.contains(&"images/test/".to_string()));
    }

    #[test]
    fn test_matches_pattern() {
        // Extension matching
        assert!(matches_pattern("images/test.png", "**/*.png"));
        assert!(!matches_pattern("images/test.jpg", "**/*.png"));

        // Directory matching
        assert!(matches_pattern("photos/image.png", "photos/"));
        assert!(matches_pattern("photos/sub/image.png", "photos/**"));

        // Prefix matching
        assert!(matches_pattern("images/photos/test.png", "**/photos/**"));
    }

    #[test]
    fn test_sparse_status_percentage() {
        let status = SparseStatus {
            enabled: true,
            patterns: vec![],
            checked_out_files: 50,
            total_files: Some(200),
        };
        assert!((status.percentage().unwrap() - 25.0).abs() < 0.01);
    }
}
