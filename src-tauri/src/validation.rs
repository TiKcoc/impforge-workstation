// SPDX-License-Identifier: Apache-2.0
//! Input Validation Helpers
//!
//! Reusable validation functions for Tauri command parameters.
//! All validators return `Result<(), String>` for direct use in
//! commands that return `Result<T, String>`.
//!
//! Enterprise Bestimmung: Prevent invalid data from reaching backend
//! logic, reducing panics and providing clear user-facing error messages.

/// Validate that a string field is not empty or whitespace-only.
pub fn validate_not_empty(value: &str, field_name: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{} cannot be empty", field_name));
    }
    Ok(())
}

/// Basic email format check (contains `@` and `.` after `@`).
/// This is intentionally lenient -- full RFC 5322 validation is overkill
/// for a desktop app.  The real validation happens when the mail server
/// accepts or rejects the address.
pub fn validate_email(email: &str) -> Result<(), String> {
    let trimmed = email.trim();
    if trimmed.is_empty() {
        return Err("Email address cannot be empty".into());
    }
    let at_pos = trimmed.find('@');
    match at_pos {
        Some(pos) if pos > 0 => {
            let domain = &trimmed[pos + 1..];
            if domain.contains('.') && domain.len() >= 3 {
                Ok(())
            } else {
                Err("Invalid email address: domain must contain a dot".into())
            }
        }
        _ => Err("Invalid email address: missing '@'".into()),
    }
}

/// Validate that a string looks like an HTTP(S) URL.
pub fn validate_url(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
        return Err("URL must start with http:// or https://".into());
    }
    // Minimal length: "http://x" = 8 chars
    if trimmed.len() < 8 {
        return Err("URL is too short to be valid".into());
    }
    Ok(())
}

/// Validate that a string does not exceed a maximum byte length.
pub fn validate_max_length(value: &str, max: usize, field_name: &str) -> Result<(), String> {
    if value.len() > max {
        return Err(format!(
            "{} exceeds maximum length of {} (got {})",
            field_name,
            max,
            value.len()
        ));
    }
    Ok(())
}

/// Sanitize a string for use as a filename.
/// Removes any character that is not alphanumeric, `-`, `_`, or `.`.
/// Also collapses consecutive dots and strips leading dots to prevent
/// path traversal (`../`) and hidden files (`.hidden`).
pub fn sanitize_filename(name: &str) -> String {
    let filtered: String = name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
        .collect();
    // Collapse runs of dots into a single dot, then strip leading dots
    let mut result = String::with_capacity(filtered.len());
    let mut prev_dot = false;
    for c in filtered.chars() {
        if c == '.' {
            if !prev_dot {
                result.push('.');
                prev_dot = true;
            }
            // Skip consecutive dots
        } else {
            prev_dot = false;
            result.push(c);
        }
    }
    // Strip leading dots to prevent hidden files / traversal
    result.trim_start_matches('.').to_string()
}

/// Validate that a numeric value falls within an inclusive range.
pub fn validate_range(value: f64, min: f64, max: f64, field_name: &str) -> Result<(), String> {
    if value < min || value > max {
        return Err(format!(
            "{} must be between {} and {} (got {})",
            field_name, min, max, value
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_not_empty() {
        assert!(validate_not_empty("hello", "name").is_ok());
        assert!(validate_not_empty("", "name").is_err());
        assert!(validate_not_empty("   ", "name").is_err());
    }

    #[test]
    fn test_validate_email() {
        assert!(validate_email("user@example.com").is_ok());
        assert!(validate_email("a@b.co").is_ok());
        assert!(validate_email("invalid").is_err());
        assert!(validate_email("@missing.com").is_err());
        assert!(validate_email("user@").is_err());
        assert!(validate_email("").is_err());
    }

    #[test]
    fn test_validate_url() {
        assert!(validate_url("https://example.com").is_ok());
        assert!(validate_url("http://localhost:8080").is_ok());
        assert!(validate_url("ftp://bad.com").is_err());
        assert!(validate_url("").is_err());
        assert!(validate_url("http://").is_err());
    }

    #[test]
    fn test_validate_max_length() {
        assert!(validate_max_length("short", 100, "field").is_ok());
        assert!(validate_max_length("ab", 2, "field").is_ok());
        assert!(validate_max_length("abc", 2, "field").is_err());
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("hello world!.txt"), "helloworld.txt");
        assert_eq!(sanitize_filename("my-file_v2.rs"), "my-file_v2.rs");
        assert_eq!(sanitize_filename("../../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_filename(""), "");
    }

    #[test]
    fn test_validate_range() {
        assert!(validate_range(5.0, 0.0, 10.0, "val").is_ok());
        assert!(validate_range(0.0, 0.0, 10.0, "val").is_ok());
        assert!(validate_range(10.0, 0.0, 10.0, "val").is_ok());
        assert!(validate_range(-1.0, 0.0, 10.0, "val").is_err());
        assert!(validate_range(11.0, 0.0, 10.0, "val").is_err());
    }
}
