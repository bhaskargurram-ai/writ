//! `writ run`: governed process wrap (mode B).

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{anyhow, Context, Result};
use writ_core::call::InterceptMode;
use writ_core::ledger::LedgerWriter;
use writ_core::pipeline::handle_call;
use writ_core::Timestamp;
use writ_tui::{render_call_line, render_rule_note, TuiApprover};

use crate::cmds::{banner, emit_span, load_engine, make_call};

/// Network for the wrapped agent.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum NetMode {
    /// The agent can reach the network (its model API); not filtered.
    Open,
    /// No network, kernel-enforced.
    None,
}

/// Everything `writ run` was asked to do.
#[derive(Debug)]
pub struct RunArgs {
    pub policy: PathBuf,
    pub ledger: PathBuf,
    pub yolo: bool,
    pub backend: String,
    pub net: NetMode,
    pub allow_write: Vec<PathBuf>,
    pub unconfined: bool,
    pub best_effort: bool,
    pub no_hooks: bool,
    pub cmd: Vec<String>,
}

/// `writ run -- <agent>`: governed process wrap (mode B, wave-1 scope:
/// launch supervision — the wrapped process is itself a governed call, and
/// kernel enforcement status is reported honestly by `writ doctor`).
pub fn run(args: &RunArgs) -> Result<()> {
    let (policy, ledger, yolo, backend, cmd) = (
        args.policy.as_path(),
        args.ledger.as_path(),
        args.yolo,
        args.backend.as_str(),
        args.cmd.as_slice(),
    );
    // Not yet implemented: refuse rather than silently ignore a safety flag.
    if args.net != NetMode::Open
        || !args.allow_write.is_empty()
        || args.unconfined
        || args.best_effort
        || args.no_hooks
    {
        anyhow::bail!("writ run: --net/--allow-write/--unconfined/--best-effort/--no-hooks are not implemented in this build");
    }
    let engine = load_engine(policy, yolo)?;
    banner(&engine, policy, ledger);
    eprintln!("  mode: process wrap · backend: {backend} · per-tool coverage: run `writ doctor`");

    let mut store = writ_ledger::open_store(ledger).map_err(|e| anyhow!(e.to_string()))?;
    let session = format!("run-{}-{}", std::process::id(), Timestamp::now().epoch_ms());
    let cmdline = cmd.join(" ");
    let call = make_call(
        &session,
        0,
        "process.exec",
        serde_json::json!({ "command": cmdline }),
        InterceptMode::ProcessWrap,
    );

    let outcome = {
        let mut writer = LedgerWriter::new(&mut *store);
        handle_call(&call, &engine, &mut writer, &TuiApprover::new())
            .map_err(|e| anyhow!(e.to_string()))?
    };
    emit_span(ledger, &call, &outcome.verdict);

    eprintln!(
        "{}",
        render_call_line("process.exec", &cmdline, &outcome.verdict)
    );
    if let Some(note) = render_rule_note(&outcome.verdict) {
        eprintln!("{note}");
    }
    if !outcome.should_dispatch() {
        std::process::exit(126); // denied
    }

    // Dispatch: interactive stdio inheritance; no content capture by default
    // (spec §9). The execution record stores the exit status and a hash of
    // what was captured — which is intentionally nothing.
    let status = std::process::Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("spawn {}", cmd[0]))?;
    let code = status.code().unwrap_or(-1);

    let mut writer = LedgerWriter::new(&mut *store);
    writer
        .record_execution(&outcome.record, backend, code, &[])
        .map_err(|e| anyhow!(e.to_string()))?;
    eprintln!("  writ · run complete · exit {code} · recorded");
    std::process::exit(code);
}
