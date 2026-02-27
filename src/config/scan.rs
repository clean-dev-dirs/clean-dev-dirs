//! Scanning configuration for directory traversal.
//!
//! This module defines the options that control how directories are scanned
//! and what information is collected during the scanning process.

use std::path::PathBuf;

/// Configuration for directory scanning behavior.
///
/// This struct contains options that control how directories are traversed
/// and what information is collected during the scanning process.
#[derive(Clone)]
pub struct ScanOptions {
    /// Whether to show verbose output including scan errors
    pub verbose: bool,

    /// Number of threads to use for scanning (0 = default)
    pub threads: usize,

    /// List of directory patterns to skip during scanning
    pub skip: Vec<PathBuf>,

    /// Maximum directory depth to scan (None = unlimited)
    pub max_depth: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_options_creation() {
        let scan_opts = ScanOptions {
            verbose: true,
            threads: 4,
            skip: vec![PathBuf::from("test")],
            max_depth: None,
        };

        assert!(scan_opts.verbose);
        assert_eq!(scan_opts.threads, 4);
        assert_eq!(scan_opts.skip.len(), 1);
    }

    #[test]
    fn test_scan_options_clone() {
        let original = ScanOptions {
            verbose: true,
            threads: 4,
            skip: vec![PathBuf::from("test")],
            max_depth: None,
        };
        let cloned = original.clone();

        assert_eq!(original.verbose, cloned.verbose);
        assert_eq!(original.threads, cloned.threads);
        assert_eq!(original.skip, cloned.skip);
    }
}
