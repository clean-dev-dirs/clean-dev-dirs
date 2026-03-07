//! Build directory cleanup functionality.
//!
//! This module provides the core cleanup logic for removing build directories
//! from detected development projects. It handles parallel processing, progress
//! reporting, error handling, and provides detailed statistics about the
//! cleanup operation.

use anyhow::Result;
use colored::Colorize;
use humansize::{DECIMAL, format_size};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::sync::{Arc, Mutex};

use crate::executables;
use crate::project::{Project, Projects};

/// Strategy for removing build directories.
#[derive(Clone, Copy)]
pub enum RemovalStrategy {
    /// Permanently delete the directory (default, uses `fs::remove_dir_all`).
    Permanent,

    /// Move the directory to the system trash (recoverable deletion).
    Trash,
}

impl RemovalStrategy {
    /// Create a removal strategy from the `use_trash` boolean flag.
    #[must_use]
    pub const fn from_use_trash(use_trash: bool) -> Self {
        if use_trash {
            Self::Trash
        } else {
            Self::Permanent
        }
    }
}

/// Structured result returned after a cleanup operation.
///
/// Contains all the data needed to render either human-readable or JSON output.
pub struct CleanResult {
    /// Number of projects successfully cleaned.
    pub success_count: usize,

    /// Total bytes actually freed during cleanup.
    pub total_freed: u64,

    /// Estimated total size before cleanup (from cached scan data).
    pub estimated_size: u64,

    /// Error messages for projects that failed to clean.
    pub errors: Vec<String>,
}

/// Handles the cleanup of build directories from development projects.
///
/// The `Cleaner` struct provides methods for removing build directories
/// (such as `target/` for Rust projects and `node_modules/` for Node.js projects)
/// with parallel processing, progress reporting, and comprehensive error handling.
pub struct Cleaner;

impl Cleaner {
    /// Create a new cleaner instance.
    ///
    /// # Returns
    ///
    /// A new `Cleaner` instance ready to perform cleanup operations.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::Cleaner;
    /// let cleaner = Cleaner::new();
    /// ```
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Clean build directories from a collection of projects.
    ///
    /// This method performs the main cleanup operation by:
    /// 1. Setting up a progress bar for user feedback (unless `quiet`)
    /// 2. Processing projects in parallel for efficiency
    /// 3. Collecting and reporting any errors that occur
    /// 4. Returning a [`CleanResult`] with detailed statistics
    ///
    /// # Arguments
    ///
    /// * `projects` - A collection of projects to clean
    /// * `keep_executables` - Whether to preserve compiled executables before cleaning
    /// * `quiet` - When `true`, suppresses all human-readable output (progress bars, messages).
    ///   Used by the `--json` flag so that only the final JSON is printed.
    /// * `removal_strategy` - Whether to permanently delete or move to system trash
    ///
    /// # Panics
    ///
    /// This method may panic if the progress bar template string is invalid,
    /// though this should not occur under normal circumstances as the template
    /// is hardcoded and valid.
    ///
    /// # Returns
    ///
    /// A [`CleanResult`] containing success/failure counts, total freed bytes,
    /// and any error messages.
    ///
    /// # Performance
    ///
    /// This method uses parallel processing to clean multiple projects
    /// simultaneously, which can significantly reduce cleanup time for
    /// large numbers of projects.
    ///
    /// # Error Handling
    ///
    /// Individual project cleanup failures do not stop the overall process.
    /// All errors are collected and reported in the returned [`CleanResult`],
    /// allowing the cleanup to proceed for projects that can be successfully processed.
    #[must_use]
    pub fn clean_projects(
        projects: Projects,
        keep_executables: bool,
        quiet: bool,
        removal_strategy: RemovalStrategy,
    ) -> CleanResult {
        let total_projects = projects.len();
        let total_size: u64 = projects.get_total_size();

        let progress = if quiet {
            ProgressBar::hidden()
        } else {
            let action = match removal_strategy {
                RemovalStrategy::Permanent => "🧹 Starting cleanup...",
                RemovalStrategy::Trash => "🗑️  Moving to trash...",
            };
            println!("\n{}", action.cyan());

            let pb = ProgressBar::new(total_projects as u64);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                    .unwrap()
                    .progress_chars("█▉▊▋▌▍▎▏  "),
            );
            pb
        };

        let cleaned_size = Arc::new(Mutex::new(0u64));
        let errors = Arc::new(Mutex::new(Vec::new()));

        // Clean projects in parallel
        projects.into_par_iter().for_each(|project| {
            let result = clean_single_project(&project, keep_executables, removal_strategy);

            let action = match removal_strategy {
                RemovalStrategy::Permanent => "Cleaned",
                RemovalStrategy::Trash => "Trashed",
            };

            match result {
                Ok(freed_size) => {
                    *cleaned_size.lock().unwrap() += freed_size;

                    progress.set_message(format!(
                        "{action} {} ({})",
                        project
                            .root_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown"),
                        format_size(freed_size, DECIMAL)
                    ));
                }
                Err(e) => {
                    errors.lock().unwrap().push(format!(
                        "Failed to clean {}: {e}",
                        project.root_path.display()
                    ));
                }
            }

            progress.inc(1);
        });

        let finish_msg = match removal_strategy {
            RemovalStrategy::Permanent => "✅ Cleanup complete",
            RemovalStrategy::Trash => "✅ Moved to trash",
        };
        progress.finish_with_message(finish_msg);

        let final_cleaned_size = *cleaned_size.lock().unwrap();
        let errors = Arc::try_unwrap(errors)
            .expect("all parallel tasks should be complete")
            .into_inner()
            .unwrap();

        let success_count = total_projects - errors.len();

        CleanResult {
            success_count,
            total_freed: final_cleaned_size,
            estimated_size: total_size,
            errors,
        }
    }

    /// Print a human-readable cleanup summary to stdout.
    ///
    /// This is called from `main` when `--json` is **not** active.
    pub fn print_summary(result: &CleanResult) {
        if !result.errors.is_empty() {
            println!("\n{}", "⚠️  Some errors occurred during cleanup:".yellow());
            for error in &result.errors {
                eprintln!("  {}", error.red());
            }
        }

        println!("\n{}", "📊 Cleanup Summary:".bold());
        println!(
            "  ✅ Successfully cleaned: {} projects",
            result.success_count.to_string().green()
        );

        if !result.errors.is_empty() {
            println!(
                "  ❌ Failed to clean: {} projects",
                result.errors.len().to_string().red()
            );
        }

        println!(
            "  💾 Total space freed: {}",
            format_size(result.total_freed, DECIMAL)
                .bright_green()
                .bold()
        );

        if result.total_freed != result.estimated_size {
            let difference = result.estimated_size.abs_diff(result.total_freed);
            println!(
                "  📋 Difference from estimate: {}",
                format_size(difference, DECIMAL).yellow()
            );
        }
    }
}

/// Clean the build directory for a single project.
///
/// This function handles the cleanup of an individual project's build directory.
/// It calculates the actual size before deletion and then removes the entire
/// directory tree, either permanently or by moving it to the system trash.
///
/// # Arguments
///
/// * `project` - The project whose build directory should be cleaned
/// * `keep_executables` - Whether to preserve compiled executables before cleaning
/// * `removal_strategy` - Whether to permanently delete or move to system trash
///
/// # Returns
///
/// - `Ok(u64)` - The number of bytes freed by the cleanup
/// - `Err(anyhow::Error)` - If the cleanup operation failed
///
/// # Behavior
///
/// 1. Checks if the build directory exists (returns 0 if not)
/// 2. Optionally preserves compiled executables
/// 3. Calculates the actual size of the directory before deletion
/// 4. Removes the directory (permanently or via trash, based on `removal_strategy`)
/// 5. Returns the amount of space freed
///
/// # Error Conditions
///
/// This function can fail if:
/// - The build directory cannot be removed due to permission issues
/// - Files within the directory are locked or in use by other processes
/// - The file system encounters I/O errors during deletion
/// - The system trash is not available (when using [`RemovalStrategy::Trash`])
fn clean_single_project(
    project: &Project,
    keep_executables: bool,
    removal_strategy: RemovalStrategy,
) -> Result<u64> {
    // Preserve executables before deletion if requested
    if keep_executables {
        match executables::preserve_executables(project) {
            Ok(preserved) => {
                if !preserved.is_empty() {
                    eprintln!(
                        "  Preserved {} executable(s) from {}",
                        preserved.len(),
                        project
                            .root_path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown")
                    );
                }
            }
            Err(e) => {
                eprintln!(
                    "  Warning: failed to preserve executables for {}: {e}",
                    project.root_path.display()
                );
            }
        }
    }

    let mut total_freed = 0u64;

    for artifact in &project.build_arts {
        let build_dir = &artifact.path;

        if !build_dir.exists() {
            continue;
        }

        // Get the actual size before deletion (might be different from the cached size)
        total_freed += crate::utils::calculate_dir_size(build_dir);

        // Remove the build directory using the chosen strategy
        match removal_strategy {
            RemovalStrategy::Permanent => fs::remove_dir_all(build_dir)?,
            RemovalStrategy::Trash => {
                trash::delete(build_dir)
                    .map_err(|e| anyhow::anyhow!("failed to move to trash: {e}"))?;
            }
        }
    }

    Ok(total_freed)
}

impl Default for Cleaner {
    /// Create a default cleaner instance.
    ///
    /// This implementation allows `Cleaner::default()` to be used as an
    /// alternative to `Cleaner::new()` for creating cleaner instances.
    ///
    /// # Returns
    ///
    /// A new `Cleaner` instance with default settings.
    fn default() -> Self {
        Self::new()
    }
}
