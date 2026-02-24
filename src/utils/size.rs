//! Size parsing and manipulation utilities.
//!
//! This module provides functions for parsing human-readable size strings
//! (like "100MB" or "1.5GiB") into byte values, and for measuring directory
//! sizes on disk.

use std::path::Path;

use anyhow::Result;
use walkdir::WalkDir;

/// Calculate the total size of a directory and all its contents, in bytes.
///
/// Recursively traverses the directory tree using `walkdir` and sums the sizes
/// of all files found. Errors for individual entries (permission denied, broken
/// symlinks, etc.) are silently skipped so the function always returns a result.
///
/// Returns `0` if the path does not exist or cannot be traversed at the root level.
pub fn calculate_dir_size(path: &Path) -> u64 {
    let mut total = 0u64;

    for entry in WalkDir::new(path) {
        if let Ok(entry) = entry {
            if entry.file_type().is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total += metadata.len();
                }
            }
        }
    }

    total
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
    fn test_parse_size_zero() {
        assert_eq!(parse_size("0").unwrap(), 0);
    }

    #[test]
    fn test_parse_size_plain_bytes() {
        assert_eq!(parse_size("1000").unwrap(), 1000);
        assert_eq!(parse_size("12345").unwrap(), 12345);
        assert_eq!(parse_size("1").unwrap(), 1);
    }

    #[test]
    fn test_parse_size_decimal_units() {
        assert_eq!(parse_size("1KB").unwrap(), 1_000);
        assert_eq!(parse_size("100KB").unwrap(), 100_000);
        assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
        assert_eq!(parse_size("5MB").unwrap(), 5_000_000);
        assert_eq!(parse_size("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_size("2GB").unwrap(), 2_000_000_000);
    }

    #[test]
    fn test_parse_size_binary_units() {
        assert_eq!(parse_size("1KiB").unwrap(), 1_024);
        assert_eq!(parse_size("1MiB").unwrap(), 1_048_576);
        assert_eq!(parse_size("1GiB").unwrap(), 1_073_741_824);
        assert_eq!(parse_size("2KiB").unwrap(), 2_048);
        assert_eq!(parse_size("10MiB").unwrap(), 10_485_760);
    }

    #[test]
    fn test_parse_size_case_insensitive() {
        assert_eq!(parse_size("1kb").unwrap(), 1_000);
        assert_eq!(parse_size("1Kb").unwrap(), 1_000);
        assert_eq!(parse_size("1kB").unwrap(), 1_000);
        assert_eq!(parse_size("1mb").unwrap(), 1_000_000);
        assert_eq!(parse_size("1mib").unwrap(), 1_048_576);
        assert_eq!(parse_size("1gib").unwrap(), 1_073_741_824);
    }

    #[test]
    fn test_parse_size_decimal_values() {
        assert_eq!(parse_size("1.5KB").unwrap(), 1_500);
        assert_eq!(parse_size("2.5MB").unwrap(), 2_500_000);
        assert_eq!(parse_size("1.5MiB").unwrap(), 1_572_864); // 1.5 * 1048576
        assert_eq!(parse_size("0.5GB").unwrap(), 500_000_000);
        assert_eq!(parse_size("0.1KB").unwrap(), 100);
    }

    #[test]
    fn test_parse_size_complex_decimals() {
        assert_eq!(parse_size("1.25MB").unwrap(), 1_250_000);
        assert_eq!(parse_size("3.14159KB").unwrap(), 3_141); // Truncated due to precision
        assert_eq!(parse_size("2.75GiB").unwrap(), 2_952_790_016); // 2.75 * 1073741824
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
    fn test_parse_size_unit_order() {
        // Test that longer units are matched first (GiB before GB, MiB before MB, etc.)
        assert_eq!(parse_size("1GiB").unwrap(), 1_073_741_824);
        assert_eq!(parse_size("1GB").unwrap(), 1_000_000_000);
        assert_eq!(parse_size("1MiB").unwrap(), 1_048_576);
        assert_eq!(parse_size("1MB").unwrap(), 1_000_000);
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
    fn test_parse_fractional_part() {
        assert_eq!(parse_fractional_part("5").unwrap(), 500_000_000);
        assert_eq!(parse_fractional_part("25").unwrap(), 250_000_000);
        assert_eq!(parse_fractional_part("125").unwrap(), 125_000_000);
        assert_eq!(parse_fractional_part("999999999").unwrap(), 999_999_999);

        // Too many decimal places
        assert!(parse_fractional_part("1234567890").is_err());
    }

    #[test]
    fn test_multiply_with_overflow_check() {
        assert_eq!(multiply_with_overflow_check(100, 200).unwrap(), 20_000);
        assert_eq!(multiply_with_overflow_check(0, 999).unwrap(), 0);
        assert_eq!(multiply_with_overflow_check(1, 1).unwrap(), 1);

        // Test overflow
        assert!(multiply_with_overflow_check(u64::MAX, 2).is_err());
        assert!(multiply_with_overflow_check(u64::MAX / 2 + 1, 2).is_err());
    }

    #[test]
    fn test_add_with_overflow_check() {
        assert_eq!(add_with_overflow_check(100, 200).unwrap(), 300);
        assert_eq!(add_with_overflow_check(0, 999).unwrap(), 999);
        assert_eq!(add_with_overflow_check(u64::MAX - 1, 1).unwrap(), u64::MAX);

        // Test overflow
        assert!(add_with_overflow_check(u64::MAX, 1).is_err());
        assert!(add_with_overflow_check(u64::MAX - 1, 2).is_err());
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
    fn test_parse_decimal_size() {
        assert_eq!(parse_decimal_size("1.5", 1_000_000).unwrap(), 1_500_000);
        assert_eq!(parse_decimal_size("2.25", 1_000).unwrap(), 2_250);
        assert_eq!(
            parse_decimal_size("0.5", 2_000_000_000).unwrap(),
            1_000_000_000
        );

        // Invalid formats
        assert!(parse_decimal_size("1.2.3", 1000).is_err());
        assert!(parse_decimal_size("invalid", 1000).is_err());
    }

    #[test]
    fn test_parse_integer_size() {
        assert_eq!(parse_integer_size("100", 1_000).unwrap(), 100_000);
        assert_eq!(parse_integer_size("0", 999).unwrap(), 0);
        assert_eq!(
            parse_integer_size("1", 1_000_000_000).unwrap(),
            1_000_000_000
        );

        // Invalid format
        assert!(parse_integer_size("not_a_number", 1000).is_err());
    }

    #[test]
    fn test_edge_cases() {
        // Very small decimal
        assert_eq!(parse_size("0.001KB").unwrap(), 1);

        // Very large valid number
        let large_but_valid = (u64::MAX / 1_000_000_000).to_string() + "GB";
        assert!(parse_size(&large_but_valid).is_ok());

        // Zero with units
        assert_eq!(parse_size("0KB").unwrap(), 0);
        assert_eq!(parse_size("0.0MB").unwrap(), 0);
    }
}
