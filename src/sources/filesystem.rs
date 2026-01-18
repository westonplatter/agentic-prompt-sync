//! Filesystem source adapter for local file system sources.

use crate::error::Result;
use crate::sources::{AsAny, ResolvedSource, SourceAdapter};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::{Path, PathBuf};

/// Local filesystem source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemSource {
    /// Root directory for resolving paths
    pub root: String,
    /// Optional path within the root directory
    #[serde(default)]
    pub path: Option<String>,
    /// Whether to create symlinks instead of copying files (default: true)
    #[serde(default = "default_symlink")]
    pub symlink: bool,
}

fn default_symlink() -> bool {
    true
}

impl AsAny for FilesystemSource {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl SourceAdapter for FilesystemSource {
    fn source_type(&self) -> &'static str {
        "filesystem"
    }

    fn display_name(&self) -> String {
        format!("filesystem:{}", self.root)
    }

    fn resolve(&self, manifest_dir: &Path) -> Result<ResolvedSource> {
        let root_path = if Path::new(&self.root).is_absolute() {
            PathBuf::from(&self.root)
        } else {
            manifest_dir.join(&self.root)
        };

        let path = self.path();
        let source_path = if path == "." {
            root_path
        } else {
            root_path.join(path)
        };

        Ok(ResolvedSource {
            source_path,
            source_display: self.display_name(),
            git_info: None,
            use_symlink: self.symlink,
            _temp_holder: None,
        })
    }

    fn supports_symlink(&self) -> bool {
        true
    }

    fn path(&self) -> &str {
        self.path.as_deref().unwrap_or(".")
    }

    fn clone_box(&self) -> Box<dyn SourceAdapter> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn test_source_type() {
        let source = FilesystemSource {
            root: "/tmp/test".to_string(),
            path: None,
            symlink: true,
        };
        assert_eq!(source.source_type(), "filesystem");
    }

    #[test]
    fn test_display_name() {
        let source = FilesystemSource {
            root: "/tmp/test".to_string(),
            path: None,
            symlink: true,
        };
        assert_eq!(source.display_name(), "filesystem:/tmp/test");
    }

    #[test]
    fn test_supports_symlink() {
        let source = FilesystemSource {
            root: "/tmp/test".to_string(),
            path: None,
            symlink: true,
        };
        assert!(source.supports_symlink());
    }

    #[test]
    fn test_path_default() {
        let source = FilesystemSource {
            root: "/tmp/test".to_string(),
            path: None,
            symlink: true,
        };
        assert_eq!(source.path(), ".");
    }

    #[test]
    fn test_path_custom() {
        let source = FilesystemSource {
            root: "/tmp/test".to_string(),
            path: Some("subdir".to_string()),
            symlink: true,
        };
        assert_eq!(source.path(), "subdir");
    }

    #[test]
    fn test_resolve_absolute_root() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path().to_string_lossy().to_string();

        let source = FilesystemSource {
            root: root.clone(),
            path: None,
            symlink: true,
        };

        let manifest_dir = PathBuf::from("/some/other/path");
        let resolved = source.resolve(&manifest_dir).unwrap();

        assert_eq!(resolved.source_path, temp_dir.path());
        assert!(resolved.use_symlink);
        assert!(resolved.git_info.is_none());
    }

    #[test]
    fn test_resolve_relative_root() {
        let source = FilesystemSource {
            root: "../shared".to_string(),
            path: Some("assets".to_string()),
            symlink: false,
        };

        let manifest_dir = PathBuf::from("/home/user/project");
        let resolved = source.resolve(&manifest_dir).unwrap();

        assert_eq!(
            resolved.source_path,
            PathBuf::from("/home/user/project/../shared/assets")
        );
        assert!(!resolved.use_symlink);
    }
}
