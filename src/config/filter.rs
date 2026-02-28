//! Filtering configuration for project selection.
//!
//! This module defines the filtering options, project type filters, and sorting
//! criteria used to determine which projects should be scanned, cleaned, and
//! how they should be ordered in the output.

use clap::ValueEnum;

/// Enumeration of supported project type filters.
///
/// This enum is used to restrict scanning and cleaning to specific types of
/// development projects.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum, Default)]
pub enum ProjectFilter {
    /// Include all supported project types
    #[default]
    All,

    /// Include only Rust projects (Cargo.toml + target/)
    Rust,

    /// Include only Node.js projects (package.json + `node_modules`/)
    Node,

    /// Include only Python projects (Python config files + cache dirs)
    Python,

    /// Include only Go projects (go.mod + vendor/)
    Go,

    /// Include only Java/Kotlin projects (pom.xml or build.gradle + target/ or build/)
    Java,

    /// Include only C/C++ projects (CMakeLists.txt or Makefile + build/)
    Cpp,

    /// Include only Swift projects (Package.swift + .build/)
    Swift,

    /// Include only .NET/C# projects (.csproj + bin/ + obj/)
    #[value(name = "dotnet")]
    DotNet,

    /// Include only Ruby projects (Gemfile + .bundle/ or vendor/bundle/)
    Ruby,

    /// Include only Elixir projects (mix.exs + _build/)
    Elixir,

    /// Include only Deno projects (deno.json + vendor/ or `node_modules`/)
    Deno,

    /// Include only PHP projects (composer.json + vendor/)
    #[value(name = "php")]
    Php,

    /// Include only Haskell projects (stack.yaml or cabal.project + .stack-work/ or dist-newstyle/)
    Haskell,

    /// Include only Dart/Flutter projects (pubspec.yaml + `.dart_tool`/ or build/)
    Dart,

    /// Include only Zig projects (build.zig + zig-cache/ or zig-out/)
    Zig,

    /// Include only Scala projects (build.sbt + target/)
    Scala,
}

/// Configuration for project filtering criteria.
///
/// This struct contains the filtering options used to determine which projects
/// should be considered for cleanup based on size and modification time.
#[derive(Clone)]
pub struct FilterOptions {
    /// Minimum size threshold for build directories
    pub keep_size: String,

    /// Minimum age in days for projects to be considered
    pub keep_days: u32,
}

/// Enumeration of supported sorting criteria for project output.
///
/// This enum determines how projects are ordered in the output.
/// Each variant has a natural default direction:
/// - `Size`: largest first (descending)
/// - `Age`: oldest first (ascending)
/// - `Name`: alphabetical (ascending)
/// - `Type`: grouped by type name alphabetically
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum SortCriteria {
    /// Sort by build artifacts size (largest first by default)
    Size,

    /// Sort by build artifacts modification time (oldest first by default)
    Age,

    /// Sort by project name alphabetically (A-Z by default)
    Name,

    /// Sort by project type name alphabetically
    Type,
}

/// Configuration for project sorting behavior.
///
/// Controls how the list of projects is ordered before display or processing.
/// When `criteria` is `None`, projects are displayed in scan order.
#[derive(Clone)]
pub struct SortOptions {
    /// The sorting criterion to apply, or `None` to preserve scan order
    pub criteria: Option<SortCriteria>,

    /// Whether to reverse the sort order
    pub reverse: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_project_filter_equality() {
        assert_eq!(ProjectFilter::All, ProjectFilter::All);
        assert_eq!(ProjectFilter::Rust, ProjectFilter::Rust);
        assert_eq!(ProjectFilter::Node, ProjectFilter::Node);
        assert_eq!(ProjectFilter::Python, ProjectFilter::Python);
        assert_eq!(ProjectFilter::Go, ProjectFilter::Go);
        assert_eq!(ProjectFilter::Java, ProjectFilter::Java);
        assert_eq!(ProjectFilter::Cpp, ProjectFilter::Cpp);
        assert_eq!(ProjectFilter::Swift, ProjectFilter::Swift);
        assert_eq!(ProjectFilter::DotNet, ProjectFilter::DotNet);
        assert_eq!(ProjectFilter::Ruby, ProjectFilter::Ruby);
        assert_eq!(ProjectFilter::Elixir, ProjectFilter::Elixir);
        assert_eq!(ProjectFilter::Deno, ProjectFilter::Deno);
        assert_eq!(ProjectFilter::Php, ProjectFilter::Php);
        assert_eq!(ProjectFilter::Haskell, ProjectFilter::Haskell);
        assert_eq!(ProjectFilter::Dart, ProjectFilter::Dart);
        assert_eq!(ProjectFilter::Zig, ProjectFilter::Zig);
        assert_eq!(ProjectFilter::Scala, ProjectFilter::Scala);

        assert_ne!(ProjectFilter::All, ProjectFilter::Rust);
        assert_ne!(ProjectFilter::Rust, ProjectFilter::Node);
        assert_ne!(ProjectFilter::Node, ProjectFilter::Python);
        assert_ne!(ProjectFilter::Python, ProjectFilter::Go);
        assert_ne!(ProjectFilter::Go, ProjectFilter::Java);
        assert_ne!(ProjectFilter::Java, ProjectFilter::Cpp);
        assert_ne!(ProjectFilter::Cpp, ProjectFilter::Swift);
        assert_ne!(ProjectFilter::Swift, ProjectFilter::DotNet);
        assert_ne!(ProjectFilter::DotNet, ProjectFilter::Ruby);
        assert_ne!(ProjectFilter::Ruby, ProjectFilter::Elixir);
        assert_ne!(ProjectFilter::Elixir, ProjectFilter::Deno);
        assert_ne!(ProjectFilter::Deno, ProjectFilter::Php);
        assert_ne!(ProjectFilter::Php, ProjectFilter::Haskell);
        assert_ne!(ProjectFilter::Haskell, ProjectFilter::Dart);
        assert_ne!(ProjectFilter::Dart, ProjectFilter::Zig);
        assert_ne!(ProjectFilter::Zig, ProjectFilter::Scala);
    }

    #[test]
    fn test_project_filter_copy() {
        let original = ProjectFilter::Rust;
        let copied = original;

        assert_eq!(original, copied);
    }

    #[test]
    fn test_project_filter_default() {
        let default_filter = ProjectFilter::default();
        assert_eq!(default_filter, ProjectFilter::All);
    }

    #[test]
    fn test_filter_options_creation() {
        let filter_opts = FilterOptions {
            keep_size: "100MB".to_string(),
            keep_days: 30,
        };

        assert_eq!(filter_opts.keep_size, "100MB");
        assert_eq!(filter_opts.keep_days, 30);
    }

    #[test]
    fn test_filter_options_clone() {
        let original = FilterOptions {
            keep_size: "100MB".to_string(),
            keep_days: 30,
        };
        let cloned = original.clone();

        assert_eq!(original.keep_size, cloned.keep_size);
        assert_eq!(original.keep_days, cloned.keep_days);
    }

    #[test]
    fn test_sort_criteria_equality() {
        assert_eq!(SortCriteria::Size, SortCriteria::Size);
        assert_eq!(SortCriteria::Age, SortCriteria::Age);
        assert_eq!(SortCriteria::Name, SortCriteria::Name);
        assert_eq!(SortCriteria::Type, SortCriteria::Type);

        assert_ne!(SortCriteria::Size, SortCriteria::Age);
        assert_ne!(SortCriteria::Name, SortCriteria::Type);
    }

    #[test]
    fn test_sort_criteria_copy() {
        let original = SortCriteria::Size;
        let copied = original;
        assert_eq!(original, copied);
    }

    #[test]
    fn test_sort_options_creation() {
        let sort_opts = SortOptions {
            criteria: Some(SortCriteria::Size),
            reverse: false,
        };
        assert_eq!(sort_opts.criteria, Some(SortCriteria::Size));
        assert!(!sort_opts.reverse);
    }

    #[test]
    fn test_sort_options_none_criteria() {
        let sort_opts = SortOptions {
            criteria: None,
            reverse: false,
        };
        assert!(sort_opts.criteria.is_none());
    }

    #[test]
    fn test_sort_options_clone() {
        let original = SortOptions {
            criteria: Some(SortCriteria::Age),
            reverse: true,
        };
        let cloned = original.clone();

        assert_eq!(original.criteria, cloned.criteria);
        assert_eq!(original.reverse, cloned.reverse);
    }
}
