use crate::cli::{InitArgs, ManifestFormat, PullArgs, StatusArgs, ValidateArgs};
use crate::error::{ApsError, Result};
use crate::install::{install_all, InstallOptions};
use crate::lockfile::{display_status, Lockfile, LOCKFILE_NAME};
use crate::manifest::{
    discover_manifest, manifest_dir, validate_manifest, Manifest, DEFAULT_MANIFEST_NAME,
};
use std::fs;
use std::io::Write;
use std::path::Path;
use tracing::info;

/// Execute the `aps init` command
pub fn cmd_init(args: InitArgs) -> Result<()> {
    let manifest_path = args
        .manifest
        .unwrap_or_else(|| std::env::current_dir().unwrap().join(DEFAULT_MANIFEST_NAME));

    // Check if manifest already exists
    if manifest_path.exists() {
        return Err(ApsError::ManifestAlreadyExists {
            path: manifest_path,
        });
    }

    // Create default manifest
    let manifest = Manifest::default();

    let content = match args.format {
        ManifestFormat::Yaml => {
            serde_yaml::to_string(&manifest).expect("Failed to serialize manifest")
        }
        ManifestFormat::Toml => {
            // For TOML, we'd need a different serializer, but YAML is default
            // This is a simplified version
            return Err(ApsError::ManifestParseError {
                message: "TOML format not yet implemented".to_string(),
            });
        }
    };

    // Write manifest file
    fs::write(&manifest_path, &content)
        .map_err(|e| ApsError::io(e, format!("Failed to write manifest to {:?}", manifest_path)))?;

    println!("Created manifest at {:?}", manifest_path);
    info!("Created manifest at {:?}", manifest_path);

    // Update .gitignore
    update_gitignore(&manifest_path)?;

    Ok(())
}

/// Update .gitignore to include the lockfile
fn update_gitignore(manifest_path: &Path) -> Result<()> {
    let manifest_dir = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."));

    let gitignore_path = manifest_dir.join(".gitignore");
    let lockfile_entry = LOCKFILE_NAME;
    let backup_entry = ".aps-backups/";

    // Read existing .gitignore or start with empty
    let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();

    let needs_lockfile = !existing.lines().any(|line| line.trim() == lockfile_entry);
    let needs_backup = !existing.lines().any(|line| line.trim() == backup_entry);

    if !needs_lockfile && !needs_backup {
        info!(".gitignore already contains required entries");
        return Ok(());
    }

    // Append entries
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&gitignore_path)
        .map_err(|e| ApsError::io(e, "Failed to open .gitignore"))?;

    // Add newline if file doesn't end with one
    if !existing.is_empty() && !existing.ends_with('\n') {
        writeln!(file).map_err(|e| ApsError::io(e, "Failed to write to .gitignore"))?;
    }

    // Add comment and entries
    if needs_lockfile || needs_backup {
        writeln!(file, "\n# APS (Agentic Prompt Sync)")
            .map_err(|e| ApsError::io(e, "Failed to write to .gitignore"))?;
    }

    if needs_lockfile {
        writeln!(file, "{}", lockfile_entry)
            .map_err(|e| ApsError::io(e, "Failed to write to .gitignore"))?;
        println!("Added {} to .gitignore", lockfile_entry);
    }

    if needs_backup {
        writeln!(file, "{}", backup_entry)
            .map_err(|e| ApsError::io(e, "Failed to write to .gitignore"))?;
        println!("Added {} to .gitignore", backup_entry);
    }

    Ok(())
}

/// Execute the `aps pull` command
pub fn cmd_pull(args: PullArgs) -> Result<()> {
    // Discover and load manifest
    let (manifest, manifest_path) = discover_manifest(args.manifest.as_deref())?;
    let base_dir = manifest_dir(&manifest_path);

    // Validate manifest
    validate_manifest(&manifest)?;

    // Load existing lockfile (or create new)
    let lockfile_path = Lockfile::path_for_manifest(&manifest_path);
    let mut lockfile = Lockfile::load(&lockfile_path).unwrap_or_else(|_| {
        info!("No existing lockfile, creating new one");
        Lockfile::new()
    });

    // Set up install options
    let options = InstallOptions {
        dry_run: args.dry_run,
        yes: args.yes,
    };

    // Install all entries
    let results = install_all(&manifest, &base_dir, &lockfile, &options)?;

    // Update lockfile with results
    if !args.dry_run {
        for result in &results {
            if let Some(ref locked_entry) = result.locked_entry {
                lockfile.upsert(result.id.clone(), locked_entry.clone());
            }
        }

        // Save lockfile
        lockfile.save(&lockfile_path)?;
    }

    // Print summary
    let installed_count = results.iter().filter(|r| r.installed).count();
    let skipped_count = results.iter().filter(|r| r.skipped_no_change).count();

    if args.dry_run {
        println!("\n[dry-run] Would install {} entries, {} already up to date",
            results.len() - skipped_count, skipped_count);
    } else {
        println!("\nInstalled {} entries, {} already up to date",
            installed_count, skipped_count);
    }

    Ok(())
}

/// Execute the `aps validate` command
pub fn cmd_validate(args: ValidateArgs) -> Result<()> {
    // Discover and load manifest
    let (manifest, manifest_path) = discover_manifest(args.manifest.as_deref())?;
    println!("Validating manifest at {:?}", manifest_path);

    // Validate schema
    validate_manifest(&manifest)?;

    // Check sources are reachable
    let base_dir = manifest_dir(&manifest_path);
    for entry in &manifest.entries {
        match &entry.source {
            crate::manifest::Source::Filesystem { root } => {
                let root_path = if Path::new(root).is_absolute() {
                    std::path::PathBuf::from(root)
                } else {
                    base_dir.join(root)
                };
                let source_path = root_path.join(&entry.path);

                if !source_path.exists() {
                    if args.strict {
                        return Err(ApsError::SourcePathNotFound { path: source_path });
                    } else {
                        println!("Warning: Source path not found: {:?}", source_path);
                    }
                } else {
                    println!("  [OK] {} -> {:?}", entry.id, source_path);
                }
            }
            crate::manifest::Source::Git { url, .. } => {
                // Git validation not yet implemented
                println!("  [SKIP] {} (git source: {})", entry.id, url);
            }
        }
    }

    println!("\nManifest is valid.");
    Ok(())
}

/// Execute the `aps status` command
pub fn cmd_status(args: StatusArgs) -> Result<()> {
    // Discover manifest to find lockfile location
    let (_, manifest_path) = discover_manifest(args.manifest.as_deref())?;
    let lockfile_path = Lockfile::path_for_manifest(&manifest_path);

    // Load lockfile
    let lockfile = Lockfile::load(&lockfile_path)?;

    // Display status
    display_status(&lockfile);

    Ok(())
}
