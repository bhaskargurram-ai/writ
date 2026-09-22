//! Command implementations. Every path honours the core invariants:
//! exactly one decision record per call, dispatch only after the record is
//! durable, fail closed on any ambiguity.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::rc::Rc;

use anyhow::{anyhow, bail, Context, Result};
use writ_core::approver::FailClosedApprover;
use writ_core::call::{CallerIdentity, InterceptMode, ToolCall};
use writ_core::ledger::{LedgerRecord, LedgerStore, LedgerWriter};
use writ_core::pipeline::handle_call;
use writ_core::verdict::Verdict;
use writ_core::{PolicyEngine, Timestamp};
use writ_ledger::{FileLedgerStore, SessionSummary};
use writ_mcp::{stdio_transport, CredentialStore, McpProxy, ProxyConfig, ProxyDecision};
use writ_policy::NativePolicyEngine;
use writ_tui::{render_call_line, render_rule_note, TuiApprover};

fn load_engine(policy_path: &Path, yolo: bool) -> Result<NativePolicyEngine> {
    if !policy_path.exists() {
        bail!(
            "no policy file at {} — create one (see examples/writ.yaml) or pass --policy",
            policy_path.display()
        );
    }
    let mut source = std::fs::read_to_string(policy_path)
        .with_context(|| format!("read {}", policy_path.display()))?;
    if yolo {
        eprintln!("\x1b[31m⚠ --yolo: policy default flipped to ALLOW. Nothing will ask.\x1b[0m");
        source = source.replacen("default: ask", "default: allow", 1);
    }
    NativePolicyEngine::from_source(&source).map_err(|e| anyhow!(e.to_string()))
}

fn banner(engine: &NativePolicyEngine, policy: &Path, ledger: &Path) {
    eprintln!(
        "  writ · {} rules loaded from {} · ledger: {}",
        engine.rule_count(),
        policy.display(),
        ledger.display()
    );
}

fn make_call(session: &str, seq: u64, tool: &str, args: serde_json::Value, mode: InterceptMode) -> ToolCall {
    ToolCall {
        call_id: format!("{session}-{seq}"),
        session_id: session.to_string(),
        caller: CallerIdentity {
            agent: std::env::var("WRIT_AGENT").unwrap_or_else(|_| "unknown-agent".into()),
            agent_version: None,
            user: std::env::var("USER").or_else(|_| std::env::var("USERNAME")).ok(),
            non_human_id: None,
        },
        mode,
        tool: tool.to_string(),
        args,
        server: None,
        trust: None,
        captured_at: Timestamp::now(),
    }
}

/// `writ run -- <agent>`: governed process wrap (mode B, wave-1 scope:
/// launch supervision — the wrapped process is itself a governed call, and
/// kernel enforcement status is reported honestly by `writ doctor`).
pub fn run(policy: &Path, ledger: &Path, yolo: bool, backend: &str, cmd: &[String]) -> Result<()> {
    let engine = load_engine(policy, yolo)?;
    banner(&engine, policy, ledger);
    eprintln!("  mode: process wrap · backend: {backend} · per-tool coverage: run `writ doctor`");

    let mut store = FileLedgerStore::open(ledger).map_err(|e| anyhow!(e.to_string()))?;
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
        let mut writer = LedgerWriter::new(&mut store);
        handle_call(&call, &engine, &mut writer, &TuiApprover::new())
            .map_err(|e| anyhow!(e.to_string()))?
    };

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

    let mut writer = LedgerWriter::new(&mut store);
    writer
        .record_execution(&outcome.record, backend, code, &[])
        .map_err(|e| anyhow!(e.to_string()))?;
    eprintln!("  writ · run complete · exit {code} · recorded");
    std::process::exit(code);
}
/// `writ proxy --mcp --server <name> -- <cmd>`: real interception (mode A).
/// stdout is the JSON-RPC channel, so all human output goes to stderr.
/// `ask` fails closed here (headless) — interactive approval needs the TUI
/// in `writ run` or an out-of-band approver (wave 2+).
pub fn proxy(
    policy: &Path,
    ledger: &Path,
    yolo: bool,
    mcp: bool,
    server: &str,
    cmd: &[String],
) -> Result<()> {
    if !mcp {
        bail!("only --mcp is supported in this build (SSE/HTTP: wave 2)");
    }
    let engine = load_engine(policy, yolo)?;
    banner(&engine, policy, ledger);
    eprintln!("  mode: mcp proxy · downstream server: {server} · cmd: {}", cmd.join(" "));

    let mut creds = CredentialStore::new();
    creds.load_from_env(server);
    if creds.has_any(server) {
        eprintln!("  credentials: injected at dispatch for '{server}' (agent never sees them)");
    }

    let spawned = writ_mcp::spawn_stdio_server(
        &cmd[0],
        &cmd[1..],
        server,
        &creds,
        &BTreeMap::new(),
    )
    .map_err(|e| anyhow!(e.to_string()))?;

    let store = Rc::new(RefCell::new(
        FileLedgerStore::open(ledger).map_err(|e| anyhow!(e.to_string()))?,
    ));
    let decisions: Rc<RefCell<HashMap<String, LedgerRecord>>> =
        Rc::new(RefCell::new(HashMap::new()));

    // Decision hook: full pipeline per call — evaluate, approve-or-fail-closed,
    // record. Forward only on allow/redact.
    let (store_d, decisions_d, engine_d) = (Rc::clone(&store), Rc::clone(&decisions), engine);
    let decide = Box::new(move |call: &ToolCall| {
        let mut s = store_d.borrow_mut();
        let mut writer = LedgerWriter::new(&mut *s);
        match handle_call(call, &engine_d, &mut writer, &FailClosedApprover) {
            Ok(outcome) => {
                eprintln!(
                    "{}",
                    render_call_line(&call.tool, &summarize(&call.args), &outcome.verdict)
                );
                if let Some(note) = render_rule_note(&outcome.verdict) {
                    eprintln!("{note}");
                }
                if outcome.should_dispatch() {
                    decisions_d
                        .borrow_mut()
                        .insert(call.call_id.clone(), outcome.record);
                    ProxyDecision::Forward
                } else {
                    ProxyDecision::Refuse {
                        message: refusal_message(&outcome.verdict),
                    }
                }
            }
            Err(e) => ProxyDecision::Refuse {
                message: format!("writ internal error (fail closed): {e}"),
            },
        }
    }) as Box<dyn FnMut(&ToolCall) -> ProxyDecision>;

    // Observation hook: execution record with output hash for forwarded calls.
    let (store_o, decisions_o) = (Rc::clone(&store), Rc::clone(&decisions));
    let mut proxy = McpProxy::new(
        stdio_transport(),
        spawned.transport,
        ProxyConfig {
            server_name: server.to_string(),
            agent: std::env::var("WRIT_AGENT").unwrap_or_else(|_| "unknown-agent".into()),
            trust: None,
        },
        decide,
    );
    proxy.on_result(Box::new(move |call: &ToolCall, result: &serde_json::Value| {
        let Some(decision) = decisions_o.borrow().get(&call.call_id).cloned() else {
            return;
        };
        let bytes = serde_json::to_vec(result).unwrap_or_default();
        let mut s = store_o.borrow_mut();
        let mut writer = LedgerWriter::new(&mut *s);
        let _ = writer.record_execution(&decision, "mcp-proxy", 0, &bytes);
    }));

    proxy.run().map_err(|e| anyhow!(e.to_string()))?;
    eprintln!("  writ · session closed · ledger: {}", ledger.display());
    Ok(())
}

/// Structured refusal text (spec §12 voice: name the rule, give the reason).
fn refusal_message(v: &Verdict) -> String {
    match v {
        Verdict::Deny {
            rule_id,
            reason,
            location,
        } => format!(
            "writ denied this: rule \"{rule_id}\" — {reason} ({})",
            location.as_deref().unwrap_or("policy default")
        ),
        Verdict::Ask { rule_id, .. } => format!(
            "writ denied this: rule \"{rule_id}\" requires human approval; none available in this context (fail closed)"
        ),
        other => format!("writ denied this: {other:?}"),
    }
}

fn summarize(args: &serde_json::Value) -> String {
    let s = match args {
        serde_json::Value::Object(m) => m
            .values()
            .next()
            .map(|v| v.as_str().map(String::from).unwrap_or_else(|| v.to_string()))
            .unwrap_or_default(),
        other => other.to_string(),
    };
    let s = s.replace('\n', " ");
    if s.chars().count() > 72 {
        format!("{}…", s.chars().take(71).collect::<String>())
    } else {
        s
    }
}
/// `writ log` — the answer to "what did my agent do last night".
pub fn log(ledger: &Path) -> Result<()> {
    if !ledger.exists() {
        bail!("no ledger at {} — nothing recorded yet", ledger.display());
    }
    let sessions: Vec<SessionSummary> =
        writ_ledger::sessions(ledger).map_err(|e| anyhow!(e.to_string()))?;
    let total: u64 = sessions.iter().map(|s| s.records).sum();
    let denied: u64 = sessions.iter().map(|s| s.denied).sum();
    let human: u64 = sessions.iter().filter(|s| s.approved_by_human).count() as u64;
    println!(
        "{} sessions · {} records · {} denied · {} sessions with your approvals",
        sessions.len(),
        total,
        denied,
        human
    );
    println!("{:<34} {:>8} {:>8}  approved", "session", "records", "denied");
    for s in sessions {
        println!(
            "{:<34} {:>8} {:>8}  {}",
            s.session_id,
            s.records,
            s.denied,
            if s.approved_by_human { "yes" } else { "-" }
        );
    }
    Ok(())
}

/// `writ show <call-id>` — one decision, in full.
pub fn show(ledger: &Path, call_id: &str) -> Result<()> {
    let records =
        writ_ledger::find_by_call_id(ledger, call_id).map_err(|e| anyhow!(e.to_string()))?;
    if records.is_empty() {
        bail!("no records for call-id {call_id}");
    }
    for r in &records {
        println!("{}", serde_json::to_string_pretty(r)?);
    }
    Ok(())
}

/// `writ verify` — prove the ledger was not edited.
pub fn verify(ledger: &Path) -> Result<()> {
    if !ledger.exists() {
        bail!("no ledger at {}", ledger.display());
    }
    let report = writ_ledger::verify(ledger).map_err(|e| anyhow!(e.to_string()))?;
    if report.intact {
        println!("chain intact · {} records · no gaps", report.records);
        Ok(())
    } else {
        eprintln!(
            "chain BROKEN at record {} · {} records verified before the break",
            report.broken_at.unwrap_or(0),
            report.records
        );
        std::process::exit(1);
    }
}

/// `writ policy test` — unit-test rules against fixtures.
pub fn policy_test(policy: &Path, fixtures: Option<PathBuf>) -> Result<()> {
    let source = std::fs::read_to_string(policy)
        .with_context(|| format!("read {}", policy.display()))?;
    let dir = fixtures.unwrap_or_else(|| PathBuf::from("fixtures"));
    if !dir.is_dir() {
        bail!(
            "no fixture directory at {} — pass --fixtures (repo copy: crates/writ-policy/fixtures)",
            dir.display()
        );
    }
    let cases = writ_policy::load_fixtures_dir(&dir).map_err(|e| anyhow!(e.to_string()))?;
    let report = writ_policy::run_fixtures(&source, &cases);
    println!("{}", report.summary());
    if !report.is_pass() {
        std::process::exit(1);
    }
    Ok(())
}

/// `writ policy add <pack>` — install a community pack; print its checksum
/// so the operator can verify it (registry verification lands in wave 3).
pub fn policy_add(pack: &str) -> Result<()> {
    let search = [
        PathBuf::from(format!("packs/{pack}/pack.yaml")),
        PathBuf::from(format!("packs/{pack}.yaml")),
    ];
    let src = search
        .iter()
        .find(|p| p.exists())
        .ok_or_else(|| anyhow!("pack '{pack}' not found in ./packs (registry: wave 3)"))?;
    let bytes = std::fs::read(src)?;
    let dest_dir = PathBuf::from(".writ/packs");
    std::fs::create_dir_all(&dest_dir)?;
    let dest = dest_dir.join(format!("{pack}.yaml"));
    std::fs::write(&dest, &bytes)?;
    println!(
        "installed pack '{pack}' → {}\nsha256: {}\nverify this checksum against the pack registry before trusting it in production",
        dest.display(),
        LedgerRecord::hash_bytes(&bytes)
    );
    Ok(())
}
/// `writ doctor` — coverage honesty (spec §6): exactly which call paths are
/// governed on this machine and which are blind.
pub fn doctor(policy: &Path, ledger: &Path) -> Result<()> {
    println!("writ doctor · {}", env!("CARGO_PKG_VERSION"));
    println!();

    match std::fs::read_to_string(policy) {
        Ok(src) => match NativePolicyEngine::from_source(&src) {
            Ok(e) => println!(
                "policy    : {} ({} rules, default applies to unmatched calls)",
                policy.display(),
                e.rule_count()
            ),
            Err(e) => println!("policy    : {} FAILED TO COMPILE: {e}", policy.display()),
        },
        Err(_) => println!("policy    : none at {}", policy.display()),
    }

    if ledger.exists() {
        match writ_ledger::verify(ledger) {
            Ok(r) if r.intact => println!(
                "ledger    : {} · {} records · chain intact",
                ledger.display(),
                r.records
            ),
            Ok(r) => println!(
                "ledger    : {} · CHAIN BROKEN at record {}",
                ledger.display(),
                r.broken_at.unwrap_or(0)
            ),
            Err(e) => println!("ledger    : {} · unreadable: {e}", ledger.display()),
        }
    } else {
        println!("ledger    : none at {} yet", ledger.display());
    }
    println!();

    println!("sandbox backends:");
    for b in writ_sandbox::detect_backends() {
        println!(
            "  {:<14} {} — {}",
            b.name,
            if b.available { "available" } else { "not available" },
            b.notes
        );
    }
    let kh = writ_sandbox::kernel_hardening();
    println!();
    println!("kernel boundary ({}): {}", kh.platform, kh.mechanism);
    println!("  enforced: {} — {}", kh.enforced, kh.notes);
    println!();

    println!("interception coverage:");
    println!("  MCP proxy (mode A)     : ready — `writ proxy --mcp` governs every MCP tool call");
    println!("  process wrap (mode B)  : launch supervision only in this build; kernel floor NOT enforced (wave 2)");
    println!("  SDK hooks (mode C)     : wave 3 — LangGraph / OpenAI Agents SDK / Claude Agent SDK");
    println!();
    println!("\x1b[33mblind spots, stated plainly:\x1b[0m");
    println!("  · in MCP-proxy-only mode the agent's own shell, file writes and direct HTTP are NOT governed");
    println!("  · an agent can bypass Writ entirely unless you pair mode A with mode B or C");
    println!("  · see docs/THREAT_MODEL.md for the full residual-risk table");
    Ok(())
}
/// `writ report` — shareable single-file HTML run summary (spec §9).
pub fn report(ledger: &Path, out: &Path) -> Result<()> {
    if !ledger.exists() {
        bail!("no ledger at {}", ledger.display());
    }
    let store = FileLedgerStore::open(ledger).map_err(|e| anyhow!(e.to_string()))?;
    let mut rows = String::new();
    let (mut n_allow, mut n_deny, mut n_ask, mut n_redact) = (0u64, 0u64, 0u64, 0u64);
    for rec in store.iter() {
        let rec = rec.map_err(|e| anyhow!(e.to_string()))?;
        if rec.kind != writ_core::RecordKind::Decision {
            continue;
        }
        let (tool, summary) = rec
            .call
            .as_ref()
            .map(|c| (c.tool.clone(), summarize(&c.args)))
            .unwrap_or_else(|| ("?".into(), String::new()));
        let (kind, rule) = match &rec.verdict {
            Some(Verdict::Allow { rule_id }) => {
                n_allow += 1;
                ("allow", rule_id.clone().unwrap_or_else(|| "default".into()))
            }
            Some(Verdict::Deny { rule_id, .. }) => {
                n_deny += 1;
                ("deny", rule_id.clone())
            }
            Some(Verdict::Ask { rule_id, .. }) => {
                n_ask += 1;
                ("ask", rule_id.clone())
            }
            Some(Verdict::Redact { rule_id, .. }) => {
                n_redact += 1;
                ("redact", rule_id.clone())
            }
            None => continue,
        };
        rows.push_str(&format!(
            "<tr class=\"{kind}\"><td>{}</td><td>{}</td><td>{}</td><td><span class=\"badge {kind}\">{kind}</span></td><td>{}</td><td>{}</td></tr>\n",
            rec.index,
            esc(&rec.recorded_at.to_string()),
            esc(&tool),
            esc(&rule),
            esc(&summary),
        ));
    }
    let html = format!(
        include_str!("report_template.html"),
        n_allow, n_deny, n_ask, n_redact, rows
    );
    std::fs::write(out, html)?;
    println!(
        "wrote {} — open it anywhere, it is fully self-contained",
        out.display()
    );
    Ok(())
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}