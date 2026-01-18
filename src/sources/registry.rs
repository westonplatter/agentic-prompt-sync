//! Source registry for dynamic source type parsing.

use crate::error::{ApsError, Result};
use crate::sources::{FilesystemSource, GitSource, SourceAdapter};
use std::collections::HashMap;

type ParserFn = Box<dyn Fn(&serde_yaml::Value) -> Result<Box<dyn SourceAdapter>> + Send + Sync>;

/// Registry for source type parsers.
///
/// This allows dynamic registration and parsing of source types,
/// making it easy to add new source types without modifying core code.
pub struct SourceRegistry {
    parsers: HashMap<String, ParserFn>,
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceRegistry {
    /// Create a new registry with built-in source types registered.
    pub fn new() -> Self {
        let mut registry = Self {
            parsers: HashMap::new(),
        };

        // Register built-in source types
        registry.register("filesystem", |v| {
            let source: FilesystemSource =
                serde_yaml::from_value(v.clone()).map_err(|e| ApsError::ManifestParseError {
                    message: format!("Failed to parse filesystem source: {}", e),
                })?;
            Ok(Box::new(source) as Box<dyn SourceAdapter>)
        });

        registry.register("git", |v| {
            let source: GitSource =
                serde_yaml::from_value(v.clone()).map_err(|e| ApsError::ManifestParseError {
                    message: format!("Failed to parse git source: {}", e),
                })?;
            Ok(Box::new(source) as Box<dyn SourceAdapter>)
        });

        registry
    }

    /// Register a new source type parser.
    pub fn register<F>(&mut self, source_type: &str, parser: F)
    where
        F: Fn(&serde_yaml::Value) -> Result<Box<dyn SourceAdapter>> + Send + Sync + 'static,
    {
        self.parsers.insert(source_type.to_string(), Box::new(parser));
    }

    /// Parse a source value into a SourceAdapter.
    ///
    /// The value should be a YAML mapping with a "type" field indicating
    /// the source type.
    pub fn parse(&self, value: &serde_yaml::Value) -> Result<Box<dyn SourceAdapter>> {
        let source_type = value
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ApsError::ManifestParseError {
                message: "Source must have a 'type' field".to_string(),
            })?;

        self.parse_typed(source_type, value)
    }

    /// Parse a source value with a known type.
    pub fn parse_typed(
        &self,
        source_type: &str,
        value: &serde_yaml::Value,
    ) -> Result<Box<dyn SourceAdapter>> {
        let parser = self.parsers.get(source_type).ok_or_else(|| {
            ApsError::InvalidSourceType {
                source_type: source_type.to_string(),
            }
        })?;

        parser(value)
    }

    /// Get list of registered source types.
    #[allow(dead_code)]
    pub fn registered_types(&self) -> Vec<&str> {
        self.parsers.keys().map(|s| s.as_str()).collect()
    }
}

/// Global registry instance for convenience.
/// In a real application, you might want to use dependency injection instead.
#[allow(dead_code)]
pub fn default_registry() -> SourceRegistry {
    SourceRegistry::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_builtin_types() {
        let registry = SourceRegistry::new();
        let types = registry.registered_types();
        assert!(types.contains(&"filesystem"));
        assert!(types.contains(&"git"));
    }

    #[test]
    fn test_parse_filesystem_source() {
        let registry = SourceRegistry::new();
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
            type: filesystem
            root: ../shared
            symlink: true
        "#,
        )
        .unwrap();

        let source = registry.parse(&yaml).unwrap();
        assert_eq!(source.source_type(), "filesystem");
        assert_eq!(source.display_name(), "filesystem:../shared");
    }

    #[test]
    fn test_parse_git_source() {
        let registry = SourceRegistry::new();
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
            type: git
            repo: https://github.com/example/repo.git
            ref: main
        "#,
        )
        .unwrap();

        let source = registry.parse(&yaml).unwrap();
        assert_eq!(source.source_type(), "git");
        assert_eq!(source.display_name(), "https://github.com/example/repo.git");
    }

    #[test]
    fn test_parse_unknown_type() {
        let registry = SourceRegistry::new();
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
            type: s3
            bucket: my-bucket
        "#,
        )
        .unwrap();

        let result = registry.parse(&yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_type() {
        let registry = SourceRegistry::new();
        let yaml = serde_yaml::from_str::<serde_yaml::Value>(
            r#"
            root: ../shared
        "#,
        )
        .unwrap();

        let result = registry.parse(&yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_custom_source_registration() {
        use crate::sources::FilesystemSource;

        let mut registry = SourceRegistry::new();
        registry.register("custom", |v| {
            // Just reuse filesystem for testing
            let source: FilesystemSource = serde_yaml::from_value(v.clone()).map_err(|e| {
                ApsError::ManifestParseError {
                    message: format!("Failed to parse custom source: {}", e),
                }
            })?;
            Ok(Box::new(source) as Box<dyn SourceAdapter>)
        });

        let types = registry.registered_types();
        assert!(types.contains(&"custom"));
    }
}
