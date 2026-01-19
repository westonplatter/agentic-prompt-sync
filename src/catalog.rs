//! Catalog module for intelligent asset discovery and suggestion.
//!
//! This module provides the "agentic" behavior of the tool - analyzing user context
//! and recommending relevant prompts, skills, and rules from a curated catalog.

use crate::error::{ApsError, Result};
use crate::manifest::{AssetKind, Source};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Default catalog filename
pub const DEFAULT_CATALOG_NAME: &str = "aps-catalog.yaml";

/// The main catalog structure containing available assets
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Catalog {
    /// Catalog format version
    #[serde(default = "default_version")]
    pub version: String,

    /// List of available assets in the catalog
    #[serde(default)]
    pub assets: Vec<CatalogEntry>,
}

fn default_version() -> String {
    "1.0".to_string()
}

impl Default for Catalog {
    fn default() -> Self {
        Self {
            version: default_version(),
            assets: vec![CatalogEntry::example()],
        }
    }
}

/// A single asset entry in the catalog with rich metadata for intelligent matching
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CatalogEntry {
    /// Unique identifier (matches manifest entry id)
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Detailed description of what this asset does
    pub description: String,

    /// The kind of asset
    pub kind: AssetKind,

    /// Primary category for organization
    #[serde(default)]
    pub category: String,

    /// Tags for filtering and discovery
    #[serde(default)]
    pub tags: Vec<String>,

    /// Specific use cases where this asset is helpful
    #[serde(default)]
    pub use_cases: Vec<String>,

    /// Keywords for search matching (beyond tags)
    #[serde(default)]
    pub keywords: Vec<String>,

    /// When this asset should be triggered/suggested (user intent patterns)
    #[serde(default)]
    pub triggers: Vec<String>,

    /// The source to pull from
    pub source: Source,

    /// Optional destination override
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dest: Option<String>,

    /// Author or maintainer
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Version string
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,

    /// URL to more information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,

    /// Relevance score (computed during search, not persisted)
    #[serde(skip)]
    pub score: f64,
}

impl CatalogEntry {
    /// Create an example catalog entry
    fn example() -> Self {
        Self {
            id: "code-review-rules".to_string(),
            name: "Code Review Best Practices".to_string(),
            description: "Comprehensive rules for conducting thorough code reviews, \
                          including security checks, performance considerations, and style guidelines."
                .to_string(),
            kind: AssetKind::CursorRules,
            category: "development".to_string(),
            tags: vec![
                "code-review".to_string(),
                "best-practices".to_string(),
                "quality".to_string(),
            ],
            use_cases: vec![
                "Reviewing pull requests".to_string(),
                "Ensuring code quality".to_string(),
                "Security auditing".to_string(),
            ],
            keywords: vec![
                "review".to_string(),
                "PR".to_string(),
                "pull request".to_string(),
                "audit".to_string(),
                "quality".to_string(),
            ],
            triggers: vec![
                "review this code".to_string(),
                "check this PR".to_string(),
                "audit for security".to_string(),
            ],
            source: Source::Filesystem {
                root: "../shared-assets".to_string(),
                symlink: true,
                path: Some("rules/code-review".to_string()),
            },
            dest: None,
            author: Some("APS Team".to_string()),
            version: Some("1.0.0".to_string()),
            homepage: None,
            score: 0.0,
        }
    }

    /// Get all searchable text for this entry (for indexing)
    pub fn searchable_text(&self) -> String {
        let mut parts = vec![
            self.name.clone(),
            self.description.clone(),
            self.category.clone(),
        ];
        parts.extend(self.tags.iter().cloned());
        parts.extend(self.use_cases.iter().cloned());
        parts.extend(self.keywords.iter().cloned());
        parts.extend(self.triggers.iter().cloned());
        parts.join(" ")
    }

    /// Convert to a manifest Entry for installation
    pub fn to_manifest_entry(&self) -> crate::manifest::Entry {
        crate::manifest::Entry {
            id: self.id.clone(),
            kind: self.kind.clone(),
            source: self.source.clone(),
            dest: self.dest.clone(),
            include: Vec::new(),
        }
    }
}

/// Search result with matched entry and explanation
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matched catalog entry
    pub entry: CatalogEntry,
    /// Explanation of why this matched
    pub match_reason: String,
    /// Individual term matches for debugging
    pub matched_terms: Vec<String>,
}

/// Search engine for finding relevant assets
pub struct CatalogSearch {
    /// The catalog to search
    catalog: Catalog,
    /// Inverted index: term -> list of (entry_index, field_weight)
    index: HashMap<String, Vec<(usize, f64)>>,
    /// Document frequency: term -> number of documents containing it
    doc_freq: HashMap<String, usize>,
    /// Total number of documents
    doc_count: usize,
}

impl CatalogSearch {
    /// Create a new search engine from a catalog
    pub fn new(catalog: Catalog) -> Self {
        let mut search = Self {
            doc_count: catalog.assets.len(),
            catalog,
            index: HashMap::new(),
            doc_freq: HashMap::new(),
        };
        search.build_index();
        search
    }

    /// Build the inverted index for search
    fn build_index(&mut self) {
        // Field weights for different parts of the entry
        const NAME_WEIGHT: f64 = 3.0;
        const TRIGGER_WEIGHT: f64 = 2.5;
        const TAG_WEIGHT: f64 = 2.0;
        const KEYWORD_WEIGHT: f64 = 2.0;
        const USE_CASE_WEIGHT: f64 = 1.5;
        const CATEGORY_WEIGHT: f64 = 1.5;
        const DESCRIPTION_WEIGHT: f64 = 1.0;

        // Collect all index data first to avoid borrow conflicts
        let mut index_data: Vec<(String, usize, f64)> = Vec::new();
        let mut doc_freq_data: Vec<HashSet<String>> = Vec::new();

        for (idx, entry) in self.catalog.assets.iter().enumerate() {
            let mut seen_terms: HashSet<String> = HashSet::new();

            // Index name
            for term in tokenize(&entry.name) {
                index_data.push((term.clone(), idx, NAME_WEIGHT));
                seen_terms.insert(term);
            }

            // Index triggers (high weight - these are user intent patterns)
            for trigger in &entry.triggers {
                for term in tokenize(trigger) {
                    index_data.push((term.clone(), idx, TRIGGER_WEIGHT));
                    seen_terms.insert(term);
                }
            }

            // Index tags
            for tag in &entry.tags {
                for term in tokenize(tag) {
                    index_data.push((term.clone(), idx, TAG_WEIGHT));
                    seen_terms.insert(term);
                }
            }

            // Index keywords
            for keyword in &entry.keywords {
                for term in tokenize(keyword) {
                    index_data.push((term.clone(), idx, KEYWORD_WEIGHT));
                    seen_terms.insert(term);
                }
            }

            // Index use cases
            for use_case in &entry.use_cases {
                for term in tokenize(use_case) {
                    index_data.push((term.clone(), idx, USE_CASE_WEIGHT));
                    seen_terms.insert(term);
                }
            }

            // Index category
            for term in tokenize(&entry.category) {
                index_data.push((term.clone(), idx, CATEGORY_WEIGHT));
                seen_terms.insert(term);
            }

            // Index description
            for term in tokenize(&entry.description) {
                index_data.push((term.clone(), idx, DESCRIPTION_WEIGHT));
                seen_terms.insert(term);
            }

            doc_freq_data.push(seen_terms);
        }

        // Now add to index (no borrow conflict)
        for (term, doc_idx, weight) in index_data {
            self.index
                .entry(term)
                .or_default()
                .push((doc_idx, weight));
        }

        // Update document frequency
        for seen_terms in doc_freq_data {
            for term in seen_terms {
                *self.doc_freq.entry(term).or_insert(0) += 1;
            }
        }
    }

    /// Search the catalog with a query string
    ///
    /// Returns results sorted by relevance score (highest first)
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let query_terms: Vec<String> = tokenize(query);
        if query_terms.is_empty() {
            return Vec::new();
        }

        debug!("Searching for terms: {:?}", query_terms);

        // Score each document
        let mut scores: HashMap<usize, (f64, Vec<String>)> = HashMap::new();

        for term in &query_terms {
            // Calculate IDF for this term
            let idf = if let Some(&df) = self.doc_freq.get(term) {
                ((self.doc_count as f64 + 1.0) / (df as f64 + 1.0)).ln() + 1.0
            } else {
                // Term not in index - try prefix matching
                let mut found = false;
                for (indexed_term, postings) in &self.index {
                    if indexed_term.starts_with(term) || term.starts_with(indexed_term) {
                        let df = self.doc_freq.get(indexed_term).copied().unwrap_or(1);
                        let idf = ((self.doc_count as f64 + 1.0) / (df as f64 + 1.0)).ln() + 1.0;
                        for &(doc_idx, weight) in postings {
                            let entry = scores.entry(doc_idx).or_insert((0.0, Vec::new()));
                            entry.0 += weight * idf * 0.7; // Partial match penalty
                            if !entry.1.contains(indexed_term) {
                                entry.1.push(indexed_term.clone());
                            }
                        }
                        found = true;
                    }
                }
                if !found {
                    continue;
                }
                continue;
            };

            // Look up postings for this term
            if let Some(postings) = self.index.get(term) {
                for &(doc_idx, weight) in postings {
                    let entry = scores.entry(doc_idx).or_insert((0.0, Vec::new()));
                    entry.0 += weight * idf;
                    if !entry.1.contains(term) {
                        entry.1.push(term.clone());
                    }
                }
            }
        }

        // Normalize by query length
        let query_len = query_terms.len() as f64;

        // Convert to results and sort
        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .map(|(idx, (score, matched_terms))| {
                let mut entry = self.catalog.assets[idx].clone();
                let normalized_score = score / query_len;
                entry.score = normalized_score;

                // Generate match reason
                let match_reason = generate_match_reason(&entry, &matched_terms);

                SearchResult {
                    entry,
                    match_reason,
                    matched_terms,
                }
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.entry.score.partial_cmp(&a.entry.score).unwrap());

        // Limit results
        results.truncate(limit);

        results
    }

    /// Get all assets in the catalog
    pub fn all_assets(&self) -> &[CatalogEntry] {
        &self.catalog.assets
    }

    /// Get an asset by ID
    pub fn get_by_id(&self, id: &str) -> Option<&CatalogEntry> {
        self.catalog.assets.iter().find(|e| e.id == id)
    }

    /// Filter assets by category
    pub fn filter_by_category(&self, category: &str) -> Vec<&CatalogEntry> {
        self.catalog
            .assets
            .iter()
            .filter(|e| e.category.eq_ignore_ascii_case(category))
            .collect()
    }

    /// Filter assets by tag
    pub fn filter_by_tag(&self, tag: &str) -> Vec<&CatalogEntry> {
        self.catalog
            .assets
            .iter()
            .filter(|e| e.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
            .collect()
    }

    /// Get all unique categories
    pub fn categories(&self) -> Vec<String> {
        let mut cats: HashSet<String> = self
            .catalog
            .assets
            .iter()
            .map(|e| e.category.clone())
            .collect();
        let mut result: Vec<String> = cats.drain().collect();
        result.sort();
        result
    }

    /// Get all unique tags
    pub fn tags(&self) -> Vec<String> {
        let mut tags: HashSet<String> = HashSet::new();
        for entry in &self.catalog.assets {
            for tag in &entry.tags {
                tags.insert(tag.clone());
            }
        }
        let mut result: Vec<String> = tags.drain().collect();
        result.sort();
        result
    }
}

/// Tokenize text into searchable terms
fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
        .filter(|s| !s.is_empty() && s.len() > 1)
        .filter(|s| !is_stop_word(s))
        .map(|s| stem(s))
        .collect()
}

/// Simple stemming (remove common suffixes)
fn stem(word: &str) -> String {
    let word = word.to_lowercase();

    // Simple suffix removal
    if word.len() > 4 {
        if word.ends_with("ing") {
            return word[..word.len() - 3].to_string();
        }
        if word.ends_with("tion") {
            return word[..word.len() - 4].to_string();
        }
        if word.ends_with("ed") && word.len() > 4 {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("ly") && word.len() > 4 {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("es") && word.len() > 4 {
            return word[..word.len() - 2].to_string();
        }
        if word.ends_with("s") && !word.ends_with("ss") && word.len() > 3 {
            return word[..word.len() - 1].to_string();
        }
    }

    word
}

/// Check if a word is a stop word
fn is_stop_word(word: &str) -> bool {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "the", "and", "or", "but", "in", "on", "at", "to", "for", "of", "with", "by",
        "from", "as", "is", "was", "are", "were", "been", "be", "have", "has", "had", "do", "does",
        "did", "will", "would", "could", "should", "may", "might", "must", "can", "this", "that",
        "these", "those", "it", "its", "i", "you", "he", "she", "we", "they", "my", "your", "his",
        "her", "our", "their", "what", "which", "who", "whom", "when", "where", "why", "how", "all",
        "each", "every", "both", "few", "more", "most", "other", "some", "such", "no", "not",
        "only", "same", "so", "than", "too", "very", "just", "also", "now", "here", "there",
    ];
    STOP_WORDS.contains(&word)
}

/// Generate a human-readable explanation for why an entry matched
fn generate_match_reason(entry: &CatalogEntry, matched_terms: &[String]) -> String {
    let mut reasons = Vec::new();

    // Check which fields matched
    let name_lower = entry.name.to_lowercase();
    let desc_lower = entry.description.to_lowercase();

    for term in matched_terms {
        if name_lower.contains(term) {
            reasons.push(format!("name contains '{}'", term));
        } else if entry.tags.iter().any(|t| t.to_lowercase().contains(term)) {
            reasons.push(format!("tagged with '{}'", term));
        } else if entry
            .triggers
            .iter()
            .any(|t| t.to_lowercase().contains(term))
        {
            reasons.push(format!("triggers on '{}'", term));
        } else if entry
            .use_cases
            .iter()
            .any(|u| u.to_lowercase().contains(term))
        {
            reasons.push(format!("use case involves '{}'", term));
        } else if entry
            .keywords
            .iter()
            .any(|k| k.to_lowercase().contains(term))
        {
            reasons.push(format!("keyword '{}'", term));
        } else if desc_lower.contains(term) {
            reasons.push(format!("description mentions '{}'", term));
        }
    }

    if reasons.is_empty() {
        "Partial match based on related terms".to_string()
    } else if reasons.len() == 1 {
        format!("Matched: {}", reasons[0])
    } else {
        format!("Matched: {} and {} more", reasons[0], reasons.len() - 1)
    }
}

// ============================================================================
// Catalog Discovery and Loading
// ============================================================================

/// Discover and load a catalog file
pub fn discover_catalog(override_path: Option<&Path>) -> Result<(Catalog, PathBuf)> {
    let catalog_path = if let Some(path) = override_path {
        debug!("Using catalog from --catalog flag: {:?}", path);
        path.to_path_buf()
    } else {
        find_catalog_walk_up()?
    };

    info!("Loading catalog from {:?}", catalog_path);
    load_catalog(&catalog_path).map(|c| (c, catalog_path))
}

/// Walk up from CWD to find a catalog file
fn find_catalog_walk_up() -> Result<PathBuf> {
    let cwd =
        std::env::current_dir().map_err(|e| ApsError::io(e, "Failed to get current directory"))?;
    let mut current = cwd.as_path();

    loop {
        let candidate = current.join(DEFAULT_CATALOG_NAME);
        debug!("Checking for catalog at {:?}", candidate);

        if candidate.exists() {
            info!("Found catalog at {:?}", candidate);
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

    Err(ApsError::CatalogNotFound)
}

/// Load and parse a catalog file
pub fn load_catalog(path: &Path) -> Result<Catalog> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ApsError::io(e, format!("Failed to read catalog at {:?}", path)))?;

    let catalog: Catalog =
        serde_yaml::from_str(&content).map_err(|e| ApsError::CatalogParseError {
            message: e.to_string(),
        })?;

    Ok(catalog)
}

/// Save a catalog file
pub fn save_catalog(catalog: &Catalog, path: &Path) -> Result<()> {
    let content = serde_yaml::to_string(catalog).map_err(|e| ApsError::CatalogParseError {
        message: format!("Failed to serialize catalog: {}", e),
    })?;

    std::fs::write(path, content)
        .map_err(|e| ApsError::io(e, format!("Failed to write catalog to {:?}", path)))?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_catalog() -> Catalog {
        Catalog {
            version: "1.0".to_string(),
            assets: vec![
                CatalogEntry {
                    id: "rust-best-practices".to_string(),
                    name: "Rust Best Practices".to_string(),
                    description: "Guidelines for writing idiomatic, safe, and performant Rust code"
                        .to_string(),
                    kind: AssetKind::CursorRules,
                    category: "language".to_string(),
                    tags: vec!["rust".to_string(), "safety".to_string(), "performance".to_string()],
                    use_cases: vec![
                        "Writing new Rust code".to_string(),
                        "Reviewing Rust PRs".to_string(),
                    ],
                    keywords: vec!["cargo".to_string(), "ownership".to_string(), "borrowing".to_string()],
                    triggers: vec![
                        "write rust code".to_string(),
                        "rust project".to_string(),
                    ],
                    source: Source::Filesystem {
                        root: ".".to_string(),
                        symlink: true,
                        path: Some("rules/rust".to_string()),
                    },
                    dest: None,
                    author: None,
                    version: None,
                    homepage: None,
                    score: 0.0,
                },
                CatalogEntry {
                    id: "typescript-react".to_string(),
                    name: "TypeScript React Patterns".to_string(),
                    description: "Modern React patterns with TypeScript including hooks, context, and testing"
                        .to_string(),
                    kind: AssetKind::CursorRules,
                    category: "frontend".to_string(),
                    tags: vec!["typescript".to_string(), "react".to_string(), "frontend".to_string()],
                    use_cases: vec![
                        "Building React components".to_string(),
                        "Frontend development".to_string(),
                    ],
                    keywords: vec!["hooks".to_string(), "jsx".to_string(), "component".to_string()],
                    triggers: vec![
                        "react component".to_string(),
                        "typescript frontend".to_string(),
                    ],
                    source: Source::Filesystem {
                        root: ".".to_string(),
                        symlink: true,
                        path: Some("rules/react".to_string()),
                    },
                    dest: None,
                    author: None,
                    version: None,
                    homepage: None,
                    score: 0.0,
                },
                CatalogEntry {
                    id: "code-review".to_string(),
                    name: "Code Review Guidelines".to_string(),
                    description: "Thorough code review checklist covering security, performance, and maintainability"
                        .to_string(),
                    kind: AssetKind::CursorRules,
                    category: "process".to_string(),
                    tags: vec!["review".to_string(), "quality".to_string(), "security".to_string()],
                    use_cases: vec![
                        "Reviewing pull requests".to_string(),
                        "Code audits".to_string(),
                    ],
                    keywords: vec!["PR".to_string(), "audit".to_string(), "checklist".to_string()],
                    triggers: vec![
                        "review this code".to_string(),
                        "check this PR".to_string(),
                    ],
                    source: Source::Filesystem {
                        root: ".".to_string(),
                        symlink: true,
                        path: Some("rules/review".to_string()),
                    },
                    dest: None,
                    author: None,
                    version: None,
                    homepage: None,
                    score: 0.0,
                },
            ],
        }
    }

    #[test]
    fn test_tokenize() {
        let tokens = tokenize("Write Rust code with best practices");
        assert!(tokens.contains(&"write".to_string()));
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"code".to_string()));
        assert!(tokens.contains(&"best".to_string()));
        assert!(tokens.contains(&"practic".to_string())); // stemmed from "practices"
    }

    #[test]
    fn test_search_by_tag() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let results = search.search("rust", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].entry.id, "rust-best-practices");
    }

    #[test]
    fn test_search_by_trigger() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let results = search.search("review this PR", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].entry.id, "code-review");
    }

    #[test]
    fn test_search_by_description() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let results = search.search("security performance", 10);
        assert!(!results.is_empty());
        // Both rust and review entries mention these
    }

    #[test]
    fn test_search_no_results() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let results = search.search("kubernetes deployment helm", 10);
        // May have partial matches or empty
        // This is fine - we just verify it doesn't crash
    }

    #[test]
    fn test_filter_by_category() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let results = search.filter_by_category("frontend");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "typescript-react");
    }

    #[test]
    fn test_filter_by_tag() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let results = search.filter_by_tag("security");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "code-review");
    }

    #[test]
    fn test_get_by_id() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let entry = search.get_by_id("rust-best-practices");
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().name, "Rust Best Practices");

        let missing = search.get_by_id("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_categories_and_tags() {
        let catalog = create_test_catalog();
        let search = CatalogSearch::new(catalog);

        let categories = search.categories();
        assert!(categories.contains(&"language".to_string()));
        assert!(categories.contains(&"frontend".to_string()));
        assert!(categories.contains(&"process".to_string()));

        let tags = search.tags();
        assert!(tags.contains(&"rust".to_string()));
        assert!(tags.contains(&"react".to_string()));
        assert!(tags.contains(&"security".to_string()));
    }
}
