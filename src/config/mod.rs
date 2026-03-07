//! Configuration types and options for the application.
//!
//! This module contains all configuration structures used throughout the application,
//! including filtering, scanning, execution options, and persistent file-based configuration.

pub mod execution;
pub mod file;
pub mod filter;
pub mod scan;

pub use execution::ExecutionOptions;
pub use file::FileConfig;
pub use filter::{FilterOptions, ProjectFilter, SortCriteria, SortOptions};
pub use scan::ScanOptions;
