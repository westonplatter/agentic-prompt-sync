use crate::error::{ApsError, Result};
use sha2::{Digest, Sha256};
use std::path::Path;
use walkdir::WalkDir;

/// Compute a deterministic SHA256 checksum for a file or directory
pub fn compute_checksum(path: &Path) -> Result<String> {
    let mut hasher = Sha256::new();

    if path.is_file() {
        let content = std::fs::read(path).map_err(|e| {
            ApsError::io(e, format!("Failed to read file for checksum: {:?}", path))
        })?;
        hasher.update(&content);
    } else if path.is_dir() {
        // Collect all file paths relative to the directory, sorted for determinism
        // Exclude .git directories since their contents vary between clones
        let mut files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                // Exclude .git directories
                !e.path().components().any(|c| c.as_os_str() == ".git")
            })
            .filter(|e| e.file_type().is_file())
            .map(|e| e.path().to_path_buf())
            .collect();

        files.sort();

        for file_path in files {
            // Hash the relative path
            let relative = file_path
                .strip_prefix(path)
                .unwrap_or(&file_path)
                .to_string_lossy();
            hasher.update(relative.as_bytes());
            hasher.update(b"\0"); // separator

            // Hash the file content
            let content = std::fs::read(&file_path).map_err(|e| {
                ApsError::io(
                    e,
                    format!("Failed to read file for checksum: {:?}", file_path),
                )
            })?;
            hasher.update(&content);
        }
    }

    let result = hasher.finalize();
    Ok(format!("sha256:{}", hex::encode(result)))
}

/// Compute checksum for source content (before copying)
pub fn compute_source_checksum(source_path: &Path) -> Result<String> {
    compute_checksum(source_path)
}

/// Compute checksum for string content (for composed files)
pub fn compute_string_checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    let result = hasher.finalize();
    format!("sha256:{}", hex::encode(result))
}
