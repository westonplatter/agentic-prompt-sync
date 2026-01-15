use crate::backup::{create_backup, has_conflict};
use crate::checksum::compute_source_checksum;
use crate::error::{ApsError, Result};
use crate::lockfile::{LockedEntry, Lockfile};
use crate::manifest::{AssetKind, Entry, Manifest, Source};
use dialoguer::Confirm;
use std::io::IsTerminal;
use std::path::Path;
use tracing::{debug, info};

/// Options for the install operation
pub struct InstallOptions {
    pub dry_run: bool,
    pub yes: bool,
}

/// Result of an install operation
pub struct InstallResult {
    pub id: String,
    pub installed: bool,
    pub skipped_no_change: bool,
    #[allow(dead_code)] // Used for reporting in future checkpoints
    pub backed_up: bool,
    pub locked_entry: Option<LockedEntry>,
}

/// Install all entries from a manifest
pub fn install_all(
    manifest: &Manifest,
    manifest_dir: &Path,
    lockfile: &Lockfile,
    options: &InstallOptions,
) -> Result<Vec<InstallResult>> {
    let mut results = Vec::new();

    for entry in &manifest.entries {
        let result = install_entry(entry, manifest_dir, lockfile, options)?;
        results.push(result);
    }

    Ok(results)
}

/// Install a single entry
pub fn install_entry(
    entry: &Entry,
    manifest_dir: &Path,
    lockfile: &Lockfile,
    options: &InstallOptions,
) -> Result<InstallResult> {
    info!("Processing entry: {}", entry.id);

    // Resolve source path
    let source_path = resolve_source_path(&entry.source, &entry.path, manifest_dir)?;
    debug!("Source path: {:?}", source_path);

    // Verify source exists
    if !source_path.exists() {
        return Err(ApsError::SourcePathNotFound { path: source_path });
    }

    // Compute checksum
    let checksum = compute_source_checksum(&source_path)?;
    debug!("Source checksum: {}", checksum);

    // Check if content is unchanged (no-op)
    if lockfile.checksum_matches(&entry.id, &checksum) {
        info!("Entry {} is up to date (checksum match)", entry.id);
        return Ok(InstallResult {
            id: entry.id.clone(),
            installed: false,
            skipped_no_change: true,
            backed_up: false,
            locked_entry: None,
        });
    }

    // Resolve destination path
    let dest_path = manifest_dir.join(entry.destination());
    debug!("Destination path: {:?}", dest_path);

    // Check for conflicts
    let mut backed_up = false;
    if has_conflict(&dest_path) {
        info!("Conflict detected at {:?}", dest_path);

        if options.dry_run {
            println!("[dry-run] Would backup and overwrite: {:?}", dest_path);
        } else {
            // Handle conflict
            let should_overwrite = if options.yes {
                true
            } else if std::io::stdin().is_terminal() {
                // Interactive prompt
                Confirm::new()
                    .with_prompt(format!(
                        "Overwrite existing content at {:?}?",
                        dest_path
                    ))
                    .default(false)
                    .interact()
                    .map_err(|_| ApsError::Cancelled)?
            } else {
                // Non-interactive without --yes
                return Err(ApsError::RequiresYesFlag);
            };

            if !should_overwrite {
                info!("User declined to overwrite {:?}", dest_path);
                return Err(ApsError::Cancelled);
            }

            // Create backup
            let backup_path = create_backup(manifest_dir, &dest_path)?;
            println!("Created backup at: {:?}", backup_path);
            backed_up = true;
        }
    }

    // Perform the install
    if options.dry_run {
        println!("[dry-run] Would install {} to {:?}", entry.id, dest_path);
    } else {
        install_asset(&entry.kind, &source_path, &dest_path)?;
        println!("Installed {} to {:?}", entry.id, dest_path);
    }

    // Create locked entry
    let locked_entry = LockedEntry::new_filesystem(
        &entry.source.display_name(),
        &dest_path.to_string_lossy(),
        checksum,
    );

    Ok(InstallResult {
        id: entry.id.clone(),
        installed: !options.dry_run,
        skipped_no_change: false,
        backed_up,
        locked_entry: Some(locked_entry),
    })
}

/// Resolve the source path based on source type
fn resolve_source_path(source: &Source, path: &str, manifest_dir: &Path) -> Result<std::path::PathBuf> {
    match source {
        Source::Filesystem { root } => {
            let root_path = if Path::new(root).is_absolute() {
                std::path::PathBuf::from(root)
            } else {
                manifest_dir.join(root)
            };
            Ok(root_path.join(path))
        }
        Source::Git { .. } => {
            // Git source not yet implemented (Checkpoint 9-10)
            todo!("Git source support not yet implemented")
        }
    }
}

/// Install an asset based on its kind
fn install_asset(kind: &AssetKind, source: &Path, dest: &Path) -> Result<()> {
    match kind {
        AssetKind::AgentsMd => {
            // Single file copy
            if let Some(parent) = dest.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent)
                        .map_err(|e| ApsError::io(e, "Failed to create destination directory"))?;
                }
            }
            std::fs::copy(source, dest)
                .map_err(|e| ApsError::io(e, format!("Failed to copy {:?} to {:?}", source, dest)))?;
            debug!("Copied file {:?} to {:?}", source, dest);
        }
        AssetKind::CursorRules | AssetKind::CursorSkillsRoot => {
            // Directory copy (Checkpoint 7-8)
            // For now, basic directory copy
            copy_directory(source, dest)?;
        }
    }
    Ok(())
}

/// Copy a directory recursively
fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_dir_all(dst)
            .map_err(|e| ApsError::io(e, format!("Failed to remove existing directory {:?}", dst)))?;
    }

    std::fs::create_dir_all(dst)
        .map_err(|e| ApsError::io(e, format!("Failed to create directory {:?}", dst)))?;

    for entry in std::fs::read_dir(src)
        .map_err(|e| ApsError::io(e, format!("Failed to read directory {:?}", src)))?
    {
        let entry = entry.map_err(|e| ApsError::io(e, "Failed to read directory entry"))?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)
                .map_err(|e| ApsError::io(e, format!("Failed to copy {:?}", src_path)))?;
        }
    }

    debug!("Copied directory {:?} to {:?}", src, dst);
    Ok(())
}
