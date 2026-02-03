//! Claude Code settings composition module.
//!
//! Merges multiple permission YAML fragments into a single
//! Claude Code settings.json file. Each source provides a YAML
//! file with `allow` and/or `deny` permission lists.
//!
//! Merge strategy:
//! - Union all `allow` entries from all fragments
//! - Union all `deny` entries from all fragments
//! - Remove any entries from `allow` that also appear in `deny`
//! - Sort all lists alphabetically for determinism
//! - Deduplicate

use crate::error::{ApsError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;
use tracing::{debug, info};

/// A permission fragment from a single source YAML file.
///
/// Example YAML:
/// ```yaml
/// allow:
///   - "Bash(cat:*)"
///   - "Bash(git checkout:*)"
/// deny:
///   - "Bash(rm -rf:*)"
/// ```
#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct PermissionFragment {
    #[serde(default)]
    pub allow: Vec<String>,
    #[serde(default)]
    pub deny: Vec<String>,
}

/// Read a permission fragment from a YAML file.
pub fn read_permission_fragment(path: &Path) -> Result<PermissionFragment> {
    let content = std::fs::read_to_string(path).map_err(|e| {
        ApsError::io(
            e,
            format!("Failed to read permission fragment: {:?}", path),
        )
    })?;

    let fragment: PermissionFragment =
        serde_yaml::from_str(&content).map_err(|e| ApsError::ClaudeSettingsError {
            message: format!("Failed to parse permission fragment {:?}: {}", path, e),
        })?;

    debug!(
        "Read permission fragment from {:?}: {} allow, {} deny",
        path,
        fragment.allow.len(),
        fragment.deny.len()
    );

    Ok(fragment)
}

/// Composed permissions ready for JSON output.
#[derive(Debug, Serialize)]
pub struct ComposedPermissions {
    pub allow: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
}

/// Claude Code settings.json output structure.
#[derive(Debug, Serialize)]
pub struct ClaudeSettingsOutput {
    pub permissions: ComposedPermissions,
}

/// Compose multiple permission fragments into a single settings JSON string.
///
/// Merge strategy:
/// 1. Union all `allow` entries from all fragments
/// 2. Union all `deny` entries from all fragments
/// 3. Remove any entries from `allow` that also appear in `deny`
/// 4. Sort all lists alphabetically (BTreeSet handles this)
/// 5. Deduplicate (BTreeSet handles this)
pub fn compose_permissions(fragments: &[PermissionFragment]) -> Result<String> {
    if fragments.is_empty() {
        return Err(ApsError::ClaudeSettingsError {
            message: "No permission fragments provided for composition".to_string(),
        });
    }

    info!("Composing {} permission fragment(s)", fragments.len());

    let mut all_allow: BTreeSet<String> = BTreeSet::new();
    let mut all_deny: BTreeSet<String> = BTreeSet::new();

    for fragment in fragments {
        all_allow.extend(fragment.allow.iter().cloned());
        all_deny.extend(fragment.deny.iter().cloned());
    }

    // Remove denied entries from allow list
    for denied in &all_deny {
        all_allow.remove(denied);
    }

    let output = ClaudeSettingsOutput {
        permissions: ComposedPermissions {
            allow: all_allow.into_iter().collect(),
            deny: all_deny.into_iter().collect(),
        },
    };

    let json = serde_json::to_string_pretty(&output).map_err(|e| {
        ApsError::ClaudeSettingsError {
            message: format!("Failed to serialize settings to JSON: {}", e),
        }
    })?;

    debug!("Composed settings JSON: {} bytes", json.len());

    Ok(json)
}

/// Write the composed settings JSON to a destination file.
pub fn write_settings_file(content: &str, dest: &Path) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ApsError::io(e, format!("Failed to create directory: {:?}", parent))
            })?;
        }
    }

    // Write with trailing newline
    let content_with_newline = if content.ends_with('\n') {
        content.to_string()
    } else {
        format!("{}\n", content)
    };

    std::fs::write(dest, content_with_newline)
        .map_err(|e| ApsError::io(e, format!("Failed to write settings file: {:?}", dest)))?;

    info!("Wrote settings file to {:?}", dest);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_compose_single_fragment_allow_only() {
        let fragments = vec![PermissionFragment {
            allow: vec![
                "Bash(cat:*)".to_string(),
                "Bash(ls:*)".to_string(),
                "WebSearch".to_string(),
            ],
            deny: vec![],
        }];

        let result = compose_permissions(&fragments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 3);
        // BTreeSet sorts alphabetically
        assert_eq!(allow[0], "Bash(cat:*)");
        assert_eq!(allow[1], "Bash(ls:*)");
        assert_eq!(allow[2], "WebSearch");

        // No deny section when empty
        assert!(parsed["permissions"]["deny"].is_null());
    }

    #[test]
    fn test_compose_multiple_fragments_union() {
        let fragments = vec![
            PermissionFragment {
                allow: vec!["Bash(cat:*)".to_string(), "Bash(ls:*)".to_string()],
                deny: vec![],
            },
            PermissionFragment {
                allow: vec![
                    "Bash(ls:*)".to_string(), // duplicate
                    "WebSearch".to_string(),
                ],
                deny: vec![],
            },
        ];

        let result = compose_permissions(&fragments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 3); // deduped
        assert_eq!(allow[0], "Bash(cat:*)");
        assert_eq!(allow[1], "Bash(ls:*)");
        assert_eq!(allow[2], "WebSearch");
    }

    #[test]
    fn test_compose_deny_removes_from_allow() {
        let fragments = vec![
            PermissionFragment {
                allow: vec![
                    "Bash(cat:*)".to_string(),
                    "Bash(curl:*)".to_string(),
                    "Bash(ls:*)".to_string(),
                ],
                deny: vec![],
            },
            PermissionFragment {
                allow: vec![],
                deny: vec!["Bash(curl:*)".to_string()],
            },
        ];

        let result = compose_permissions(&fragments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 2);
        assert_eq!(allow[0], "Bash(cat:*)");
        assert_eq!(allow[1], "Bash(ls:*)");

        let deny = parsed["permissions"]["deny"].as_array().unwrap();
        assert_eq!(deny.len(), 1);
        assert_eq!(deny[0], "Bash(curl:*)");
    }

    #[test]
    fn test_compose_deny_from_multiple_fragments() {
        let fragments = vec![
            PermissionFragment {
                allow: vec![
                    "Bash(cat:*)".to_string(),
                    "Bash(curl:*)".to_string(),
                    "Bash(rm -rf:*)".to_string(),
                ],
                deny: vec![],
            },
            PermissionFragment {
                allow: vec![],
                deny: vec![
                    "Bash(curl:*)".to_string(),
                    "Bash(rm -rf:*)".to_string(),
                ],
            },
        ];

        let result = compose_permissions(&fragments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 1);
        assert_eq!(allow[0], "Bash(cat:*)");

        let deny = parsed["permissions"]["deny"].as_array().unwrap();
        assert_eq!(deny.len(), 2);
    }

    #[test]
    fn test_compose_empty_fragments_error() {
        let fragments: Vec<PermissionFragment> = vec![];
        let result = compose_permissions(&fragments);
        assert!(result.is_err());
    }

    #[test]
    fn test_compose_sorted_output() {
        let fragments = vec![PermissionFragment {
            allow: vec![
                "WebSearch".to_string(),
                "Bash(cat:*)".to_string(),
                "Bash(ls:*)".to_string(),
                "Bash(git checkout:*)".to_string(),
            ],
            deny: vec![],
        }];

        let result = compose_permissions(&fragments).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();

        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        // Should be sorted alphabetically
        assert_eq!(allow[0], "Bash(cat:*)");
        assert_eq!(allow[1], "Bash(git checkout:*)");
        assert_eq!(allow[2], "Bash(ls:*)");
        assert_eq!(allow[3], "WebSearch");
    }

    #[test]
    fn test_read_permission_fragment_yaml() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("permissions.yaml");
        std::fs::write(
            &path,
            r#"allow:
  - "Bash(cat:*)"
  - "Bash(ls:*)"
  - "WebSearch"
deny:
  - "Bash(rm -rf:*)"
"#,
        )
        .unwrap();

        let fragment = read_permission_fragment(&path).unwrap();
        assert_eq!(fragment.allow.len(), 3);
        assert_eq!(fragment.deny.len(), 1);
        assert_eq!(fragment.allow[0], "Bash(cat:*)");
        assert_eq!(fragment.deny[0], "Bash(rm -rf:*)");
    }

    #[test]
    fn test_read_permission_fragment_allow_only() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("permissions.yaml");
        std::fs::write(
            &path,
            r#"allow:
  - "Bash(cat:*)"
  - "WebSearch"
"#,
        )
        .unwrap();

        let fragment = read_permission_fragment(&path).unwrap();
        assert_eq!(fragment.allow.len(), 2);
        assert!(fragment.deny.is_empty());
    }

    #[test]
    fn test_write_settings_file() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join(".claude").join("settings.json");

        let content = r#"{"permissions":{"allow":["WebSearch"]}}"#;
        write_settings_file(content, &dest).unwrap();

        let written = std::fs::read_to_string(&dest).unwrap();
        assert!(written.contains("WebSearch"));
        assert!(written.ends_with('\n'));
    }

    #[test]
    fn test_write_settings_creates_parent_dirs() {
        let dir = tempdir().unwrap();
        let dest = dir.path().join("deep").join("nested").join("settings.json");

        let content = r#"{"test": true}"#;
        write_settings_file(content, &dest).unwrap();

        assert!(dest.exists());
    }

    #[test]
    fn test_compose_produces_valid_json() {
        let fragments = vec![
            PermissionFragment {
                allow: vec![
                    "Bash(cat:*)".to_string(),
                    "Bash(git checkout:*)".to_string(),
                    "Bash(git fetch:*)".to_string(),
                    "WebSearch".to_string(),
                    "WebFetch(domain:github.com)".to_string(),
                ],
                deny: vec![],
            },
            PermissionFragment {
                allow: vec![
                    "Bash(ls:*)".to_string(),
                    "Bash(find:*)".to_string(),
                    "mcp__context7__query-docs".to_string(),
                ],
                deny: vec![],
            },
        ];

        let result = compose_permissions(&fragments).unwrap();

        // Should be valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed["permissions"]["allow"].is_array());

        // Should have the union of all permissions (8 unique)
        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 8);
    }
}
