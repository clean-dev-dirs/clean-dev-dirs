//! Global caches of package managers and build tools.
//!
//! Package managers and build tools keep large caches outside of the
//! projects: downloaded crates, npm tarballs, Maven artifacts, Go modules,
//! Xcode build products, and more. This module finds these caches, measures
//! them, and removes them.
//!
//! Every cache in this module is safe to delete: the tool that owns it
//! downloads or rebuilds the content the next time it needs it. The module
//! never deletes configuration files, installed binaries, or credentials.
//! For example, it removes `~/.cargo/registry` but never `~/.cargo/bin`.

use std::{
    collections::{HashMap, HashSet},
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::Result;
use clap::ValueEnum;
use colored::Colorize;
use humansize::{DECIMAL, format_size};
use rayon::prelude::*;
use serde::Serialize;

use crate::{
    cleaner::{CleanResult, RemovalStrategy},
    utils::calculate_dir_size,
};

/// A tool that owns a global cache.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, ValueEnum, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheKind {
    /// Cargo registry and git checkouts (`~/.cargo/registry`, `~/.cargo/git`).
    Cargo,
    /// npm package cache (`~/.npm/_cacache`).
    Npm,
    /// pnpm content-addressable store.
    Pnpm,
    /// Yarn package cache (classic and Berry).
    Yarn,
    /// Maven local repository (`~/.m2/repository`).
    Maven,
    /// Gradle caches and wrapper distributions (`~/.gradle/caches`, `~/.gradle/wrapper/dists`).
    Gradle,
    /// Go module cache and Go build cache.
    Go,
    /// pip download and wheel cache.
    Pip,
    /// uv cache.
    Uv,
    /// Xcode derived data (`~/Library/Developer/Xcode/DerivedData`, macOS only).
    Xcode,
}

impl CacheKind {
    /// All the cache kinds, in display order.
    pub const ALL: [Self; 10] = [
        Self::Cargo,
        Self::Npm,
        Self::Pnpm,
        Self::Yarn,
        Self::Maven,
        Self::Gradle,
        Self::Go,
        Self::Pip,
        Self::Uv,
        Self::Xcode,
    ];

    /// Human-readable name of the tool that owns the cache.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cargo => "Cargo",
            Self::Npm => "npm",
            Self::Pnpm => "pnpm",
            Self::Yarn => "Yarn",
            Self::Maven => "Maven",
            Self::Gradle => "Gradle",
            Self::Go => "Go",
            Self::Pip => "pip",
            Self::Uv => "uv",
            Self::Xcode => "Xcode",
        }
    }

    /// Short ASCII tag, in the same style as the project type tags.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Cargo => "[rs]",
            Self::Npm | Self::Pnpm | Self::Yarn => "[js]",
            Self::Maven | Self::Gradle => "[java]",
            Self::Go => "[go]",
            Self::Pip | Self::Uv => "[py]",
            Self::Xcode => "[swift]",
        }
    }
}

/// Environment variables that move a cache away from its default location.
const CACHE_ENV_VARS: &[&str] = &[
    "CARGO_HOME",
    "npm_config_cache",
    "NPM_CONFIG_CACHE",
    "npm_config_store_dir",
    "NPM_CONFIG_STORE_DIR",
    "YARN_CACHE_FOLDER",
    "GRADLE_USER_HOME",
    "GOMODCACHE",
    "GOPATH",
    "GOCACHE",
    "PIP_CACHE_DIR",
    "UV_CACHE_DIR",
];

/// The directories and variables that decide where the caches are.
///
/// [`CacheEnvironment::from_system`] reads them from the current user and
/// process. Tests build one by hand to point every cache into a temporary
/// directory.
#[derive(Clone, Debug, Default)]
pub struct CacheEnvironment {
    /// Home directory of the user.
    pub home: PathBuf,
    /// Platform cache directory (`~/Library/Caches`, `~/.cache`, `%LOCALAPPDATA%`).
    pub cache_dir: Option<PathBuf>,
    /// Platform local data directory (`~/Library/Application Support`,
    /// `~/.local/share`, `%LOCALAPPDATA%`).
    pub data_local_dir: Option<PathBuf>,
    /// Values of the environment variables in [`CACHE_ENV_VARS`].
    pub vars: HashMap<String, String>,
}

impl CacheEnvironment {
    /// Read the environment of the current user and process.
    ///
    /// Returns `None` if the home directory cannot be found.
    #[must_use]
    pub fn from_system() -> Option<Self> {
        let vars = CACHE_ENV_VARS
            .iter()
            .filter_map(|name| std::env::var(name).ok().map(|v| ((*name).to_string(), v)))
            .collect();

        Some(Self {
            home: dirs::home_dir()?,
            cache_dir: dirs::cache_dir(),
            data_local_dir: dirs::data_local_dir(),
            vars,
        })
    }

    /// Return the value of the first variable in `names` that is set and not empty.
    fn var(&self, names: &[&str]) -> Option<PathBuf> {
        names
            .iter()
            .filter_map(|name| self.vars.get(*name))
            .find(|value| !value.is_empty())
            .map(PathBuf::from)
    }

    /// Join `parts` to the platform cache directory, if it is known.
    fn in_cache_dir(&self, parts: &[&str]) -> Option<PathBuf> {
        self.cache_dir.as_ref().map(|dir| join_all(dir, parts))
    }

    /// Join `parts` to the platform local data directory, if it is known.
    fn in_data_local_dir(&self, parts: &[&str]) -> Option<PathBuf> {
        self.data_local_dir.as_ref().map(|dir| join_all(dir, parts))
    }

    /// Join `parts` to the home directory.
    fn in_home(&self, parts: &[&str]) -> PathBuf {
        join_all(&self.home, parts)
    }
}

/// Join every element of `parts` to `base`.
fn join_all(base: &Path, parts: &[&str]) -> PathBuf {
    parts
        .iter()
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// Return the directory in the first set variable of `names`, or else the
/// known `defaults`.
fn overridable(
    env: &CacheEnvironment,
    names: &[&str],
    defaults: Vec<Option<PathBuf>>,
) -> Vec<PathBuf> {
    env.var(names)
        .map_or_else(|| defaults.into_iter().flatten().collect(), |dir| vec![dir])
}

/// Return the paths where a cache of `kind` can be, on any platform.
///
/// Most of the paths do not exist on a given machine: the caller keeps only
/// the directories that exist. When an environment variable moves a cache,
/// the default locations of that cache are not returned.
#[must_use]
pub fn candidate_paths(kind: CacheKind, env: &CacheEnvironment) -> Vec<PathBuf> {
    match kind {
        CacheKind::Cargo => {
            let cargo_home = env
                .var(&["CARGO_HOME"])
                .unwrap_or_else(|| env.in_home(&[".cargo"]));
            vec![cargo_home.join("registry"), cargo_home.join("git")]
        }
        CacheKind::Npm => overridable(
            env,
            &["npm_config_cache", "NPM_CONFIG_CACHE"],
            vec![
                Some(env.in_home(&[".npm"])),
                env.in_data_local_dir(&["npm-cache"]),
            ],
        )
        .into_iter()
        .map(|dir| dir.join("_cacache"))
        .collect(),
        CacheKind::Pnpm => overridable(
            env,
            &["npm_config_store_dir", "NPM_CONFIG_STORE_DIR"],
            vec![
                Some(env.in_home(&["Library", "pnpm", "store"])),
                Some(env.in_home(&[".local", "share", "pnpm", "store"])),
                env.in_data_local_dir(&["pnpm", "store"]),
                Some(env.in_home(&[".pnpm-store"])),
            ],
        ),
        CacheKind::Yarn => overridable(
            env,
            &["YARN_CACHE_FOLDER"],
            vec![
                env.in_cache_dir(&["Yarn"]),
                env.in_cache_dir(&["yarn"]),
                env.in_data_local_dir(&["Yarn", "Cache"]),
                Some(env.in_home(&[".yarn", "berry", "cache"])),
            ],
        ),
        CacheKind::Maven => vec![env.in_home(&[".m2", "repository"])],
        CacheKind::Gradle => {
            let gradle_home = env
                .var(&["GRADLE_USER_HOME"])
                .unwrap_or_else(|| env.in_home(&[".gradle"]));
            vec![
                gradle_home.join("caches"),
                join_all(&gradle_home, &["wrapper", "dists"]),
            ]
        }
        CacheKind::Go => {
            let module_cache = env.var(&["GOMODCACHE"]).unwrap_or_else(|| {
                env.var(&["GOPATH"])
                    .and_then(|gopath| std::env::split_paths(&gopath).next())
                    .unwrap_or_else(|| env.in_home(&["go"]))
                    .join("pkg")
                    .join("mod")
            });
            let build_cache = env
                .var(&["GOCACHE"])
                .or_else(|| env.in_cache_dir(&["go-build"]));
            std::iter::once(module_cache).chain(build_cache).collect()
        }
        CacheKind::Pip => overridable(
            env,
            &["PIP_CACHE_DIR"],
            vec![
                env.in_cache_dir(&["pip"]),
                env.in_data_local_dir(&["pip", "Cache"]),
            ],
        ),
        CacheKind::Uv => overridable(
            env,
            &["UV_CACHE_DIR"],
            vec![
                Some(env.in_home(&[".cache", "uv"])),
                env.in_cache_dir(&["uv"]),
                env.in_data_local_dir(&["uv", "cache"]),
            ],
        ),
        CacheKind::Xcode => vec![env.in_home(&["Library", "Developer", "Xcode", "DerivedData"])],
    }
}

/// A global cache found on the disk.
#[derive(Clone, Debug, Serialize)]
pub struct GlobalCache {
    /// The tool that owns the cache.
    pub kind: CacheKind,
    /// Canonical path of the cache directory (symbolic links are resolved).
    pub path: PathBuf,
    /// Disk space that deleting the cache frees, in bytes.
    pub size: u64,
}

/// Find the global caches of the given kinds and measure them.
///
/// An empty `kinds` slice selects every kind. The function keeps only the
/// candidate paths that are directories, resolves symbolic links, and
/// removes duplicates (for example `~/.cache/uv` and `$XDG_CACHE_HOME/uv`
/// on Linux, or `Yarn` and `yarn` on a case-insensitive file system).
/// It measures the caches in parallel, drops the empty ones, and sorts them
/// by size, largest first.
#[must_use]
pub fn find_caches(env: &CacheEnvironment, kinds: &[CacheKind]) -> Vec<GlobalCache> {
    let kinds: &[CacheKind] = if kinds.is_empty() {
        &CacheKind::ALL
    } else {
        kinds
    };

    let mut seen = HashSet::new();
    let mut found = Vec::new();

    for &kind in kinds {
        for candidate in candidate_paths(kind, env) {
            let Ok(path) = fs::canonicalize(&candidate) else {
                continue;
            };

            if path.is_dir() && is_safe_cache_path(&path, env) && seen.insert(path.clone()) {
                found.push((kind, path));
            }
        }
    }

    let mut caches: Vec<GlobalCache> = found
        .into_par_iter()
        .map(|(kind, path)| GlobalCache {
            kind,
            size: calculate_dir_size(&path),
            path,
        })
        .filter(|cache| cache.size > 0)
        .collect();

    caches.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
    caches
}

/// Refuse paths that can never be a cache, even when an environment variable
/// points to them: the file system root and the home directory itself.
fn is_safe_cache_path(path: &Path, env: &CacheEnvironment) -> bool {
    let is_home = fs::canonicalize(&env.home).is_ok_and(|home| home == path);
    path.parent().is_some() && !is_home
}

/// Remove one cache directory and return the disk space that it freed.
///
/// # Errors
///
/// Returns an error if the directory cannot be moved to the trash or
/// deleted.
pub fn remove_cache(cache: &GlobalCache, strategy: RemovalStrategy) -> Result<u64> {
    let path = &cache.path;

    if !path.exists() {
        return Ok(0);
    }

    let size = calculate_dir_size(path);

    match strategy {
        RemovalStrategy::Trash => {
            trash::delete(path).map_err(|e| anyhow::anyhow!("failed to move to trash: {e}"))?;
        }
        RemovalStrategy::Permanent => remove_dir_all_forced(path)?,
    }

    Ok(size)
}

/// Delete a directory tree, also when some of its directories are read-only.
///
/// The Go module cache makes its directories read-only, so a plain
/// `fs::remove_dir_all` fails on it. On a permission error, this function
/// gives the owner write access to every directory in the tree and tries
/// again.
fn remove_dir_all_forced(path: &Path) -> io::Result<()> {
    match fs::remove_dir_all(path) {
        Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
            make_dirs_writable(path);
            fs::remove_dir_all(path)
        }
        other => other,
    }
}

/// Give the owner write access to every directory under `path`.
#[cfg(unix)]
fn make_dirs_writable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    for entry in walkdir::WalkDir::new(path).into_iter().flatten() {
        if entry.file_type().is_dir()
            && let Ok(metadata) = entry.metadata()
        {
            let mut permissions = metadata.permissions();
            permissions.set_mode(permissions.mode() | 0o700);
            let _ = fs::set_permissions(entry.path(), permissions);
        }
    }
}

/// Directory permissions are not changed on this platform.
#[cfg(not(unix))]
const fn make_dirs_writable(_path: &Path) {}

/// Remove the given caches in parallel.
///
/// A failure on one cache does not stop the others. The errors are in the
/// returned [`CleanResult`].
#[must_use]
pub fn clean_caches(caches: &[GlobalCache], strategy: RemovalStrategy) -> CleanResult {
    let results: Vec<Result<u64, String>> = caches
        .par_iter()
        .map(|cache| {
            remove_cache(cache, strategy)
                .map_err(|e| format!("Failed to clean {}: {e}", cache.path.display()))
        })
        .collect();

    let mut total_freed = 0u64;
    let mut errors = Vec::new();

    for result in results {
        match result {
            Ok(freed) => total_freed = total_freed.saturating_add(freed),
            Err(error) => errors.push(error),
        }
    }

    CleanResult {
        success_count: caches.len() - errors.len(),
        total_freed,
        estimated_size: total_size(caches),
        errors,
    }
}

/// Sum the sizes of the given caches.
#[must_use]
pub fn total_size(caches: &[GlobalCache]) -> u64 {
    caches
        .iter()
        .fold(0u64, |acc, cache| acc.saturating_add(cache.size))
}

/// Format one cache as a single line: tag, tool, size, and path.
#[must_use]
pub fn format_cache_line(cache: &GlobalCache) -> String {
    format!(
        "{:<7} {:<7} {:>10}  {}",
        cache.kind.tag(),
        cache.kind.label(),
        format_size(cache.size, DECIMAL),
        cache.path.display()
    )
}

/// Print the list of caches and their total size.
pub fn print_caches(caches: &[GlobalCache]) {
    for cache in caches {
        println!("  {}", format_cache_line(cache));
    }

    println!(
        "  Total reclaimable space: {}",
        format_size(total_size(caches), DECIMAL)
            .bright_green()
            .bold()
    );
}

/// Print a human-readable summary of a cache cleanup.
pub fn print_clean_summary(result: &CleanResult) {
    if !result.errors.is_empty() {
        println!("\n{}", "[!] Some errors occurred during cleanup:".yellow());
        for error in &result.errors {
            eprintln!("  {}", error.red());
        }
    }

    println!("\n{}", "Cleanup Summary:".bold());
    println!(
        "  [OK] Successfully cleaned: {} caches",
        result.success_count.to_string().green()
    );

    if !result.errors.is_empty() {
        println!(
            "  [FAIL] Failed to clean: {} caches",
            result.errors.len().to_string().red()
        );
    }

    println!(
        "  Total space freed: {}",
        format_size(result.total_freed, DECIMAL)
            .bright_green()
            .bold()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Build an environment where every directory is inside `root`.
    fn test_env(root: &Path) -> CacheEnvironment {
        CacheEnvironment {
            home: root.join("home"),
            cache_dir: Some(root.join("home/.cache")),
            data_local_dir: Some(root.join("home/.local/share")),
            vars: HashMap::new(),
        }
    }

    /// Create `path` with one file of `len` bytes inside it.
    fn make_cache(path: &Path, len: usize) -> anyhow::Result<()> {
        fs::create_dir_all(path)?;
        fs::write(path.join("blob.bin"), vec![b'x'; len])?;
        Ok(())
    }

    #[test]
    fn test_candidates_use_home_defaults() {
        let env = test_env(Path::new("/r"));

        let cargo = candidate_paths(CacheKind::Cargo, &env);
        assert_eq!(
            cargo,
            vec![
                PathBuf::from("/r/home/.cargo/registry"),
                PathBuf::from("/r/home/.cargo/git")
            ]
        );

        assert_eq!(
            candidate_paths(CacheKind::Maven, &env),
            vec![PathBuf::from("/r/home/.m2/repository")]
        );
        assert!(
            candidate_paths(CacheKind::Go, &env).contains(&PathBuf::from("/r/home/go/pkg/mod"))
        );
        assert!(
            candidate_paths(CacheKind::Go, &env)
                .contains(&PathBuf::from("/r/home/.cache/go-build"))
        );
    }

    #[test]
    fn test_candidates_never_include_cargo_bin() {
        let env = test_env(Path::new("/r"));
        let all: Vec<PathBuf> = CacheKind::ALL
            .iter()
            .flat_map(|kind| candidate_paths(*kind, &env))
            .collect();

        assert!(!all.iter().any(|p| p.ends_with("bin")));
        assert!(!all.contains(&PathBuf::from("/r/home/.cargo")));
        assert!(!all.contains(&PathBuf::from("/r/home/.m2")));
        assert!(!all.contains(&PathBuf::from("/r/home/.gradle")));
    }

    #[test]
    fn test_env_vars_replace_defaults() {
        let mut env = test_env(Path::new("/r"));
        env.vars.insert("CARGO_HOME".into(), "/opt/cargo".into());
        env.vars.insert("GOMODCACHE".into(), "/opt/gomod".into());
        env.vars.insert("GOCACHE".into(), "/opt/gobuild".into());
        env.vars.insert("PIP_CACHE_DIR".into(), "/opt/pip".into());
        env.vars
            .insert("npm_config_cache".into(), "/opt/npm".into());

        assert_eq!(
            candidate_paths(CacheKind::Cargo, &env),
            vec![
                PathBuf::from("/opt/cargo/registry"),
                PathBuf::from("/opt/cargo/git")
            ]
        );
        assert_eq!(
            candidate_paths(CacheKind::Go, &env),
            vec![PathBuf::from("/opt/gomod"), PathBuf::from("/opt/gobuild")]
        );
        assert_eq!(
            candidate_paths(CacheKind::Pip, &env),
            vec![PathBuf::from("/opt/pip")]
        );
        assert_eq!(
            candidate_paths(CacheKind::Npm, &env),
            vec![PathBuf::from("/opt/npm/_cacache")]
        );
    }

    #[test]
    fn test_empty_env_var_is_ignored() {
        let mut env = test_env(Path::new("/r"));
        env.vars.insert("CARGO_HOME".into(), String::new());

        assert_eq!(
            candidate_paths(CacheKind::Cargo, &env)[0],
            PathBuf::from("/r/home/.cargo/registry")
        );
    }

    #[test]
    fn test_gopath_uses_first_entry() {
        let mut env = test_env(Path::new("/r"));
        let gopath = std::env::join_paths(["/first", "/second"]).map_or_else(
            |_| String::from("/first"),
            |p| p.to_string_lossy().into_owned(),
        );
        env.vars.insert("GOPATH".into(), gopath);

        assert_eq!(
            candidate_paths(CacheKind::Go, &env)[0],
            PathBuf::from("/first/pkg/mod")
        );
    }

    #[test]
    fn test_find_caches_keeps_existing_dirs_sorted_by_size() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        make_cache(&env.home.join(".cargo/registry"), 10_000)?;
        make_cache(&env.home.join(".m2/repository"), 200_000)?;
        fs::create_dir_all(env.home.join(".cargo/bin"))?;

        let caches = find_caches(&env, &[]);

        assert_eq!(caches.len(), 2);
        assert_eq!(caches[0].kind, CacheKind::Maven);
        assert_eq!(caches[1].kind, CacheKind::Cargo);
        assert!(caches[0].size >= caches[1].size);
        assert!(caches.iter().all(|c| !c.path.ends_with("bin")));
        Ok(())
    }

    #[test]
    fn test_find_caches_skips_empty_caches() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        fs::create_dir_all(env.home.join("Library/Developer/Xcode/DerivedData"))?;
        make_cache(&env.home.join(".m2/repository"), 1_000)?;

        let caches = find_caches(&env, &[]);

        assert_eq!(caches.len(), 1);
        assert_eq!(caches[0].kind, CacheKind::Maven);
        Ok(())
    }

    #[test]
    fn test_find_caches_filters_by_kind() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        make_cache(&env.home.join(".cargo/registry"), 1_000)?;
        make_cache(&env.home.join(".m2/repository"), 1_000)?;

        let caches = find_caches(&env, &[CacheKind::Cargo]);

        assert_eq!(caches.len(), 1);
        assert_eq!(caches[0].kind, CacheKind::Cargo);
        Ok(())
    }

    #[test]
    fn test_find_caches_removes_duplicate_paths() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        // `~/.cache/uv` and `<cache_dir>/uv` are the same directory here.
        make_cache(&env.home.join(".cache/uv"), 1_000)?;

        let caches = find_caches(&env, &[CacheKind::Uv]);

        assert_eq!(caches.len(), 1);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_find_caches_resolves_symlinks() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        let real = tmp.path().join("external/m2-repository");
        make_cache(&real, 1_000)?;
        fs::create_dir_all(env.home.join(".m2"))?;
        std::os::unix::fs::symlink(&real, env.home.join(".m2/repository"))?;

        let caches = find_caches(&env, &[CacheKind::Maven]);

        assert_eq!(caches.len(), 1);
        assert_eq!(caches[0].path, fs::canonicalize(&real)?);
        assert!(caches[0].size > 0);
        Ok(())
    }

    #[test]
    fn test_find_caches_refuses_home_directory() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let mut env = test_env(tmp.path());
        make_cache(&env.home, 1_000)?;
        env.vars
            .insert("PIP_CACHE_DIR".into(), env.home.display().to_string());

        assert!(find_caches(&env, &[CacheKind::Pip]).is_empty());
        Ok(())
    }

    #[test]
    fn test_remove_cache_permanent_keeps_siblings() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        make_cache(&env.home.join(".cargo/registry"), 50_000)?;
        make_cache(&env.home.join(".cargo/bin"), 1_000)?;

        let caches = find_caches(&env, &[CacheKind::Cargo]);
        let result = clean_caches(&caches, RemovalStrategy::Permanent);

        assert_eq!(result.success_count, 1);
        assert_eq!(result.errors, Vec::<String>::new());
        assert!(result.total_freed >= 50_000);
        assert!(!env.home.join(".cargo/registry").exists());
        assert!(env.home.join(".cargo/bin/blob.bin").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_remove_cache_permanent_handles_read_only_dirs() -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;

        let tmp = TempDir::new()?;
        let env = test_env(tmp.path());

        // The Go module cache makes its module directories read-only.
        let module = env.home.join("go/pkg/mod/example.com/lib@v1.0.0");
        make_cache(&module, 1_000)?;
        fs::set_permissions(&module, fs::Permissions::from_mode(0o555))?;

        let caches = find_caches(&env, &[CacheKind::Go]);
        let result = clean_caches(&caches, RemovalStrategy::Permanent);

        assert_eq!(result.errors, Vec::<String>::new());
        assert!(!env.home.join("go/pkg/mod").exists());
        Ok(())
    }

    #[test]
    fn test_remove_cache_missing_dir_frees_nothing() -> anyhow::Result<()> {
        let cache = GlobalCache {
            kind: CacheKind::Npm,
            path: PathBuf::from("/nonexistent/cache/dir"),
            size: 42,
        };

        assert_eq!(remove_cache(&cache, RemovalStrategy::Permanent)?, 0);
        Ok(())
    }

    #[test]
    fn test_total_size_sums_caches() {
        let caches = [
            GlobalCache {
                kind: CacheKind::Npm,
                path: PathBuf::from("/a"),
                size: 10,
            },
            GlobalCache {
                kind: CacheKind::Go,
                path: PathBuf::from("/b"),
                size: 32,
            },
        ];

        assert_eq!(total_size(&caches), 42);
    }
}
