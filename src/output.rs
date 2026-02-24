//! Structured JSON output for scripting and piping.
//!
//! This module provides serializable data structures that represent the
//! complete output of a scan or cleanup operation. When the `--json` flag
//! is passed, these structures are serialized to stdout as a single JSON
//! object, replacing all human-readable output.

use std::collections::BTreeMap;

use humansize::{DECIMAL, format_size};
use serde::Serialize;

use crate::project::{Project, ProjectType};

/// Top-level JSON output emitted when `--json` is active.
#[derive(Serialize)]
pub struct JsonOutput {
    /// The execution mode: `"dry_run"` or `"cleanup"`.
    pub mode: String,

    /// List of projects that were found (and matched filters).
    pub projects: Vec<JsonProjectEntry>,

    /// Aggregated summary statistics.
    pub summary: JsonSummary,

    /// Cleanup results. Present only when an actual cleanup was performed
    /// (i.e. not in dry-run mode).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup: Option<JsonCleanupResult>,
}

/// A single project entry in the JSON output.
#[derive(Serialize)]
pub struct JsonProjectEntry {
    /// Project name extracted from config files, or `null`.
    pub name: Option<String>,

    /// Project type (`"rust"`, `"node"`, `"python"`, `"go"`, `"java"`, `"cpp"`, `"swift"`, `"dot_net"`).
    #[serde(rename = "type")]
    pub project_type: ProjectType,

    /// Absolute path to the project root directory.
    pub root_path: String,

    /// Absolute path to the build artifacts directory.
    pub build_artifacts_path: String,

    /// Size of the build artifacts in bytes.
    pub build_artifacts_size: u64,

    /// Human-readable formatted size (e.g. `"1.23 GB"`).
    pub build_artifacts_size_formatted: String,
}

/// Aggregated summary across all matched projects.
#[derive(Serialize)]
pub struct JsonSummary {
    /// Total number of projects found.
    pub total_projects: usize,

    /// Total reclaimable size in bytes.
    pub total_size: u64,

    /// Human-readable formatted total size.
    pub total_size_formatted: String,

    /// Per-type breakdown (key is the project type name).
    pub by_type: BTreeMap<String, JsonTypeSummary>,
}

/// Per-project-type count and size.
#[derive(Serialize)]
pub struct JsonTypeSummary {
    /// Number of projects of this type.
    pub count: usize,

    /// Total size in bytes for this type.
    pub size: u64,

    /// Human-readable formatted size.
    pub size_formatted: String,
}

/// Results of a cleanup operation.
#[derive(Serialize)]
pub struct JsonCleanupResult {
    /// Number of projects successfully cleaned.
    pub success_count: usize,

    /// Number of projects that failed to clean.
    pub failure_count: usize,

    /// Total bytes actually freed.
    pub total_freed: u64,

    /// Human-readable formatted freed size.
    pub total_freed_formatted: String,

    /// Error messages for projects that failed.
    pub errors: Vec<String>,
}

impl JsonOutput {
    /// Build a `JsonOutput` from a slice of projects in dry-run mode.
    #[must_use]
    pub fn from_projects_dry_run(projects: &[Project]) -> Self {
        Self {
            mode: "dry_run".to_string(),
            projects: projects
                .iter()
                .map(JsonProjectEntry::from_project)
                .collect(),
            summary: JsonSummary::from_projects(projects),
            cleanup: None,
        }
    }

    /// Build a `JsonOutput` from a slice of projects after a cleanup operation.
    #[must_use]
    pub fn from_projects_cleanup(
        projects: &[Project],
        clean_result: &crate::cleaner::CleanResult,
    ) -> Self {
        Self {
            mode: "cleanup".to_string(),
            projects: projects
                .iter()
                .map(JsonProjectEntry::from_project)
                .collect(),
            summary: JsonSummary::from_projects(projects),
            cleanup: Some(JsonCleanupResult::from_clean_result(clean_result)),
        }
    }
}

impl JsonProjectEntry {
    /// Convert a `Project` into a `JsonProjectEntry`.
    #[must_use]
    pub fn from_project(project: &Project) -> Self {
        Self {
            name: project.name.clone(),
            project_type: project.kind.clone(),
            root_path: project.root_path.display().to_string(),
            build_artifacts_path: project.build_arts.path.display().to_string(),
            build_artifacts_size: project.build_arts.size,
            build_artifacts_size_formatted: format_size(project.build_arts.size, DECIMAL),
        }
    }
}

impl JsonSummary {
    /// Compute summary statistics from a slice of projects.
    #[must_use]
    pub fn from_projects(projects: &[Project]) -> Self {
        let mut by_type: BTreeMap<String, (usize, u64)> = BTreeMap::new();

        for project in projects {
            let key = match project.kind {
                ProjectType::Rust => "rust",
                ProjectType::Node => "node",
                ProjectType::Python => "python",
                ProjectType::Go => "go",
                ProjectType::Java => "java",
                ProjectType::Cpp => "cpp",
                ProjectType::Swift => "swift",
                ProjectType::DotNet => "dotnet",
                ProjectType::Ruby => "ruby",
                ProjectType::Elixir => "elixir",
                ProjectType::Deno => "deno",
            };

            let entry = by_type.entry(key.to_string()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += project.build_arts.size;
        }

        let total_size: u64 = projects.iter().map(|p| p.build_arts.size).sum();

        Self {
            total_projects: projects.len(),
            total_size,
            total_size_formatted: format_size(total_size, DECIMAL),
            by_type: by_type
                .into_iter()
                .map(|(k, (count, size))| {
                    (
                        k,
                        JsonTypeSummary {
                            count,
                            size,
                            size_formatted: format_size(size, DECIMAL),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl JsonCleanupResult {
    /// Convert a `CleanResult` into a `JsonCleanupResult`.
    #[must_use]
    pub fn from_clean_result(result: &crate::cleaner::CleanResult) -> Self {
        Self {
            success_count: result.success_count,
            failure_count: result.errors.len(),
            total_freed: result.total_freed,
            total_freed_formatted: format_size(result.total_freed, DECIMAL),
            errors: result.errors.clone(),
        }
    }
}
