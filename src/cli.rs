use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "aps",
    version,
    about = "Manifest-driven CLI for syncing agentic assets",
    long_about = "APS (Agentic Prompt Sync) syncs Cursor rules, Cursor skills, and AGENTS.md files \
                  from git or filesystem sources into your repository in a safe, repeatable way."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Enable verbose logging output
    #[arg(short, long, global = true)]
    pub verbose: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Initialize a new manifest file
    Init(InitArgs),

    /// Pull and install assets from manifest sources
    Pull(PullArgs),

    /// Validate manifest and sources
    Validate(ValidateArgs),

    /// Display status from lockfile
    Status(StatusArgs),
}

#[derive(Parser, Debug)]
pub struct InitArgs {
    /// Output format for the manifest
    #[arg(long, value_enum, default_value = "yaml")]
    pub format: ManifestFormat,

    /// Path for the manifest file
    #[arg(long)]
    pub manifest: Option<PathBuf>,
}

#[derive(ValueEnum, Clone, Debug, Default)]
pub enum ManifestFormat {
    #[default]
    Yaml,
    Toml,
}

#[derive(Parser, Debug)]
pub struct PullArgs {
    /// Path to the manifest file
    #[arg(long)]
    pub manifest: Option<PathBuf>,

    /// Only pull specific entry IDs (can be repeated)
    #[arg(long = "only")]
    pub only: Vec<String>,

    /// Skip confirmation prompts and allow overwrites
    #[arg(long, short = 'y')]
    pub yes: bool,

    /// Ignore manifest (v0: not implemented)
    #[arg(long, hide = true)]
    pub ignore_manifest: bool,

    /// Show what would be done without making changes
    #[arg(long)]
    pub dry_run: bool,

    /// Treat warnings as errors (e.g., missing SKILL.md)
    #[arg(long)]
    pub strict: bool,
}

#[derive(Parser, Debug)]
pub struct ValidateArgs {
    /// Path to the manifest file
    #[arg(long)]
    pub manifest: Option<PathBuf>,

    /// Treat warnings as errors
    #[arg(long)]
    pub strict: bool,
}

#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Path to the manifest file
    #[arg(long)]
    pub manifest: Option<PathBuf>,
}
