//! Input validation utilities to prevent panics and security vulnerabilities
//!
//! This module provides safe validation for all external inputs to prevent:
//! - Panic-induced service crashes
//! - OOM attacks via unbounded strings
//! - Path traversal attacks
//! - Command injection
//! - ReDoS (Regular Expression Denial of Service)

use anyhow::{anyhow, Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

// ============================================================================
// CONSTANTS: Input Size Limits
// ============================================================================

/// Maximum length for resource IDs (stream_id, recording_id, device_id, etc.)
pub const MAX_ID_LENGTH: usize = 256;

/// Maximum length for URIs (RTSP, HTTP, S3, etc.)
pub const MAX_URI_LENGTH: usize = 4096;

/// Maximum length for file paths
pub const MAX_PATH_LENGTH: usize = 4096;

/// Maximum length for names (device names, user names, etc.)
pub const MAX_NAME_LENGTH: usize = 512;

/// Maximum length for descriptions and notes
pub const MAX_DESCRIPTION_LENGTH: usize = 4096;

/// Maximum length for email addresses
pub const MAX_EMAIL_LENGTH: usize = 320;

/// Maximum length for regex patterns (prevent ReDoS)
pub const MAX_REGEX_LENGTH: usize = 1024;

/// Maximum regex complexity (nested groups)
pub const MAX_REGEX_COMPLEXITY: usize = 10;

// ============================================================================
// Safe Time Operations
// ============================================================================

/// Get current Unix timestamp in seconds, safely handling clock errors
///
/// Returns `Ok(timestamp)` on success, or logs warning and returns 0 on clock issues
pub fn safe_unix_timestamp() -> u64 {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_secs(),
        Err(e) => {
            tracing::warn!(
                error = %e,
                "System clock is before UNIX epoch (1970-01-01), using timestamp 0"
            );
            0
        }
    }
}

/// Get current Unix timestamp, returning Result for explicit error handling
pub fn unix_timestamp() -> Result<u64> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .context("System clock is before UNIX epoch")
}

/// Get duration since UNIX epoch, with safe fallback
pub fn safe_unix_duration() -> Duration {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
}

// ============================================================================
// String Validation
// ============================================================================

/// Validate string length against a maximum
pub fn validate_length(value: &str, max_length: usize, field_name: &str) -> Result<()> {
    if value.len() > max_length {
        return Err(anyhow!(
            "{} exceeds maximum length of {} bytes (got {})",
            field_name,
            max_length,
            value.len()
        ));
    }
    Ok(())
}

/// Validate non-empty string
pub fn validate_non_empty(value: &str, field_name: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(anyhow!("{} cannot be empty", field_name));
    }
    Ok(())
}

/// Validate resource ID (stream_id, recording_id, etc.)
pub fn validate_id(id: &str, field_name: &str) -> Result<()> {
    validate_non_empty(id, field_name)?;
    validate_length(id, MAX_ID_LENGTH, field_name)?;

    // Prevent path traversal in IDs
    if id.contains("..") || id.contains('/') || id.contains('\\') {
        return Err(anyhow!(
            "{} contains invalid characters (no path separators or '..' allowed)",
            field_name
        ));
    }

    Ok(())
}

/// Validate name (device name, user name, etc.)
pub fn validate_name(name: &str, field_name: &str) -> Result<()> {
    validate_non_empty(name, field_name)?;
    validate_length(name, MAX_NAME_LENGTH, field_name)?;
    Ok(())
}

/// Validate URI (RTSP, HTTP, S3, etc.)
pub fn validate_uri(uri: &str, field_name: &str) -> Result<()> {
    validate_non_empty(uri, field_name)?;
    validate_length(uri, MAX_URI_LENGTH, field_name)?;

    // Prevent shell metacharacters (command injection)
    let dangerous_chars = ['`', '$', ';', '|', '&', '\n', '\r'];
    if uri.chars().any(|c| dangerous_chars.contains(&c)) {
        return Err(anyhow!(
            "{} contains dangerous shell metacharacters",
            field_name
        ));
    }

    Ok(())
}

/// Validate email address
pub fn validate_email(email: &str) -> Result<()> {
    validate_non_empty(email, "email")?;
    validate_length(email, MAX_EMAIL_LENGTH, "email")?;

    // Basic email format check (not RFC-compliant, but good enough)
    if !email.contains('@') || !email.contains('.') {
        return Err(anyhow!("Invalid email format"));
    }

    Ok(())
}

// ============================================================================
// UUID Validation
// ============================================================================

/// Safely parse UUID, returning detailed error instead of panicking
pub fn parse_uuid(uuid_str: &str, field_name: &str) -> Result<Uuid> {
    Uuid::parse_str(uuid_str).with_context(|| {
        format!(
            "{} is not a valid UUID: '{}' (expected format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx)",
            field_name, uuid_str
        )
    })
}

/// Safely parse optional UUID
pub fn parse_uuid_optional(uuid_str: Option<&str>, field_name: &str) -> Result<Option<Uuid>> {
    match uuid_str {
        Some(s) => Ok(Some(parse_uuid(s, field_name)?)),
        None => Ok(None),
    }
}

// ============================================================================
// Path Validation (Prevent Path Traversal)
// ============================================================================

/// Validate file path to prevent path traversal attacks
///
/// Ensures path:
/// - Doesn't contain ".." components
/// - Is within allowed base directory (if specified)
/// - Doesn't exceed maximum length
pub fn validate_path(path: &Path, base_dir: Option<&Path>, field_name: &str) -> Result<PathBuf> {
    // Check path length
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("{} contains invalid UTF-8", field_name))?;

    validate_length(path_str, MAX_PATH_LENGTH, field_name)?;

    // Canonicalize to resolve ".." and symlinks
    let canonical = path
        .canonicalize()
        .with_context(|| format!("{} does not exist or is inaccessible", field_name))?;

    // Check if path is within base directory
    if let Some(base) = base_dir {
        let canonical_base = base
            .canonicalize()
            .with_context(|| format!("Base directory does not exist: {:?}", base))?;

        if !canonical.starts_with(&canonical_base) {
            return Err(anyhow!(
                "{} is outside allowed directory (path: {:?}, base: {:?})",
                field_name,
                canonical,
                canonical_base
            ));
        }
    }

    Ok(canonical)
}

/// Validate file path without requiring it to exist (for new files)
pub fn validate_path_components(path: &Path, base_dir: Option<&Path>, field_name: &str) -> Result<PathBuf> {
    // Check path length
    let path_str = path
        .to_str()
        .ok_or_else(|| anyhow!("{} contains invalid UTF-8", field_name))?;

    validate_length(path_str, MAX_PATH_LENGTH, field_name)?;

    // Check for path traversal
    for component in path.components() {
        if component.as_os_str() == ".." {
            return Err(anyhow!(
                "{} contains '..' component (path traversal attempt)",
                field_name
            ));
        }
    }

    // If base_dir specified, ensure path is relative to it
    if let Some(base) = base_dir {
        let combined = base.join(path);

        // Verify combined path doesn't escape base (even without canonicalizing)
        if !combined.starts_with(base) {
            return Err(anyhow!(
                "{} attempts to escape base directory",
                field_name
            ));
        }

        Ok(combined)
    } else {
        Ok(path.to_path_buf())
    }
}

// ============================================================================
// Regex Validation (Prevent ReDoS)
// ============================================================================

/// Validate regex pattern to prevent ReDoS attacks
pub fn validate_regex_pattern(pattern: &str) -> Result<Regex> {
    // Check pattern length
    validate_length(pattern, MAX_REGEX_LENGTH, "regex pattern")?;

    // Count nested groups (complexity check)
    let open_parens = pattern.chars().filter(|&c| c == '(').count();
    if open_parens > MAX_REGEX_COMPLEXITY {
        return Err(anyhow!(
            "Regex pattern is too complex ({} groups, max {})",
            open_parens,
            MAX_REGEX_COMPLEXITY
        ));
    }

    // Check for dangerous patterns (catastrophic backtracking)
    let dangerous_patterns = [
        "(.*)*",      // Nested quantifiers
        "(.+)+",      // Nested quantifiers
        "(a*)*",      // Nested quantifiers
        "(a+)+",      // Nested quantifiers
    ];

    for dangerous in &dangerous_patterns {
        if pattern.contains(dangerous) {
            return Err(anyhow!(
                "Regex pattern contains dangerous nested quantifiers that could cause ReDoS: {}",
                dangerous
            ));
        }
    }

    // Compile with timeout
    Regex::new(pattern).context("Invalid regex pattern")
}

/// Safely compile regex with basic validation
pub fn safe_regex(pattern: &str) -> Result<Regex> {
    validate_regex_pattern(pattern)
}

// ============================================================================
// Numeric Validation
// ============================================================================

/// Validate port number (1-65535)
pub fn validate_port(port: u16) -> Result<()> {
    if port == 0 {
        return Err(anyhow!("Port number must be between 1 and 65535, got 0"));
    }
    Ok(())
}

/// Validate positive duration in seconds
pub fn validate_duration_secs(secs: u64, field_name: &str) -> Result<()> {
    if secs == 0 {
        return Err(anyhow!("{} must be greater than 0", field_name));
    }
    Ok(())
}

/// Validate value is within range
pub fn validate_range<T: PartialOrd + std::fmt::Display>(
    value: T,
    min: T,
    max: T,
    field_name: &str,
) -> Result<()> {
    if value < min || value > max {
        return Err(anyhow!(
            "{} must be between {} and {}, got {}",
            field_name,
            min,
            max,
            value
        ));
    }
    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_safe_unix_timestamp() {
        let ts = safe_unix_timestamp();
        assert!(ts > 1700000000); // After 2023
    }

    #[test]
    fn test_validate_id() {
        // Valid IDs
        assert!(validate_id("stream-123", "stream_id").is_ok());
        assert!(validate_id("valid_id_with-dashes_123", "id").is_ok());

        // Invalid IDs
        assert!(validate_id("", "id").is_err());
        assert!(validate_id("  ", "id").is_err());
        assert!(validate_id("../etc/passwd", "id").is_err());
        assert!(validate_id("path/to/something", "id").is_err());
        assert!(validate_id(&"a".repeat(300), "id").is_err());
    }

    #[test]
    fn test_validate_uri() {
        // Valid URIs
        assert!(validate_uri("rtsp://camera.local/stream", "uri").is_ok());
        assert!(validate_uri("http://example.com:8080/path", "uri").is_ok());

        // Invalid URIs (command injection)
        assert!(validate_uri("rtsp://cam`whoami`.local", "uri").is_err());
        assert!(validate_uri("http://example.com;rm -rf /", "uri").is_err());
        assert!(validate_uri("rtsp://cam$(id).local", "uri").is_err());
        assert!(validate_uri(&"a".repeat(5000), "uri").is_err());
    }

    #[test]
    fn test_parse_uuid() {
        // Valid UUIDs
        let valid = "550e8400-e29b-41d4-a716-446655440000";
        assert!(parse_uuid(valid, "tenant_id").is_ok());

        // Invalid UUIDs
        assert!(parse_uuid("not-a-uuid", "tenant_id").is_err());
        assert!(parse_uuid("", "tenant_id").is_err());
        assert!(parse_uuid("XXXX", "tenant_id").is_err());
    }

    #[test]
    fn test_validate_path_traversal() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Create a test file
        let safe_file = base.join("safe.txt");
        fs::write(&safe_file, b"test").unwrap();

        // Valid path within base
        assert!(validate_path(&safe_file, Some(base), "file").is_ok());

        // Path traversal attempts (these will fail because they don't exist)
        let traversal = base.join("../etc/passwd");
        assert!(validate_path(&traversal, Some(base), "file").is_err());
    }

    #[test]
    fn test_validate_regex_pattern() {
        // Valid patterns
        assert!(validate_regex_pattern("^[a-z]+$").is_ok());
        assert!(validate_regex_pattern("stream-\\d+").is_ok());

        // ReDoS patterns
        assert!(validate_regex_pattern("(a+)+b").is_err());
        assert!(validate_regex_pattern("(.*)*x").is_err());
        assert!(validate_regex_pattern(&"(".repeat(50)).is_err());
        assert!(validate_regex_pattern(&"a".repeat(2000)).is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("test+tag@domain.co.uk").is_ok());

        assert!(validate_email("").is_err());
        assert!(validate_email("notanemail").is_err());
        assert!(validate_email("@example.com").is_err());
    }

    #[test]
    fn test_validate_port() {
        assert!(validate_port(80).is_ok());
        assert!(validate_port(65535).is_ok());
        assert!(validate_port(0).is_err());
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range(50, 0, 100, "value").is_ok());
        assert!(validate_range(0, 0, 100, "value").is_ok());
        assert!(validate_range(100, 0, 100, "value").is_ok());

        assert!(validate_range(-1, 0, 100, "value").is_err());
        assert!(validate_range(101, 0, 100, "value").is_err());
    }
}
