//! Path sanitization utilities for file transfer operations.

use std::path::{Component, Path, PathBuf};

use crate::TransferError;

/// Sanitize a filename to prevent directory traversal attacks.
///
/// This function:
/// - Strips normal directory components (e.g., `dir/file.txt` -> `file.txt`)
/// - Rejects paths with `..` components (directory traversal)
/// - Rejects absolute paths (paths with root components)
/// - Rejects empty filenames
/// - Rejects filenames with null bytes
/// - Rejects filenames that are just dots (., ..)
/// - Rejects hidden files (starting with .)
/// - Rejects filenames with control characters
///
/// # Errors
///
/// Returns `TransferError::PathInvalid` if the filename is invalid or potentially malicious.
pub fn sanitize_filename(filename: &str) -> Result<String, TransferError> {
    // Reject empty filenames
    if filename.is_empty() {
        return Err(TransferError::PathInvalid);
    }

    // Reject filenames with null bytes
    if filename.contains('\0') {
        return Err(TransferError::PathInvalid);
    }

    // Parse the path
    let path = Path::new(filename);

    // Check each component for directory traversal attempts
    for component in path.components() {
        match component {
            Component::CurDir | Component::ParentDir => {
                // Reject paths with . or .. components (directory traversal)
                return Err(TransferError::PathInvalid);
            }
            Component::Prefix(_) | Component::RootDir => {
                // Reject absolute paths
                return Err(TransferError::PathInvalid);
            }
            Component::Normal(_) => {
                // Allow normal components (will be stripped to just filename)
            }
        }
    }

    // Get the filename component (strips directory components)
    let file_name = path
        .file_name()
        .ok_or(TransferError::PathInvalid)?
        .to_str()
        .ok_or(TransferError::PathInvalid)?;

    // Reject if the filename is empty after parsing
    if file_name.is_empty() {
        return Err(TransferError::PathInvalid);
    }

    // Reject filenames that are just dots
    if file_name == "." || file_name == ".." {
        return Err(TransferError::PathInvalid);
    }

    // Reject filenames that start with a dot (hidden files on Unix)
    // This is a security measure to prevent overwriting .htaccess, .git, etc.
    if file_name.starts_with('.') {
        return Err(TransferError::PathInvalid);
    }

    // Reject filenames with control characters
    if file_name.chars().any(|c| c.is_control()) {
        return Err(TransferError::PathInvalid);
    }

    Ok(file_name.to_string())
}

/// Validate that a target path is within the allowed base directory.
///
/// This prevents directory traversal attacks where a malicious filename
/// might escape the intended upload directory.
///
/// Note: This function does not require the paths to exist on disk.
/// It validates based on path components alone.
///
/// # Errors
///
/// Returns `TransferError::PathInvalid` if the target path is outside the base directory.
pub fn validate_path_within_base(base_dir: &Path, target_path: &Path) -> Result<(), TransferError> {
    // Normalize both paths by resolving components without requiring them to exist
    let normalized_base = normalize_path(base_dir);
    let normalized_target = normalize_path(target_path);

    // Check that the target starts with the base directory
    if !normalized_target.starts_with(&normalized_base) {
        return Err(TransferError::PathInvalid);
    }

    Ok(())
}

/// Normalize a path by resolving `.` and `..` components without requiring the path to exist.
fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {
                // Skip `.` components
            }
            Component::ParentDir => {
                // Go up one directory for `..` components
                normalized.pop();
            }
            _ => {
                // Keep all other components (Normal, Prefix, RootDir)
                normalized.push(component);
            }
        }
    }

    normalized
}

/// Construct a safe target path by joining a base directory with a sanitized filename.
///
/// This is a convenience function that combines `sanitize_filename` and path joining.
///
/// # Errors
///
/// Returns `TransferError::PathInvalid` if the filename is invalid.
pub fn safe_upload_path(base_dir: &Path, filename: &str) -> Result<PathBuf, TransferError> {
    let sanitized = sanitize_filename(filename)?;
    let target = base_dir.join(&sanitized);

    // Validate that the constructed path is still within the base directory
    // This is a defense-in-depth measure
    validate_path_within_base(base_dir, &target)?;

    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn sanitize_accepts_normal_filename() {
        assert_eq!(sanitize_filename("document.txt").unwrap(), "document.txt");
        assert_eq!(sanitize_filename("image.png").unwrap(), "image.png");
        assert_eq!(sanitize_filename("archive.zip").unwrap(), "archive.zip");
    }

    #[test]
    fn sanitize_strips_directory_components() {
        assert_eq!(sanitize_filename("dir/file.txt").unwrap(), "file.txt");
        assert_eq!(sanitize_filename("a/b/c/file.txt").unwrap(), "file.txt");
    }

    #[test]
    fn sanitize_rejects_traversal_attempts() {
        assert!(sanitize_filename("/etc/passwd").is_err());
        assert!(sanitize_filename("../secret.txt").is_err());
    }

    #[test]
    fn sanitize_rejects_empty_filename() {
        assert!(sanitize_filename("").is_err());
    }

    #[test]
    fn sanitize_rejects_null_bytes() {
        assert!(sanitize_filename("file\0.txt").is_err());
        assert!(sanitize_filename("\0").is_err());
    }

    #[test]
    fn sanitize_rejects_dot_filenames() {
        assert!(sanitize_filename(".").is_err());
        assert!(sanitize_filename("..").is_err());
        assert!(sanitize_filename("./file.txt").is_err());
        assert!(sanitize_filename("../file.txt").is_err());
    }

    #[test]
    fn sanitize_rejects_hidden_files() {
        assert!(sanitize_filename(".htaccess").is_err());
        assert!(sanitize_filename(".git").is_err());
        assert!(sanitize_filename(".secret").is_err());
    }

    #[test]
    fn sanitize_rejects_control_characters() {
        assert!(sanitize_filename("file\x00.txt").is_err());
        assert!(sanitize_filename("file\x01.txt").is_err());
        assert!(sanitize_filename("file\n.txt").is_err());
    }

    #[test]
    fn validate_path_accepts_path_within_base() {
        let temp_dir = std::env::temp_dir().join("oxidebbs_test_validate");
        fs::create_dir_all(&temp_dir).unwrap();

        let target = temp_dir.join("file.txt");
        fs::write(&target, "test").unwrap();

        assert!(validate_path_within_base(&temp_dir, &target).is_ok());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn validate_path_rejects_path_outside_base() {
        let temp_dir = std::env::temp_dir().join("oxidebbs_test_validate2");
        fs::create_dir_all(&temp_dir).unwrap();

        let outside = temp_dir.parent().unwrap().join("outside.txt");
        fs::write(&outside, "test").unwrap();

        assert!(validate_path_within_base(&temp_dir, &outside).is_err());

        fs::remove_file(&outside).unwrap();
        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn safe_upload_path_constructs_valid_path() {
        let temp_dir = std::env::temp_dir().join("oxidebbs_test_safe_upload");
        fs::create_dir_all(&temp_dir).unwrap();

        let result = safe_upload_path(&temp_dir, "document.txt").unwrap();
        assert_eq!(result, temp_dir.join("document.txt"));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn safe_upload_path_rejects_traversal_attempts() {
        let temp_dir = std::env::temp_dir().join("oxidebbs_test_safe_upload2");
        fs::create_dir_all(&temp_dir).unwrap();

        // These should be rejected for security
        assert!(safe_upload_path(&temp_dir, "../secret.txt").is_err());
        assert!(safe_upload_path(&temp_dir, "/etc/passwd").is_err());

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
