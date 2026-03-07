//! Execution configuration for cleanup operations.
//!
//! This module defines the options that control how cleanup operations are executed,
//! including dry-run mode and interactive selection.

/// Configuration for cleanup execution behavior.
///
/// This struct provides a simplified interface to execution-related options,
/// controlling how the cleanup process runs.
#[derive(Clone)]
#[allow(clippy::struct_excessive_bools)]
pub struct ExecutionOptions {
    /// Whether to run in dry-run mode (no actual deletion)
    pub dry_run: bool,

    /// Whether to use interactive project selection
    pub interactive: bool,

    /// Whether to preserve compiled executables before cleaning
    pub keep_executables: bool,

    /// Whether to move directories to the system trash instead of permanently deleting them.
    ///
    /// Defaults to `true`. Set to `false` via the `--permanent` CLI flag or
    /// `use_trash = false` in the config file.
    pub use_trash: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_options_creation() {
        let exec_opts = ExecutionOptions {
            dry_run: true,
            interactive: false,
            keep_executables: false,
            use_trash: false,
        };

        assert!(exec_opts.dry_run);
        assert!(!exec_opts.interactive);
        assert!(!exec_opts.keep_executables);
        assert!(!exec_opts.use_trash);
    }

    #[test]
    fn test_execution_options_clone() {
        let original = ExecutionOptions {
            dry_run: true,
            interactive: false,
            keep_executables: true,
            use_trash: true,
        };
        let cloned = original.clone();

        assert_eq!(original.dry_run, cloned.dry_run);
        assert_eq!(original.interactive, cloned.interactive);
        assert_eq!(original.keep_executables, cloned.keep_executables);
        assert_eq!(original.use_trash, cloned.use_trash);
    }
}
