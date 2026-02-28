//! Configuration file support for persistent settings.
//!
//! This module provides support for loading configuration from a TOML file
//! located at `~/.config/clean-dev-dirs/config.toml` (or the platform-specific
//! equivalent). Configuration file values serve as defaults that can be
//! overridden by CLI arguments.
//!
//! # Layering
//!
//! The precedence order is: **CLI argument > config file > hardcoded default**.
//!
//! # Example config
//!
//! ```toml
//! project_type = "rust"
//! # Single directory (legacy):
//! # dir = "~/Projects"
//! # Multiple directories:
//! # dirs = ["~/Projects", "~/work/client"]
//!
//! [filtering]
//! keep_size = "50MB"
//! keep_days = 7
//! sort = "size"
//! reverse = false
//!
//! [scanning]
//! threads = 4
//! verbose = true
//! skip = [".cargo", "vendor"]
//! ignore = [".git"]
//! max_depth = 5
//!
//! [execution]
//! keep_executables = true
//! interactive = false
//! dry_run = false
//! use_trash = true    # default; set to false for permanent deletion
//! ```

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Top-level configuration file structure.
///
/// All fields are `Option<T>` so we can detect which values are present in the
/// config file and apply layered configuration (CLI > config file > defaults).
#[derive(Deserialize, Default, Debug)]
pub struct FileConfig {
    /// Default project type filter (e.g., `"rust"`, `"node"`, `"all"`)
    pub project_type: Option<String>,

    /// Default directories to scan (plural; takes priority over `dir`)
    pub dirs: Option<Vec<PathBuf>>,

    /// Default directory to scan (legacy single-dir; kept for backward compatibility)
    pub dir: Option<PathBuf>,

    /// Filtering options
    #[serde(default)]
    pub filtering: FileFilterConfig,

    /// Scanning options
    #[serde(default)]
    pub scanning: FileScanConfig,

    /// Execution options
    #[serde(default)]
    pub execution: FileExecutionConfig,
}

/// Filtering options from the configuration file.
#[derive(Deserialize, Default, Debug)]
pub struct FileFilterConfig {
    /// Minimum size threshold (e.g., `"50MB"`)
    pub keep_size: Option<String>,

    /// Minimum age in days
    pub keep_days: Option<u32>,

    /// Sort criterion for project output (`"size"`, `"age"`, `"name"`, `"type"`)
    pub sort: Option<String>,

    /// Whether to reverse the sort order
    pub reverse: Option<bool>,
}

/// Scanning options from the configuration file.
#[derive(Deserialize, Default, Debug)]
pub struct FileScanConfig {
    /// Number of threads for scanning
    pub threads: Option<usize>,

    /// Whether to show verbose output
    pub verbose: Option<bool>,

    /// Directories to skip during scanning
    pub skip: Option<Vec<PathBuf>>,

    /// Directories to ignore during scanning
    pub ignore: Option<Vec<PathBuf>>,

    /// Maximum directory depth to scan
    pub max_depth: Option<usize>,
}

/// Execution options from the configuration file.
#[derive(Deserialize, Default, Debug)]
pub struct FileExecutionConfig {
    /// Whether to preserve compiled executables
    pub keep_executables: Option<bool>,

    /// Whether to use interactive selection
    pub interactive: Option<bool>,

    /// Whether to run in dry-run mode
    pub dry_run: Option<bool>,

    /// Whether to move directories to the system trash instead of permanently deleting them.
    /// Defaults to `true` when absent. Set to `false` for permanent deletion.
    pub use_trash: Option<bool>,
}

/// Expand a leading `~` in a path to the user's home directory.
///
/// Paths that don't start with `~` are returned unchanged.
///
/// # Examples
///
/// ```
/// # use std::path::PathBuf;
/// # use clean_dev_dirs::config::file::expand_tilde;
/// let absolute = PathBuf::from("/absolute/path");
/// assert_eq!(expand_tilde(&absolute), PathBuf::from("/absolute/path"));
/// ```
#[must_use]
pub fn expand_tilde(path: &Path) -> PathBuf {
    if let Ok(rest) = path.strip_prefix("~")
        && let Some(home) = dirs::home_dir()
    {
        return home.join(rest);
    }
    path.to_path_buf()
}

impl FileConfig {
    /// Returns the path where the configuration file is expected.
    ///
    /// The configuration file is located at `<config_dir>/clean-dev-dirs/config.toml`,
    /// where `<config_dir>` is the platform-specific configuration directory
    /// (e.g., `~/.config` on Linux/macOS, `%APPDATA%` on Windows).
    ///
    /// # Returns
    ///
    /// `Some(PathBuf)` with the config file path, or `None` if the config
    /// directory cannot be determined.
    #[must_use]
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("clean-dev-dirs").join("config.toml"))
    }

    /// Load configuration from the default config file location.
    ///
    /// If the config file doesn't exist, returns a default (empty) configuration.
    /// If the file exists but is malformed, returns an error.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The config file exists but cannot be read
    /// - The config file exists but contains invalid TOML or unexpected fields
    pub fn load() -> anyhow::Result<Self> {
        let Some(path) = Self::config_path() else {
            return Ok(Self::default());
        };

        if !path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&path).map_err(|e| {
            anyhow::anyhow!("Failed to read config file at {}: {e}", path.display())
        })?;

        let config: Self = toml::from_str(&content).map_err(|e| {
            anyhow::anyhow!("Failed to parse config file at {}: {e}", path.display())
        })?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_file_config() {
        let config = FileConfig::default();

        assert!(config.project_type.is_none());
        assert!(config.dirs.is_none());
        assert!(config.dir.is_none());
        assert!(config.filtering.keep_size.is_none());
        assert!(config.filtering.keep_days.is_none());
        assert!(config.filtering.sort.is_none());
        assert!(config.filtering.reverse.is_none());
        assert!(config.scanning.threads.is_none());
        assert!(config.scanning.verbose.is_none());
        assert!(config.scanning.skip.is_none());
        assert!(config.scanning.ignore.is_none());
        assert!(config.execution.keep_executables.is_none());
        assert!(config.execution.interactive.is_none());
        assert!(config.execution.dry_run.is_none());
        assert!(config.execution.use_trash.is_none());
    }

    #[test]
    fn test_parse_full_config() {
        let toml_content = r#"
project_type = "rust"
dir = "~/Projects"

[filtering]
keep_size = "50MB"
keep_days = 7
sort = "size"
reverse = true

[scanning]
threads = 4
verbose = true
skip = [".cargo", "vendor"]
ignore = [".git"]

[execution]
keep_executables = true
interactive = false
dry_run = false
use_trash = true
"#;

        let config: FileConfig = toml::from_str(toml_content).unwrap();

        assert_eq!(config.project_type, Some("rust".to_string()));
        assert_eq!(config.dir, Some(PathBuf::from("~/Projects")));
        assert_eq!(config.filtering.keep_size, Some("50MB".to_string()));
        assert_eq!(config.filtering.keep_days, Some(7));
        assert_eq!(config.filtering.sort, Some("size".to_string()));
        assert_eq!(config.filtering.reverse, Some(true));
        assert_eq!(config.scanning.threads, Some(4));
        assert_eq!(config.scanning.verbose, Some(true));
        assert_eq!(
            config.scanning.skip,
            Some(vec![PathBuf::from(".cargo"), PathBuf::from("vendor")])
        );
        assert_eq!(config.scanning.ignore, Some(vec![PathBuf::from(".git")]));
        assert_eq!(config.execution.keep_executables, Some(true));
        assert_eq!(config.execution.interactive, Some(false));
        assert_eq!(config.execution.dry_run, Some(false));
        assert_eq!(config.execution.use_trash, Some(true));
    }

    #[test]
    fn test_parse_dirs_field() {
        let toml_content = r#"dirs = ["~/Projects", "~/work"]"#;
        let config: FileConfig = toml::from_str(toml_content).unwrap();

        assert_eq!(
            config.dirs,
            Some(vec![PathBuf::from("~/Projects"), PathBuf::from("~/work")])
        );
        assert!(config.dir.is_none());
    }

    #[test]
    fn test_parse_partial_config() {
        let toml_content = r#"
[filtering]
keep_size = "100MB"
"#;

        let config: FileConfig = toml::from_str(toml_content).unwrap();

        assert!(config.project_type.is_none());
        assert!(config.dir.is_none());
        assert_eq!(config.filtering.keep_size, Some("100MB".to_string()));
        assert!(config.filtering.keep_days.is_none());
        assert!(config.filtering.sort.is_none());
        assert!(config.filtering.reverse.is_none());
        assert!(config.scanning.threads.is_none());
    }

    #[test]
    fn test_parse_empty_config() {
        let toml_content = "";
        let config: FileConfig = toml::from_str(toml_content).unwrap();

        assert!(config.project_type.is_none());
        assert!(config.dir.is_none());
    }

    #[test]
    fn test_malformed_config_errors() {
        let toml_content = r#"
[filtering]
keep_days = "not_a_number"
"#;
        let result = toml::from_str::<FileConfig>(toml_content);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_path_returns_expected_suffix() {
        let path = FileConfig::config_path();
        if let Some(p) = path {
            assert!(p.ends_with("clean-dev-dirs/config.toml"));
        }
    }

    #[test]
    fn test_load_returns_defaults_when_no_file() {
        let config = FileConfig::load().unwrap();
        assert!(config.project_type.is_none());
        assert!(config.dir.is_none());
    }

    #[test]
    fn test_expand_tilde_with_home() {
        let path = PathBuf::from("~/Projects");
        let expanded = expand_tilde(&path);

        if let Some(home) = dirs::home_dir() {
            assert_eq!(expanded, home.join("Projects"));
        }
    }

    #[test]
    fn test_expand_tilde_absolute_path_unchanged() {
        let path = PathBuf::from("/absolute/path");
        let expanded = expand_tilde(&path);
        assert_eq!(expanded, PathBuf::from("/absolute/path"));
    }

    #[test]
    fn test_expand_tilde_relative_path_unchanged() {
        let path = PathBuf::from("relative/path");
        let expanded = expand_tilde(&path);
        assert_eq!(expanded, PathBuf::from("relative/path"));
    }

    #[test]
    fn test_expand_tilde_bare() {
        let path = PathBuf::from("~");
        let expanded = expand_tilde(&path);

        if let Some(home) = dirs::home_dir() {
            assert_eq!(expanded, home);
        }
    }

    // ── Platform-specific config path tests ─────────────────────────────

    #[test]
    fn test_config_path_is_platform_appropriate() {
        let path = FileConfig::config_path();

        // config_path might return None in CI environments without a home dir,
        // but when it does return a path, it must match platform conventions.
        if let Some(p) = &path {
            let path_str = p.to_string_lossy();

            #[cfg(target_os = "linux")]
            assert!(
                path_str.contains(".config"),
                "Linux config path should be under $XDG_CONFIG_HOME or ~/.config, got: {path_str}"
            );

            #[cfg(target_os = "macos")]
            assert!(
                path_str.contains("Application Support") || path_str.contains(".config"),
                "macOS config path should be under Library/Application Support, got: {path_str}"
            );

            #[cfg(target_os = "windows")]
            assert!(
                path_str.contains("AppData"),
                "Windows config path should be under AppData, got: {path_str}"
            );

            // Common: always ends with our application config file name
            assert!(
                p.ends_with("clean-dev-dirs/config.toml")
                    || p.ends_with(Path::new("clean-dev-dirs").join("config.toml"))
            );
        }
    }

    #[test]
    fn test_config_path_parent_exists_or_can_be_created() {
        // Verify the parent of the config path is a real, accessible directory
        // (or at least its grandparent exists — the app dir might not exist yet).
        if let Some(path) = FileConfig::config_path()
            && let Some(grandparent) = path.parent().and_then(Path::parent)
        {
            // The system config directory should exist
            assert!(
                grandparent.exists(),
                "Config grandparent directory should exist: {}",
                grandparent.display()
            );
        }
    }

    #[test]
    fn test_expand_tilde_deeply_nested() {
        let path = PathBuf::from("~/a/b/c/d");
        let expanded = expand_tilde(&path);

        if let Some(home) = dirs::home_dir() {
            assert_eq!(expanded, home.join("a").join("b").join("c").join("d"));
            assert!(!expanded.to_string_lossy().contains('~'));
        }
    }

    #[test]
    fn test_expand_tilde_no_effect_on_non_tilde() {
        // Relative paths without ~ should be unchanged
        let relative = PathBuf::from("some/relative/path");
        assert_eq!(expand_tilde(&relative), relative);

        // Absolute Unix-style paths should be unchanged
        let absolute = PathBuf::from("/usr/local/bin");
        assert_eq!(expand_tilde(&absolute), absolute);

        // Windows-style absolute paths should be unchanged
        #[cfg(windows)]
        {
            let win_abs = PathBuf::from(r"C:\Users\user\Documents");
            assert_eq!(expand_tilde(&win_abs), win_abs);
        }
    }

    #[test]
    fn test_config_toml_parsing_with_platform_paths() {
        // Test that TOML parsing handles paths from any platform
        let toml_unix = "dir = \"/home/user/projects\"\n";
        let config: FileConfig = toml::from_str(toml_unix).unwrap();
        assert_eq!(config.dir, Some(PathBuf::from("/home/user/projects")));

        let toml_tilde = "dir = \"~/Projects\"\n";
        let config: FileConfig = toml::from_str(toml_tilde).unwrap();
        assert_eq!(config.dir, Some(PathBuf::from("~/Projects")));

        let toml_relative = "dir = \"./projects\"\n";
        let config: FileConfig = toml::from_str(toml_relative).unwrap();
        assert_eq!(config.dir, Some(PathBuf::from("./projects")));
    }

    #[test]
    fn test_file_config_all_execution_options_parse() {
        let toml_content = r"
[execution]
keep_executables = true
interactive = false
dry_run = true
use_trash = false
";
        let config: FileConfig = toml::from_str(toml_content).unwrap();

        assert_eq!(config.execution.keep_executables, Some(true));
        assert_eq!(config.execution.interactive, Some(false));
        assert_eq!(config.execution.dry_run, Some(true));
        assert_eq!(config.execution.use_trash, Some(false));
    }
}
