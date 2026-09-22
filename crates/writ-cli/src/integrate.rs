//! `writ integrate`: wire writ into an agent's own configuration.

use std::path::Path;

use anyhow::{bail, Result};

/// Agents `writ integrate` knows how to configure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    /// Claude Code hooks in `.claude/settings.json`.
    ClaudeCode,
}

pub fn integrate(_policy: &Path, _ledger: &Path, _target: Target, _print: bool) -> Result<()> {
    bail!("writ integrate is not implemented in this build")
}

/// The Claude Code settings fragment (`{"hooks": {...}}`) that routes every
/// tool call through `writ check --format claude-code`. Shared by
/// `writ integrate claude-code` and `writ run -- claude` (which passes it to
/// Claude Code with `--settings`). `writ_exe` is the absolute path of the
/// writ binary; `policy`/`ledger` are passed through as absolute paths.
#[allow(dead_code)] // wired up by `writ integrate` and `writ run`
pub(crate) fn claude_code_hook_settings(
    writ_exe: &Path,
    policy: &Path,
    ledger: &Path,
) -> serde_json::Value {
    let command = format!(
        "\"{}\" --policy \"{}\" --ledger \"{}\" check --format claude-code --ask defer",
        writ_exe.display(),
        policy.display(),
        ledger.display()
    );
    let hook = serde_json::json!([{
        "matcher": "*",
        "hooks": [{ "type": "command", "command": command }]
    }]);
    serde_json::json!({ "hooks": { "PreToolUse": hook, "PostToolUse": hook } })
}
