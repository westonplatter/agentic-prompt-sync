use crate::error::{ApsError, Result};
use serde_yaml::Value;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy)]
pub enum HookKind {
    Cursor,
    Claude,
}

pub fn validate_cursor_hooks(hooks_dir: &Path, strict: bool) -> Result<Vec<String>> {
    validate_hooks(HookKind::Cursor, hooks_dir, strict)
}

pub fn validate_claude_hooks(hooks_dir: &Path, strict: bool) -> Result<Vec<String>> {
    validate_hooks(HookKind::Claude, hooks_dir, strict)
}

fn validate_hooks(kind: HookKind, hooks_dir: &Path, strict: bool) -> Result<Vec<String>> {
    let mut warnings = Vec::new();

    let config_path = hooks_dir.join(config_filename(kind));
    if !config_path.exists() {
        warn_or_error(
            &mut warnings,
            strict,
            ApsError::MissingHooksConfig {
                path: config_path.clone(),
            },
        )?;
        return Ok(warnings);
    }

    let config_value = match read_hooks_config(&config_path) {
        Ok(value) => value,
        Err(err) => {
            warn_or_error(&mut warnings, strict, err)?;
            return Ok(warnings);
        }
    };

    let hooks_section = match get_hooks_section(&config_value) {
        Some(section) => section,
        None => {
            warn_or_error(
                &mut warnings,
                strict,
                ApsError::MissingHooksSection {
                    path: config_path.clone(),
                },
            )?;
            return Ok(warnings);
        }
    };

    let commands = collect_hook_commands(hooks_section);
    let referenced_scripts = collect_hook_script_paths(&commands, kind);

    for rel_path in referenced_scripts {
        let script_path = hooks_dir.join(rel_path);
        if !script_path.exists() {
            warn_or_error(
                &mut warnings,
                strict,
                ApsError::HookScriptNotFound { path: script_path },
            )?;
        }
    }

    Ok(warnings)
}

fn config_filename(kind: HookKind) -> &'static str {
    match kind {
        HookKind::Cursor => "hooks.json",
        HookKind::Claude => "settings.json",
    }
}

fn read_hooks_config(path: &Path) -> Result<Value> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| ApsError::io(e, "Failed to read hooks config"))?;

    serde_yaml::from_str(&content).map_err(|e| ApsError::InvalidHooksConfig {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

fn get_hooks_section(config: &Value) -> Option<&Value> {
    let map = match config {
        Value::Mapping(map) => map,
        _ => return None,
    };

    map.get(Value::String("hooks".to_string()))
}

fn collect_hook_commands(section: &Value) -> Vec<String> {
    let mut commands = Vec::new();
    collect_command_values(section, &mut commands);
    commands
}

fn collect_command_values(value: &Value, commands: &mut Vec<String>) {
    match value {
        Value::Mapping(map) => {
            for (key, val) in map {
                if matches!(key, Value::String(k) if k == "command") {
                    if let Value::String(command) = val {
                        commands.push(command.clone());
                        continue;
                    }
                }
                collect_command_values(val, commands);
            }
        }
        Value::Sequence(seq) => {
            for val in seq {
                collect_command_values(val, commands);
            }
        }
        _ => {}
    }
}

fn collect_hook_script_paths(commands: &[String], kind: HookKind) -> HashSet<PathBuf> {
    let mut scripts = HashSet::new();
    let prefixes = match kind {
        HookKind::Cursor => vec![
            ".cursor/hooks/",
            "./.cursor/hooks/",
            "hooks/",
            "./hooks/",
            ".cursor\\hooks\\",
            ".\\.cursor\\hooks\\",
            "hooks\\",
            ".\\hooks\\",
        ],
        HookKind::Claude => vec![
            ".claude/hooks/",
            "./.claude/hooks/",
            "$CLAUDE_PROJECT_DIR/.claude/hooks/",
            "${CLAUDE_PROJECT_DIR}/.claude/hooks/",
            ".claude\\hooks\\",
            ".\\.claude\\hooks\\",
            "$CLAUDE_PROJECT_DIR\\.claude\\hooks\\",
            "${CLAUDE_PROJECT_DIR}\\.claude\\hooks\\",
        ],
    };

    for command in commands {
        for token in command.split_whitespace() {
            let token = trim_token(token);
            for prefix in &prefixes {
                if let Some(rel_path) = extract_relative_path(token, prefix) {
                    scripts.insert(PathBuf::from(rel_path));
                }
            }
        }
    }

    scripts
}

fn extract_relative_path(token: &str, prefix: &str) -> Option<String> {
    let position = token.find(prefix)?;
    let mut rel = &token[position + prefix.len()..];
    rel = rel.trim_matches(|c: char| matches!(c, '"' | '\'' | ';' | ')' | '(' | ','));
    if rel.is_empty() {
        None
    } else {
        Some(rel.to_string())
    }
}

fn trim_token(token: &str) -> &str {
    token.trim_matches(|c: char| matches!(c, '"' | '\'' | ';' | ')' | '(' | ','))
}

fn warn_or_error(warnings: &mut Vec<String>, strict: bool, error: ApsError) -> Result<()> {
    if strict {
        return Err(error);
    }

    warnings.push(error.to_string());
    Ok(())
}
