//! Executable preservation logic.
//!
//! This module provides functionality to copy compiled executables out of
//! build directories before they are deleted during cleanup. This allows
//! users to retain usable binaries while still reclaiming build artifact space.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::project::{Project, ProjectType};

/// Extensions to exclude when looking for Rust executables.
const RUST_EXCLUDED_EXTENSIONS: &[&str] = &["d", "rmeta", "rlib", "a", "so", "dylib", "dll", "pdb"];

/// Check whether a file is an executable binary.
///
/// On Unix, this inspects the permission bits for the executable flag.
/// On Windows, this checks for the `.exe` file extension.
#[cfg(unix)]
fn is_executable(path: &Path, metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let _ = path; // unused on Unix – we rely on permission bits
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(windows)]
fn is_executable(path: &Path, _metadata: &fs::Metadata) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
}

/// A record of a single preserved executable file.
#[derive(Debug)]
pub struct PreservedExecutable {
    /// Original path inside the build directory
    pub source: PathBuf,
    /// Destination path where the file was copied
    pub destination: PathBuf,
}

/// Preserve compiled executables from a project's build directory.
///
/// Copies executable files to `<project_root>/bin/` before the build
/// directory is deleted. The behavior depends on the project type:
///
/// - **Rust**: copies executables from `target/release/` and `target/debug/`
/// - **Python**: copies `.whl` files from `dist/` and `.so`/`.pyd` extensions from `build/`
/// - **Node / Go / Java / C++ / Swift / .NET**: no-op (their cleanable dirs are dependencies or build outputs not easily preservable)
///
/// # Errors
///
/// Returns an error if creating destination directories or copying files fails.
pub fn preserve_executables(project: &Project) -> Result<Vec<PreservedExecutable>> {
    match project.kind {
        ProjectType::Rust => preserve_rust_executables(project),
        ProjectType::Python => preserve_python_executables(project),
        ProjectType::Node
        | ProjectType::Go
        | ProjectType::Java
        | ProjectType::Cpp
        | ProjectType::Swift
        | ProjectType::DotNet
        | ProjectType::Ruby
        | ProjectType::Elixir
        | ProjectType::Deno => Ok(Vec::new()),
    }
}

/// Preserve Rust executables from `target/release/` and `target/debug/`.
fn preserve_rust_executables(project: &Project) -> Result<Vec<PreservedExecutable>> {
    let Some(primary) = project.build_arts.first() else {
        return Ok(Vec::new());
    };
    let target_dir = &primary.path;
    let bin_dir = project.root_path.join("bin");
    let mut preserved = Vec::new();

    for profile in &["release", "debug"] {
        let profile_dir = target_dir.join(profile);
        if !profile_dir.is_dir() {
            continue;
        }

        let dest_dir = bin_dir.join(profile);
        let executables = find_rust_executables(&profile_dir)?;

        if executables.is_empty() {
            continue;
        }

        fs::create_dir_all(&dest_dir)
            .with_context(|| format!("Failed to create {}", dest_dir.display()))?;

        for exe_path in executables {
            let file_name = exe_path
                .file_name()
                .expect("executable path should have a file name");
            let dest_path = dest_dir.join(file_name);

            fs::copy(&exe_path, &dest_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    exe_path.display(),
                    dest_path.display()
                )
            })?;

            preserved.push(PreservedExecutable {
                source: exe_path,
                destination: dest_path,
            });
        }
    }

    Ok(preserved)
}

/// Find executable files in a Rust profile directory (e.g. `target/release/`).
///
/// Returns files that pass [`is_executable`] and are not build metadata
/// (excludes `.d`, `.rmeta`, `.rlib`, `.a`, `.so`, `.dylib`, `.dll`, `.pdb`
/// extensions).
fn find_rust_executables(profile_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut executables = Vec::new();

    let entries = fs::read_dir(profile_dir)
        .with_context(|| format!("Failed to read {}", profile_dir.display()))?;

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        // Skip files with excluded extensions
        if let Some(ext) = path.extension().and_then(|e| e.to_str())
            && RUST_EXCLUDED_EXTENSIONS.contains(&ext)
        {
            continue;
        }

        // Check if file is executable
        let metadata = path.metadata()?;
        if is_executable(&path, &metadata) {
            executables.push(path);
        }
    }

    Ok(executables)
}

/// Preserve Python build outputs: `.whl` from `dist/` and C extensions from `build/`.
fn preserve_python_executables(project: &Project) -> Result<Vec<PreservedExecutable>> {
    let root = &project.root_path;
    let bin_dir = root.join("bin");
    let mut preserved = Vec::new();

    collect_wheel_files(&root.join("dist"), &bin_dir, &mut preserved)?;
    collect_native_extensions(&root.join("build"), &bin_dir, &mut preserved)?;

    Ok(preserved)
}

/// Copy `.whl` wheel files from the `dist/` directory into `bin_dir`.
fn collect_wheel_files(
    dist_dir: &Path,
    bin_dir: &Path,
    preserved: &mut Vec<PreservedExecutable>,
) -> Result<()> {
    if !dist_dir.is_dir() {
        return Ok(());
    }

    let Ok(entries) = fs::read_dir(dist_dir) else {
        return Ok(());
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("whl") {
            copy_to_bin(&path, bin_dir, preserved)?;
        }
    }

    Ok(())
}

/// Recursively copy `.so` / `.pyd` C extension files from the `build/` directory into `bin_dir`.
fn collect_native_extensions(
    build_dir: &Path,
    bin_dir: &Path,
    preserved: &mut Vec<PreservedExecutable>,
) -> Result<()> {
    if !build_dir.is_dir() {
        return Ok(());
    }

    for entry in walkdir::WalkDir::new(build_dir)
        .into_iter()
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let is_native_ext = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| ext == "so" || ext == "pyd");

        if is_native_ext {
            copy_to_bin(path, bin_dir, preserved)?;
        }
    }

    Ok(())
}

/// Copy a single file into `bin_dir`, creating the directory if needed,
/// and record it as a [`PreservedExecutable`].
fn copy_to_bin(
    source: &Path,
    bin_dir: &Path,
    preserved: &mut Vec<PreservedExecutable>,
) -> Result<()> {
    fs::create_dir_all(bin_dir)
        .with_context(|| format!("Failed to create {}", bin_dir.display()))?;

    let file_name = source
        .file_name()
        .expect("source path should have a file name");
    let dest_path = bin_dir.join(file_name);

    fs::copy(source, &dest_path).with_context(|| {
        format!(
            "Failed to copy {} to {}",
            source.display(),
            dest_path.display()
        )
    })?;

    preserved.push(PreservedExecutable {
        source: source.to_path_buf(),
        destination: dest_path,
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::BuildArtifacts;
    use tempfile::TempDir;

    fn create_test_project(tmp: &TempDir, kind: ProjectType) -> Project {
        let root = tmp.path().to_path_buf();
        let build_dir = match kind {
            ProjectType::Rust | ProjectType::Java => root.join("target"),
            ProjectType::Python => root.join("__pycache__"),
            ProjectType::Node | ProjectType::Deno => root.join("node_modules"),
            ProjectType::Go | ProjectType::Ruby => root.join("vendor"),
            ProjectType::Cpp => root.join("build"),
            ProjectType::Swift => root.join(".build"),
            ProjectType::DotNet => root.join("obj"),
            ProjectType::Elixir => root.join("_build"),
        };

        fs::create_dir_all(&build_dir).unwrap();

        Project::new(
            kind,
            root,
            vec![BuildArtifacts {
                path: build_dir,
                size: 0,
            }],
            Some("test-project".to_string()),
        )
    }

    #[test]
    #[cfg(unix)]
    fn test_preserve_rust_executables_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        // Create target/release/ with an executable and a metadata file
        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        let exe_path = release_dir.join("my-binary");
        fs::write(&exe_path, b"fake binary").unwrap();
        fs::set_permissions(&exe_path, fs::Permissions::from_mode(0o755)).unwrap();

        let dep_file = release_dir.join("my-binary.d");
        fs::write(&dep_file, b"dep info").unwrap();

        let result = preserve_executables(&project).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].destination,
            tmp.path().join("bin/release/my-binary")
        );
        assert!(result[0].destination.exists());
    }

    #[test]
    #[cfg(windows)]
    fn test_preserve_rust_executables_windows() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // On Windows, executables have the .exe extension
        let exe_path = release_dir.join("my-binary.exe");
        fs::write(&exe_path, b"fake binary").unwrap();

        let dep_file = release_dir.join("my-binary.d");
        fs::write(&dep_file, b"dep info").unwrap();

        let result = preserve_executables(&project).unwrap();

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].destination,
            tmp.path().join("bin/release/my-binary.exe")
        );
        assert!(result[0].destination.exists());
    }

    #[test]
    #[cfg(unix)]
    fn test_preserve_rust_skips_non_executable_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // Non-executable file (mode 0o644)
        let non_exe = release_dir.join("some-file");
        fs::write(&non_exe, b"not executable").unwrap();
        fs::set_permissions(&non_exe, fs::Permissions::from_mode(0o644)).unwrap();

        let result = preserve_executables(&project).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    #[cfg(windows)]
    fn test_preserve_rust_skips_non_executable_windows() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // On Windows, a file without .exe extension is not treated as executable
        let non_exe = release_dir.join("some-file.txt");
        fs::write(&non_exe, b"not executable").unwrap();

        let result = preserve_executables(&project).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_node_is_noop() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Node);

        let result = preserve_executables(&project).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_go_is_noop() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Go);

        let result = preserve_executables(&project).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_preserve_rust_no_profile_dirs() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        // target/ exists but no release/ or debug/ subdirs
        let result = preserve_executables(&project).unwrap();
        assert!(result.is_empty());
        assert!(!tmp.path().join("bin").exists());
    }

    // ── Unix-specific tests ─────────────────────────────────────────────

    #[test]
    #[cfg(unix)]
    fn test_find_multiple_rust_executables_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // Create multiple executables
        for name in &["binary-a", "binary-b", "binary-c"] {
            let exe_path = release_dir.join(name);
            fs::write(&exe_path, b"fake binary").unwrap();
            fs::set_permissions(&exe_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = preserve_executables(&project).unwrap();
        assert_eq!(result.len(), 3);

        for preserved in &result {
            assert!(preserved.destination.exists());
            assert!(
                preserved
                    .destination
                    .starts_with(tmp.path().join("bin/release"))
            );
        }
    }

    #[test]
    #[cfg(unix)]
    fn test_find_rust_executables_excludes_metadata_even_if_executable_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // Create files with excluded extensions but with executable permissions
        let excluded_files = [
            "dep.d",
            "lib.rmeta",
            "lib.rlib",
            "archive.a",
            "shared.so",
            "shared.dylib",
            "shared.dll",
            "debug.pdb",
        ];

        for name in &excluded_files {
            let file_path = release_dir.join(name);
            fs::write(&file_path, b"fake content").unwrap();
            fs::set_permissions(&file_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        // Also add a real executable to make sure it IS found
        let exe_path = release_dir.join("real-binary");
        fs::write(&exe_path, b"real binary").unwrap();
        fs::set_permissions(&exe_path, fs::Permissions::from_mode(0o755)).unwrap();

        let result = preserve_executables(&project).unwrap();
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .destination
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("real-binary")
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_is_executable_permission_variants_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();

        // Test user-only execute (0o100)
        let user_exe = tmp.path().join("user_exe");
        fs::write(&user_exe, b"content").unwrap();
        fs::set_permissions(&user_exe, fs::Permissions::from_mode(0o700)).unwrap();
        let meta = user_exe.metadata().unwrap();
        assert!(is_executable(&user_exe, &meta));

        // Test group-only execute (0o010)
        let group_exe = tmp.path().join("group_exe");
        fs::write(&group_exe, b"content").unwrap();
        fs::set_permissions(&group_exe, fs::Permissions::from_mode(0o070)).unwrap();
        let meta = group_exe.metadata().unwrap();
        assert!(is_executable(&group_exe, &meta));

        // Test other-only execute (0o001)
        let other_exe = tmp.path().join("other_exe");
        fs::write(&other_exe, b"content").unwrap();
        fs::set_permissions(&other_exe, fs::Permissions::from_mode(0o601)).unwrap();
        let meta = other_exe.metadata().unwrap();
        assert!(is_executable(&other_exe, &meta));

        // Test no execute at all (0o644)
        let no_exe = tmp.path().join("no_exe");
        fs::write(&no_exe, b"content").unwrap();
        fs::set_permissions(&no_exe, fs::Permissions::from_mode(0o644)).unwrap();
        let meta = no_exe.metadata().unwrap();
        assert!(!is_executable(&no_exe, &meta));
    }

    #[test]
    #[cfg(unix)]
    fn test_preserve_rust_debug_and_release_unix() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        // Create executables in both debug and release
        for profile in &["debug", "release"] {
            let profile_dir = tmp.path().join("target").join(profile);
            fs::create_dir_all(&profile_dir).unwrap();

            let exe_path = profile_dir.join("my-binary");
            fs::write(&exe_path, b"fake binary").unwrap();
            fs::set_permissions(&exe_path, fs::Permissions::from_mode(0o755)).unwrap();
        }

        let result = preserve_executables(&project).unwrap();
        assert_eq!(result.len(), 2);

        // Verify both profiles have preserved executables
        let dest_names: Vec<_> = result
            .iter()
            .map(|p| p.destination.to_string_lossy().to_string())
            .collect();

        assert!(dest_names.iter().any(|d| d.contains("bin/release")));
        assert!(dest_names.iter().any(|d| d.contains("bin/debug")));
    }

    #[test]
    #[cfg(unix)]
    fn test_preserve_python_so_extensions_unix() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        // Create a build/ directory with .so extensions
        let build_dir = tmp.path().join("build/lib.linux-x86_64-3.9");
        fs::create_dir_all(&build_dir).unwrap();

        fs::write(
            build_dir.join("mymodule.cpython-39-x86_64-linux-gnu.so"),
            b"shared object",
        )
        .unwrap();
        fs::write(build_dir.join("another.so"), b"shared object").unwrap();

        let result = preserve_python_executables(&project).unwrap();
        assert_eq!(result.len(), 2);

        for preserved in &result {
            assert!(preserved.destination.exists());
            assert!(preserved.destination.starts_with(tmp.path().join("bin")));
        }
    }

    // ── Windows-specific tests ──────────────────────────────────────────

    #[test]
    #[cfg(windows)]
    fn test_is_executable_case_insensitive_exe_windows() {
        let tmp = TempDir::new().unwrap();

        // .exe
        let exe = tmp.path().join("app.exe");
        fs::write(&exe, b"content").unwrap();
        let meta = exe.metadata().unwrap();
        assert!(is_executable(&exe, &meta));

        // .EXE
        let exe_upper = tmp.path().join("app.EXE");
        fs::write(&exe_upper, b"content").unwrap();
        let meta = exe_upper.metadata().unwrap();
        assert!(is_executable(&exe_upper, &meta));

        // .Exe
        let exe_mixed = tmp.path().join("app.Exe");
        fs::write(&exe_mixed, b"content").unwrap();
        let meta = exe_mixed.metadata().unwrap();
        assert!(is_executable(&exe_mixed, &meta));

        // Not an exe
        let not_exe = tmp.path().join("app.txt");
        fs::write(&not_exe, b"content").unwrap();
        let meta = not_exe.metadata().unwrap();
        assert!(!is_executable(&not_exe, &meta));

        // No extension
        let no_ext = tmp.path().join("app");
        fs::write(&no_ext, b"content").unwrap();
        let meta = no_ext.metadata().unwrap();
        assert!(!is_executable(&no_ext, &meta));
    }

    #[test]
    #[cfg(windows)]
    fn test_preserve_rust_debug_and_release_windows() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        for profile in &["debug", "release"] {
            let profile_dir = tmp.path().join("target").join(profile);
            fs::create_dir_all(&profile_dir).unwrap();

            let exe_path = profile_dir.join("my-binary.exe");
            fs::write(&exe_path, b"fake binary").unwrap();
        }

        let result = preserve_executables(&project).unwrap();
        assert_eq!(result.len(), 2);

        let dest_names: Vec<_> = result
            .iter()
            .map(|p| p.destination.to_string_lossy().to_string())
            .collect();

        assert!(dest_names.iter().any(|d| d.contains("release")));
        assert!(dest_names.iter().any(|d| d.contains("debug")));
    }

    #[test]
    #[cfg(windows)]
    fn test_find_rust_executables_excludes_metadata_windows() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // Files with excluded extensions should be skipped
        fs::write(release_dir.join("dep.d"), b"dep info").unwrap();
        fs::write(release_dir.join("lib.dll"), b"library").unwrap();
        fs::write(release_dir.join("debug.pdb"), b"symbols").unwrap();
        fs::write(release_dir.join("lib.rlib"), b"rust lib").unwrap();

        // Only .exe should be found
        fs::write(release_dir.join("my-binary.exe"), b"real binary").unwrap();

        let result = preserve_executables(&project).unwrap();
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .destination
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("my-binary.exe")
        );
    }

    #[test]
    #[cfg(windows)]
    fn test_find_multiple_rust_executables_windows() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Rust);

        let release_dir = tmp.path().join("target/release");
        fs::create_dir_all(&release_dir).unwrap();

        // Create multiple .exe files
        for name in &["binary-a.exe", "binary-b.exe", "binary-c.exe"] {
            fs::write(release_dir.join(name), b"fake binary").unwrap();
        }

        let result = preserve_executables(&project).unwrap();
        assert_eq!(result.len(), 3);
    }

    #[test]
    #[cfg(windows)]
    fn test_preserve_python_pyd_extensions_windows() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        let build_dir = tmp.path().join("build/lib.win-amd64-3.9");
        fs::create_dir_all(&build_dir).unwrap();

        fs::write(
            build_dir.join("mymodule.cp39-win_amd64.pyd"),
            b"python extension",
        )
        .unwrap();
        fs::write(build_dir.join("another.pyd"), b"python extension").unwrap();

        let result = preserve_python_executables(&project).unwrap();
        assert_eq!(result.len(), 2);

        for preserved in &result {
            assert!(preserved.destination.exists());
        }
    }

    // ── Cross-platform tests (run on all OS) ────────────────────────────

    #[test]
    fn test_preserve_python_whl_files() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        // Create dist/ with .whl files
        let dist_dir = tmp.path().join("dist");
        fs::create_dir_all(&dist_dir).unwrap();

        fs::write(
            dist_dir.join("mypackage-1.0.0-py3-none-any.whl"),
            b"wheel content",
        )
        .unwrap();
        fs::write(dist_dir.join("mypackage-1.0.0.tar.gz"), b"tarball content").unwrap();

        let result = preserve_python_executables(&project).unwrap();
        // Only .whl should be preserved, not .tar.gz
        assert_eq!(result.len(), 1);
        assert!(
            result[0]
                .destination
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("whl"))
        );
    }

    #[test]
    fn test_preserve_python_no_dist_no_build() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        // No dist/ or build/ dirs exist
        let result = preserve_python_executables(&project).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_preserve_python_empty_dist_and_build() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        // Create empty dist/ and build/ directories
        fs::create_dir_all(tmp.path().join("dist")).unwrap();
        fs::create_dir_all(tmp.path().join("build")).unwrap();

        let result = preserve_python_executables(&project).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_preserve_python_whl_and_extensions_combined() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        // Create dist/ with .whl files
        let dist_dir = tmp.path().join("dist");
        fs::create_dir_all(&dist_dir).unwrap();
        fs::write(dist_dir.join("mypackage-1.0.0-py3-none-any.whl"), b"wheel").unwrap();

        // Create build/ with extensions (.so on Unix, .pyd on Windows)
        let build_dir = tmp.path().join("build/lib");
        fs::create_dir_all(&build_dir).unwrap();

        #[cfg(unix)]
        fs::write(build_dir.join("native.so"), b"shared object").unwrap();

        #[cfg(windows)]
        fs::write(build_dir.join("native.pyd"), b"python extension").unwrap();

        let result = preserve_python_executables(&project).unwrap();
        // Should find both the .whl and the platform-specific extension
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_preserve_executables_returns_correct_source_paths() {
        let tmp = TempDir::new().unwrap();
        let project = create_test_project(&tmp, ProjectType::Python);

        let dist_dir = tmp.path().join("dist");
        fs::create_dir_all(&dist_dir).unwrap();
        let whl_path = dist_dir.join("pkg-1.0-py3-none-any.whl");
        fs::write(&whl_path, b"wheel content").unwrap();

        let result = preserve_python_executables(&project).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].source, whl_path);
        assert_eq!(
            result[0].destination,
            tmp.path().join("bin/pkg-1.0-py3-none-any.whl")
        );
    }
}
