//! Directory scanning and project detection functionality.
//!
//! This module provides the core scanning logic that traverses directory trees
//! to find development projects and their build artifacts. It supports parallel
//! processing for improved performance and handles various error conditions
//! gracefully.

use std::{
    fs,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde_json::{Value, from_str};
use walkdir::{DirEntry, WalkDir};

use crate::{
    config::{ProjectFilter, ScanOptions},
    project::{BuildArtifacts, Project, ProjectType},
};

/// Directory scanner for detecting development projects.
///
/// The `Scanner` struct encapsulates the logic for traversing directory trees
/// and identifying development projects (Rust and Node.js) along with their
/// build artifacts. It supports configurable filtering and parallel processing
/// for efficient scanning of large directory structures.
pub struct Scanner {
    /// Configuration options for scanning behavior
    scan_options: ScanOptions,

    /// Filter to restrict scanning to specific project types
    project_filter: ProjectFilter,

    /// When `true`, suppresses progress spinner output (used by `--json` mode).
    quiet: bool,
}

impl Scanner {
    /// Create a new scanner with the specified options.
    ///
    /// # Arguments
    ///
    /// * `scan_options` - Configuration for scanning behavior (threads, verbosity, etc.)
    /// * `project_filter` - Filter to restrict scanning to specific project types
    ///
    /// # Returns
    ///
    /// A new `Scanner` instance configured with the provided options.
    ///
    /// # Examples
    ///
    /// ```
    /// # use crate::{Scanner, ScanOptions, ProjectFilter};
    /// let scan_options = ScanOptions {
    ///     verbose: true,
    ///     threads: 4,
    ///     skip: vec![],
    /// };
    ///
    /// let scanner = Scanner::new(scan_options, ProjectFilter::All);
    /// ```
    #[must_use]
    pub const fn new(scan_options: ScanOptions, project_filter: ProjectFilter) -> Self {
        Self {
            scan_options,
            project_filter,
            quiet: false,
        }
    }

    /// Enable or disable quiet mode (suppresses progress spinner).
    ///
    /// When quiet mode is active the scanning spinner is hidden, which is
    /// required for `--json` output so that only the final JSON is printed.
    #[must_use]
    pub const fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    /// Scan a directory tree for development projects.
    ///
    /// This method performs a recursive scan of the specified directory to find
    /// development projects. It operates in two phases:
    /// 1. Directory traversal to identify potential projects
    /// 2. Parallel size calculation for build directories
    ///
    /// # Arguments
    ///
    /// * `root` - The root directory to start scanning from
    ///
    /// # Returns
    ///
    /// A vector of `Project` instances representing all detected projects with
    /// non-zero build directory sizes.
    ///
    /// # Panics
    ///
    /// This method may panic if the progress bar template string is invalid,
    /// though this should not occur under normal circumstances as the template
    /// is hardcoded and valid.
    ///
    /// # Examples
    ///
    /// ```
    /// # use std::path::Path;
    /// # use crate::Scanner;
    /// let projects = scanner.scan_directory(Path::new("/path/to/projects"));
    /// println!("Found {} projects", projects.len());
    /// ```
    ///
    /// # Performance
    ///
    /// This method uses parallel processing for both directory traversal and
    /// size calculation to maximize performance on systems with multiple cores
    /// and fast storage.
    pub fn scan_directory(&self, root: &Path) -> Vec<Project> {
        let errors = Arc::new(Mutex::new(Vec::<String>::new()));

        let progress = if self.quiet {
            ProgressBar::hidden()
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} {msg}")
                    .unwrap(),
            );
            pb.set_message("Scanning...");
            pb.enable_steady_tick(std::time::Duration::from_millis(100));
            pb
        };

        let found_count = Arc::new(AtomicUsize::new(0));
        let progress_clone = progress.clone();
        let count_clone = Arc::clone(&found_count);

        // Find all potential project directories
        let potential_projects: Vec<_> = WalkDir::new(root)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| self.should_scan_entry(entry))
            .collect::<Vec<_>>()
            .into_par_iter()
            .filter_map(|entry| {
                let result = self.detect_project(&entry, &errors);
                if result.is_some() {
                    let n = count_clone.fetch_add(1, Ordering::Relaxed) + 1;
                    progress_clone.set_message(format!("Scanning... {n} found"));
                }
                result
            })
            .collect();

        progress.finish_with_message("✅ Directory scan complete");

        // Process projects in parallel to calculate sizes
        let projects_with_sizes: Vec<_> = potential_projects
            .into_par_iter()
            .filter_map(|mut project| {
                for artifact in &mut project.build_arts {
                    if artifact.size == 0 {
                        artifact.size = Self::calculate_build_dir_size(&artifact.path);
                    }
                }

                if project.total_size() > 0 {
                    Some(project)
                } else {
                    None
                }
            })
            .collect();

        // Print errors if verbose
        if self.scan_options.verbose {
            let errors = errors.lock().unwrap();
            for error in errors.iter() {
                eprintln!("{}", error.red());
            }
        }

        projects_with_sizes
    }

    /// Calculate the total size of a build directory.
    ///
    /// This method recursively traverses the specified directory and sums up
    /// the sizes of all files contained within it. It handles errors gracefully
    /// and optionally reports them in verbose mode.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the build directory to measure
    ///
    /// # Returns
    ///
    /// The total size of all files in the directory, in bytes. Returns 0 if
    /// the directory doesn't exist or cannot be accessed.
    ///
    /// # Performance
    ///
    /// This method can be CPU and I/O intensive for large directories with
    /// many files. It's designed to be called in parallel for multiple
    /// directories to maximize throughput.
    fn calculate_build_dir_size(path: &Path) -> u64 {
        if !path.exists() {
            return 0;
        }

        crate::utils::calculate_dir_size(path)
    }

    /// Detect a Node.js project in the specified directory.
    ///
    /// This method checks for the presence of both `package.json` and `node_modules/`
    /// directory to identify a Node.js project. If found, it attempts to extract
    /// the project name from the `package.json` file.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to check for Node.js project
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(Project)` if a valid Node.js project is detected
    /// - `None` if the directory doesn't contain a Node.js project
    ///
    /// # Detection Criteria
    ///
    /// 1. `package.json` file exists in directory
    /// 2. `node_modules/` subdirectory exists in directory
    /// 3. The project name is extracted from `package.json` if possible
    fn detect_node_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let package_json = path.join("package.json");
        let node_modules = path.join("node_modules");

        if package_json.exists() && node_modules.exists() {
            let name = self.extract_node_project_name(&package_json, errors);

            let build_arts = vec![BuildArtifacts {
                path: path.join("node_modules"),
                size: 0, // Will be calculated later
            }];

            return Some(Project::new(
                ProjectType::Node,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        None
    }

    /// Detect if a directory entry represents a development project.
    ///
    /// This method examines a directory entry and determines if it contains
    /// a development project based on the presence of characteristic files
    /// and directories. It respects the project filter settings.
    ///
    /// # Arguments
    ///
    /// * `entry` - The directory entry to examine
    /// * `errors` - Shared error collection for reporting issues
    ///
    /// # Returns
    ///
    /// - `Some(Project)` if a valid project is detected
    /// - `None` if no project is found or the entry doesn't match filters
    ///
    /// # Project Detection Logic
    ///
    /// - **Rust projects**: Presence of both `Cargo.toml` and `target/` directory
    /// - **Deno projects**: Presence of `deno.json`/`deno.jsonc` with `vendor/` or `node_modules/`
    /// - **Node.js projects**: Presence of both `package.json` and `node_modules/` directory
    /// - **Python projects**: Presence of configuration files and cache directories
    /// - **Go projects**: Presence of both `go.mod` and `vendor/` directory
    /// - **Java/Kotlin projects**: Presence of `pom.xml` or `build.gradle` with `target/` or `build/`
    /// - **C/C++ projects**: Presence of `CMakeLists.txt` or `Makefile` with `build/`
    /// - **Swift projects**: Presence of `Package.swift` with `.build/`
    /// - **.NET/C# projects**: Presence of `.csproj` files with `bin/` or `obj/`
    /// - **Ruby projects**: Presence of `Gemfile` with `.bundle/` or `vendor/bundle/`
    /// - **Elixir projects**: Presence of `mix.exs` with `_build/`
    fn detect_project(
        &self,
        entry: &DirEntry,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let path = entry.path();

        if !entry.file_type().is_dir() {
            return None;
        }

        // Detectors are tried in order; the first match wins.
        // More specific ecosystems are checked before more generic ones
        // (e.g. Java before C/C++, since both can use `build/`; Deno before
        // Node since Deno 2 projects may also have a node_modules/).
        self.try_detect(ProjectFilter::Rust, || {
            self.detect_rust_project(path, errors)
        })
        .or_else(|| {
            self.try_detect(ProjectFilter::Deno, || {
                self.detect_deno_project(path, errors)
            })
        })
        .or_else(|| {
            self.try_detect(ProjectFilter::Node, || {
                self.detect_node_project(path, errors)
            })
        })
        .or_else(|| {
            self.try_detect(ProjectFilter::Java, || {
                self.detect_java_project(path, errors)
            })
        })
        .or_else(|| {
            self.try_detect(ProjectFilter::Swift, || {
                self.detect_swift_project(path, errors)
            })
        })
        .or_else(|| self.try_detect(ProjectFilter::DotNet, || Self::detect_dotnet_project(path)))
        .or_else(|| {
            self.try_detect(ProjectFilter::Python, || {
                self.detect_python_project(path, errors)
            })
        })
        .or_else(|| self.try_detect(ProjectFilter::Go, || self.detect_go_project(path, errors)))
        .or_else(|| self.try_detect(ProjectFilter::Cpp, || self.detect_cpp_project(path, errors)))
        .or_else(|| {
            self.try_detect(ProjectFilter::Ruby, || {
                self.detect_ruby_project(path, errors)
            })
        })
        .or_else(|| {
            self.try_detect(ProjectFilter::Elixir, || {
                self.detect_elixir_project(path, errors)
            })
        })
    }

    /// Run a detector only if the current project filter allows it.
    ///
    /// Returns `None` immediately (without calling `detect`) when the
    /// active filter doesn't include `filter`.
    fn try_detect(
        &self,
        filter: ProjectFilter,
        detect: impl FnOnce() -> Option<Project>,
    ) -> Option<Project> {
        if self.project_filter == ProjectFilter::All || self.project_filter == filter {
            detect()
        } else {
            None
        }
    }

    /// Detect a Rust project in the specified directory.
    ///
    /// This method checks for the presence of both `Cargo.toml` and `target/`
    /// directory to identify a Rust project. If found, it attempts to extract
    /// the project name from the `Cargo.toml` file.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to check for a Rust project
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(Project)` if a valid Rust project is detected
    /// - `None` if the directory doesn't contain a Rust project
    ///
    /// # Detection Criteria
    ///
    /// 1. `Cargo.toml` file exists in directory
    /// 2. `target/` subdirectory exists in directory
    /// 3. The project name is extracted from `Cargo.toml` if possible
    fn detect_rust_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let cargo_toml = path.join("Cargo.toml");
        let target_dir = path.join("target");

        if cargo_toml.exists() && target_dir.exists() {
            // Skip workspace members — their artifacts are managed by the workspace root.
            if Self::is_inside_cargo_workspace(path) {
                return None;
            }

            let name = self.extract_rust_project_name(&cargo_toml, errors);

            let build_arts = vec![BuildArtifacts {
                path: path.join("target"),
                size: 0, // Will be calculated later
            }];

            return Some(Project::new(
                ProjectType::Rust,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        None
    }

    /// Return true if the given `Cargo.toml` declares a `[workspace]` section.
    fn is_cargo_workspace_root(cargo_toml: &Path) -> bool {
        fs::read_to_string(cargo_toml)
            .map(|content| content.lines().any(|line| line.trim() == "[workspace]"))
            .unwrap_or(false)
    }

    /// Return true if `path` is inside a Rust workspace (an ancestor directory
    /// contains a `Cargo.toml` that declares `[workspace]`).
    fn is_inside_cargo_workspace(path: &Path) -> bool {
        path.ancestors()
            .skip(1) // skip `path` itself
            .any(|ancestor| {
                let cargo_toml = ancestor.join("Cargo.toml");
                cargo_toml.exists() && Self::is_cargo_workspace_root(&cargo_toml)
            })
    }

    /// Extract the project name from a Cargo.toml file.
    ///
    /// This method performs simple TOML parsing to extract the project name
    /// from a Rust project's `Cargo.toml` file. It uses a line-by-line approach
    /// rather than a full TOML parser for simplicity and performance.
    ///
    /// # Arguments
    ///
    /// * `cargo_toml` - Path to the Cargo.toml file
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(String)` containing the project name if successfully extracted
    /// - `None` if the name cannot be found or parsed
    ///
    /// # Parsing Strategy
    ///
    /// The method looks for lines matching the pattern `name = "project_name"`
    /// and extracts the quoted string value. This trivial approach handles
    /// most common cases without requiring a full TOML parser.
    fn extract_rust_project_name(
        &self,
        cargo_toml: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let content = self.read_file_content(cargo_toml, errors)?;
        Self::parse_toml_name_field(&content)
    }

    /// Extract a quoted string value from a line.
    fn extract_quoted_value(line: &str) -> Option<String> {
        let start = line.find('"')?;
        let end = line.rfind('"')?;

        if start == end {
            return None;
        }

        Some(line[start + 1..end].to_string())
    }

    /// Extract the name from a single TOML line if it contains a name field.
    fn extract_name_from_line(line: &str) -> Option<String> {
        if !Self::is_name_line(line) {
            return None;
        }

        Self::extract_quoted_value(line)
    }

    /// Extract the project name from a package.json file.
    ///
    /// This method parses a Node.js project's `package.json` file to extract
    /// the project name. It uses full JSON parsing to handle the file format
    /// correctly and safely.
    ///
    /// # Arguments
    ///
    /// * `package_json` - Path to the package.json file
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(String)` containing the project name if successfully extracted
    /// - `None` if the name cannot be found, parsed, or the file is invalid
    ///
    /// # Error Handling
    ///
    /// This method handles both file I/O errors and JSON parsing errors gracefully.
    /// Errors are optionally reported to the shared error collection in verbose mode.
    fn extract_node_project_name(
        &self,
        package_json: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        match fs::read_to_string(package_json) {
            Ok(content) => match from_str::<Value>(&content) {
                Ok(json) => json
                    .get("name")
                    .and_then(|v| v.as_str())
                    .map(std::string::ToString::to_string),
                Err(e) => {
                    if self.scan_options.verbose {
                        errors
                            .lock()
                            .unwrap()
                            .push(format!("Error parsing {}: {e}", package_json.display()));
                    }
                    None
                }
            },
            Err(e) => {
                if self.scan_options.verbose {
                    errors
                        .lock()
                        .unwrap()
                        .push(format!("Error reading {}: {e}", package_json.display()));
                }
                None
            }
        }
    }

    /// Check if a line contains a name field assignment.
    fn is_name_line(line: &str) -> bool {
        line.starts_with("name") && line.contains('=')
    }

    /// Log a file reading error if verbose mode is enabled.
    fn log_file_error(
        &self,
        file_path: &Path,
        error: &std::io::Error,
        errors: &Arc<Mutex<Vec<String>>>,
    ) {
        if self.scan_options.verbose {
            errors
                .lock()
                .unwrap()
                .push(format!("Error reading {}: {error}", file_path.display()));
        }
    }

    /// Parse the name field from TOML content.
    fn parse_toml_name_field(content: &str) -> Option<String> {
        for line in content.lines() {
            if let Some(name) = Self::extract_name_from_line(line.trim()) {
                return Some(name);
            }
        }
        None
    }

    /// Read the content of a file and handle errors appropriately.
    fn read_file_content(
        &self,
        file_path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        match fs::read_to_string(file_path) {
            Ok(content) => Some(content),
            Err(e) => {
                self.log_file_error(file_path, &e, errors);
                None
            }
        }
    }

    /// Determine if a directory entry should be scanned for projects.
    ///
    /// This method implements the filtering logic to decide whether a directory
    /// should be traversed during the scanning process. It applies various
    /// exclusion rules to improve performance and avoid scanning irrelevant
    /// directories.
    ///
    /// # Arguments
    ///
    /// * `entry` - The directory entry to evaluate
    ///
    /// # Returns
    ///
    /// - `true` if the directory should be scanned
    /// - `false` if the directory should be skipped
    ///
    /// # Exclusion Rules
    ///
    /// The following directories are excluded from scanning:
    /// - Directories in the user-specified skip list
    /// - Any directory inside a `node_modules/` directory (to avoid deep nesting)
    /// - Hidden directories (starting with `.`) except `.cargo`
    /// - Common build/temporary directories: `target`, `build`, `dist`, `out`, etc.
    /// - Version control directories: `.git`, `.svn`, `.hg`
    /// - Python cache and virtual environment directories
    /// - Temporary directories: `temp`, `tmp`
    /// - Go vendor directory
    /// - Python pytest cache
    /// - Python tox environments
    /// - Python setuptools
    /// - Python coverage files
    /// - Node.js modules (already handled above but added for completeness)
    /// - .NET `obj/` directory
    fn should_scan_entry(&self, entry: &DirEntry) -> bool {
        let path = entry.path();

        // Early return if path is in skip list
        if self.is_path_in_skip_list(path) {
            return false;
        }

        // Skip any directory inside a node_modules directory
        if path
            .ancestors()
            .any(|ancestor| ancestor.file_name().and_then(|n| n.to_str()) == Some("node_modules"))
        {
            return false;
        }

        // Skip hidden directories (except .cargo for Rust)
        if Self::is_hidden_directory_to_skip(path) {
            return false;
        }

        // Skip common non-project directories
        !Self::is_excluded_directory(path)
    }

    /// Check if a path is in the skip list
    fn is_path_in_skip_list(&self, path: &Path) -> bool {
        self.scan_options.skip.iter().any(|skip| {
            path.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|name| name == skip.to_string_lossy())
            })
        })
    }

    /// Check if directory is hidden and should be skipped
    fn is_hidden_directory_to_skip(path: &Path) -> bool {
        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| name.starts_with('.') && name != ".cargo")
    }

    /// Check if directory is in the excluded list
    fn is_excluded_directory(path: &Path) -> bool {
        let excluded_dirs = [
            "target",
            "build",
            "dist",
            "out",
            ".git",
            ".svn",
            ".hg",
            "__pycache__",
            "venv",
            ".venv",
            "env",
            ".env",
            "temp",
            "tmp",
            "vendor",
            ".pytest_cache",
            ".tox",
            ".eggs",
            ".coverage",
            "node_modules",
            "obj",
            "_build",
        ];

        path.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|name| excluded_dirs.contains(&name))
    }

    /// Detect a Python project in the specified directory.
    ///
    /// This method checks for Python configuration files and associated cache directories.
    /// It looks for multiple build artifacts that can be cleaned.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to check for a Python project
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(Project)` if a valid Python project is detected
    /// - `None` if the directory doesn't contain a Python project
    ///
    /// # Detection Criteria
    ///
    /// A Python project is identified by having:
    /// 1. At least one of: requirements.txt, setup.py, pyproject.toml, setup.cfg, Pipfile
    /// 2. At least one of the cache/build directories: `__pycache__`, `.pytest_cache`, venv, .venv, build, dist, .eggs
    fn detect_python_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let config_files = [
            "requirements.txt",
            "setup.py",
            "pyproject.toml",
            "setup.cfg",
            "Pipfile",
            "pipenv.lock",
            "poetry.lock",
        ];

        let build_dirs = [
            "__pycache__",
            ".pytest_cache",
            "venv",
            ".venv",
            "build",
            "dist",
            ".eggs",
            ".tox",
            ".coverage",
        ];

        // Check if any config file exists
        let has_config = config_files.iter().any(|&file| path.join(file).exists());

        if !has_config {
            return None;
        }

        // Collect all existing cache/build directories.
        let mut build_arts: Vec<BuildArtifacts> = build_dirs
            .iter()
            .filter_map(|&dir_name| {
                let dir_path = path.join(dir_name);
                if dir_path.exists() && dir_path.is_dir() {
                    let size = crate::utils::calculate_dir_size(&dir_path);
                    Some(BuildArtifacts {
                        path: dir_path,
                        size,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Also collect any *.egg-info directories present in the project root.
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir()
                    && entry_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.ends_with(".egg-info"))
                {
                    let size = crate::utils::calculate_dir_size(&entry_path);
                    build_arts.push(BuildArtifacts {
                        path: entry_path,
                        size,
                    });
                }
            }
        }

        if build_arts.is_empty() {
            return None;
        }

        let name = self.extract_python_project_name(path, errors);

        Some(Project::new(
            ProjectType::Python,
            path.to_path_buf(),
            build_arts,
            name,
        ))
    }

    /// Detect a Go project in the specified directory.
    ///
    /// This method checks for the presence of both `go.mod` and `vendor/`
    /// directory to identify a Go project. If found, it attempts to extract
    /// the project name from the `go.mod` file.
    ///
    /// # Arguments
    ///
    /// * `path` - Directory path to check for a Go project
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(Project)` if a valid Go project is detected
    /// - `None` if the directory doesn't contain a Go project
    ///
    /// # Detection Criteria
    ///
    /// 1. `go.mod` file exists in directory
    /// 2. `vendor/` subdirectory exists in directory
    /// 3. The project name is extracted from `go.mod` if possible
    fn detect_go_project(&self, path: &Path, errors: &Arc<Mutex<Vec<String>>>) -> Option<Project> {
        let go_mod = path.join("go.mod");
        let vendor_dir = path.join("vendor");

        if go_mod.exists() && vendor_dir.exists() {
            let name = self.extract_go_project_name(&go_mod, errors);

            let build_arts = vec![BuildArtifacts {
                path: path.join("vendor"),
                size: 0, // Will be calculated later
            }];

            return Some(Project::new(
                ProjectType::Go,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        None
    }

    /// Extract the project name from a Python project directory.
    ///
    /// This method attempts to extract the project name from various Python
    /// configuration files in order of preference.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the Python project directory
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(String)` containing the project name if successfully extracted
    /// - `None` if the name cannot be found or parsed
    ///
    /// # Extraction Order
    ///
    /// 1. pyproject.toml (from [project] name or [tool.poetry] name)
    /// 2. setup.py (from name= parameter)
    /// 3. setup.cfg (from [metadata] name)
    /// 4. Use directory name as a fallback
    fn extract_python_project_name(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        // Try files in order of preference
        self.try_extract_from_pyproject_toml(path, errors)
            .or_else(|| self.try_extract_from_setup_py(path, errors))
            .or_else(|| self.try_extract_from_setup_cfg(path, errors))
            .or_else(|| Self::fallback_to_directory_name(path))
    }

    /// Try to extract project name from pyproject.toml
    fn try_extract_from_pyproject_toml(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let pyproject_toml = path.join("pyproject.toml");
        if !pyproject_toml.exists() {
            return None;
        }

        let content = self.read_file_content(&pyproject_toml, errors)?;
        Self::extract_name_from_toml_like_content(&content)
    }

    /// Try to extract project name from setup.py
    fn try_extract_from_setup_py(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let setup_py = path.join("setup.py");
        if !setup_py.exists() {
            return None;
        }

        let content = self.read_file_content(&setup_py, errors)?;
        Self::extract_name_from_python_content(&content)
    }

    /// Try to extract project name from setup.cfg
    fn try_extract_from_setup_cfg(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let setup_cfg = path.join("setup.cfg");
        if !setup_cfg.exists() {
            return None;
        }

        let content = self.read_file_content(&setup_cfg, errors)?;
        Self::extract_name_from_cfg_content(&content)
    }

    /// Extract name from TOML-like content (pyproject.toml)
    fn extract_name_from_toml_like_content(content: &str) -> Option<String> {
        content
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("name") && line.contains('='))
            .and_then(Self::extract_quoted_value)
    }

    /// Extract name from Python content (setup.py)
    fn extract_name_from_python_content(content: &str) -> Option<String> {
        content
            .lines()
            .map(str::trim)
            .find(|line| line.contains("name") && line.contains('='))
            .and_then(Self::extract_quoted_value)
    }

    /// Extract name from INI-style configuration content (setup.cfg)
    fn extract_name_from_cfg_content(content: &str) -> Option<String> {
        let mut in_metadata_section = false;

        for line in content.lines() {
            let line = line.trim();

            if line == "[metadata]" {
                in_metadata_section = true;
            } else if line.starts_with('[') && line.ends_with(']') {
                in_metadata_section = false;
            } else if in_metadata_section && line.starts_with("name") && line.contains('=') {
                return line.split('=').nth(1).map(|name| name.trim().to_string());
            }
        }

        None
    }

    /// Fallback to directory name
    fn fallback_to_directory_name(path: &Path) -> Option<String> {
        path.file_name()
            .and_then(|name| name.to_str())
            .map(std::string::ToString::to_string)
    }

    /// Extract the project name from a `go.mod` file.
    ///
    /// This method parses a Go project's `go.mod` file to extract
    /// the module name, which typically represents the project.
    ///
    /// # Arguments
    ///
    /// * `go_mod` - Path to the `go.mod` file
    /// * `errors` - Shared error collection for reporting parsing issues
    ///
    /// # Returns
    ///
    /// - `Some(String)` containing the module name if successfully extracted
    /// - `None` if the name cannot be found or parsed
    ///
    /// # Parsing Strategy
    ///
    /// The method looks for the first line starting with `module ` and extracts
    /// the module path. For better display, it takes the last component of the path.
    fn extract_go_project_name(
        &self,
        go_mod: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let content = self.read_file_content(go_mod, errors)?;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("module ") {
                let module_path = line.strip_prefix("module ")?.trim();

                // Take the last component of the module path for a cleaner name
                if let Some(name) = module_path.split('/').next_back() {
                    return Some(name.to_string());
                }

                return Some(module_path.to_string());
            }
        }

        None
    }

    /// Detect a Java/Kotlin project in the specified directory.
    ///
    /// This method checks for Maven (`pom.xml`) or Gradle (`build.gradle`,
    /// `build.gradle.kts`) configuration files and their associated build output
    /// directories (`target/` for Maven, `build/` for Gradle).
    ///
    /// # Detection Criteria
    ///
    /// 1. `pom.xml` + `target/` directory (Maven)
    /// 2. `build.gradle` or `build.gradle.kts` + `build/` directory (Gradle)
    fn detect_java_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let pom_xml = path.join("pom.xml");
        let target_dir = path.join("target");

        // Maven project: pom.xml + target/
        if pom_xml.exists() && target_dir.exists() {
            let name = self.extract_java_maven_project_name(&pom_xml, errors);

            let build_arts = vec![BuildArtifacts {
                path: target_dir,
                size: 0,
            }];

            return Some(Project::new(
                ProjectType::Java,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        // Gradle project: build.gradle(.kts) + build/
        let has_gradle =
            path.join("build.gradle").exists() || path.join("build.gradle.kts").exists();
        let build_dir = path.join("build");

        if has_gradle && build_dir.exists() {
            let name = self.extract_java_gradle_project_name(path, errors);

            let build_arts = vec![BuildArtifacts {
                path: build_dir,
                size: 0,
            }];

            return Some(Project::new(
                ProjectType::Java,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        None
    }

    /// Extract the project name from a Maven `pom.xml` file.
    ///
    /// Looks for `<artifactId>` tags and extracts the text content.
    fn extract_java_maven_project_name(
        &self,
        pom_xml: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let content = self.read_file_content(pom_xml, errors)?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("<artifactId>") && trimmed.ends_with("</artifactId>") {
                let name = trimmed
                    .strip_prefix("<artifactId>")?
                    .strip_suffix("</artifactId>")?;
                return Some(name.to_string());
            }
        }

        None
    }

    /// Extract the project name from a Gradle project.
    ///
    /// Looks for `settings.gradle` or `settings.gradle.kts` and extracts
    /// the `rootProject.name` value. Falls back to directory name.
    fn extract_java_gradle_project_name(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        for settings_file in &["settings.gradle", "settings.gradle.kts"] {
            let settings_path = path.join(settings_file);
            if settings_path.exists()
                && let Some(content) = self.read_file_content(&settings_path, errors)
            {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains("rootProject.name") && trimmed.contains('=') {
                        return Self::extract_quoted_value(trimmed).or_else(|| {
                            trimmed
                                .split('=')
                                .nth(1)
                                .map(|s| s.trim().trim_matches('\'').to_string())
                        });
                    }
                }
            }
        }

        Self::fallback_to_directory_name(path)
    }

    /// Detect a C/C++ project in the specified directory.
    ///
    /// This method checks for `CMakeLists.txt` or `Makefile` alongside a `build/`
    /// directory to identify C/C++ projects.
    ///
    /// # Detection Criteria
    ///
    /// 1. `CMakeLists.txt` + `build/` directory (`CMake`)
    /// 2. `Makefile` + `build/` directory (`Make`)
    fn detect_cpp_project(&self, path: &Path, errors: &Arc<Mutex<Vec<String>>>) -> Option<Project> {
        let build_dir = path.join("build");

        if !build_dir.exists() {
            return None;
        }

        let cmake_file = path.join("CMakeLists.txt");
        let makefile = path.join("Makefile");

        if cmake_file.exists() || makefile.exists() {
            let name = if cmake_file.exists() {
                self.extract_cpp_cmake_project_name(&cmake_file, errors)
            } else {
                Self::fallback_to_directory_name(path)
            };

            let build_arts = vec![BuildArtifacts {
                path: build_dir,
                size: 0,
            }];

            return Some(Project::new(
                ProjectType::Cpp,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        None
    }

    /// Extract the project name from a `CMakeLists.txt` file.
    ///
    /// Looks for `project(name` patterns and extracts the project name.
    fn extract_cpp_cmake_project_name(
        &self,
        cmake_file: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let content = self.read_file_content(cmake_file, errors)?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("project(") || trimmed.starts_with("PROJECT(") {
                let inner = trimmed
                    .trim_start_matches("project(")
                    .trim_start_matches("PROJECT(")
                    .trim_end_matches(')')
                    .trim();

                // The project name is the first word/token
                let name = inner.split_whitespace().next()?;
                // Remove possible surrounding quotes
                let name = name.trim_matches('"').trim_matches('\'');
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
        }

        Self::fallback_to_directory_name(cmake_file.parent()?)
    }

    /// Detect a Swift project in the specified directory.
    ///
    /// This method checks for a `Package.swift` manifest and the `.build/`
    /// directory to identify Swift Package Manager projects.
    ///
    /// # Detection Criteria
    ///
    /// 1. `Package.swift` file exists
    /// 2. `.build/` directory exists
    fn detect_swift_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let package_swift = path.join("Package.swift");
        let build_dir = path.join(".build");

        if package_swift.exists() && build_dir.exists() {
            let name = self.extract_swift_project_name(&package_swift, errors);

            let build_arts = vec![BuildArtifacts {
                path: build_dir,
                size: 0,
            }];

            return Some(Project::new(
                ProjectType::Swift,
                path.to_path_buf(),
                build_arts,
                name,
            ));
        }

        None
    }

    /// Extract the project name from a `Package.swift` file.
    ///
    /// Looks for `name:` inside the `Package(` initializer.
    fn extract_swift_project_name(
        &self,
        package_swift: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let content = self.read_file_content(package_swift, errors)?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("name:") {
                return Self::extract_quoted_value(trimmed);
            }
        }

        Self::fallback_to_directory_name(package_swift.parent()?)
    }

    /// Detect a .NET/C# project in the specified directory.
    ///
    /// This method checks for `.csproj` files alongside `bin/` and/or `obj/`
    /// directories to identify .NET projects.
    ///
    /// # Detection Criteria
    ///
    /// 1. At least one `.csproj` file exists in the directory
    /// 2. At least one of `bin/` or `obj/` directories exists
    fn detect_dotnet_project(path: &Path) -> Option<Project> {
        let bin_dir = path.join("bin");
        let obj_dir = path.join("obj");

        let has_build_dir = bin_dir.exists() || obj_dir.exists();
        if !has_build_dir {
            return None;
        }

        let csproj_file = Self::find_file_with_extension(path, "csproj")?;

        // Collect bin/ and obj/ as separate build artifacts (both when present).
        let build_arts: Vec<BuildArtifacts> = match (bin_dir.exists(), obj_dir.exists()) {
            (true, true) => {
                let bin_size = crate::utils::calculate_dir_size(&bin_dir);
                let obj_size = crate::utils::calculate_dir_size(&obj_dir);
                vec![
                    BuildArtifacts {
                        path: bin_dir,
                        size: bin_size,
                    },
                    BuildArtifacts {
                        path: obj_dir,
                        size: obj_size,
                    },
                ]
            }
            (true, false) => vec![BuildArtifacts {
                path: bin_dir,
                size: 0,
            }],
            (false, true) => vec![BuildArtifacts {
                path: obj_dir,
                size: 0,
            }],
            (false, false) => return None,
        };

        let name = csproj_file
            .file_stem()
            .and_then(|s| s.to_str())
            .map(std::string::ToString::to_string);

        Some(Project::new(
            ProjectType::DotNet,
            path.to_path_buf(),
            build_arts,
            name,
        ))
    }

    /// Find the first file with a given extension in a directory.
    fn find_file_with_extension(dir: &Path, extension: &str) -> Option<std::path::PathBuf> {
        let entries = fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some(extension) {
                return Some(path);
            }
        }
        None
    }

    /// Detect a Deno project in the specified directory.
    ///
    /// This method checks for a `deno.json` or `deno.jsonc` manifest alongside a
    /// `vendor/` directory (from `deno vendor`) or a `node_modules/` directory
    /// (Deno 2 npm support without a `package.json` to avoid overlap with Node.js).
    ///
    /// Deno detection runs before Node.js so that a project with `deno.json` and
    /// `node_modules/` (but no `package.json`) is classified as Deno.
    fn detect_deno_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let deno_json = path.join("deno.json");
        let deno_jsonc = path.join("deno.jsonc");

        if !deno_json.exists() && !deno_jsonc.exists() {
            return None;
        }

        let config_path = if deno_json.exists() {
            deno_json
        } else {
            deno_jsonc
        };

        // vendor/ directory (created by `deno vendor`)
        let vendor_dir = path.join("vendor");
        if vendor_dir.exists() {
            let name = self.extract_deno_project_name(&config_path, errors);
            return Some(Project::new(
                ProjectType::Deno,
                path.to_path_buf(),
                vec![BuildArtifacts {
                    path: vendor_dir,
                    size: 0,
                }],
                name,
            ));
        }

        // node_modules/ (Deno 2 npm support) — only when no package.json exists
        let node_modules = path.join("node_modules");
        if node_modules.exists() && !path.join("package.json").exists() {
            let name = self.extract_deno_project_name(&config_path, errors);
            return Some(Project::new(
                ProjectType::Deno,
                path.to_path_buf(),
                vec![BuildArtifacts {
                    path: node_modules,
                    size: 0,
                }],
                name,
            ));
        }

        None
    }

    /// Extract the project name from a `deno.json` or `deno.jsonc` file.
    ///
    /// Parses the JSON file and reads the top-level `"name"` field.
    /// Falls back to the directory name if the field is absent or the file cannot be parsed.
    fn extract_deno_project_name(
        &self,
        config_path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        match fs::read_to_string(config_path) {
            Ok(content) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content)
                    && let Some(name) = json.get("name").and_then(|v| v.as_str())
                {
                    return Some(name.to_string());
                }
                Self::fallback_to_directory_name(config_path.parent()?)
            }
            Err(e) => {
                self.log_file_error(config_path, &e, errors);
                Self::fallback_to_directory_name(config_path.parent()?)
            }
        }
    }

    /// Detect a Ruby project in the specified directory.
    ///
    /// This method checks for a `Gemfile` alongside a `.bundle/` or `vendor/bundle/`
    /// directory. When both exist the larger one is selected as the primary artifact.
    ///
    /// # Detection Criteria
    ///
    /// 1. `Gemfile` file exists in directory
    /// 2. At least one of `.bundle/` or `vendor/bundle/` directories exists
    fn detect_ruby_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let gemfile = path.join("Gemfile");
        if !gemfile.exists() {
            return None;
        }

        let bundle_dir = path.join(".bundle");
        let vendor_bundle_dir = path.join("vendor").join("bundle");

        let build_arts: Vec<BuildArtifacts> =
            match (bundle_dir.exists(), vendor_bundle_dir.exists()) {
                (true, true) => {
                    let bundle_size = crate::utils::calculate_dir_size(&bundle_dir);
                    let vendor_size = crate::utils::calculate_dir_size(&vendor_bundle_dir);
                    vec![
                        BuildArtifacts {
                            path: bundle_dir,
                            size: bundle_size,
                        },
                        BuildArtifacts {
                            path: vendor_bundle_dir,
                            size: vendor_size,
                        },
                    ]
                }
                (true, false) => vec![BuildArtifacts {
                    path: bundle_dir,
                    size: 0,
                }],
                (false, true) => vec![BuildArtifacts {
                    path: vendor_bundle_dir,
                    size: 0,
                }],
                (false, false) => return None,
            };

        let name = self.extract_ruby_project_name(path, errors);

        Some(Project::new(
            ProjectType::Ruby,
            path.to_path_buf(),
            build_arts,
            name,
        ))
    }

    /// Extract the project name from a Ruby project directory.
    ///
    /// Looks for a `.gemspec` file and parses the `spec.name` or `s.name` assignment.
    /// Falls back to the directory name.
    fn extract_ruby_project_name(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let entries = fs::read_dir(path).ok()?;
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_file()
                && entry_path.extension().and_then(|e| e.to_str()) == Some("gemspec")
                && let Some(content) = self.read_file_content(&entry_path, errors)
            {
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.contains(".name")
                        && trimmed.contains('=')
                        && let Some(name) = Self::extract_quoted_value(trimmed)
                    {
                        return Some(name);
                    }
                }
            }
        }

        Self::fallback_to_directory_name(path)
    }

    /// Detect an Elixir project in the specified directory.
    ///
    /// This method checks for the presence of both `mix.exs` and `_build/`
    /// to identify an Elixir/Mix project.
    ///
    /// # Detection Criteria
    ///
    /// 1. `mix.exs` file exists in directory
    /// 2. `_build/` subdirectory exists in directory
    fn detect_elixir_project(
        &self,
        path: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<Project> {
        let mix_exs = path.join("mix.exs");
        let build_dir = path.join("_build");

        if mix_exs.exists() && build_dir.exists() {
            let name = self.extract_elixir_project_name(&mix_exs, errors);

            return Some(Project::new(
                ProjectType::Elixir,
                path.to_path_buf(),
                vec![BuildArtifacts {
                    path: build_dir,
                    size: 0,
                }],
                name,
            ));
        }

        None
    }

    /// Extract the project name from a `mix.exs` file.
    ///
    /// Looks for the `app: :atom_name` pattern inside the Mix project definition.
    /// Falls back to the directory name.
    fn extract_elixir_project_name(
        &self,
        mix_exs: &Path,
        errors: &Arc<Mutex<Vec<String>>>,
    ) -> Option<String> {
        let content = self.read_file_content(mix_exs, errors)?;

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.contains("app:")
                && let Some(pos) = trimmed.find("app:")
            {
                let after = trimmed[pos + 4..].trim_start();
                if let Some(atom) = after.strip_prefix(':') {
                    // Elixir atom names consist of alphanumeric chars and underscores
                    let name: String = atom
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        return Some(name);
                    }
                }
            }
        }

        Self::fallback_to_directory_name(mix_exs.parent()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    /// Create a scanner with default options and the given filter.
    fn default_scanner(filter: ProjectFilter) -> Scanner {
        Scanner::new(
            ScanOptions {
                verbose: false,
                threads: 1,
                skip: vec![],
            },
            filter,
        )
    }

    /// Helper to create a file with content, ensuring parent dirs exist.
    fn create_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, content).unwrap();
    }

    // ── Static helper method tests ──────────────────────────────────────

    #[test]
    fn test_is_hidden_directory_to_skip() {
        // Hidden directories should be skipped
        assert!(Scanner::is_hidden_directory_to_skip(Path::new(
            "/some/.hidden"
        )));
        assert!(Scanner::is_hidden_directory_to_skip(Path::new(
            "/some/.git"
        )));
        assert!(Scanner::is_hidden_directory_to_skip(Path::new(
            "/some/.svn"
        )));
        assert!(Scanner::is_hidden_directory_to_skip(Path::new(".env")));

        // .cargo is the special exception — should NOT be skipped
        assert!(!Scanner::is_hidden_directory_to_skip(Path::new(
            "/home/user/.cargo"
        )));
        assert!(!Scanner::is_hidden_directory_to_skip(Path::new(".cargo")));

        // Non-hidden directories should not be skipped
        assert!(!Scanner::is_hidden_directory_to_skip(Path::new(
            "/some/visible"
        )));
        assert!(!Scanner::is_hidden_directory_to_skip(Path::new("src")));
    }

    #[test]
    fn test_is_excluded_directory() {
        // Build/artifact directories should be excluded
        assert!(Scanner::is_excluded_directory(Path::new("/some/target")));
        assert!(Scanner::is_excluded_directory(Path::new(
            "/some/node_modules"
        )));
        assert!(Scanner::is_excluded_directory(Path::new(
            "/some/__pycache__"
        )));
        assert!(Scanner::is_excluded_directory(Path::new("/some/vendor")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/build")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/dist")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/out")));

        // VCS directories should be excluded
        assert!(Scanner::is_excluded_directory(Path::new("/some/.git")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.svn")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.hg")));

        // Python-specific directories
        assert!(Scanner::is_excluded_directory(Path::new(
            "/some/.pytest_cache"
        )));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.tox")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.eggs")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.coverage")));

        // Virtual environments
        assert!(Scanner::is_excluded_directory(Path::new("/some/venv")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.venv")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/env")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/.env")));

        // Temp directories
        assert!(Scanner::is_excluded_directory(Path::new("/some/temp")));
        assert!(Scanner::is_excluded_directory(Path::new("/some/tmp")));

        // Non-excluded directories
        assert!(!Scanner::is_excluded_directory(Path::new("/some/src")));
        assert!(!Scanner::is_excluded_directory(Path::new("/some/lib")));
        assert!(!Scanner::is_excluded_directory(Path::new("/some/app")));
        assert!(!Scanner::is_excluded_directory(Path::new("/some/tests")));
    }

    #[test]
    fn test_extract_quoted_value() {
        assert_eq!(
            Scanner::extract_quoted_value(r#"name = "my-project""#),
            Some("my-project".to_string())
        );
        assert_eq!(
            Scanner::extract_quoted_value(r#"name = "with spaces""#),
            Some("with spaces".to_string())
        );
        assert_eq!(Scanner::extract_quoted_value("no quotes here"), None);
        // Single quote mark is not a pair
        assert_eq!(Scanner::extract_quoted_value(r#"only "one"#), None);
    }

    #[test]
    fn test_is_name_line() {
        assert!(Scanner::is_name_line("name = \"test\""));
        assert!(Scanner::is_name_line("name=\"test\""));
        assert!(!Scanner::is_name_line("version = \"1.0\""));
        assert!(!Scanner::is_name_line("# name = \"commented\""));
        assert!(!Scanner::is_name_line("name: \"yaml style\""));
    }

    #[test]
    fn test_parse_toml_name_field() {
        let content = "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\n";
        assert_eq!(
            Scanner::parse_toml_name_field(content),
            Some("test-project".to_string())
        );

        let no_name = "[package]\nversion = \"0.1.0\"\n";
        assert_eq!(Scanner::parse_toml_name_field(no_name), None);

        let empty = "";
        assert_eq!(Scanner::parse_toml_name_field(empty), None);
    }

    #[test]
    fn test_extract_name_from_cfg_content() {
        let content = "[metadata]\nname = my-package\nversion = 1.0\n";
        assert_eq!(
            Scanner::extract_name_from_cfg_content(content),
            Some("my-package".to_string())
        );

        // Name in wrong section should not be found
        let wrong_section = "[options]\nname = not-this\n";
        assert_eq!(Scanner::extract_name_from_cfg_content(wrong_section), None);

        // Multiple sections — name must be in [metadata]
        let multi = "[options]\nkey = val\n\n[metadata]\nname = correct\n\n[other]\nname = wrong\n";
        assert_eq!(
            Scanner::extract_name_from_cfg_content(multi),
            Some("correct".to_string())
        );
    }

    #[test]
    fn test_extract_name_from_python_content() {
        let content = "from setuptools import setup\nsetup(\n    name=\"my-pkg\",\n)\n";
        assert_eq!(
            Scanner::extract_name_from_python_content(content),
            Some("my-pkg".to_string())
        );

        let no_name = "from setuptools import setup\nsetup(version=\"1.0\")\n";
        assert_eq!(Scanner::extract_name_from_python_content(no_name), None);
    }

    #[test]
    fn test_fallback_to_directory_name() {
        assert_eq!(
            Scanner::fallback_to_directory_name(Path::new("/some/project-name")),
            Some("project-name".to_string())
        );
        assert_eq!(
            Scanner::fallback_to_directory_name(Path::new("/some/my_app")),
            Some("my_app".to_string())
        );
    }

    #[test]
    fn test_is_path_in_skip_list() {
        let scanner = Scanner::new(
            ScanOptions {
                verbose: false,
                threads: 1,
                skip: vec![PathBuf::from("skip-me"), PathBuf::from("also-skip")],
            },
            ProjectFilter::All,
        );

        assert!(scanner.is_path_in_skip_list(Path::new("/root/skip-me/project")));
        assert!(scanner.is_path_in_skip_list(Path::new("/root/also-skip")));
        assert!(!scanner.is_path_in_skip_list(Path::new("/root/keep-me")));
        assert!(!scanner.is_path_in_skip_list(Path::new("/root/src")));
    }

    #[test]
    fn test_is_path_in_empty_skip_list() {
        let scanner = default_scanner(ProjectFilter::All);
        assert!(!scanner.is_path_in_skip_list(Path::new("/any/path")));
    }

    // ── Scanning with special path characters ───────────────────────────

    #[test]
    fn test_scan_directory_with_spaces_in_path() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path().join("path with spaces");
        fs::create_dir_all(&base).unwrap();

        let project = base.join("my project");
        create_file(
            &project.join("Cargo.toml"),
            "[package]\nname = \"spaced\"\nversion = \"0.1.0\"",
        );
        create_file(&project.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust);
        let projects = scanner.scan_directory(&base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("spaced"));
    }

    #[test]
    fn test_scan_directory_with_unicode_names() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("プロジェクト");
        create_file(
            &project.join("package.json"),
            r#"{"name": "unicode-project"}"#,
        );
        create_file(&project.join("node_modules/dep.js"), "module.exports = {};");

        let scanner = default_scanner(ProjectFilter::Node);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("unicode-project"));
    }

    #[test]
    fn test_scan_directory_with_special_characters_in_name() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("project-with-dashes_and_underscores.v2");
        create_file(
            &project.join("Cargo.toml"),
            "[package]\nname = \"special-chars\"\nversion = \"0.1.0\"",
        );
        create_file(&project.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("special-chars"));
    }

    // ── Unix-specific scanning tests ────────────────────────────────────

    #[test]
    #[cfg(unix)]
    fn test_hidden_directory_itself_not_detected_as_project_unix() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // A hidden directory with Cargo.toml + target/ directly inside it
        // should NOT be detected because the .hidden entry is filtered by
        // is_hidden_directory_to_skip. However, non-hidden children inside
        // hidden dirs CAN still be found because WalkDir descends into them.
        let hidden = base.join(".hidden-project");
        create_file(
            &hidden.join("Cargo.toml"),
            "[package]\nname = \"hidden\"\nversion = \"0.1.0\"",
        );
        create_file(&hidden.join("target/dummy"), "content");

        // A visible project should be found
        let visible = base.join("visible-project");
        create_file(
            &visible.join("Cargo.toml"),
            "[package]\nname = \"visible\"\nversion = \"0.1.0\"",
        );
        create_file(&visible.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust);
        let projects = scanner.scan_directory(base);

        // Only the visible project should be found; the hidden one is excluded
        // because its directory name starts with '.'
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("visible"));
    }

    #[test]
    #[cfg(unix)]
    fn test_projects_inside_hidden_dirs_are_still_traversed_unix() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // A non-hidden project nested inside a hidden directory.
        // WalkDir still descends into .hidden, so the child project IS found.
        let nested = base.join(".hidden-parent/visible-child");
        create_file(
            &nested.join("Cargo.toml"),
            "[package]\nname = \"nested\"\nversion = \"0.1.0\"",
        );
        create_file(&nested.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust);
        let projects = scanner.scan_directory(base);

        // The child project has a non-hidden name, so it IS detected
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("nested"));
    }

    #[test]
    #[cfg(unix)]
    fn test_dotcargo_directory_not_skipped_unix() {
        // .cargo is the exception — hidden but should NOT be skipped.
        // Verify via the static method.
        assert!(!Scanner::is_hidden_directory_to_skip(Path::new(
            "/home/user/.cargo"
        )));

        // Other dot-dirs ARE skipped
        assert!(Scanner::is_hidden_directory_to_skip(Path::new(
            "/home/user/.local"
        )));
        assert!(Scanner::is_hidden_directory_to_skip(Path::new(
            "/home/user/.npm"
        )));
    }

    // ── Python project detection tests ──────────────────────────────────

    #[test]
    fn test_detect_python_with_pyproject_toml() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("py-project");
        create_file(
            &project.join("pyproject.toml"),
            "[project]\nname = \"my-py-lib\"\nversion = \"1.0.0\"\n",
        );
        let pycache = project.join("__pycache__");
        fs::create_dir_all(&pycache).unwrap();
        create_file(&pycache.join("module.pyc"), "bytecode");

        let scanner = default_scanner(ProjectFilter::Python);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Python);
    }

    #[test]
    fn test_detect_python_with_setup_py() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("setup-project");
        create_file(
            &project.join("setup.py"),
            "from setuptools import setup\nsetup(name=\"setup-lib\")\n",
        );
        let pycache = project.join("__pycache__");
        fs::create_dir_all(&pycache).unwrap();
        create_file(&pycache.join("module.pyc"), "bytecode");

        let scanner = default_scanner(ProjectFilter::Python);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
    }

    #[test]
    fn test_detect_python_with_pipfile() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("pipenv-project");
        create_file(
            &project.join("Pipfile"),
            "[[source]]\nurl = \"https://pypi.org/simple\"",
        );
        let pycache = project.join("__pycache__");
        fs::create_dir_all(&pycache).unwrap();
        create_file(&pycache.join("module.pyc"), "bytecode");

        let scanner = default_scanner(ProjectFilter::Python);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
    }

    // ── Go project detection tests ──────────────────────────────────────

    #[test]
    fn test_detect_go_extracts_module_name() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("go-service");
        create_file(
            &project.join("go.mod"),
            "module github.com/user/my-service\n\ngo 1.21\n",
        );
        let vendor = project.join("vendor");
        fs::create_dir_all(&vendor).unwrap();
        create_file(&vendor.join("modules.txt"), "vendor manifest");

        let scanner = default_scanner(ProjectFilter::Go);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        // Should extract last path component as name
        assert_eq!(projects[0].name.as_deref(), Some("my-service"));
    }

    // ── Java/Kotlin project detection tests ────────────────────────────

    #[test]
    fn test_detect_java_maven_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("java-maven");
        create_file(
            &project.join("pom.xml"),
            "<project>\n  <artifactId>my-java-app</artifactId>\n</project>",
        );
        create_file(&project.join("target/classes/Main.class"), "bytecode");

        let scanner = default_scanner(ProjectFilter::Java);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Java);
        assert_eq!(projects[0].name.as_deref(), Some("my-java-app"));
    }

    #[test]
    fn test_detect_java_gradle_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("java-gradle");
        create_file(&project.join("build.gradle"), "apply plugin: 'java'");
        create_file(
            &project.join("settings.gradle"),
            "rootProject.name = \"my-gradle-app\"",
        );
        create_file(&project.join("build/classes/main/Main.class"), "bytecode");

        let scanner = default_scanner(ProjectFilter::Java);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Java);
        assert_eq!(projects[0].name.as_deref(), Some("my-gradle-app"));
    }

    #[test]
    fn test_detect_java_gradle_kts_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("kotlin-gradle");
        create_file(
            &project.join("build.gradle.kts"),
            "plugins { kotlin(\"jvm\") }",
        );
        create_file(
            &project.join("settings.gradle.kts"),
            "rootProject.name = \"my-kotlin-app\"",
        );
        create_file(
            &project.join("build/classes/kotlin/main/MainKt.class"),
            "bytecode",
        );

        let scanner = default_scanner(ProjectFilter::Java);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Java);
        assert_eq!(projects[0].name.as_deref(), Some("my-kotlin-app"));
    }

    // ── C/C++ project detection tests ────────────────────────────────────

    #[test]
    fn test_detect_cpp_cmake_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("cpp-cmake");
        create_file(
            &project.join("CMakeLists.txt"),
            "project(my-cpp-lib)\ncmake_minimum_required(VERSION 3.10)",
        );
        create_file(&project.join("build/CMakeCache.txt"), "cache");

        let scanner = default_scanner(ProjectFilter::Cpp);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Cpp);
        assert_eq!(projects[0].name.as_deref(), Some("my-cpp-lib"));
    }

    #[test]
    fn test_detect_cpp_makefile_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("cpp-make");
        create_file(&project.join("Makefile"), "all:\n\tg++ -o main main.cpp");
        create_file(&project.join("build/main.o"), "object");

        let scanner = default_scanner(ProjectFilter::Cpp);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Cpp);
    }

    // ── Swift project detection tests ────────────────────────────────────

    #[test]
    fn test_detect_swift_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("swift-pkg");
        create_file(
            &project.join("Package.swift"),
            "let package = Package(\n    name: \"my-swift-lib\",\n    targets: []\n)",
        );
        create_file(&project.join(".build/debug/my-swift-lib"), "binary");

        let scanner = default_scanner(ProjectFilter::Swift);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Swift);
        assert_eq!(projects[0].name.as_deref(), Some("my-swift-lib"));
    }

    // ── .NET/C# project detection tests ──────────────────────────────────

    #[test]
    fn test_detect_dotnet_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("dotnet-app");
        create_file(
            &project.join("MyApp.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\">\n</Project>",
        );
        create_file(&project.join("bin/Debug/net8.0/MyApp.dll"), "assembly");
        create_file(&project.join("obj/Debug/net8.0/MyApp.dll"), "intermediate");

        let scanner = default_scanner(ProjectFilter::DotNet);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::DotNet);
        assert_eq!(projects[0].name.as_deref(), Some("MyApp"));
    }

    #[test]
    fn test_detect_dotnet_project_obj_only() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("dotnet-obj-only");
        create_file(
            &project.join("Lib.csproj"),
            "<Project Sdk=\"Microsoft.NET.Sdk\">\n</Project>",
        );
        create_file(&project.join("obj/Debug/net8.0/Lib.dll"), "intermediate");

        let scanner = default_scanner(ProjectFilter::DotNet);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::DotNet);
        assert_eq!(projects[0].name.as_deref(), Some("Lib"));
    }

    // ── Excluded directory tests ─────────────────────────────────────────

    #[test]
    fn test_obj_directory_is_excluded() {
        assert!(Scanner::is_excluded_directory(Path::new("/some/obj")));
    }

    // ── Cross-platform calculate_build_dir_size ─────────────────────────

    #[test]
    fn test_calculate_build_dir_size_empty() {
        let tmp = TempDir::new().unwrap();
        let empty_dir = tmp.path().join("empty");
        fs::create_dir_all(&empty_dir).unwrap();

        assert_eq!(Scanner::calculate_build_dir_size(&empty_dir), 0);
    }

    #[test]
    fn test_calculate_build_dir_size_nonexistent() {
        assert_eq!(
            Scanner::calculate_build_dir_size(Path::new("/nonexistent/path")),
            0
        );
    }

    #[test]
    fn test_calculate_build_dir_size_with_nested_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("nested");

        create_file(&dir.join("file1.txt"), "hello"); // 5 bytes
        create_file(&dir.join("sub/file2.txt"), "world!"); // 6 bytes
        create_file(&dir.join("sub/deep/file3.txt"), "!"); // 1 byte

        let size = Scanner::calculate_build_dir_size(&dir);
        assert_eq!(size, 12);
    }

    // ── Quiet mode ──────────────────────────────────────────────────────

    #[test]
    fn test_scanner_quiet_mode() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("quiet-project");
        create_file(
            &project.join("Cargo.toml"),
            "[package]\nname = \"quiet\"\nversion = \"0.1.0\"",
        );
        create_file(&project.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust).with_quiet(true);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
    }

    // ── Ruby project detection tests ─────────────────────────────────────

    #[test]
    fn test_detect_ruby_with_vendor_bundle() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("ruby-project");
        create_file(
            &project.join("Gemfile"),
            "source 'https://rubygems.org'\ngem 'rails'",
        );
        create_file(
            &project.join("my-app.gemspec"),
            "Gem::Specification.new do |spec|\n  spec.name = \"my-ruby-gem\"\nend",
        );
        create_file(
            &project.join("vendor/bundle/ruby/3.2.0/gems/rails/init.rb"),
            "# rails",
        );

        let scanner = default_scanner(ProjectFilter::Ruby);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Ruby);
        assert_eq!(projects[0].name.as_deref(), Some("my-ruby-gem"));
    }

    #[test]
    fn test_detect_ruby_with_dot_bundle() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("ruby-dot-bundle");
        create_file(&project.join("Gemfile"), "source 'https://rubygems.org'");
        create_file(&project.join(".bundle/gems/rack-2.0/lib/rack.rb"), "# rack");

        let scanner = default_scanner(ProjectFilter::Ruby);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Ruby);
    }

    #[test]
    fn test_detect_ruby_no_artifact_not_detected() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // Gemfile exists but no .bundle/ or vendor/bundle/
        let project = base.join("gemfile-only");
        create_file(&project.join("Gemfile"), "source 'https://rubygems.org'");

        let scanner = default_scanner(ProjectFilter::Ruby);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_detect_ruby_fallback_to_dir_name() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("my-ruby-app");
        create_file(&project.join("Gemfile"), "source 'https://rubygems.org'");
        create_file(
            &project.join("vendor/bundle/gems/sinatra/lib/sinatra.rb"),
            "# sinatra",
        );

        let scanner = default_scanner(ProjectFilter::Ruby);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("my-ruby-app"));
    }

    // ── Elixir project detection tests ───────────────────────────────────

    #[test]
    fn test_detect_elixir_project() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("elixir-project");
        create_file(
            &project.join("mix.exs"),
            "defmodule MyApp.MixProject do\n  def project do\n    [app: :my_app,\n     version: \"0.1.0\"]\n  end\nend",
        );
        create_file(
            &project.join("_build/dev/lib/my_app/.mix/compile.elixir"),
            "# build",
        );

        let scanner = default_scanner(ProjectFilter::Elixir);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Elixir);
        assert_eq!(projects[0].name.as_deref(), Some("my_app"));
    }

    #[test]
    fn test_detect_elixir_no_build_not_detected() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("mix-only");
        create_file(
            &project.join("mix.exs"),
            "defmodule MixOnly.MixProject do\n  def project do\n    [app: :mix_only]\n  end\nend",
        );

        let scanner = default_scanner(ProjectFilter::Elixir);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_detect_elixir_fallback_to_dir_name() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("my_elixir_project");
        create_file(&project.join("mix.exs"), "# minimal mix.exs without app:");
        create_file(
            &project.join("_build/prod/lib/my_elixir_project.beam"),
            "bytecode",
        );

        let scanner = default_scanner(ProjectFilter::Elixir);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name.as_deref(), Some("my_elixir_project"));
    }

    // ── Deno project detection tests ─────────────────────────────────────

    #[test]
    fn test_detect_deno_with_vendor() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("deno-project");
        create_file(
            &project.join("deno.json"),
            r#"{"name": "my-deno-app", "imports": {}}"#,
        );
        create_file(&project.join("vendor/modules.json"), "{}");

        let scanner = default_scanner(ProjectFilter::Deno);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Deno);
        assert_eq!(projects[0].name.as_deref(), Some("my-deno-app"));
    }

    #[test]
    fn test_detect_deno_jsonc_config() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("deno-jsonc-project");
        create_file(
            &project.join("deno.jsonc"),
            r#"{"name": "my-deno-jsonc-app", "tasks": {}}"#,
        );
        create_file(&project.join("vendor/modules.json"), "{}");

        let scanner = default_scanner(ProjectFilter::Deno);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Deno);
        assert_eq!(projects[0].name.as_deref(), Some("my-deno-jsonc-app"));
    }

    #[test]
    fn test_detect_deno_node_modules_without_package_json() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("deno-npm-project");
        create_file(&project.join("deno.json"), r#"{"nodeModulesDir": "auto"}"#);
        create_file(
            &project.join("node_modules/.deno/lodash/index.js"),
            "// lodash",
        );

        let scanner = default_scanner(ProjectFilter::Deno);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Deno);
    }

    #[test]
    fn test_detect_deno_node_modules_with_package_json_becomes_node() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // deno.json + package.json + node_modules → Node project (not Deno)
        let project = base.join("ambiguous-project");
        create_file(&project.join("deno.json"), r"{}");
        create_file(&project.join("package.json"), r#"{"name": "my-node-app"}"#);
        create_file(&project.join("node_modules/dep/index.js"), "// dep");

        let scanner = default_scanner(ProjectFilter::All);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].kind, ProjectType::Node);
    }

    #[test]
    fn test_detect_deno_no_artifact_not_detected() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        let project = base.join("deno-no-artifact");
        create_file(&project.join("deno.json"), r"{}");

        let scanner = default_scanner(ProjectFilter::Deno);
        let projects = scanner.scan_directory(base);
        assert_eq!(projects.len(), 0);
    }

    #[test]
    fn test_build_directory_is_excluded() {
        assert!(Scanner::is_excluded_directory(Path::new("/some/_build")));
    }

    // ── Rust workspace awareness tests ─────────────────────────────────

    #[test]
    fn test_is_cargo_workspace_root() {
        let tmp = TempDir::new().unwrap();
        let cargo_toml = tmp.path().join("Cargo.toml");

        // A workspace root must contain a bare `[workspace]` section header.
        create_file(
            &cargo_toml,
            "[workspace]\nmembers = [\"crate-a\", \"crate-b\"]\n",
        );
        assert!(Scanner::is_cargo_workspace_root(&cargo_toml));

        // A regular package Cargo.toml is not a workspace root.
        create_file(
            &cargo_toml,
            "[package]\nname = \"my-crate\"\nversion = \"0.1.0\"\n",
        );
        assert!(!Scanner::is_cargo_workspace_root(&cargo_toml));

        // A non-existent file returns false.
        assert!(!Scanner::is_cargo_workspace_root(Path::new(
            "/nonexistent/Cargo.toml"
        )));
    }

    #[test]
    fn test_workspace_root_detected() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // Workspace root: has [workspace] in Cargo.toml and a target/ dir with content.
        let workspace = base.join("my-workspace");
        create_file(
            &workspace.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate-a\"]\n\n[package]\nname = \"my-workspace\"\nversion = \"0.1.0\"\n",
        );
        create_file(&workspace.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust);
        let projects = scanner.scan_directory(base);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].root_path, workspace);
    }

    #[test]
    fn test_workspace_member_with_own_target_skipped() {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();

        // Workspace root with content in target/.
        let workspace = base.join("my-workspace");
        create_file(
            &workspace.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crate-a\"]\n\n[package]\nname = \"my-workspace\"\nversion = \"0.1.0\"\n",
        );
        create_file(&workspace.join("target/dummy"), "content");

        // Workspace member that also happens to have its own target/ directory.
        let member = workspace.join("crate-a");
        create_file(
            &member.join("Cargo.toml"),
            "[package]\nname = \"crate-a\"\nversion = \"0.1.0\"\n",
        );
        create_file(&member.join("target/dummy"), "content");

        let scanner = default_scanner(ProjectFilter::Rust);
        let projects = scanner.scan_directory(base);

        // Only the workspace root should be reported; the member must be skipped.
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].root_path, workspace);
    }
}
