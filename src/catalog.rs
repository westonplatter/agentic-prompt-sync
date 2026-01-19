//! Catalog module for generating asset catalogs from manifest entries.
//!
//! The catalog provides a mechanical listing of all individual assets
//! that are synced via the manifest. Each asset kind is enumerated:
//! - agents_md: One entry per file
//! - cursor_rules: One entry per individual rule file
//! - cursor_skills_root: One entry per skill folder
//! - agent_skill: One entry per skill folder

use crate::error::{ApsError, Result};
use crate::manifest::{AssetKind, Entry, Manifest, Source};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Default catalog filename
pub const CATALOG_FILENAME: &str = "aps.catalog.yaml";

/// The catalog structure containing all enumerated assets
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Catalog {
    /// Version of the catalog format
    #[serde(default = "default_version")]
    pub version: u32,

    /// List of catalog entries
    #[serde(default)]
    pub entries: Vec<CatalogEntry>,
}

fn default_version() -> u32 {
    1
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: default_version(),
            entries: Vec::new(),
        }
    }
}

/// A single entry in the catalog representing an individual asset
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogEntry {
    /// Unique identifier for this catalog entry (derived from manifest entry id + asset name)
    pub id: String,

    /// The manifest entry ID this asset belongs to
    pub manifest_entry_id: String,

    /// Human-readable name of the asset
    pub name: String,

    /// The kind of asset
    pub kind: AssetKind,

    /// Relative path from the source root to this asset
    pub source_path: String,

    /// Description of the source (repo URL or filesystem path)
    pub source_description: String,
}

impl Catalog {
    /// Create a new empty catalog
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the catalog path relative to the manifest
    pub fn path_for_manifest(manifest_path: &Path) -> PathBuf {
        manifest_path
            .parent()
            .map(|p| p.join(CATALOG_FILENAME))
            .unwrap_or_else(|| PathBuf::from(CATALOG_FILENAME))
    }

    /// Load a catalog from disk
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(ApsError::CatalogNotFound);
        }

        let content = std::fs::read_to_string(path)
            .map_err(|e| ApsError::io(e, format!("Failed to read catalog at {:?}", path)))?;

        let catalog: Catalog =
            serde_yaml::from_str(&content).map_err(|e| ApsError::CatalogReadError {
                message: e.to_string(),
            })?;

        debug!("Loaded catalog with {} entries", catalog.entries.len());
        Ok(catalog)
    }

    /// Save the catalog to disk
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_yaml::to_string(self).map_err(|e| ApsError::CatalogReadError {
            message: format!("Failed to serialize catalog: {}", e),
        })?;

        std::fs::write(path, content)
            .map_err(|e| ApsError::io(e, format!("Failed to write catalog at {:?}", path)))?;

        info!("Saved catalog to {:?}", path);
        Ok(())
    }

    /// Generate a catalog from a manifest by enumerating all individual assets
    pub fn generate_from_manifest(manifest: &Manifest, manifest_dir: &Path) -> Result<Self> {
        let mut catalog = Catalog::new();

        for entry in &manifest.entries {
            let entries = enumerate_entry_assets(entry, manifest_dir)?;
            catalog.entries.extend(entries);
        }

        info!(
            "Generated catalog with {} entries from {} manifest entries",
            catalog.entries.len(),
            manifest.entries.len()
        );

        Ok(catalog)
    }
}

/// Enumerate all individual assets from a manifest entry
fn enumerate_entry_assets(entry: &Entry, manifest_dir: &Path) -> Result<Vec<CatalogEntry>> {
    let adapter = entry.source.to_adapter();
    let resolved = adapter.resolve(manifest_dir)?;

    if !resolved.source_path.exists() {
        return Err(ApsError::SourcePathNotFound {
            path: resolved.source_path,
        });
    }

    let source_description = get_source_description(&entry.source);
    let mut catalog_entries = Vec::new();

    match entry.kind {
        AssetKind::AgentsMd => {
            // Single file - create one entry
            let name = resolved
                .source_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "AGENTS.md".to_string());

            catalog_entries.push(CatalogEntry {
                id: format!("{}:{}", entry.id, name),
                manifest_entry_id: entry.id.clone(),
                name,
                kind: AssetKind::AgentsMd,
                source_path: resolved.source_path.to_string_lossy().to_string(),
                source_description: source_description.clone(),
            });
        }
        AssetKind::CursorRules => {
            // Enumerate each rule file in the directory
            let files = enumerate_files(&resolved.source_path, &entry.include)?;
            for file_path in files {
                let name = file_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                if name.is_empty() {
                    continue;
                }

                catalog_entries.push(CatalogEntry {
                    id: format!("{}:{}", entry.id, name),
                    manifest_entry_id: entry.id.clone(),
                    name,
                    kind: AssetKind::CursorRules,
                    source_path: file_path.to_string_lossy().to_string(),
                    source_description: source_description.clone(),
                });
            }
        }
        AssetKind::CursorSkillsRoot | AssetKind::AgentSkill => {
            // Enumerate each skill folder in the directory
            let folders = enumerate_folders(&resolved.source_path, &entry.include)?;
            for folder_path in folders {
                let name = folder_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();

                if name.is_empty() {
                    continue;
                }

                catalog_entries.push(CatalogEntry {
                    id: format!("{}:{}", entry.id, name),
                    manifest_entry_id: entry.id.clone(),
                    name,
                    kind: entry.kind.clone(),
                    source_path: folder_path.to_string_lossy().to_string(),
                    source_description: source_description.clone(),
                });
            }
        }
    }

    Ok(catalog_entries)
}

/// Enumerate all files in a directory, optionally filtering by include prefixes
fn enumerate_files(dir: &Path, include: &[String]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    for entry in std::fs::read_dir(dir)
        .map_err(|e| ApsError::io(e, format!("Failed to read directory {:?}", dir)))?
    {
        let entry = entry.map_err(|e| ApsError::io(e, "Failed to read directory entry"))?;
        let path = entry.path();

        // Only include files (not directories)
        if !path.is_file() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        // Apply include filter if specified
        if !include.is_empty() {
            let matches = include.iter().any(|prefix| name.starts_with(prefix));
            if !matches {
                continue;
            }
        }

        files.push(path);
    }

    // Sort for deterministic output
    files.sort();
    Ok(files)
}

/// Enumerate all folders in a directory, optionally filtering by include prefixes
fn enumerate_folders(dir: &Path, include: &[String]) -> Result<Vec<PathBuf>> {
    let mut folders = Vec::new();

    for entry in std::fs::read_dir(dir)
        .map_err(|e| ApsError::io(e, format!("Failed to read directory {:?}", dir)))?
    {
        let entry = entry.map_err(|e| ApsError::io(e, "Failed to read directory entry"))?;
        let path = entry.path();

        // Only include directories (not files)
        if !path.is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy().to_string();

        // Apply include filter if specified
        if !include.is_empty() {
            let matches = include.iter().any(|prefix| name.starts_with(prefix));
            if !matches {
                continue;
            }
        }

        folders.push(path);
    }

    // Sort for deterministic output
    folders.sort();
    Ok(folders)
}

/// Get a human-readable description of the source
fn get_source_description(source: &Source) -> String {
    match source {
        Source::Git { repo, r#ref, .. } => {
            format!("{} @ {}", repo, r#ref)
        }
        Source::Filesystem { root, path, .. } => {
            if let Some(p) = path {
                format!("{}/{}", root, p)
            } else {
                root.clone()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_catalog_default() {
        let catalog = Catalog::default();
        assert_eq!(catalog.version, 1);
        assert!(catalog.entries.is_empty());
    }

    #[test]
    fn test_catalog_path_for_manifest() {
        let manifest_path = PathBuf::from("/home/user/project/aps.yaml");
        let catalog_path = Catalog::path_for_manifest(&manifest_path);
        assert_eq!(
            catalog_path,
            PathBuf::from("/home/user/project/aps.catalog.yaml")
        );
    }

    #[test]
    fn test_enumerate_files() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create test files
        std::fs::write(dir.join("rule1.mdc"), "content1").unwrap();
        std::fs::write(dir.join("rule2.mdc"), "content2").unwrap();
        std::fs::write(dir.join("other.txt"), "content3").unwrap();
        std::fs::create_dir(dir.join("subdir")).unwrap();

        // Test without filter
        let files = enumerate_files(dir, &[])?;
        assert_eq!(files.len(), 3);

        // Test with filter
        let files = enumerate_files(dir, &["rule".to_string()])?;
        assert_eq!(files.len(), 2);

        Ok(())
    }

    #[test]
    fn test_enumerate_folders() -> Result<()> {
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path();

        // Create test folders
        std::fs::create_dir(dir.join("skill1")).unwrap();
        std::fs::create_dir(dir.join("skill2")).unwrap();
        std::fs::create_dir(dir.join("other")).unwrap();
        std::fs::write(dir.join("file.txt"), "content").unwrap();

        // Test without filter
        let folders = enumerate_folders(dir, &[])?;
        assert_eq!(folders.len(), 3);

        // Test with filter
        let folders = enumerate_folders(dir, &["skill".to_string()])?;
        assert_eq!(folders.len(), 2);

        Ok(())
    }
}
