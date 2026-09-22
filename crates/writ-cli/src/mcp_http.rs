//! `writ proxy --mcp --transport http`: the MCP proxy over Streamable HTTP
//! and SSE. Stub: not implemented in this build.

use std::path::Path;

use anyhow::{bail, Result};

/// MCP transport between the agent and writ.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum Transport {
    /// Spawn the downstream server and speak JSON-RPC over its stdio.
    #[default]
    Stdio,
    /// Listen for the agent over Streamable HTTP (and legacy SSE) and
    /// forward to an upstream HTTP MCP server.
    Http,
}

/// Flags for the HTTP transport (flattened into `writ proxy`).
#[derive(Clone, Debug, Default, clap::Args)]
pub struct HttpArgs {
    /// Transport the agent uses to reach writ.
    #[arg(long, value_enum, default_value_t = Transport::Stdio)]
    pub transport: Transport,
    /// Address to listen on for the agent (http transport).
    #[arg(long, value_name = "ADDR", default_value = "127.0.0.1:0")]
    pub listen: String,
    /// Upstream MCP server URL (http transport).
    #[arg(long, value_name = "URL")]
    pub upstream: Option<String>,
}

impl HttpArgs {
    pub fn is_http(&self) -> bool {
        self.transport == Transport::Http
    }
}

pub fn proxy_http(
    _policy: &Path,
    _ledger: &Path,
    _yolo: bool,
    _mcp: bool,
    _server: &str,
    _args: &HttpArgs,
) -> Result<()> {
    bail!("writ proxy --transport http is not implemented in this build")
}
