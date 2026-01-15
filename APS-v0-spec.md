# APS v0 Specification (Agentic Prompt Sync)

This document defines the **v0 specification** for **APS (Agentic Prompt Sync)** — a manifest-driven CLI for syncing agentic assets (Cursor rules, Cursor skills, AGENTS.md) from git or filesystem sources into a repository in a safe, repeatable, and incremental way.

---

## Implementation Status

| Checkpoint | Description | Status |
|------------|-------------|--------|
| 0 | CLI Skeleton | ✅ Complete |
| 1 | `aps init` | ✅ Complete |
| 2 | Manifest Discovery & Parsing | ✅ Complete |
| 3 | Schema Validation | ✅ Complete |
| 4 | Filesystem Source + `agents_md` | ✅ Complete |
| 5 | Conflict Handling | ✅ Complete |
| 6 | Lockfile + `aps status` | ✅ Complete |
| 7 | Directory Install (`cursor_rules`) | ⏳ Pending |
| 8 | Skills Root Adapter | ⏳ Pending |
| 9 | Git Source (Read-only) | ⏳ Pending |
| 10 | Git Source Install | ⏳ Pending |
| 11 | Polish | ⏳ Pending |

### Project Structure

```
src/
├── main.rs       # Entry point, CLI parsing, logging setup
├── cli.rs        # Clap argument definitions for all commands
├── commands.rs   # Command implementations (init, pull, validate, status)
├── manifest.rs   # Manifest types, discovery, and validation
├── lockfile.rs   # Lockfile types and I/O
├── install.rs    # Asset installation logic
├── backup.rs     # Backup creation for conflict handling
├── checksum.rs   # SHA256 checksum computation
└── error.rs      # Error types with miette diagnostics
```

---

## Tool Overview

- **Tool name:** aps
- **Binary:** `aps`
- **Manifest:** `aps.yaml` (default)
- **Lockfile:** `.aps.lock`
- **Backups:** `.aps-backups/`

---

## Goals

- Declarative, manifest-driven sync of agentic assets
- Safe installs with conflict detection and backups
- Deterministic lockfile enabling no-op pulls
- Scriptable CLI with optional interactivity

### Non-goals (v0)

- Full-screen TUI
- Ad-hoc (manifest-less) pulls
- Parallel execution
- Rich diff UI

---

## CLI Stack (v0)

- **Argument parsing:** `clap`
- **Interactive prompts:** `dialoguer`
- **Errors & diagnostics:** `miette`, `thiserror`
- **Logging:** `tracing`, `tracing-subscriber`
- **Serialization:** `serde_yaml` (YAML-first)

---

## Assets (MVP)

### Cursor Rules

- **Kind:** `cursor_rules`
- **Source path:** directory tree
- **Default dest:** `./.cursor/rules/`
- **Behavior:** copy directory contents preserving structure

### Cursor Skills Root

- **Kind:** `cursor_skills_root`
- **Source path:** directory containing immediate child skill folders
- **Default dest:** `./.cursor/skills/`
- **Behavior:**
  - Each immediate child directory is treated as a skill
  - Each skill is copied to `dest/<skill-name>/`
  - Warn if `SKILL.md` is missing (case-sensitive)
  - Still copy even if warning
- **Validation strict mode:** `--strict` turns warnings into errors

### AGENTS.md

- **Kind:** `agents_md`
- **Source path:** file exactly named `AGENTS.md`
- **Default dest:** `./AGENTS.md` (relative to manifest directory)
- **Behavior:** copy single file

---

## Sources (MVP)

### Git Source

```yaml
type: git
url: git@github.com:org/repo.git
ref: auto        # optional
shallow: true    # optional
```

- Supports SSH and HTTPS URLs
- If `ref` is missing or `auto`:
  - Try `main`, then `master`
- Record resolved ref name and commit SHA in lockfile

### Filesystem Source

```yaml
type: filesystem
root: ../shared-assets
```

- Pull item paths are resolved relative to `root`

---

## Manifest Behavior

- Manifest is the primary UX for `aps pull`
- Discovery:
  - If `--manifest` is provided, use it
  - Else walk up from CWD until `.git/` or filesystem root
- If no manifest found: error with hint to run `aps init`

### Interactive Manifest Prompt

Shown only if:
- Running in an interactive TTY
- Manifest exists
- Manifest is not explicitly listed in `.gitignore`

Prompt:
```
Use manifest at <path>? (Y/n)
```

If declined, behave as `--ignore-manifest` (v0: error with guidance)

---

## Conflict Behavior

If install would overwrite existing content:

1. Create backup at:
   ```
   .aps-backups/<dest-path>-<YYYY-MM-DD-HHMM>/
   ```
2. Interactive:
   - Prompt via `dialoguer`
3. Non-interactive:
   - Require `--yes`, otherwise error

### Conflict Definition

- **agents_md:** destination exists and differs
- **directories:** destination exists and is non-empty (v0 simplification)

---

## Lockfile

Stored at `.aps.lock`.

Per item:

- `source`
- `dest`
- `resolved_ref`
- `commit`
- `last_updated_at`
- `checksum`

### Checksum

- Deterministic SHA256 hash
- Hash over sorted relative paths + file bytes
- Stored as `sha256:<hex>`

If checksum unchanged on pull → no-op.

---

## CLI Commands

### `aps init`

Creates a manifest and updates `.gitignore`.

Flags:
- `--format yaml|toml` (default: yaml)
- `--manifest <path>`

---

### `aps pull`

Reads manifest, pulls all entries, installs them.

Flags:
- `--manifest <path>`
- `--only <id>` (repeatable)
- `--yes`
- `--ignore-manifest` (v0: error unless extended later)
- `--dry-run`
- `--verbose`

---

### `aps validate`

Validates manifest and sources.

Checks:
- Manifest schema
- Sources reachable
- Paths exist at resolved refs
- Skill warnings vs `--strict`

Flags:
- `--manifest <path>`
- `--strict`

---

### `aps status`

Displays last pull info from lockfile.

Per item:
- id
- source
- dest
- resolved ref
- commit
- last updated
- checksum

---

## Incremental Build Checkpoints

Each checkpoint results in a working CLI.

### Checkpoint 0 — CLI Skeleton ✅ COMPLETE
- `aps --help` works
- Subcommands registered (init, pull, validate, status)
- `--verbose` toggles logging via tracing

**Implementation:** `src/main.rs`, `src/cli.rs`

### Checkpoint 1 — `aps init` ✅ COMPLETE
- Creates `aps.yaml` manifest with example entry
- Adds `.aps.lock` and `.aps-backups/` to `.gitignore`
- Idempotent behavior (errors if manifest exists)

**Implementation:** `src/commands.rs::cmd_init()`

### Checkpoint 2 — Manifest Discovery & Parsing ✅ COMPLETE
- Walk-up discovery from CWD to `.git` directory
- `--manifest` override supported
- Clear error with hint if missing: "Run `aps init` to create a manifest"

**Implementation:** `src/manifest.rs::discover_manifest()`, `find_manifest_walk_up()`

### Checkpoint 3 — Schema Validation ✅ COMPLETE
- Unknown kinds/sources validated via serde
- Duplicate IDs error with clear diagnostic

**Implementation:** `src/manifest.rs::validate_manifest()`, `src/error.rs::ApsError`

### Checkpoint 4 — Filesystem Source + `agents_md` ✅ COMPLETE
- Install AGENTS.md from filesystem sources
- `--dry-run` shows what would happen without making changes
- Paths resolved relative to manifest directory

**Implementation:** `src/install.rs::install_entry()`, `install_asset()`

### Checkpoint 5 — Conflict Handling ✅ COMPLETE
- Backups created at `.aps-backups/<dest-path>-<YYYY-MM-DD-HHMM>/`
- Interactive overwrite prompt via dialoguer (TTY detection)
- `--yes` bypasses prompts in non-interactive mode
- Non-interactive without `--yes` returns clear error

**Implementation:** `src/backup.rs`, `src/install.rs`

### Checkpoint 6 — Lockfile + `aps status` ✅ COMPLETE
- Write `.aps.lock` after install with:
  - source, dest, resolved_ref, commit, last_updated_at, checksum
- SHA256 checksums enable no-op detection (idempotent pulls)
- `aps status` displays all synced entries with formatted output

**Implementation:** `src/lockfile.rs`, `src/checksum.rs`, `src/commands.rs::cmd_status()`

---

### Checkpoint 7 — Directory Install (`cursor_rules`) ⏳ PENDING
- Recursive copy
- Conflict detection
- Checksums

**Note:** Basic directory copy is implemented but needs cursor_rules-specific handling.

### Checkpoint 8 — Skills Root Adapter ⏳ PENDING
- Skill folder fan-out
- `SKILL.md` warnings
- `--strict` enforcement

### Checkpoint 9 — Git Source (Read-only) ⏳ PENDING
- Clone/fetch
- Ref auto-resolution
- Path existence validation

### Checkpoint 10 — Git Source Install ⏳ PENDING
- Install all asset kinds from git
- Record resolved refs + commits
- No-op pulls

### Checkpoint 11 — Polish ⏳ PENDING
- `--only` support
- Improved UX messages
- Interactive manifest confirmation

---

## Acceptance Criteria for v0

- Deterministic installs
- Safe overwrite behavior
- Idempotent pulls
- Clear diagnostics
- Scriptable defaults

---

_End of APS v0 Specification_
