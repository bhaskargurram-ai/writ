//! `writ receipt`: signed receipts over the ledger. Stub: not implemented.

use std::path::{Path, PathBuf};

use anyhow::{bail, Result};

#[derive(Clone, Debug, clap::Subcommand)]
pub enum ReceiptCmd {
    /// Create a signing key pair.
    Keygen {
        /// Where to write the private key (the public key goes next to it).
        #[arg(long)]
        out: PathBuf,
    },
    /// Sign a receipt over the ledger (or one session) as it stands now.
    Create {
        #[arg(long)]
        key: PathBuf,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    /// Verify a receipt against the ledger and a public key.
    Verify {
        receipt: PathBuf,
        #[arg(long)]
        pubkey: Option<PathBuf>,
    },
}

pub fn run(_policy: &Path, _ledger: &Path, _sub: &ReceiptCmd) -> Result<()> {
    bail!("writ receipt is not implemented in this build")
}
