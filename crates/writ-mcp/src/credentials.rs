//! Per-server credential isolation (spec §11): the agent never holds the
//! token — Writ injects it into the downstream server's environment at
//! dispatch. Secrets live in `SecretString`, which cannot be serialized,
//! cloned, or Debug-printed, so they can never reach a ToolCall, a ledger
//! record, or a log line (plan §4.3 invariant).

use std::collections::{BTreeMap, HashMap};
use std::io::BufReader;
use std::process::{Child, Command, Stdio};

use writ_core::{Result, SecretString, WritError};

use crate::transport::StreamTransport;

/// server name → (env key → secret value)
#[derive(Default)]
pub struct CredentialStore {
    secrets: HashMap<String, BTreeMap<String, SecretString>>,
}

impl CredentialStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, server: &str, env_key: &str, value: SecretString) {
        self.secrets
            .entry(server.to_string())
            .or_default()
            .insert(env_key.to_string(), value);
    }

    /// Load credentials for `server` from process env vars named
    /// `WRIT_CRED_<SERVER>_<KEY>` (server uppercased, dashes→underscores).
    pub fn load_from_env(&mut self, server: &str) {
        let prefix = format!("WRIT_CRED_{}_", server.to_uppercase().replace('-', "_"));
        for (k, v) in std::env::vars() {
            if let Some(key) = k.strip_prefix(&prefix) {
                self.add(server, key, SecretString::new(v));
            }
        }
    }

    pub fn has_any(&self, server: &str) -> bool {
        self.secrets.get(server).map(|m| !m.is_empty()).unwrap_or(false)
    }

    /// Inject this server's secrets into an environment map about to be used
    /// for a downstream spawn. This is the ONLY path secrets take out of the
    /// store, and it targets a child-process environment — never a message.
    pub fn inject_env(&self, server: &str, env: &mut BTreeMap<String, String>) {
        if let Some(map) = self.secrets.get(server) {
            for (k, v) in map {
                env.insert(k.clone(), v.expose().to_string());
            }
        }
    }
}

/// A spawned downstream MCP server: child handle + framed transport.
pub struct StdioServer {
    pub child: Child,
    pub transport:
        StreamTransport<BufReader<std::process::ChildStdout>, std::process::ChildStdin>,
}

/// Spawn a downstream stdio MCP server with its credentials injected.
/// The child environment is clean (PATH + declared env + injected secrets).
pub fn spawn_stdio_server(
    program: &str,
    args: &[String],
    server_name: &str,
    creds: &CredentialStore,
    extra_env: &BTreeMap<String, String>,
) -> Result<StdioServer> {
    let mut env = extra_env.clone();
    creds.inject_env(server_name, &mut env);

    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .envs(&env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| {
            WritError::Intercept(format!("spawn MCP server {server_name} ({program}): {e}"))
        })?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WritError::Intercept("child stdout not piped".into()))?;
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| WritError::Intercept("child stdin not piped".into()))?;
    Ok(StdioServer {
        child,
        transport: StreamTransport::new(BufReader::new(stdout), stdin),
    })
}
