//! Project detection and management functionality.
//!
//! This module contains the core data structures and logic for representing
//! and managing development projects. It provides types for individual projects,
//! collections of projects, and the operations that can be performed on them.
//!
//! ## Main Parts
//!
//! - [`Project`] - Represents an individual development project with build artifacts
//! - [`Projects`] - A collection of projects with batch operations
//! - [`ProjectType`] - Enumeration of supported project types (Rust, Node.js, Python, Go, Java, C/C++, Swift, .NET, Ruby, Elixir, Deno)
//! - [`BuildArtifacts`] - Information about build directories and their sizes

#[allow(clippy::module_inception)]
// This is acceptable as it is the main module for project management
pub mod project;
pub mod projects;

pub use project::{BuildArtifacts, Project, ProjectType};
pub use projects::Projects;
