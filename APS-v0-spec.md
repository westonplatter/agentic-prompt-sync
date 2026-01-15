# APS v0 Specification (Agentic Prompt Sync)

This document defines the **v0 specification** for **APS (Agentic Prompt Sync)** — a manifest-driven CLI for syncing agentic assets (Cursor rules, Cursor skills, AGENTS.md) from git or filesystem sources into a repository in a safe, repeatable, and incremental way.

---

## Tool Overview

- **Tool name:** aps
- **Binary:** `aps`
- **Manifest:** `promptsync.yaml` (default)
- **Lockfile:** `.promptsync.lock`
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

Stored at `.promptsync.lock`.

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

### Checkpoint 0 — CLI Skeleton
- `aps --help` works
- Subcommands registered
- `--verbose` toggles logging

### Checkpoint 1 — `aps init`
- Creates manifest
- Adds lockfile to `.gitignore`
- Idempotent behavior

### Checkpoint 2 — Manifest Discovery & Parsing
- Walk-up discovery
- `--manifest` override
- Clear error if missing

### Checkpoint 3 — Schema Validation
- Unknown kinds/sources error
- Duplicate IDs error

### Checkpoint 4 — Filesystem Source + `agents_md`
- Install AGENTS.md from filesystem
- `--dry-run` support

### Checkpoint 5 — Conflict Handling
- Backups created
- Interactive overwrite prompt
- `--yes` bypass

### Checkpoint 6 — Lockfile + `aps status`
- Write lockfile after install
- Display status

### Checkpoint 7 — Directory Install (`cursor_rules`)
- Recursive copy
- Conflict detection
- Checksums

### Checkpoint 8 — Skills Root Adapter
- Skill folder fan-out
- `SKILL.md` warnings
- `--strict` enforcement

### Checkpoint 9 — Git Source (Read-only)
- Clone/fetch
- Ref auto-resolution
- Path existence validation

### Checkpoint 10 — Git Source Install
- Install all asset kinds from git
- Record resolved refs + commits
- No-op pulls

### Checkpoint 11 — Polish
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
