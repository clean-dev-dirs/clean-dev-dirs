//! Collection management and operations for development projects.
//!
//! This module provides the `Projects` struct which wraps a collection of
//! development projects and provides various operations on them, including
//! interactive selection, summary reporting, and parallel iteration support.

use anyhow::Result;
use colored::Colorize;
use humansize::{DECIMAL, format_size};
use inquire::MultiSelect;
use rayon::prelude::*;

use crate::project::ProjectType;

use super::Project;

/// A collection of development projects with associated operations.
///
/// The `Projects` struct wraps a vector of `Project` instances and provides
/// higher-level operations such as interactive selection, summary reporting,
/// and parallel processing support. It serves as the main data structure
/// for managing collections of projects throughout the application.
pub struct Projects(Vec<Project>);

impl From<Vec<Project>> for Projects {
    /// Create a `Projects` collection from a vector of projects.
    ///
    /// This conversion allows easy creation of a `Projects` instance from
    /// any vector of `Project` objects, typically used when the scanner
    /// returns a collection of detected projects.
    ///
    /// # Arguments
    ///
    /// * `projects` - A vector of `Project` instances
    ///
    /// # Returns
    ///
    /// A new `Projects` collection containing the provided projects.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::{Projects, Project};
    /// let project_vec = vec![/* project instances */];
    /// let projects: Projects = project_vec.into();
    /// ```
    fn from(projects: Vec<Project>) -> Self {
        Self(projects)
    }
}

impl IntoParallelIterator for Projects {
    type Iter = rayon::vec::IntoIter<Project>;
    type Item = Project;

    /// Enable parallel iteration with ownership transfer.
    ///
    /// This implementation allows the collection to be consumed and processed
    /// in parallel, transferring ownership of each project to the parallel
    /// processing context.
    ///
    /// # Returns
    ///
    /// A parallel iterator that takes ownership of the projects in the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rayon::prelude::*;
    /// # use crate::Projects;
    /// let results: Vec<_> = projects.into_par_iter().map(|project| {
    ///     // Transform each project in parallel
    ///     process_project(project)
    /// }).collect();
    /// ```
    fn into_par_iter(self) -> Self::Iter {
        self.0.into_par_iter()
    }
}

impl<'a> IntoParallelIterator for &'a Projects {
    type Iter = rayon::slice::Iter<'a, Project>;
    type Item = &'a Project;

    /// Enable parallel iteration over project references.
    ///
    /// This implementation allows the collection to be processed in parallel
    /// using Rayon's parallel iterators, which can significantly improve
    /// performance for operations that can be parallelized.
    ///
    /// # Returns
    ///
    /// A parallel iterator over references to the projects in the collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use rayon::prelude::*;
    /// # use crate::Projects;
    /// projects.into_par_iter().for_each(|project| {
    ///     // Process each project in parallel
    ///     println!("Processing: {}", project.root_path.display());
    /// });
    /// ```
    fn into_par_iter(self) -> Self::Iter {
        self.0.par_iter()
    }
}

impl Projects {
    /// Calculate the total size of all build directories in the collection.
    ///
    /// This method sums up the sizes of all build directories (target/ or
    /// `node_modules`/) across all projects in the collection to provide a
    /// total estimate of reclaimable disk space.
    ///
    /// # Returns
    ///
    /// The total size in bytes of all build directories combined.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::Projects;
    /// let total_bytes = projects.get_total_size();
    /// println!("Total reclaimable space: {} bytes", total_bytes);
    /// ```
    #[must_use]
    pub fn get_total_size(&self) -> u64 {
        self.0.iter().map(Project::total_size).sum()
    }

    /// Present an interactive selection interface for choosing projects to clean.
    ///
    /// This method displays a multi-select dialog that allows users to choose
    /// which projects they want to clean. Each project is shown with its type
    /// icon, path, and reclaimable space. All projects are selected by default.
    ///
    /// # Returns
    ///
    /// - `Ok(Vec<Project>)` - The projects selected by the user
    /// - `Err(anyhow::Error)` - If the interactive dialog fails or is canceled
    ///
    /// # Interface Details
    ///
    /// - Uses a colorful theme for better visual appeal
    /// - Shows project type icons (🦀 Rust, 📦 Node.js, 🐍 Python, 🐹 Go, ☕ Java, ⚙️ C/C++, 🐦 Swift, 🔷 .NET)
    /// - Displays project paths and sizes in human-readable format
    /// - Allows toggling selections with space bar
    /// - Confirms selection with the Enter key
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::Projects;
    /// # use anyhow::Result;
    /// let selected_projects = projects.interactive_selection()?;
    /// println!("User selected {} projects", selected_projects.len());
    /// ```
    ///
    /// # Errors
    ///
    /// This method can fail if:
    /// - The terminal doesn't support interactive input
    /// - The user cancels the dialog (Ctrl+C)
    /// - There are I/O errors with the terminal
    pub fn interactive_selection(&self) -> Result<Vec<Project>> {
        let items: Vec<String> = self
            .0
            .iter()
            .map(|p| {
                let icon = icon_for_project_type(&p.kind);
                format!(
                    "{icon} {} ({})",
                    p.root_path.display(),
                    format_size(p.total_size(), DECIMAL)
                )
            })
            .collect();

        let defaults: Vec<usize> = (0..self.0.len()).collect();

        let selections = MultiSelect::new("Select projects to clean:", items)
            .with_default(&defaults)
            .prompt()?;

        Ok(selections
            .iter()
            .filter_map(|selected_item| {
                self.0
                    .iter()
                    .enumerate()
                    .find(|(_, p)| {
                        let icon = icon_for_project_type(&p.kind);
                        let expected = format!(
                            "{icon} {} ({})",
                            p.root_path.display(),
                            format_size(p.total_size(), DECIMAL)
                        );
                        &expected == selected_item
                    })
                    .map(|(i, _)| i)
            })
            .map(|i| self.0[i].clone())
            .collect())
    }

    /// Get the number of projects in the collection.
    ///
    /// # Returns
    ///
    /// The number of projects contained in this collection.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::Projects;
    /// println!("Found {} projects", projects.len());
    /// ```
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Check if the collection is empty.
    ///
    /// # Returns
    ///
    /// `true` if the collection contains no projects, `false` otherwise.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::Projects;
    /// if projects.is_empty() {
    ///     println!("No projects found");
    /// }
    /// ```
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Return a slice of the underlying project collection.
    ///
    /// Useful for inspecting projects without consuming the collection,
    /// for example to build JSON output before cleanup.
    #[must_use]
    pub fn as_slice(&self) -> &[Project] {
        &self.0
    }

    /// Print a detailed summary of the projects and their reclaimable space.
    ///
    /// This method analyzes the collection and prints statistics including:
    /// - Number and total size of Rust projects
    /// - Number and total size of Node.js projects
    /// - Number and total size of Python projects
    /// - Number and total size of Go projects
    /// - Total reclaimable space across all projects
    ///
    /// The output is formatted with colors and emoji icons for better readability.
    ///
    /// # Arguments
    ///
    /// * `total_size` - The total size in bytes (usually from `get_total_size()`)
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::Projects;
    /// let total_size = projects.get_total_size();
    /// projects.print_summary(total_size);
    /// ```
    ///
    /// # Output Format
    ///
    /// ```text
    ///   🦀 5 Rust projects (2.3 GB)
    ///   📦 3 Node.js projects (1.7 GB)
    ///   🐍 2 Python projects (1.2 GB)
    ///   🐹 1 Go project (0.5 GB)
    ///   ☕ 2 Java/Kotlin projects (0.8 GB)
    ///   ⚙️ 1 C/C++ project (0.3 GB)
    ///   🐦 1 Swift project (0.2 GB)
    ///   🔷 1 .NET/C# project (0.1 GB)
    ///   💾 Total reclaimable space: 4.0 GB
    /// ```
    pub fn print_summary(&self, total_size: u64) {
        let type_entries: &[(ProjectType, &str, &str)] = &[
            (ProjectType::Rust, "🦀", "Rust"),
            (ProjectType::Node, "📦", "Node.js"),
            (ProjectType::Python, "🐍", "Python"),
            (ProjectType::Go, "🐹", "Go"),
            (ProjectType::Java, "☕", "Java/Kotlin"),
            (ProjectType::Cpp, "⚙️", "C/C++"),
            (ProjectType::Swift, "🐦", "Swift"),
            (ProjectType::DotNet, "🔷", ".NET/C#"),
        ];

        for (kind, icon, label) in type_entries {
            let (count, size) = self.0.iter().fold((0usize, 0u64), |(c, s), p| {
                if &p.kind == kind {
                    (c + 1, s + p.total_size())
                } else {
                    (c, s)
                }
            });

            if count > 0 {
                println!(
                    "  {icon} {} {label} projects ({})",
                    count.to_string().bright_white(),
                    format_size(size, DECIMAL).bright_white()
                );
            }
        }

        println!(
            "  💾 Total reclaimable space: {}",
            format_size(total_size, DECIMAL).bright_green().bold()
        );
    }
}

/// Return the icon for a given project type.
const fn icon_for_project_type(kind: &ProjectType) -> &'static str {
    match kind {
        ProjectType::Rust => "🦀",
        ProjectType::Node => "📦",
        ProjectType::Python => "🐍",
        ProjectType::Go => "🐹",
        ProjectType::Java => "☕",
        ProjectType::Cpp => "⚙️",
        ProjectType::Swift => "🐦",
        ProjectType::DotNet => "🔷",
        ProjectType::Ruby => "💎",
        ProjectType::Elixir => "💧",
        ProjectType::Deno => "🦕",
        ProjectType::Php => "🐘",
        ProjectType::Haskell => "λ",
        ProjectType::Dart => "🎯",
        ProjectType::Zig => "⚡",
        ProjectType::Scala => "🔴",
    }
}
