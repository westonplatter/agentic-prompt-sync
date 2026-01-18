//! Git source adapter for git repository sources.

use crate::error::Result;
use crate::git::clone_and_resolve;
use crate::sources::{AsAny, GitInfo, ResolvedSource, SourceAdapter};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::path::Path;

/// Git repository source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitSource {
    /// Repository URL (SSH or HTTPS)
    #[serde(alias = "url")]
    pub repo: String,
    /// Git ref (branch, tag, commit) - "auto" tries main then master
    #[serde(default = "default_ref")]
    pub r#ref: String,
    /// Whether to use shallow clone
    #[serde(default = "default_shallow")]
    pub shallow: bool,
    /// Optional path within the repository
    #[serde(default)]
    pub path: Option<String>,
}

fn default_ref() -> String {
    "auto".to_string()
}

fn default_shallow() -> bool {
    true
}

impl AsAny for GitSource {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl SourceAdapter for GitSource {
    fn source_type(&self) -> &'static str {
        "git"
    }

    fn display_name(&self) -> String {
        self.repo.clone()
    }

    fn resolve(&self, _manifest_dir: &Path) -> Result<ResolvedSource> {
        println!("Fetching from git: {}", self.repo);
        let resolved = clone_and_resolve(&self.repo, &self.r#ref, self.shallow)?;

        let path = self.path();
        let source_path = if path == "." {
            resolved.repo_path.clone()
        } else {
            resolved.repo_path.join(path)
        };

        let git_info = GitInfo {
            resolved_ref: resolved.resolved_ref.clone(),
            commit_sha: resolved.commit_sha.clone(),
        };

        Ok(ResolvedSource {
            source_path,
            source_display: self.display_name(),
            git_info: Some(git_info),
            use_symlink: false, // Git sources always copy (temp dir)
            _temp_holder: Some(Box::new(resolved)),
        })
    }

    fn supports_symlink(&self) -> bool {
        false // Git sources always copy from temp dir
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

    #[test]
    fn test_source_type() {
        let source = GitSource {
            repo: "https://github.com/example/repo.git".to_string(),
            r#ref: "main".to_string(),
            shallow: true,
            path: None,
        };
        assert_eq!(source.source_type(), "git");
    }

    #[test]
    fn test_display_name() {
        let source = GitSource {
            repo: "https://github.com/example/repo.git".to_string(),
            r#ref: "main".to_string(),
            shallow: true,
            path: None,
        };
        assert_eq!(
            source.display_name(),
            "https://github.com/example/repo.git"
        );
    }

    #[test]
    fn test_supports_symlink() {
        let source = GitSource {
            repo: "https://github.com/example/repo.git".to_string(),
            r#ref: "main".to_string(),
            shallow: true,
            path: None,
        };
        assert!(!source.supports_symlink());
    }

    #[test]
    fn test_path_default() {
        let source = GitSource {
            repo: "https://github.com/example/repo.git".to_string(),
            r#ref: "main".to_string(),
            shallow: true,
            path: None,
        };
        assert_eq!(source.path(), ".");
    }

    #[test]
    fn test_path_custom() {
        let source = GitSource {
            repo: "https://github.com/example/repo.git".to_string(),
            r#ref: "main".to_string(),
            shallow: true,
            path: Some("src/assets".to_string()),
        };
        assert_eq!(source.path(), "src/assets");
    }

    #[test]
    fn test_default_values() {
        // Test deserialization with defaults
        let yaml = r#"
            repo: https://github.com/example/repo.git
        "#;
        let source: GitSource = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(source.r#ref, "auto");
        assert!(source.shallow);
        assert!(source.path.is_none());
    }
}
