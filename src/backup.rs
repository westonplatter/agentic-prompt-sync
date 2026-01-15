use crate::error::{ApsError, Result};
use chrono::Local;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Directory for storing backups
pub const BACKUP_DIR: &str = ".aps-backups";

/// Create a backup of an existing file or directory
pub fn create_backup(base_dir: &Path, dest_path: &Path) -> Result<PathBuf> {
    let backup_root = base_dir.join(BACKUP_DIR);

    // Create backup directory if it doesn't exist
    if !backup_root.exists() {
        std::fs::create_dir_all(&backup_root)
            .map_err(|e| ApsError::io(e, format!("Failed to create backup directory at {:?}", backup_root)))?;
        debug!("Created backup directory at {:?}", backup_root);
    }

    // Generate timestamp-based backup name
    let timestamp = Local::now().format("%Y-%m-%d-%H%M").to_string();

    // Include parent path components to avoid collisions
    let relative_path = dest_path
        .strip_prefix(base_dir)
        .unwrap_or(dest_path)
        .to_string_lossy()
        .replace(['/', '\\'], "-");

    let backup_name = format!("{}-{}", relative_path, timestamp);
    let backup_path = backup_root.join(&backup_name);

    // Copy the content to backup location
    if dest_path.is_file() {
        std::fs::copy(dest_path, &backup_path)
            .map_err(|e| ApsError::io(e, format!("Failed to backup file {:?}", dest_path)))?;
        info!("Backed up file to {:?}", backup_path);
    } else if dest_path.is_dir() {
        copy_dir_recursive(dest_path, &backup_path)?;
        info!("Backed up directory to {:?}", backup_path);
    }

    Ok(backup_path)
}

/// Recursively copy a directory
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)
        .map_err(|e| ApsError::io(e, format!("Failed to create directory {:?}", dst)))?;

    for entry in std::fs::read_dir(src)
        .map_err(|e| ApsError::io(e, format!("Failed to read directory {:?}", src)))?
    {
        let entry = entry.map_err(|e| ApsError::io(e, "Failed to read directory entry"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| ApsError::io(e, format!("Failed to copy {:?}", src_path)))?;
        }
    }

    Ok(())
}

/// Check if a destination has a conflict
pub fn has_conflict(dest_path: &Path) -> bool {
    if !dest_path.exists() {
        return false;
    }

    if dest_path.is_file() {
        // File exists - conflict
        true
    } else if dest_path.is_dir() {
        // Directory exists and is non-empty - conflict (v0 simplification)
        match std::fs::read_dir(dest_path) {
            Ok(mut entries) => entries.next().is_some(),
            Err(_) => false,
        }
    } else {
        false
    }
}
