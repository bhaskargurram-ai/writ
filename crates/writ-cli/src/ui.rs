//! `writ ui`: the local web console. Stub: not implemented in this build.

use std::path::Path;

use anyhow::{bail, Result};

/// Flags for `writ ui`.
#[derive(Clone, Debug, clap::Args)]
pub struct UiArgs {
    /// Port on 127.0.0.1 (0 picks a free one).
    #[arg(long, default_value_t = 0)]
    pub port: u16,
    /// Do not open a browser.
    #[arg(long)]
    pub no_open: bool,
}

pub fn serve(_policy: &Path, _ledger: &Path, _yolo: bool, _args: &UiArgs) -> Result<()> {
    bail!("writ ui is not implemented in this build")
}
