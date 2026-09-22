//! `writ check`: the hook gateway (INTERFACES.md Contract 6).

use std::path::Path;

use anyhow::{bail, Result};

/// Wire format of `writ check`'s stdin/stdout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// writ's own JSON protocol (SDK adapters).
    Writ,
    /// Claude Code hook payloads (PreToolUse / PostToolUse).
    ClaudeCode,
}

/// What an `ask` verdict does when `writ check` has no terminal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum AskMode {
    /// Fail closed: the call does not dispatch.
    Deny,
    /// Return `approval: "required"`; the agent's own UI asks the human.
    Defer,
}

pub fn check(
    _policy: &Path,
    _ledger: &Path,
    _yolo: bool,
    _stdio: bool,
    _format: Format,
    _ask: AskMode,
) -> Result<()> {
    bail!("writ check is not implemented in this build (fail closed)")
}
