use miette::Diagnostic;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, ApsError>;

#[derive(Error, Diagnostic, Debug)]
#[allow(dead_code)] // Some variants are prepared for future checkpoints
pub enum ApsError {
    #[error("Manifest not found")]
    #[diagnostic(
        code(aps::manifest::not_found),
        help("Run `aps init` to create a manifest, or use `--manifest <path>` to specify one")
    )]
    ManifestNotFound,

    #[error("Manifest already exists at {path}")]
    #[diagnostic(code(aps::init::already_exists))]
    ManifestAlreadyExists { path: PathBuf },

    #[error("Failed to parse manifest: {message}")]
    #[diagnostic(code(aps::manifest::parse_error))]
    ManifestParseError { message: String },

    #[error("Invalid asset kind: {kind}")]
    #[diagnostic(
        code(aps::manifest::invalid_kind),
        help("Valid kinds are: cursor_rules, cursor_skills_root, agents_md")
    )]
    InvalidAssetKind { kind: String },

    #[error("Invalid source type: {source_type}")]
    #[diagnostic(
        code(aps::manifest::invalid_source),
        help("Valid source types are: git, filesystem")
    )]
    InvalidSourceType { source_type: String },

    #[error("Duplicate entry ID: {id}")]
    #[diagnostic(code(aps::manifest::duplicate_id))]
    DuplicateId { id: String },

    #[error("Source path not found: {path}")]
    #[diagnostic(code(aps::source::path_not_found))]
    SourcePathNotFound { path: PathBuf },

    #[error("Conflict detected at {path}")]
    #[diagnostic(
        code(aps::install::conflict),
        help("Use --yes to overwrite, or back up manually")
    )]
    Conflict { path: PathBuf },

    #[error("Operation cancelled by user")]
    #[diagnostic(code(aps::cancelled))]
    Cancelled,

    #[error("Non-interactive mode requires --yes flag for overwrites")]
    #[diagnostic(
        code(aps::install::requires_yes),
        help("Run with --yes to allow overwrites in non-interactive mode")
    )]
    RequiresYesFlag,

    #[error("IO error: {message}")]
    #[diagnostic(code(aps::io))]
    Io {
        message: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Failed to read lockfile: {message}")]
    #[diagnostic(code(aps::lockfile::read_error))]
    LockfileReadError { message: String },

    #[error("No lockfile found")]
    #[diagnostic(
        code(aps::lockfile::not_found),
        help("Run `aps pull` first to create a lockfile")
    )]
    LockfileNotFound,
}

impl ApsError {
    pub fn io(err: std::io::Error, context: impl Into<String>) -> Self {
        ApsError::Io {
            message: context.into(),
            source: err,
        }
    }
}
