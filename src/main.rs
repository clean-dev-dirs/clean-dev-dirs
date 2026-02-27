//! # clean-dev-dirs
//!
//! A fast and efficient CLI tool for recursively cleaning Rust `target/` and Node.js `node_modules/`
//! directories to reclaim disk space.
//!
//! This tool scans directories to find development projects and their build artifacts, allowing
//! you to selectively or automatically clean them to free up disk space. It supports parallel
//! scanning, intelligent filtering, and interactive selection.
//!
//! ## Features
//!
//! - Multi-language support (Rust, Node.js, Python, Go, Java/Kotlin, C/C++, Swift, .NET/C#)
//! - Parallel directory scanning
//! - Size and time-based filtering
//! - Interactive project selection
//! - Dry-run mode for safe previewing
//! - Human-readable output with progress indicators
//! - Persistent configuration via `~/.config/clean-dev-dirs/config.toml`
//!
//! ## Usage
//!
//! ```bash
//! # Basic usage - clean current directory
//! clean-dev-dirs
//!
//! # Clean with filters
//! clean-dev-dirs --keep-size 100MB --keep-days 30
//!
//! # Interactive mode
//! clean-dev-dirs --interactive
//! ```

mod cli;

use anyhow::{Ok, Result, bail};
use clap::Parser;
use clean_dev_dirs::{
    cleaner::{Cleaner, RemovalStrategy},
    config::FileConfig,
    filtering::{filter_projects, sort_projects},
    output::JsonOutput,
    project::Projects,
    scanner::Scanner,
};
use cli::{Cli, Commands, ConfigCommand};
use colored::Colorize;
use humansize::{DECIMAL, format_size};
use inquire::Confirm;
use std::process::exit;

/// Entry point for the clean-dev-dirs application.
///
/// This function handles all errors gracefully by calling [`inner_main`] and printing
/// any errors to stderr before exiting with a non-zero status code.
fn main() {
    if let Err(err) = inner_main() {
        eprintln!("Error: {err}");

        exit(1);
    }
}

/// Main application logic that can return errors.
///
/// This function orchestrates the full pipeline: parse arguments, scan for
/// projects, filter/sort, and either dry-run, interactively select, or clean.
///
/// # Errors
///
/// Returns errors from thread-pool configuration, directory scanning,
/// project filtering, interactive selection, file-system operations, or
/// JSON serialization.
fn inner_main() -> Result<()> {
    let args = Cli::parse();

    if let Some(Commands::Config { command }) = &args.subcommand {
        return handle_config_command(command);
    }

    let json_mode = args.json();
    let file_config = load_config(json_mode);

    let dir = args.directory(&file_config);
    let project_filter = args.project_filter(&file_config);
    let execution_options = args.execution_options(&file_config);
    let scan_options = args.scan_options(&file_config);
    let filter_options = args.filter_options(&file_config);

    if json_mode && execution_options.interactive {
        bail!("--json and --interactive cannot be used together");
    }

    if scan_options.threads > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(scan_options.threads)
            .build_global()?;
    }

    let scanner = Scanner::new(scan_options, project_filter).with_quiet(json_mode);
    let projects = scanner.scan_directory(&dir);

    if !json_mode {
        println!("Found {} projects", projects.len());
    }

    if projects.is_empty() {
        return print_empty_result(json_mode, "✨ No development directories found!");
    }

    let sort_opts = args.sort_options(&file_config);
    let mut filtered_projects = filter_projects(projects, &filter_options)?;
    sort_projects(&mut filtered_projects, &sort_opts);

    if filtered_projects.is_empty() {
        return print_empty_result(json_mode, "✨ No directories match the specified criteria!");
    }

    let total_size: u64 = filtered_projects.iter().map(|p| p.build_arts.size).sum();
    let projects: Projects = filtered_projects.into();

    if !json_mode {
        println!("\n{}", "📊 Found projects:".bold());
        projects.print_summary(total_size);
    }

    let Some(keep_executables) = resolve_keep_executables(&projects, &execution_options)? else {
        return Ok(());
    };

    if execution_options.dry_run {
        return print_dry_run(&projects, json_mode);
    }

    run_cleanup(
        projects,
        keep_executables,
        json_mode,
        execution_options.use_trash,
    )
}

// ── Helper functions ────────────────────────────────────────────────────

// ── Config subcommand ────────────────────────────────────────────────

/// Default config file template written by `config init`.
const CONFIG_TEMPLATE: &str = r#"# clean-dev-dirs configuration
# All values shown are their defaults. Uncomment and change as needed.

# Default project type to scan (all, rust, node, python, go, java, cpp, swift, dotnet, ruby, elixir, deno)
# project_type = "all"

# Default directory to scan (defaults to current directory when not set)
# dir = "."

[filtering]
# Ignore projects whose build directory is smaller than this (e.g. "50MB", "1GiB")
# keep_size = "0"

# Ignore projects compiled within the last N days (0 = no age filter)
# keep_days = 0

# Sort output by: size, age, name, type
# sort = "size"

# Reverse the sort order
# reverse = false

[scanning]
# Number of threads to use for scanning (0 = all CPU cores)
# threads = 0

# Show access errors encountered during scanning
# verbose = false

# Directories to skip during scanning
# skip = []

# Directories to ignore entirely during scanning
# ignore = []

[execution]
# Copy compiled executables to <project>/bin/ before cleaning
# keep_executables = false

# Use interactive project selection
# interactive = false

# Preview what would be cleaned without deleting anything
# dry_run = false

# Move build dirs to system trash instead of permanently deleting (default: true)
# use_trash = true
"#;

/// Dispatch a `config` subcommand.
fn handle_config_command(cmd: &ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Path => match FileConfig::config_path() {
            Some(path) => println!("{}", path.display()),
            None => bail!("Could not determine the config directory on this platform"),
        },
        ConfigCommand::Show => show_config()?,
        ConfigCommand::Init => init_config()?,
    }
    Ok(())
}

/// Print the effective configuration (file values merged with defaults).
fn show_config() -> Result<()> {
    let path = FileConfig::config_path();

    let (file_exists, config) = match &path {
        Some(p) if p.exists() => (true, FileConfig::load()?),
        _ => (false, FileConfig::default()),
    };

    match &path {
        Some(p) if file_exists => println!("Config file: {} (found)", p.display()),
        Some(p) => println!(
            "Config file: {} (not found - showing defaults)",
            p.display()
        ),
        None => println!("Config file: (cannot determine path on this platform)"),
    }

    println!();
    println!("{}", format_config(&config));
    Ok(())
}

/// Format a [`FileConfig`] as a human-readable table, showing defaults for `None` fields.
fn format_config(config: &clean_dev_dirs::config::file::FileConfig) -> String {
    fn show_str(val: Option<&str>, default: &str) -> String {
        val.map_or_else(
            || format!("\"{default}\"  (default)"),
            |v| format!("\"{v}\""),
        )
    }
    fn show_bool(val: Option<bool>, default: bool) -> String {
        val.map_or_else(|| format!("{default}  (default)"), |v| v.to_string())
    }
    fn show_u32(val: Option<u32>, default: u32) -> String {
        val.map_or_else(|| format!("{default}  (default)"), |v| v.to_string())
    }
    fn show_usize(val: Option<usize>, default: &str) -> String {
        val.map_or_else(|| format!("{default}  (default)"), |v| v.to_string())
    }
    fn show_paths(val: Option<&[std::path::PathBuf]>) -> String {
        match val {
            Some(v) if !v.is_empty() => {
                let items: Vec<String> = v.iter().map(|p| format!("\"{}\"", p.display())).collect();
                format!("[{}]", items.join(", "))
            }
            _ => "[]  (default)".to_string(),
        }
    }

    let dir_str = config.dir.as_ref().map_or_else(
        || "\".\"  (default)".to_string(),
        |p| format!("\"{}\"", p.display()),
    );

    format!(
        "\
project_type  = {project_type}
dir           = {dir}

[filtering]
keep_size     = {keep_size}
keep_days     = {keep_days}
sort          = {sort}
reverse       = {reverse}

[scanning]
threads       = {threads}
verbose       = {verbose}
skip          = {skip}
ignore        = {ignore}

[execution]
keep_executables = {keep_executables}
interactive      = {interactive}
dry_run          = {dry_run}
use_trash        = {use_trash}",
        project_type = show_str(config.project_type.as_deref(), "all"),
        dir = dir_str,
        keep_size = show_str(config.filtering.keep_size.as_deref(), "0"),
        keep_days = show_u32(config.filtering.keep_days, 0),
        sort = config
            .filtering
            .sort
            .as_deref()
            .map_or_else(|| "(none)  (default)".to_string(), |v| format!("\"{v}\""),),
        reverse = show_bool(config.filtering.reverse, false),
        threads = show_usize(config.scanning.threads, "0 (all cores)"),
        verbose = show_bool(config.scanning.verbose, false),
        skip = show_paths(config.scanning.skip.as_deref()),
        ignore = show_paths(config.scanning.ignore.as_deref()),
        keep_executables = show_bool(config.execution.keep_executables, false),
        interactive = show_bool(config.execution.interactive, false),
        dry_run = show_bool(config.execution.dry_run, false),
        use_trash = show_bool(config.execution.use_trash, true),
    )
}

/// Write a default config template to the config file path if it does not exist yet.
fn init_config() -> Result<()> {
    let Some(path) = FileConfig::config_path() else {
        bail!("Could not determine the config directory on this platform");
    };

    if path.exists() {
        println!("Config file already exists at: {}", path.display());
        println!("Remove it first if you want to regenerate it.");
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            anyhow::anyhow!(
                "Failed to create config directory {}: {e}",
                parent.display()
            )
        })?;
    }

    std::fs::write(&path, CONFIG_TEMPLATE)
        .map_err(|e| anyhow::anyhow!("Failed to write config file {}: {e}", path.display()))?;

    println!("Config file written to: {}", path.display());
    Ok(())
}

/// Load the configuration file, falling back to defaults on failure.
fn load_config(json_mode: bool) -> FileConfig {
    match FileConfig::load() {
        std::result::Result::Ok(config) => config,
        Err(e) => {
            if !json_mode {
                eprintln!("{} {e}", "Warning: Failed to load config file:".yellow());
            }
            FileConfig::default()
        }
    }
}

/// Emit an empty-projects result in JSON or human-readable form.
fn print_empty_result(json_mode: bool, message: &str) -> Result<()> {
    if json_mode {
        let output = JsonOutput::from_projects_dry_run(&[]);
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!("{}", message.green());
    }
    Ok(())
}

/// Handle interactive project selection and the keep-executables prompt.
///
/// Returns `Ok(Some(keep))` to continue with the resolved flag, or
/// `Ok(None)` when the user selected zero projects (caller should exit).
fn resolve_keep_executables(
    projects: &Projects,
    opts: &clean_dev_dirs::ExecutionOptions,
) -> Result<Option<bool>> {
    let mut keep = opts.keep_executables;

    if opts.interactive {
        let selected = projects.interactive_selection()?;
        if selected.is_empty() {
            println!("{}", "✨ No projects selected for cleaning!".green());
            return Ok(None);
        }

        if !keep {
            keep = Confirm::new("Keep compiled executables before cleaning?")
                .with_default(false)
                .prompt()?;
        }
    }

    Ok(Some(keep))
}

/// Print dry-run results in JSON or human-readable format.
fn print_dry_run(projects: &Projects, json_mode: bool) -> Result<()> {
    if json_mode {
        let output = JsonOutput::from_projects_dry_run(projects.as_slice());
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        let size = projects.get_total_size();
        println!(
            "\n{} {}",
            "🧪 Dry run complete!".yellow(),
            format!("Would free up {}", format_size(size, DECIMAL)).bright_white()
        );
    }
    Ok(())
}

/// Perform the actual cleanup and print results.
fn run_cleanup(
    projects: Projects,
    keep_executables: bool,
    json_mode: bool,
    use_trash: bool,
) -> Result<()> {
    let removal_strategy = RemovalStrategy::from_use_trash(use_trash);
    let snapshot: Vec<_> = projects.as_slice().to_vec();
    let result = Cleaner::clean_projects(projects, keep_executables, json_mode, removal_strategy);

    if json_mode {
        let output = JsonOutput::from_projects_cleanup(&snapshot, &result);
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        Cleaner::print_summary(&result);
    }

    Ok(())
}
