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
