//! Size parsing and manipulation utilities.
//!
//! This module provides functions for parsing human-readable size strings
//! (like "100MB" or "1.5GiB") into byte values, and for measuring directory
//! sizes on disk.

use std::{collections::HashMap, fs::Metadata, path::Path};

use anyhow::Result;
use walkdir::WalkDir;

/// A file with more than one hard link, seen during a directory walk.
#[derive(Debug)]
struct LinkedFile {
    /// Number of links to this file found inside the walked directory.
    links_seen: u64,
    /// Total number of links to this file on the file system.
    links_total: u64,
    /// Disk space that this file uses, in bytes.
    disk_usage: u64,
}

/// Calculate the disk space that deleting a directory would free, in bytes.
///
/// Recursively traverses the directory tree using `walkdir` and sums the
/// disk usage of all files found. The result matches the space that a
/// deletion gives back, not the apparent size of the files:
///
/// - On Unix, the size of a file is the number of allocated blocks
///   (`st_blocks * 512`), like `du`. Sparse files count only the blocks
///   they use, and small files count a full block.
/// - A file with several hard links inside the directory counts one time.
/// - A file with a hard link outside the directory counts zero times,
///   because deleting the directory does not free its data. This is the
///   case for a `node_modules` directory that `pnpm` fills from its store.
///
/// On other platforms, the function uses the apparent size of each file.
///
/// Symbolic links are not followed. Errors for individual entries
/// (permission denied, broken symlinks, etc.) are silently skipped so the
/// function always returns a result.
///
/// Returns `0` if the path does not exist or cannot be traversed at the root level.
#[must_use]
pub fn calculate_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut linked_files: HashMap<(u64, u64), LinkedFile> = HashMap::new();

    for entry in WalkDir::new(path).into_iter().flatten() {
        if !entry.file_type().is_file() {
            continue;
        }

        let Ok(metadata) = entry.metadata() else {
            continue;
        };

        match hard_link_key(&metadata) {
            Some((key, links_total)) => {
                linked_files
                    .entry(key)
                    .or_insert_with(|| LinkedFile {
                        links_seen: 0,
                        links_total,
                        disk_usage: disk_usage(&metadata),
                    })
                    .links_seen += 1;
            }
            None => total = total.saturating_add(disk_usage(&metadata)),
        }
    }

    linked_files
        .values()
        .filter(|file| file.links_seen >= file.links_total)
        .fold(total, |acc, file| acc.saturating_add(file.disk_usage))
}

/// Return the disk space that a file uses, in bytes.
#[cfg(unix)]
fn disk_usage(metadata: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;

    // `st_blocks` is always in units of 512 bytes, whatever the block size
    // of the file system.
    metadata.blocks().saturating_mul(512)
}

/// Return the disk space that a file uses, in bytes.
#[cfg(not(unix))]
fn disk_usage(metadata: &Metadata) -> u64 {
    metadata.len()
}

/// Return the `(device, inode)` key and the link count of a file that has
/// more than one hard link, or `None` for a file with a single link.
#[cfg(unix)]
fn hard_link_key(metadata: &Metadata) -> Option<((u64, u64), u64)> {
    use std::os::unix::fs::MetadataExt;

    (metadata.nlink() > 1).then(|| ((metadata.dev(), metadata.ino()), metadata.nlink()))
}

/// Hard links are not detected on this platform: every file counts one time.
#[cfg(not(unix))]
const fn hard_link_key(_metadata: &Metadata) -> Option<((u64, u64), u64)> {
    None
}

/// Parse a human-readable size string into bytes.
///
/// Supports both decimal (KB, MB, GB) and binary (KiB, MiB, GiB) units,
/// as well as decimal numbers (e.g., "1.5GB").
///
/// # Arguments
///
/// * `size_str` - A string representing the size (e.g., "100MB", "1.5GiB", "1,000,000")
///
/// # Returns
///
/// - `Ok(u64)` - The size in bytes
/// - `Err(anyhow::Error)` - If the string format is invalid or causes overflow
///
/// # Errors
///
/// This function will return an error if:
/// - The size string format is invalid (e.g., "1.2.3MB", "invalid")
/// - The number cannot be parsed as a valid integer or decimal
/// - The resulting value would overflow `u64`
/// - The decimal has too many fractional digits (more than 9)
///
/// # Examples
///
/// ```
/// # use clean_dev_dirs::utils::parse_size;
/// # use anyhow::Result;
/// # fn main() -> Result<()> {
/// assert_eq!(parse_size("100KB")?, 100_000);
/// assert_eq!(parse_size("1.5MB")?, 1_500_000);
/// assert_eq!(parse_size("1GiB")?, 1_073_741_824);
/// # Ok(())
/// # }
/// ```
///
/// # Supported Units
///
/// - **Decimal**: KB (1000), MB (1000²), GB (1000³)
/// - **Binary**: KiB (1024), MiB (1024²), GiB (1024³)
/// - **Bytes**: Plain numbers without units
pub fn parse_size(size_str: &str) -> Result<u64> {
    if size_str == "0" {
        return Ok(0);
    }

    let size_str = size_str.to_uppercase();
    let (number_str, multiplier) = parse_size_unit(&size_str);

    if number_str.contains('.') {
        parse_decimal_size(number_str, multiplier)
    } else {
        parse_integer_size(number_str, multiplier)
    }
}

/// Parse the unit suffix and return the numeric part with its multiplier.
fn parse_size_unit(size_str: &str) -> (&str, u64) {
    const UNITS: &[(&str, u64)] = &[
        ("GIB", 1_073_741_824),
        ("MIB", 1_048_576),
        ("KIB", 1_024),
        ("GB", 1_000_000_000),
        ("MB", 1_000_000),
        ("KB", 1_000),
    ];

    for (suffix, multiplier) in UNITS {
        if size_str.ends_with(suffix) {
            return (size_str.trim_end_matches(suffix), *multiplier);
        }
    }

    (size_str, 1)
}

/// Parse a decimal size value (e.g., "1.5").
fn parse_decimal_size(number_str: &str, multiplier: u64) -> Result<u64> {
    let parts: Vec<&str> = number_str.split('.').collect();
    if parts.len() != 2 {
        return Err(anyhow::anyhow!("Invalid decimal format: {number_str}"));
    }

    let integer_part: u64 = parts[0].parse().unwrap_or(0);
    let fractional_result = parse_fractional_part(parts[1])?;

    let integer_bytes = multiply_with_overflow_check(integer_part, multiplier)?;
    let fractional_bytes =
        multiply_with_overflow_check(fractional_result, multiplier)? / 1_000_000_000;

    add_with_overflow_check(integer_bytes, fractional_bytes)
}

/// Parse the fractional part of a decimal number.
fn parse_fractional_part(fractional_str: &str) -> Result<u64> {
    let fractional_digits = fractional_str.len();
    if fractional_digits > 9 {
        return Err(anyhow::anyhow!("Too many decimal places: {fractional_str}"));
    }

    let fractional_part: u64 = fractional_str.parse()?;
    let fractional_multiplier = 10u64.pow(9 - u32::try_from(fractional_digits)?);

    Ok(fractional_part * fractional_multiplier)
}

/// Parse an integer size value.
fn parse_integer_size(number_str: &str, multiplier: u64) -> Result<u64> {
    let number: u64 = number_str.parse()?;
    multiply_with_overflow_check(number, multiplier)
}

/// Multiply two values with overflow checking.
fn multiply_with_overflow_check(a: u64, b: u64) -> Result<u64> {
    a.checked_mul(b)
        .ok_or_else(|| anyhow::anyhow!("Size value overflow: {a} * {b}"))
}

/// Add two values with overflow checking.
fn add_with_overflow_check(a: u64, b: u64) -> Result<u64> {
    a.checked_add(b)
        .ok_or_else(|| anyhow::anyhow!("Final overflow: {a} + {b}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size_zero() -> anyhow::Result<()> {
        assert_eq!(parse_size("0")?, 0);
        Ok(())
    }

    #[test]
    fn test_parse_size_plain_bytes() -> anyhow::Result<()> {
        assert_eq!(parse_size("1000")?, 1000);
        assert_eq!(parse_size("12345")?, 12345);
        assert_eq!(parse_size("1")?, 1);
        Ok(())
    }

    #[test]
    fn test_parse_size_decimal_units() -> anyhow::Result<()> {
        assert_eq!(parse_size("1KB")?, 1_000);
        assert_eq!(parse_size("100KB")?, 100_000);
        assert_eq!(parse_size("1MB")?, 1_000_000);
        assert_eq!(parse_size("5MB")?, 5_000_000);
        assert_eq!(parse_size("1GB")?, 1_000_000_000);
        assert_eq!(parse_size("2GB")?, 2_000_000_000);
        Ok(())
    }

    #[test]
    fn test_parse_size_binary_units() -> anyhow::Result<()> {
        assert_eq!(parse_size("1KiB")?, 1_024);
        assert_eq!(parse_size("1MiB")?, 1_048_576);
        assert_eq!(parse_size("1GiB")?, 1_073_741_824);
        assert_eq!(parse_size("2KiB")?, 2_048);
        assert_eq!(parse_size("10MiB")?, 10_485_760);
        Ok(())
    }

    #[test]
    fn test_parse_size_case_insensitive() -> anyhow::Result<()> {
        assert_eq!(parse_size("1kb")?, 1_000);
        assert_eq!(parse_size("1Kb")?, 1_000);
        assert_eq!(parse_size("1kB")?, 1_000);
        assert_eq!(parse_size("1mb")?, 1_000_000);
        assert_eq!(parse_size("1mib")?, 1_048_576);
        assert_eq!(parse_size("1gib")?, 1_073_741_824);
        Ok(())
    }

    #[test]
    fn test_parse_size_decimal_values() -> anyhow::Result<()> {
        assert_eq!(parse_size("1.5KB")?, 1_500);
        assert_eq!(parse_size("2.5MB")?, 2_500_000);
        assert_eq!(parse_size("1.5MiB")?, 1_572_864); // 1.5 * 1048576
        assert_eq!(parse_size("0.5GB")?, 500_000_000);
        assert_eq!(parse_size("0.1KB")?, 100);
        Ok(())
    }

    #[test]
    fn test_parse_size_complex_decimals() -> anyhow::Result<()> {
        assert_eq!(parse_size("1.25MB")?, 1_250_000);
        assert_eq!(parse_size("3.14159KB")?, 3_141); // Truncated due to precision
        assert_eq!(parse_size("2.75GiB")?, 2_952_790_016); // 2.75 * 1073741824
        Ok(())
    }

    #[test]
    fn test_parse_size_invalid_formats() {
        assert!(parse_size("").is_err());
        assert!(parse_size("invalid").is_err());
        assert!(parse_size("1.2.3MB").is_err());
        assert!(parse_size("MB1").is_err());
        assert!(parse_size("1XB").is_err());
        assert!(parse_size("-1MB").is_err());
    }

    #[test]
    fn test_parse_size_unit_order() -> anyhow::Result<()> {
        assert_eq!(parse_size("1GiB")?, 1_073_741_824);
        assert_eq!(parse_size("1GB")?, 1_000_000_000);
        assert_eq!(parse_size("1MiB")?, 1_048_576);
        assert_eq!(parse_size("1MB")?, 1_000_000);
        Ok(())
    }

    #[test]
    fn test_parse_size_overflow() {
        // Test with values that would cause overflow
        let max_u64_str = format!("{}", u64::MAX);
        let too_large = format!("{}GB", u64::MAX / 1000 + 1);

        assert!(parse_size(&max_u64_str).is_ok());
        assert!(parse_size(&too_large).is_err());
        assert!(parse_size("999999999999999999999999GB").is_err());
    }

    #[test]
    fn test_parse_fractional_part() -> anyhow::Result<()> {
        assert_eq!(parse_fractional_part("5")?, 500_000_000);
        assert_eq!(parse_fractional_part("25")?, 250_000_000);
        assert_eq!(parse_fractional_part("125")?, 125_000_000);
        assert_eq!(parse_fractional_part("999999999")?, 999_999_999);

        // Too many decimal places
        assert!(parse_fractional_part("1234567890").is_err());
        Ok(())
    }

    #[test]
    fn test_multiply_with_overflow_check() -> anyhow::Result<()> {
        assert_eq!(multiply_with_overflow_check(100, 200)?, 20_000);
        assert_eq!(multiply_with_overflow_check(0, 999)?, 0);
        assert_eq!(multiply_with_overflow_check(1, 1)?, 1);

        // Test overflow
        assert!(multiply_with_overflow_check(u64::MAX, 2).is_err());
        assert!(multiply_with_overflow_check(u64::MAX / 2 + 1, 2).is_err());
        Ok(())
    }

    #[test]
    fn test_add_with_overflow_check() -> anyhow::Result<()> {
        assert_eq!(add_with_overflow_check(100, 200)?, 300);
        assert_eq!(add_with_overflow_check(0, 999)?, 999);
        assert_eq!(add_with_overflow_check(u64::MAX - 1, 1)?, u64::MAX);

        // Test overflow
        assert!(add_with_overflow_check(u64::MAX, 1).is_err());
        assert!(add_with_overflow_check(u64::MAX - 1, 2).is_err());
        Ok(())
    }

    #[test]
    fn test_parse_size_unit() {
        assert_eq!(parse_size_unit("100GB"), ("100", 1_000_000_000));
        assert_eq!(parse_size_unit("50MIB"), ("50", 1_048_576));
        assert_eq!(parse_size_unit("1024"), ("1024", 1));
        assert_eq!(parse_size_unit("2.5KB"), ("2.5", 1_000));
        assert_eq!(parse_size_unit("1.5GIB"), ("1.5", 1_073_741_824));
    }

    #[test]
    fn test_parse_decimal_size() -> anyhow::Result<()> {
        assert_eq!(parse_decimal_size("1.5", 1_000_000)?, 1_500_000);
        assert_eq!(parse_decimal_size("2.25", 1_000)?, 2_250);
        assert_eq!(parse_decimal_size("0.5", 2_000_000_000)?, 1_000_000_000);

        // Invalid formats
        assert!(parse_decimal_size("1.2.3", 1000).is_err());
        assert!(parse_decimal_size("invalid", 1000).is_err());
        Ok(())
    }

    #[test]
    fn test_parse_integer_size() -> anyhow::Result<()> {
        assert_eq!(parse_integer_size("100", 1_000)?, 100_000);
        assert_eq!(parse_integer_size("0", 999)?, 0);
        assert_eq!(parse_integer_size("1", 1_000_000_000)?, 1_000_000_000);

        // Invalid format
        assert!(parse_integer_size("not_a_number", 1000).is_err());
        Ok(())
    }

    #[test]
    fn test_edge_cases() -> anyhow::Result<()> {
        // Very small decimal
        assert_eq!(parse_size("0.001KB")?, 1);

        // Very large valid number
        let large_but_valid = (u64::MAX / 1_000_000_000).to_string() + "GB";
        assert!(parse_size(&large_but_valid).is_ok());

        // Zero with units
        assert_eq!(parse_size("0KB")?, 0);
        assert_eq!(parse_size("0.0MB")?, 0);
        Ok(())
    }

    // ── calculate_dir_size ──────────────────────────────────────────────

    /// Write `len` bytes of real data to `path`.
    fn write_file(path: &Path, len: usize) -> anyhow::Result<()> {
        std::fs::write(path, vec![b'x'; len])?;
        Ok(())
    }

    #[test]
    fn test_dir_size_nonexistent_is_zero() {
        assert_eq!(calculate_dir_size(Path::new("/nonexistent/path")), 0);
    }

    #[test]
    fn test_dir_size_covers_file_contents() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        write_file(&tmp.path().join("a.bin"), 100_000)?;

        assert!(calculate_dir_size(tmp.path()) >= 100_000);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_dir_size_counts_blocks_on_unix() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        write_file(&tmp.path().join("tiny.txt"), 1)?;

        let size = calculate_dir_size(tmp.path());
        assert!(
            size >= 512,
            "a 1-byte file uses at least one block, got {size}"
        );
        assert!(size.is_multiple_of(512));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_dir_size_hard_links_inside_count_once() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let original = tmp.path().join("original.bin");
        write_file(&original, 100_000)?;
        let single = calculate_dir_size(tmp.path());

        std::fs::hard_link(&original, tmp.path().join("link-1.bin"))?;
        std::fs::hard_link(&original, tmp.path().join("link-2.bin"))?;

        assert_eq!(calculate_dir_size(tmp.path()), single);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_dir_size_hard_link_to_outside_is_not_reclaimable() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let store = tmp.path().join("store");
        let modules = tmp.path().join("node_modules");
        std::fs::create_dir_all(&store)?;
        std::fs::create_dir_all(&modules)?;

        let stored = store.join("package.bin");
        write_file(&stored, 100_000)?;
        std::fs::hard_link(&stored, modules.join("package.bin"))?;
        write_file(&modules.join("own.bin"), 100_000)?;

        let own_only = {
            let check = tempfile::TempDir::new()?;
            write_file(&check.path().join("own.bin"), 100_000)?;
            calculate_dir_size(check.path())
        };

        // Deleting node_modules frees only the file that it alone owns.
        assert_eq!(calculate_dir_size(&modules), own_only);
        // The parent holds every link, so the shared file counts one time.
        assert_eq!(calculate_dir_size(tmp.path()), own_only * 2);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_dir_size_sparse_file_counts_allocated_blocks() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let sparse = std::fs::File::create(tmp.path().join("sparse.img"))?;
        sparse.set_len(64 * 1024 * 1024)?;

        assert!(calculate_dir_size(tmp.path()) < 64 * 1024 * 1024);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn test_dir_size_does_not_follow_symlinks() -> anyhow::Result<()> {
        let tmp = tempfile::TempDir::new()?;
        let outside = tmp.path().join("outside");
        let inside = tmp.path().join("inside");
        std::fs::create_dir_all(&outside)?;
        std::fs::create_dir_all(&inside)?;
        write_file(&outside.join("big.bin"), 100_000)?;
        std::os::unix::fs::symlink(&outside, inside.join("link"))?;

        assert_eq!(calculate_dir_size(&inside), 0);
        Ok(())
    }
}
