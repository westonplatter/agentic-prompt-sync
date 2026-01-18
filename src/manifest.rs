use crate::error::{ApsError, Result};
use crate::sources::{FilesystemSource, SourceAdapter, SourceRegistry};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Default manifest filename
pub const DEFAULT_MANIFEST_NAME: &str = "aps.yaml";

/// The main manifest structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// List of entries to sync
    #[serde(default)]
    pub entries: Vec<Entry>,
}

impl Default for Manifest {
    fn default() -> Self {
        Self {
            entries: vec![Entry::example()],
        }
    }
}

/// A single entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Unique identifier for this entry
    pub id: String,

    /// The kind of asset
    pub kind: AssetKind,

    /// The source to pull from
    #[serde(
        deserialize_with = "deserialize_source",
        serialize_with = "serialize_source"
    )]
    pub source: Box<dyn SourceAdapter>,

    /// Optional destination override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,

    /// Optional list of prefixes to filter which files/folders to sync
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include: Vec<String>,
}

/// Custom deserializer for Box<dyn SourceAdapter>
fn deserialize_source<'de, D>(deserializer: D) -> std::result::Result<Box<dyn SourceAdapter>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_yaml::Value::deserialize(deserializer)?;
    let registry = SourceRegistry::new();
    registry
        .parse(&value)
        .map_err(serde::de::Error::custom)
}

/// Custom serializer for Box<dyn SourceAdapter>
fn serialize_source<S>(
    source: &Box<dyn SourceAdapter>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: Serializer,
{
    use serde::ser::SerializeMap;

    // We need to serialize based on the source type
    let source_type = source.source_type();

    match source_type {
        "filesystem" => {
            // Downcast and serialize
            if let Some(fs) = source.as_any().downcast_ref::<FilesystemSource>() {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "filesystem")?;
                map.serialize_entry("root", &fs.root)?;
                if let Some(ref path) = fs.path {
                    map.serialize_entry("path", path)?;
                }
                map.serialize_entry("symlink", &fs.symlink)?;
                map.end()
            } else {
                Err(serde::ser::Error::custom("Failed to downcast FilesystemSource"))
            }
        }
        "git" => {
            use crate::sources::GitSource;
            if let Some(git) = source.as_any().downcast_ref::<GitSource>() {
                let mut map = serializer.serialize_map(None)?;
                map.serialize_entry("type", "git")?;
                map.serialize_entry("repo", &git.repo)?;
                map.serialize_entry("ref", &git.r#ref)?;
                if let Some(ref path) = git.path {
                    map.serialize_entry("path", path)?;
                }
                map.serialize_entry("shallow", &git.shallow)?;
                map.end()
            } else {
                Err(serde::ser::Error::custom("Failed to downcast GitSource"))
            }
        }
        _ => Err(serde::ser::Error::custom(format!(
            "Unknown source type: {}",
            source_type
        ))),
    }
}

impl Entry {
    /// Create an example entry for the default manifest
    fn example() -> Self {
        Self {
            id: "my-agents".to_string(),
            kind: AssetKind::AgentsMd,
            source: Box::new(FilesystemSource {
                root: "../shared-assets".to_string(),
                symlink: true,
                path: Some("AGENTS.md".to_string()),
            }),
            dest: None,
            include: Vec::new(),
        }
    }

    /// Get the destination path for this entry
    pub fn destination(&self) -> PathBuf {
        if let Some(ref dest) = self.dest {
            PathBuf::from(dest)
        } else {
            self.kind.default_dest()
        }
    }
}

/// Asset kinds supported by APS
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    /// Cursor rules directory
    CursorRules,
    /// Cursor skills root directory
    CursorSkillsRoot,
    /// AGENTS.md file
    AgentsMd,
    /// Agent skill directory (per agentskills.io spec)
    AgentSkill,
}

impl AssetKind {
    /// Get the default destination for this asset kind
    pub fn default_dest(&self) -> PathBuf {
        match self {
            AssetKind::CursorRules => PathBuf::from(".cursor/rules"),
            AssetKind::CursorSkillsRoot => PathBuf::from(".cursor/skills"),
            AssetKind::AgentsMd => PathBuf::from("AGENTS.md"),
            AssetKind::AgentSkill => PathBuf::from(".claude/skills"),
        }
    }

    /// Check if this is a valid kind string (for future use)
    #[allow(dead_code)]
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "cursor_rules" => Ok(AssetKind::CursorRules),
            "cursor_skills_root" => Ok(AssetKind::CursorSkillsRoot),
            "agents_md" => Ok(AssetKind::AgentsMd),
            "agent_skill" => Ok(AssetKind::AgentSkill),
            _ => Err(ApsError::InvalidAssetKind { kind: s.to_string() }),
        }
    }
}

/// Discover and load a manifest
pub fn discover_manifest(override_path: Option<&Path>) -> Result<(Manifest, PathBuf)> {
    let manifest_path = if let Some(path) = override_path {
        debug!("Using manifest from --manifest flag: {:?}", path);
        path.to_path_buf()
    } else {
        find_manifest_walk_up()?
    };

    info!("Loading manifest from {:?}", manifest_path);
    load_manifest(&manifest_path).map(|m| (m, manifest_path))
}

/// Walk up from CWD to find a manifest file
fn find_manifest_walk_up() -> Result<PathBuf> {
    let cwd = std::env::current_dir().map_err(|e| ApsError::io(e, "Failed to get current directory"))?;
    let mut current = cwd.as_path();

    loop {
        let candidate = current.join(DEFAULT_MANIFEST_NAME);
        debug!("Checking for manifest at {:?}", candidate);

        if candidate.exists() {
            info!("Found manifest at {:?}", candidate);
            return Ok(candidate);
        }

        // Stop at .git directory or filesystem root
        let git_dir = current.join(".git");
        if git_dir.exists() {
            debug!("Reached .git directory at {:?}, stopping search", current);
            break;
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => {
                debug!("Reached filesystem root, stopping search");
                break;
            }
        }
    }

    Err(ApsError::ManifestNotFound)
}

/// Load and parse a manifest file
pub fn load_manifest(path: &Path) -> Result<Manifest> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ApsError::io(e, format!("Failed to read manifest at {:?}", path)))?;

    let manifest: Manifest = serde_yaml::from_str(&content).map_err(|e| ApsError::ManifestParseError {
        message: e.to_string(),
    })?;

    Ok(manifest)
}

/// Validate a manifest for schema correctness
pub fn validate_manifest(manifest: &Manifest) -> Result<()> {
    let mut seen_ids = HashSet::new();

    for entry in &manifest.entries {
        // Check for duplicate IDs
        if !seen_ids.insert(&entry.id) {
            return Err(ApsError::DuplicateId {
                id: entry.id.clone(),
            });
        }
    }

    info!("Manifest validation passed");
    Ok(())
}

/// Get the manifest directory (for resolving relative paths)
pub fn manifest_dir(manifest_path: &Path) -> PathBuf {
    manifest_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}
