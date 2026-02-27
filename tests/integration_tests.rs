//! Integration tests for clean-dev-dirs
//!
//! These tests create temporary file structures to test the real functionality
//! of the scanner and other components with actual filesystem operations.

use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

use clean_dev_dirs::config::{ProjectFilter, ScanOptions};
use clean_dev_dirs::project::{BuildArtifacts, ProjectType};
use clean_dev_dirs::scanner::Scanner;

/// Helper function to create a temporary directory structure for testing
fn create_test_directory() -> TempDir {
    TempDir::new().expect("Failed to create temporary directory")
}

/// Helper function to create a file with specified content
fn create_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("Failed to create parent directories");
    }
    fs::write(path, content).expect("Failed to write file");
}

/// Helper function to create a directory
fn create_dir(path: &Path) {
    fs::create_dir_all(path).expect("Failed to create directory");
}

/// Create a mock Rust project with Cargo.toml and target/ directory
fn create_rust_project(base_path: &Path, project_name: &str) -> PathBuf {
    let project_path = base_path.join(project_name);

    // Create Cargo.toml
    let cargo_toml_content = format!(
        r#"[package]
name = "{project_name}"
version = "0.1.0"
edition = "2021"

[dependencies]
"#
    );
    create_file(&project_path.join("Cargo.toml"), &cargo_toml_content);

    // Create target directory with some files
    let target_path = project_path.join("target");
    create_dir(&target_path);
    create_file(
        &target_path.join("debug").join("build.log"),
        "Build log content",
    );
    create_file(
        &target_path.join("release").join("binary"),
        "Binary content",
    );

    project_path
}

/// Create a mock Node.js project with package.json and `node_modules`/ directory
fn create_node_project(base_path: &Path, project_name: &str) -> PathBuf {
    let project_path = base_path.join(project_name);

    // Create package.json
    let package_json_content = format!(
        r#"{{
  "name": "{project_name}",
  "version": "1.0.0",
  "description": "Test Node.js project",
  "main": "index.js",
  "dependencies": {{
    "express": "^4.18.0"
  }}
}}"#
    );
    create_file(&project_path.join("package.json"), &package_json_content);

    // Create node_modules directory with some files
    let node_modules_path = project_path.join("node_modules");
    create_dir(&node_modules_path);
    create_file(
        &node_modules_path.join("express").join("package.json"),
        "{}",
    );
    create_file(
        &node_modules_path.join(".bin").join("express"),
        "#!/bin/bash",
    );

    project_path
}

/// Create a mock Python project with requirements.txt and __pycache__/ directory
fn create_python_project(base_path: &Path, project_name: &str) -> PathBuf {
    let project_path = base_path.join(project_name);

    // Create requirements.txt
    create_file(
        &project_path.join("requirements.txt"),
        "requests==2.28.0\nflask==2.3.0\n",
    );

    // Create __pycache__ directory with some files
    let pycache_path = project_path.join("__pycache__");
    create_dir(&pycache_path);
    create_file(&pycache_path.join("main.cpython-39.pyc"), "Python bytecode");
    create_file(
        &pycache_path.join("utils.cpython-39.pyc"),
        "Python bytecode",
    );

    project_path
}

/// Create a mock Go project with go.mod and vendor/ directory
fn create_go_project(base_path: &Path, project_name: &str) -> PathBuf {
    let project_path = base_path.join(project_name);

    // Create go.mod
    let go_mod_content = format!(
        r"module {project_name}

go 1.19

require (
    github.com/gin-gonic/gin v1.9.0
)
"
    );
    create_file(&project_path.join("go.mod"), &go_mod_content);

    // Create vendor directory with some files
    let vendor_path = project_path.join("vendor");
    create_dir(&vendor_path);
    create_file(
        &vendor_path
            .join("github.com")
            .join("gin-gonic")
            .join("gin")
            .join("gin.go"),
        "package gin",
    );

    project_path
}

#[test]
fn test_scanner_finds_rust_projects() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create test projects
    create_rust_project(base_path, "rust-project-1");
    create_rust_project(base_path, "rust-project-2");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 2);

    for project in &projects {
        assert_eq!(project.kind, ProjectType::Rust);
        assert!(project.name.is_some());
        assert!(project.build_arts[0].path.ends_with("target"));
        assert!(project.total_size() > 0);
    }
}

#[test]
fn test_scanner_finds_node_projects() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create test projects
    create_node_project(base_path, "node-app-1");
    create_node_project(base_path, "node-app-2");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Node);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 2);

    for project in &projects {
        assert_eq!(project.kind, ProjectType::Node);
        assert!(project.name.is_some());
        assert!(project.build_arts[0].path.ends_with("node_modules"));
        assert!(project.total_size() > 0);
    }
}

#[test]
fn test_scanner_finds_python_projects() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create test projects
    create_python_project(base_path, "python-app-1");
    create_python_project(base_path, "python-app-2");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Python);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 2);

    for project in &projects {
        assert_eq!(project.kind, ProjectType::Python);
        assert!(
            project
                .build_arts
                .iter()
                .any(|a| a.path.ends_with("__pycache__"))
        );
        assert!(project.total_size() > 0);
    }
}

#[test]
fn test_scanner_finds_go_projects() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create test projects
    create_go_project(base_path, "go-service-1");
    create_go_project(base_path, "go-service-2");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Go);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 2);

    for project in &projects {
        assert_eq!(project.kind, ProjectType::Go);
        assert!(project.name.is_some());
        assert!(project.build_arts[0].path.ends_with("vendor"));
        assert!(project.total_size() > 0);
    }
}

#[test]
fn test_scanner_finds_all_project_types() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create one of each project type
    create_rust_project(base_path, "rust-project");
    create_node_project(base_path, "node-project");
    create_python_project(base_path, "python-project");
    create_go_project(base_path, "go-project");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::All);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 4);

    let mut found_types = vec![];
    for project in &projects {
        found_types.push(project.kind.clone());
    }

    assert!(found_types.contains(&ProjectType::Rust));
    assert!(found_types.contains(&ProjectType::Node));
    assert!(found_types.contains(&ProjectType::Python));
    assert!(found_types.contains(&ProjectType::Go));
}

#[test]
fn test_scanner_skips_directories() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create projects in various subdirectories
    create_rust_project(base_path, "rust-project");
    create_rust_project(&base_path.join("target"), "nested-rust-project");
    create_rust_project(&base_path.join("skip-me"), "skipped-rust-project");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![PathBuf::from("skip-me"), PathBuf::from("target")],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    // Should only find the top-level project, not the ones in skipped directories
    assert_eq!(projects.len(), 1);
    assert!(projects[0].root_path.ends_with("rust-project"));
}

#[test]
fn test_scanner_calculates_build_directory_sizes() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    let project_path = create_rust_project(base_path, "rust-project");
    let target_path = project_path.join("target");

    // Add more files with known sizes
    create_file(&target_path.join("large-file.bin"), &"x".repeat(1000));
    create_file(&target_path.join("small-file.txt"), "small");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 1);

    let project = &projects[0];
    assert!(project.total_size() > 1000); // Should include our large file
}

#[test]
fn test_scanner_handles_empty_directories() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create a project structure but with empty build directories
    let project_path = base_path.join("empty-rust-project");
    create_file(
        &project_path.join("Cargo.toml"),
        "[package]\nname = \"empty\"\nversion = \"0.1.0\"",
    );
    create_dir(&project_path.join("target")); // Empty target directory

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    // Empty target directories should not be included (size = 0)
    assert_eq!(projects.len(), 0);
}

#[test]
fn test_scanner_handles_missing_build_directories() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create project configuration but no build directory
    let project_path = base_path.join("no-target-project");
    create_file(
        &project_path.join("Cargo.toml"),
        "[package]\nname = \"no-target\"\nversion = \"0.1.0\"",
    );
    // No target directory created

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    // Projects without build directories should not be found
    assert_eq!(projects.len(), 0);
}

#[test]
fn test_scanner_nested_projects() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create nested project structure
    create_rust_project(base_path, "parent-project");
    create_rust_project(&base_path.join("parent-project"), "child-project");
    create_node_project(&base_path.join("parent-project").join("frontend"), "ui-app");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::All);
    let projects = scanner.scan_directory(base_path);

    // Should find all 3 projects (2 Rust + 1 Node.js)
    assert_eq!(projects.len(), 3);

    let rust_count = projects
        .iter()
        .filter(|p| p.kind == ProjectType::Rust)
        .count();
    let node_count = projects
        .iter()
        .filter(|p| p.kind == ProjectType::Node)
        .count();

    assert_eq!(rust_count, 2);
    assert_eq!(node_count, 1);
}

#[test]
fn test_scanner_with_multiple_threads() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create multiple projects
    for i in 0..10 {
        create_rust_project(base_path, &format!("rust-project-{i}"));
        create_node_project(base_path, &format!("node-project-{i}"));
    }

    let scan_options = ScanOptions {
        verbose: false,
        threads: 4, // Use multiple threads
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::All);
    let projects = scanner.scan_directory(base_path);

    // Should find all 20 projects (10 Rust + 10 Node.js)
    assert_eq!(projects.len(), 20);
}

#[test]
fn test_build_artifacts_structure() {
    let temp_dir = create_test_directory();
    let project_path = create_rust_project(temp_dir.path(), "test-project");
    let target_path = project_path.join("target");

    let artifacts = BuildArtifacts {
        path: target_path.clone(),
        size: 12345,
    };

    assert_eq!(artifacts.path, target_path);
    assert_eq!(artifacts.size, 12345);

    // Test cloning
    let cloned = artifacts.clone();
    assert_eq!(artifacts.path, cloned.path);
    assert_eq!(artifacts.size, cloned.size);
}

#[test]
fn test_project_types_comprehensive() {
    // Test all project type variants
    assert_eq!(ProjectType::Rust, ProjectType::Rust);
    assert_eq!(ProjectType::Node, ProjectType::Node);
    assert_eq!(ProjectType::Python, ProjectType::Python);
    assert_eq!(ProjectType::Go, ProjectType::Go);

    // Test cloning
    let rust_type = ProjectType::Rust;
    let cloned_type = rust_type.clone();
    assert_eq!(rust_type, cloned_type);
}

// ═══════════════════════════════════════════════════════════════════════
// Cross-platform path handling tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_scanner_with_spaces_in_directory_names() {
    let temp_dir = create_test_directory();
    let base = temp_dir.path().join("directory with spaces");
    create_dir(&base);

    create_rust_project(&base, "spaced rust project");
    create_node_project(&base, "spaced node project");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::All);
    let projects = scanner.scan_directory(&base);

    assert_eq!(projects.len(), 2);
}

#[test]
fn test_scanner_with_unicode_directory_names() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Japanese characters
    create_rust_project(base_path, "プロジェクト");
    // Emoji in directory name
    create_node_project(base_path, "my-app-🚀");
    // Accented characters
    create_python_project(base_path, "café-project");
    // Chinese characters
    create_go_project(base_path, "项目");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::All);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 4);
}

#[test]
fn test_scanner_with_deeply_nested_directories() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create a deeply nested project (tests platform path length limits)
    let deep_path = base_path
        .join("level1")
        .join("level2")
        .join("level3")
        .join("level4")
        .join("level5");

    create_rust_project(&deep_path, "deep-project");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name.as_deref(), Some("deep-project"));
}

#[test]
fn test_scanner_with_special_characters_in_paths() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Dashes, underscores, dots, parentheses
    create_rust_project(base_path, "my-project_v2.0");
    create_node_project(base_path, "app (copy)");
    create_python_project(base_path, "lib.utils.v3");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::All);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 3);
}

// ═══════════════════════════════════════════════════════════════════════
// Unix-specific integration tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[cfg(unix)]
fn test_scanner_hidden_directory_itself_not_detected_unix() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Visible project — should be found
    create_rust_project(base_path, "visible-project");

    // A hidden directory that IS a project (Cargo.toml + target/ inside .hidden-project/)
    // should NOT be detected because its name starts with '.'
    let hidden_project = base_path.join(".hidden-project");
    create_dir(&hidden_project);
    create_file(
        &hidden_project.join("Cargo.toml"),
        "[package]\nname = \"hidden\"\nversion = \"0.1.0\"",
    );
    create_dir(&hidden_project.join("target"));
    create_file(&hidden_project.join("target/dummy"), "content");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    // Only the visible project should be found
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name.as_deref(), Some("visible-project"));
}

#[test]
#[cfg(unix)]
fn test_scanner_traverses_into_hidden_dirs_finds_visible_children_unix() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // A non-hidden project nested inside a hidden directory.
    // The scanner still descends into hidden dirs, so visible children are found.
    let hidden_base = base_path.join(".hidden-dir");
    create_dir(&hidden_base);
    create_rust_project(&hidden_base, "child-project");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0].name.as_deref(), Some("child-project"));
}

#[test]
#[cfg(unix)]
fn test_executable_preservation_integration_unix() {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = create_test_directory();
    let project_path = create_rust_project(temp_dir.path(), "exe-project");

    // Create a real executable in the release dir
    let release_dir = project_path.join("target/release");
    create_dir(&release_dir);
    let exe = release_dir.join("my-tool");
    create_file(&exe, "#!/bin/bash\necho hello");
    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();

    // Also create a non-executable file
    create_file(&release_dir.join("deps.d"), "dep info");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(temp_dir.path());

    assert_eq!(projects.len(), 1);

    // Preserve executables
    let preserved = clean_dev_dirs::executables::preserve_executables(&projects[0]).unwrap();
    assert_eq!(preserved.len(), 1);
    assert!(preserved[0].destination.exists());
    assert!(
        preserved[0]
            .destination
            .to_string_lossy()
            .contains("bin/release/my-tool")
    );
}

#[test]
#[cfg(unix)]
fn test_scanner_symlink_handling_unix() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create a real project
    let real_project = create_rust_project(base_path, "real-project");

    // Create a symlink to the project directory
    let link_path = base_path.join("linked-project");
    std::os::unix::fs::symlink(&real_project, &link_path).unwrap();

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    // Should find at least the real project (symlink behavior may vary)
    assert!(!projects.is_empty());
}

// ═══════════════════════════════════════════════════════════════════════
// Windows-specific integration tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
#[cfg(windows)]
fn test_executable_preservation_integration_windows() {
    let temp_dir = create_test_directory();
    let project_path = create_rust_project(temp_dir.path(), "exe-project");

    let release_dir = project_path.join("target\\release");
    create_dir(&release_dir);

    // On Windows, executables have the .exe extension
    create_file(&release_dir.join("my-tool.exe"), "fake binary");
    create_file(&release_dir.join("deps.d"), "dep info");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(temp_dir.path());

    assert_eq!(projects.len(), 1);

    let preserved = clean_dev_dirs::executables::preserve_executables(&projects[0]).unwrap();
    assert_eq!(preserved.len(), 1);
    assert!(preserved[0].destination.exists());
    assert!(
        preserved[0]
            .destination
            .to_string_lossy()
            .contains("my-tool.exe")
    );
}

#[test]
#[cfg(windows)]
fn test_scanner_with_windows_long_paths() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    // Create a project with a longer-than-typical Windows path
    let long_segments: Vec<&str> = vec!["somewhat_long_directory_name"; 6];
    let mut long_path = base_path.to_path_buf();
    for segment in &long_segments {
        long_path = long_path.join(segment);
    }

    create_rust_project(&long_path, "deep-windows-project");

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Rust);
    let projects = scanner.scan_directory(base_path);

    assert_eq!(projects.len(), 1);
}

// ═══════════════════════════════════════════════════════════════════════
// Python cross-platform extension tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_python_whl_preservation_cross_platform() {
    let temp_dir = create_test_directory();
    let project_path = create_python_project(temp_dir.path(), "py-dist-project");

    // Create dist/ with .whl files (platform-agnostic format)
    let dist_dir = project_path.join("dist");
    create_dir(&dist_dir);
    create_file(
        &dist_dir.join("mypackage-1.0.0-py3-none-any.whl"),
        "wheel content",
    );

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Python);
    let projects = scanner.scan_directory(temp_dir.path());

    assert_eq!(projects.len(), 1);

    let preserved = clean_dev_dirs::executables::preserve_executables(&projects[0]).unwrap();
    // Should find the .whl file on any platform
    assert_eq!(preserved.len(), 1);
    assert!(preserved[0].destination.to_string_lossy().ends_with(".whl"));
}

#[test]
#[cfg(unix)]
fn test_python_so_preservation_unix() {
    let temp_dir = create_test_directory();
    let project_path = create_python_project(temp_dir.path(), "py-native-project");

    // .so files are Unix shared objects
    let build_dir = project_path.join("build/lib.linux-x86_64-cpython-39");
    create_dir(&build_dir);
    create_file(
        &build_dir.join("_native.cpython-39-x86_64-linux-gnu.so"),
        "shared object",
    );

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Python);
    let projects = scanner.scan_directory(temp_dir.path());

    assert_eq!(projects.len(), 1);

    let preserved = clean_dev_dirs::executables::preserve_executables(&projects[0]).unwrap();
    assert_eq!(preserved.len(), 1);
    assert!(preserved[0].destination.to_string_lossy().ends_with(".so"));
}

#[test]
#[cfg(windows)]
fn test_python_pyd_preservation_windows() {
    let temp_dir = create_test_directory();
    let project_path = create_python_project(temp_dir.path(), "py-native-project");

    // .pyd files are Windows Python extensions
    let build_dir = project_path.join("build\\lib.win-amd64-cpython-39");
    create_dir(&build_dir);
    create_file(
        &build_dir.join("_native.cp39-win_amd64.pyd"),
        "python extension",
    );

    let scan_options = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };

    let scanner = Scanner::new(scan_options, ProjectFilter::Python);
    let projects = scanner.scan_directory(temp_dir.path());

    assert_eq!(projects.len(), 1);

    let preserved = clean_dev_dirs::executables::preserve_executables(&projects[0]).unwrap();
    assert_eq!(preserved.len(), 1);
    assert!(preserved[0].destination.to_string_lossy().ends_with(".pyd"));
}

// ═══════════════════════════════════════════════════════════════════════
// Config path cross-platform tests
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_config_path_ends_with_expected_suffix() {
    use clean_dev_dirs::FileConfig;

    if let Some(path) = FileConfig::config_path() {
        // On all platforms, the config file should be named config.toml
        // inside a clean-dev-dirs directory
        assert!(
            path.file_name().unwrap().to_str().unwrap() == "config.toml",
            "Config file should be named config.toml"
        );
        assert!(
            path.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                == "clean-dev-dirs",
            "Config should be inside clean-dev-dirs directory"
        );
    }
}

#[test]
fn test_tilde_expansion_cross_platform() {
    use clean_dev_dirs::config::file::expand_tilde;

    let path = PathBuf::from("~/my-projects");
    let expanded = expand_tilde(&path);

    if let Some(home) = dirs::home_dir() {
        assert_eq!(expanded, home.join("my-projects"));
        // Should not contain the tilde anymore
        assert!(!expanded.to_string_lossy().starts_with('~'));
    }
}

// ═══════════════════════════════════════════════════════════════════════
// Removal strategy tests (cross-platform)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_removal_strategy_from_bool() {
    use clean_dev_dirs::cleaner::RemovalStrategy;

    let trash = RemovalStrategy::from_use_trash(true);
    assert!(matches!(trash, RemovalStrategy::Trash));

    let permanent = RemovalStrategy::from_use_trash(false);
    assert!(matches!(permanent, RemovalStrategy::Permanent));
}

// ═══════════════════════════════════════════════════════════════════════
// Parallel scanning consistency (cross-platform)
// ═══════════════════════════════════════════════════════════════════════

#[test]
fn test_parallel_and_single_thread_produce_same_results() {
    let temp_dir = create_test_directory();
    let base_path = temp_dir.path();

    for i in 0..5 {
        create_rust_project(base_path, &format!("rust-{i}"));
        create_node_project(base_path, &format!("node-{i}"));
        create_python_project(base_path, &format!("python-{i}"));
        create_go_project(base_path, &format!("go-{i}"));
    }

    let single_thread = ScanOptions {
        verbose: false,
        threads: 1,
        skip: vec![],
    };
    let multi_thread = ScanOptions {
        verbose: false,
        threads: 4,
        skip: vec![],
    };

    let scanner1 = Scanner::new(single_thread, ProjectFilter::All);
    let scanner4 = Scanner::new(multi_thread, ProjectFilter::All);

    let mut projects_1t = scanner1.scan_directory(base_path);
    let mut projects_4t = scanner4.scan_directory(base_path);

    // Sort both by path for comparison
    projects_1t.sort_by(|a, b| a.root_path.cmp(&b.root_path));
    projects_4t.sort_by(|a, b| a.root_path.cmp(&b.root_path));

    assert_eq!(projects_1t.len(), projects_4t.len());

    for (p1, p4) in projects_1t.iter().zip(projects_4t.iter()) {
        assert_eq!(p1.kind, p4.kind);
        assert_eq!(p1.root_path, p4.root_path);
        assert_eq!(p1.name, p4.name);
    }
}
