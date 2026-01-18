//! Source adapters for different content sources.
//!
//! This module provides a trait-based abstraction for different source types
//! (git, filesystem, etc.) allowing for easy extensibility.

pub mod filesystem;
pub mod git;
pub mod registry;

use crate::error::Result;
use crate::lockfile::LockedEntry;
use std::any::Any;
use std::fmt::Debug;
use std::path::{Path, PathBuf};

pub use filesystem::FilesystemSource;
pub use git::GitSource;
pub use registry::SourceRegistry;

/// Helper trait for downcasting trait objects to concrete types.
pub trait AsAny {
    fn as_any(&self) -> &dyn Any;
}

/// A source that can provide content for installation.
///
/// Implementors of this trait provide a way to resolve content from
/// various sources (git repositories, local filesystem, etc.) into
/// a local path that can be installed.
pub trait SourceAdapter: Send + Sync + Debug + AsAny {
    /// Unique identifier for this source type (e.g., "git", "filesystem", "s3")
    fn source_type(&self) -> &'static str;

    /// Human-readable display name for logging
    fn display_name(&self) -> String;

    /// Resolve the source and return the path to content.
    /// Returns a ResolvedSource that may hold temporary resources.
    fn resolve(&self, manifest_dir: &Path) -> Result<ResolvedSource>;

    /// Whether this source supports symlinking (vs. must copy)
    fn supports_symlink(&self) -> bool;

    /// Get the path within the source (defaults to "." if not specified)
    fn path(&self) -> &str;

    /// Optional: Check if remote has changed without fetching content.
    /// Returns None if check not supported, Some(true) if changed, Some(false) if same.
    fn has_remote_changed(&self, _lockfile_entry: Option<&LockedEntry>) -> Result<Option<bool>> {
        Ok(None) // Default: don't know, must fetch
    }

    /// Clone this source adapter into a boxed trait object
    fn clone_box(&self) -> Box<dyn SourceAdapter>;
}

impl Clone for Box<dyn SourceAdapter> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Git-specific resolution info
#[derive(Debug, Clone)]
pub struct GitInfo {
    pub resolved_ref: String,
    pub commit_sha: String,
}

/// Resolved source information ready for installation.
pub struct ResolvedSource {
    /// Path to the actual source content
    pub source_path: PathBuf,
    /// Display name for the source
    pub source_display: String,
    /// Git-specific info (if applicable)
    pub git_info: Option<GitInfo>,
    /// Whether to create symlinks instead of copying
    pub use_symlink: bool,
    /// Holds temp resources (e.g., cloned git repo) until installation complete
    pub _temp_holder: Option<Box<dyn Any + Send>>,
}

impl Debug for ResolvedSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResolvedSource")
            .field("source_path", &self.source_path)
            .field("source_display", &self.source_display)
            .field("git_info", &self.git_info)
            .field("use_symlink", &self.use_symlink)
            .field("_temp_holder", &self._temp_holder.is_some())
            .finish()
    }
}
