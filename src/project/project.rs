//! Core project data structures and types.
//!
//! This module defines the fundamental data structures used to represent
//! development projects and their build artifacts throughout the application.

use std::{
    fmt::{Display, Formatter, Result},
    path::PathBuf,
};

use serde::Serialize;

/// Enumeration of supported development project types.
///
/// This enum distinguishes between different types of development projects
/// that the tool can detect and clean. Each project type has its own
/// characteristic files and build directories.
#[derive(Clone, PartialEq, Eq, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectType {
    /// Rust project with Cargo.toml and target/ directory
    ///
    /// Rust projects are identified by the presence of both a `Cargo.toml`
    /// file and a `target/` directory in the same location.
    Rust,

    /// Node.js project with package.json and `node_modules`/ directory
    ///
    /// Node.js projects are identified by the presence of both a `package.json`
    /// file and a `node_modules`/ directory in the same location.
    Node,

    /// Python project with requirements.txt, setup.py, or pyproject.toml and cache directories
    ///
    /// Python projects are identified by the presence of Python configuration files
    /// and various cache/build directories like `__pycache__`, `.pytest_cache`, etc.
    Python,

    /// Go project with `go.mod` and vendor/ directory
    ///
    /// Go projects are identified by the presence of both a `go.mod`
    /// file and a `vendor/` directory in the same location.
    Go,

    /// Java/Kotlin project with pom.xml or build.gradle and target/ or build/ directory
    ///
    /// Java/Kotlin projects are identified by the presence of Maven (`pom.xml`)
    /// or Gradle (`build.gradle`, `build.gradle.kts`) configuration files along
    /// with their respective build output directories.
    Java,

    /// C/C++ project with CMakeLists.txt or Makefile and build/ directory
    ///
    /// C/C++ projects are identified by the presence of build system files
    /// (`CMakeLists.txt` or `Makefile`) alongside a `build/` directory.
    Cpp,

    /// Swift project with Package.swift and .build/ directory
    ///
    /// Swift Package Manager projects are identified by the presence of a
    /// `Package.swift` manifest and the `.build/` directory.
    Swift,

    /// .NET/C# project with .csproj and bin/ + obj/ directories
    ///
    /// .NET projects are identified by the presence of `.csproj` project files
    /// alongside `bin/` and/or `obj/` output directories.
    DotNet,

    /// Ruby project with Gemfile and .bundle/ or vendor/bundle/ directory
    ///
    /// Ruby projects are identified by the presence of a `Gemfile`
    /// alongside a `.bundle/` or `vendor/bundle/` directory.
    Ruby,

    /// Elixir project with mix.exs and _build/ directory
    ///
    /// Elixir projects are identified by the presence of a `mix.exs`
    /// file and a `_build/` directory.
    Elixir,

    /// Deno project with deno.json or deno.jsonc and vendor/ or node_modules/ directory
    ///
    /// Deno projects are identified by the presence of a `deno.json` or `deno.jsonc`
    /// file alongside a `vendor/` directory (from `deno vendor`) or a `node_modules/`
    /// directory (Deno 2 npm support without a `package.json`).
    Deno,
}

/// Information about build artifacts that can be cleaned.
///
/// This struct contains metadata about the build directory or artifacts
/// that are candidates for cleanup, including their location and total size.
#[derive(Clone, Serialize)]
pub struct BuildArtifacts {
    /// Path to the build directory (target/ or `node_modules`/)
    ///
    /// This is the directory that will be deleted during cleanup operations.
    /// For Rust projects, this points to the `target/` directory.
    /// For Node.js projects, this points to the `node_modules/` directory.
    pub path: PathBuf,

    /// Total size of the build directory in bytes
    ///
    /// This value is calculated by recursively summing the sizes of all files
    /// within the build directory. It's used for filtering and reporting purposes.
    pub size: u64,
}

/// Representation of a development project with cleanable build artifacts.
///
/// This struct encapsulates all information about a development project,
/// including its type, location, build artifacts, and metadata extracted
/// from project configuration files.
#[derive(Clone, Serialize)]
pub struct Project {
    /// Type of the project (Rust or Node.js)
    pub kind: ProjectType,

    /// The root directory of the project where the configuration file is located
    ///
    /// For Rust projects, this is the directory containing `Cargo.toml`.
    /// For Node.js projects, this is the directory containing `package.json`.
    pub root_path: PathBuf,

    /// The build directory to be cleaned and its metadata
    ///
    /// Contains information about the `target/` or `node_modules/` directory
    /// that is a candidate for cleanup, including its path and total size.
    pub build_arts: BuildArtifacts,

    /// Name of the project extracted from configuration files
    ///
    /// For Rust projects, this is extracted from the `name` field in `Cargo.toml`.
    /// For Node.js projects, this is extracted from the `name` field in `package.json`.
    /// May be `None` if the name cannot be determined or parsed.
    pub name: Option<String>,
}

impl Project {
    /// Create a new project instance.
    ///
    /// This constructor creates a new `Project` with the specified parameters.
    /// It's typically used by the scanner when a valid development project
    /// is detected in the file system.
    ///
    /// # Arguments
    ///
    /// * `kind` - The type of project (Rust or Node.js)
    /// * `root_path` - Path to the project's root directory
    /// * `build_arts` - Information about the build artifacts to be cleaned
    /// * `name` - Optional project name extracted from configuration files
    ///
    /// # Returns
    ///
    /// A new `Project` instance with the specified parameters.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use std::path::PathBuf;
    /// # use crate::project::{Project, ProjectType, BuildArtifacts};
    /// let build_arts = BuildArtifacts {
    ///     path: PathBuf::from("/path/to/project/target"),
    ///     size: 1024,
    /// };
    ///
    /// let project = Project::new(
    ///     ProjectType::Rust,
    ///     PathBuf::from("/path/to/project"),
    ///     build_arts,
    ///     Some("my-project".to_string()),
    /// );
    /// ```
    #[must_use]
    pub const fn new(
        kind: ProjectType,
        root_path: PathBuf,
        build_arts: BuildArtifacts,
        name: Option<String>,
    ) -> Self {
        Self {
            kind,
            root_path,
            build_arts,
            name,
        }
    }
}

impl Display for Project {
    /// Format the project for display with the appropriate emoji and name.
    ///
    /// This implementation provides a human-readable representation of the project
    /// that includes:
    /// - An emoji indicator based on the project type (🦀 for Rust, 📦 for Node.js, 🐍 for Python, 🐹 for Go)
    /// - The project name if available, otherwise just the path
    /// - The project's root path
    ///
    /// # Examples
    ///
    /// - `🦀 my-rust-project (/path/to/project)`
    /// - `📦 my-node-app (/path/to/app)`
    /// - `🐍 my-python-project (/path/to/project)`
    /// - `🐹 my-go-project (/path/to/project)`
    /// - `☕ my-java-project (/path/to/project)`
    /// - `⚙️ my-cpp-project (/path/to/project)`
    /// - `🐦 my-swift-project (/path/to/project)`
    /// - `🔷 my-dotnet-project (/path/to/project)`
    /// - `🦀 /path/to/unnamed/project` (when no name is available)
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let icon = match self.kind {
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
        };

        if let Some(name) = &self.name {
            write!(f, "{icon} {name} ({})", self.root_path.display())
        } else {
            write!(f, "{icon} {}", self.root_path.display())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Helper function to create a test `BuildArtifacts`
    fn create_test_build_artifacts(path: &str, size: u64) -> BuildArtifacts {
        BuildArtifacts {
            path: PathBuf::from(path),
            size,
        }
    }

    /// Helper function to create a test Project
    fn create_test_project(
        kind: ProjectType,
        root_path: &str,
        build_path: &str,
        size: u64,
        name: Option<String>,
    ) -> Project {
        Project::new(
            kind,
            PathBuf::from(root_path),
            create_test_build_artifacts(build_path, size),
            name,
        )
    }

    #[test]
    fn test_project_type_equality() {
        assert_eq!(ProjectType::Rust, ProjectType::Rust);
        assert_eq!(ProjectType::Node, ProjectType::Node);
        assert_eq!(ProjectType::Python, ProjectType::Python);
        assert_eq!(ProjectType::Go, ProjectType::Go);
        assert_eq!(ProjectType::Java, ProjectType::Java);
        assert_eq!(ProjectType::Cpp, ProjectType::Cpp);
        assert_eq!(ProjectType::Swift, ProjectType::Swift);
        assert_eq!(ProjectType::DotNet, ProjectType::DotNet);
        assert_eq!(ProjectType::Ruby, ProjectType::Ruby);
        assert_eq!(ProjectType::Elixir, ProjectType::Elixir);
        assert_eq!(ProjectType::Deno, ProjectType::Deno);

        assert_ne!(ProjectType::Rust, ProjectType::Node);
        assert_ne!(ProjectType::Node, ProjectType::Python);
        assert_ne!(ProjectType::Python, ProjectType::Go);
        assert_ne!(ProjectType::Go, ProjectType::Java);
        assert_ne!(ProjectType::Java, ProjectType::Cpp);
        assert_ne!(ProjectType::Cpp, ProjectType::Swift);
        assert_ne!(ProjectType::Swift, ProjectType::DotNet);
        assert_ne!(ProjectType::DotNet, ProjectType::Ruby);
        assert_ne!(ProjectType::Ruby, ProjectType::Elixir);
        assert_ne!(ProjectType::Elixir, ProjectType::Deno);
    }

    #[test]
    fn test_build_artifacts_creation() {
        let artifacts = create_test_build_artifacts("/path/to/target", 1024);

        assert_eq!(artifacts.path, PathBuf::from("/path/to/target"));
        assert_eq!(artifacts.size, 1024);
    }

    #[test]
    fn test_project_new() {
        let project = create_test_project(
            ProjectType::Rust,
            "/path/to/project",
            "/path/to/project/target",
            1024,
            Some("test-project".to_string()),
        );

        assert_eq!(project.kind, ProjectType::Rust);
        assert_eq!(project.root_path, PathBuf::from("/path/to/project"));
        assert_eq!(
            project.build_arts.path,
            PathBuf::from("/path/to/project/target")
        );
        assert_eq!(project.build_arts.size, 1024);
        assert_eq!(project.name, Some("test-project".to_string()));
    }

    #[test]
    fn test_project_display_with_name() {
        let rust_project = create_test_project(
            ProjectType::Rust,
            "/path/to/rust-project",
            "/path/to/rust-project/target",
            1024,
            Some("my-rust-app".to_string()),
        );

        let expected = "🦀 my-rust-app (/path/to/rust-project)";
        assert_eq!(format!("{rust_project}"), expected);

        let node_project = create_test_project(
            ProjectType::Node,
            "/path/to/node-project",
            "/path/to/node-project/node_modules",
            2048,
            Some("my-node-app".to_string()),
        );

        let expected = "📦 my-node-app (/path/to/node-project)";
        assert_eq!(format!("{node_project}"), expected);

        let python_project = create_test_project(
            ProjectType::Python,
            "/path/to/python-project",
            "/path/to/python-project/__pycache__",
            512,
            Some("my-python-app".to_string()),
        );

        let expected = "🐍 my-python-app (/path/to/python-project)";
        assert_eq!(format!("{python_project}"), expected);

        let go_project = create_test_project(
            ProjectType::Go,
            "/path/to/go-project",
            "/path/to/go-project/vendor",
            4096,
            Some("my-go-app".to_string()),
        );

        let expected = "🐹 my-go-app (/path/to/go-project)";
        assert_eq!(format!("{go_project}"), expected);

        let java_project = create_test_project(
            ProjectType::Java,
            "/path/to/java-project",
            "/path/to/java-project/target",
            8192,
            Some("my-java-app".to_string()),
        );

        let expected = "☕ my-java-app (/path/to/java-project)";
        assert_eq!(format!("{java_project}"), expected);

        let cpp_project = create_test_project(
            ProjectType::Cpp,
            "/path/to/cpp-project",
            "/path/to/cpp-project/build",
            2048,
            Some("my-cpp-app".to_string()),
        );

        let expected = "⚙\u{fe0f} my-cpp-app (/path/to/cpp-project)";
        assert_eq!(format!("{cpp_project}"), expected);

        let swift_project = create_test_project(
            ProjectType::Swift,
            "/path/to/swift-project",
            "/path/to/swift-project/.build",
            1024,
            Some("my-swift-app".to_string()),
        );

        let expected = "🐦 my-swift-app (/path/to/swift-project)";
        assert_eq!(format!("{swift_project}"), expected);

        let dotnet_project = create_test_project(
            ProjectType::DotNet,
            "/path/to/dotnet-project",
            "/path/to/dotnet-project/obj",
            4096,
            Some("my-dotnet-app".to_string()),
        );

        let expected = "🔷 my-dotnet-app (/path/to/dotnet-project)";
        assert_eq!(format!("{dotnet_project}"), expected);

        let ruby_project = create_test_project(
            ProjectType::Ruby,
            "/path/to/ruby-project",
            "/path/to/ruby-project/vendor/bundle",
            2048,
            Some("my-ruby-gem".to_string()),
        );

        let expected = "💎 my-ruby-gem (/path/to/ruby-project)";
        assert_eq!(format!("{ruby_project}"), expected);

        let elixir_project = create_test_project(
            ProjectType::Elixir,
            "/path/to/elixir-project",
            "/path/to/elixir-project/_build",
            1024,
            Some("my_elixir_app".to_string()),
        );

        let expected = "💧 my_elixir_app (/path/to/elixir-project)";
        assert_eq!(format!("{elixir_project}"), expected);

        let deno_project = create_test_project(
            ProjectType::Deno,
            "/path/to/deno-project",
            "/path/to/deno-project/vendor",
            512,
            Some("my-deno-app".to_string()),
        );

        let expected = "🦕 my-deno-app (/path/to/deno-project)";
        assert_eq!(format!("{deno_project}"), expected);
    }

    #[test]
    fn test_project_display_without_name() {
        let rust_project = create_test_project(
            ProjectType::Rust,
            "/path/to/unnamed-project",
            "/path/to/unnamed-project/target",
            1024,
            None,
        );

        let expected = "🦀 /path/to/unnamed-project";
        assert_eq!(format!("{rust_project}"), expected);

        let node_project = create_test_project(
            ProjectType::Node,
            "/some/other/path",
            "/some/other/path/node_modules",
            2048,
            None,
        );

        let expected = "📦 /some/other/path";
        assert_eq!(format!("{node_project}"), expected);
    }

    #[test]
    fn test_project_clone() {
        let original = create_test_project(
            ProjectType::Rust,
            "/original/path",
            "/original/path/target",
            1024,
            Some("original-project".to_string()),
        );

        let cloned = original.clone();

        assert_eq!(original.kind, cloned.kind);
        assert_eq!(original.root_path, cloned.root_path);
        assert_eq!(original.build_arts.path, cloned.build_arts.path);
        assert_eq!(original.build_arts.size, cloned.build_arts.size);
        assert_eq!(original.name, cloned.name);
    }

    #[test]
    fn test_build_artifacts_clone() {
        let original = create_test_build_artifacts("/test/path", 2048);
        let cloned = original.clone();

        assert_eq!(original.path, cloned.path);
        assert_eq!(original.size, cloned.size);
    }

    #[test]
    fn test_project_with_zero_size() {
        let project = create_test_project(
            ProjectType::Python,
            "/empty/project",
            "/empty/project/__pycache__",
            0,
            Some("empty-project".to_string()),
        );

        assert_eq!(project.build_arts.size, 0);
        assert_eq!(format!("{project}"), "🐍 empty-project (/empty/project)");
    }

    #[test]
    fn test_project_with_large_size() {
        let large_size = u64::MAX;
        let project = create_test_project(
            ProjectType::Go,
            "/large/project",
            "/large/project/vendor",
            large_size,
            Some("huge-project".to_string()),
        );

        assert_eq!(project.build_arts.size, large_size);
    }
}
