//! `writ run`: governed process wrap (mode B).
//!
//! 1. The launch itself is a governed call: one `process.exec` decision is
//!    evaluated and recorded before anything runs.
//! 2. The agent is launched interactively (the user's terminal, exit code
//!    propagated) inside the kernel boundary of `writ_sandbox::interactive`:
//!    writes confined to the workspace (cwd), a private temp dir, the
//!    agent's own state dirs from its built-in profile, the ledger dir (when
//!    hooks are on) and any `--allow-write` paths (package caches are only
//!    suggested, never granted by default); network open-and-unfiltered
//!    (default) or none. Default mode refuses when the kernel cannot
//!    enforce that (fail closed); `--best-effort` runs and reports gaps;
//!    `--unconfined` launches with no boundary, loudly labelled.
//! 3. For Claude Code, unless `--no-hooks`, writ's hooks are passed with
//!    `--settings <file>`, so every tool call is decided by `writ check`
//!    (which runs inside the boundary and so needs the ledger dir writable).

use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use writ_core::call::InterceptMode;
use writ_core::ledger::LedgerWriter;
use writ_core::pipeline::handle_call;
use writ_core::Timestamp;
use writ_sandbox::agents::{self, AgentProfile, EnvLookup, ProcessEnv};
use writ_sandbox::interactive::{self, Mode};
use writ_sandbox::{InteractiveReport, Net, Profile};
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

/// One writable path and why it is in the set (for the banner).
struct Grant {
    path: PathBuf,
    why: String,
}

fn absolute(p: &Path, cwd: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        cwd.join(p)
    }
}

/// `canonicalize` without Windows' `\\?\` prefix (matches the sandbox's
/// canonical form, so banner reasons line up with the report).
fn canonical(p: &Path) -> Option<PathBuf> {
    let c = p.canonicalize().ok()?;
    #[cfg(windows)]
    {
        let s = c.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            if !rest.starts_with("UNC\\") && rest.len() < 248 {
                return Some(PathBuf::from(rest));
            }
        }
    }
    Some(c)
}

fn push_grant(grants: &mut Vec<Grant>, path: &Path, why: impl Into<String>) {
    if let Some(c) = canonical(path) {
        if !grants.iter().any(|g| g.path == c) {
            grants.push(Grant {
                path: c,
                why: why.into(),
            });
        }
    }
}

/// How the hooks were wired, for the banner.
enum Hooks {
    On(PathBuf),
    Off,
    Unsupported,
}

/// The writ-owned control dir holding the Claude Code settings file. It is
/// deliberately NOT in the writable set: the agent cannot rewrite its own
/// hooks. Removed after the run.
struct ControlDir(PathBuf);

impl ControlDir {
    fn create() -> Result<Self> {
        let p = std::env::temp_dir().join(format!(
            "writ-run-ctl-{}-{}",
            std::process::id(),
            Timestamp::now().epoch_ms()
        ));
        std::fs::create_dir_all(&p).with_context(|| format!("create {}", p.display()))?;
        Ok(ControlDir(p))
    }
}

impl Drop for ControlDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn refusal_hint(net: Net) -> &'static str {
    if net == Net::None {
        "hint: `--net open` gives the agent the network (reported as not filtered); \
         `--best-effort` runs anyway and reports the gap"
    } else {
        "hint: `--best-effort` runs anyway and reports each gap; `--unconfined` launches with \
         no kernel boundary (launch supervision only)"
    }
}

/// `writ run -- <agent>`.
pub fn run(args: &RunArgs) -> Result<()> {
    let (policy, ledger, backend, cmd) = (
        args.policy.as_path(),
        args.ledger.as_path(),
        args.backend.as_str(),
        args.cmd.as_slice(),
    );
    if args.unconfined && (args.best_effort || args.net == NetMode::None) {
        bail!("writ run: --unconfined applies no boundary; it cannot be combined with --best-effort or --net none");
    }
    if args.unconfined && !args.allow_write.is_empty() {
        bail!("writ run: --allow-write has no effect with --unconfined (everything is writable)");
    }
    if !args.unconfined && backend != "local-os" {
        bail!("writ run: backend {backend} cannot host an interactive agent; only local-os can (or pass --unconfined)");
    }
    let net = match args.net {
        NetMode::Open => Net::Open,
        NetMode::None => Net::None,
    };
    let mode = if args.unconfined {
        Mode::Unconfined
    } else if args.best_effort {
        Mode::BestEffort
    } else {
        Mode::Required
    };
    let cwd = std::env::current_dir().context("current directory")?;
    let profile = agents::agent_profile(&cmd[0], &ProcessEnv);
    let hooks_wanted = profile.claude_code_hooks && !args.no_hooks;
    if hooks_wanted
        && cmd[1..]
            .iter()
            .any(|a| a == "--settings" || a.starts_with("--settings="))
    {
        bail!(
            "writ run passes its Claude Code hooks with --settings; your command already has \
             --settings (Claude Code would keep only one). Move those settings into \
             .claude/settings.json, or pass --no-hooks"
        );
    }
    // Fail fast, before a human is asked to approve a launch that cannot
    // happen.
    if let Err(e) = interactive::preflight(net, mode) {
        bail!("writ run: {e}\n  {}", refusal_hint(net));
    }

    let engine = load_engine(policy, args.yolo)?;
    banner(&engine, policy, ledger);

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

    // The writable set (not needed, and not created, when unconfined).
    let mut grants: Vec<Grant> = Vec::new();
    push_grant(&mut grants, &cwd, "workspace");
    if mode != Mode::Unconfined {
        let made = agents::materialize(&profile.paths).context("create the agent's state dirs")?;
        for p in &made {
            push_grant(
                &mut grants,
                &p.path,
                format!("{} — {}", profile.display, p.why),
            );
        }
        for p in &args.allow_write {
            let abs = absolute(p, &cwd);
            if canonical(&abs).is_none() {
                bail!(
                    "writ run: --allow-write {} does not exist (fail closed)",
                    p.display()
                );
            }
            push_grant(&mut grants, &abs, "--allow-write");
        }
    }

    // Per-tool-call hooks for Claude Code.
    let _control;
    let mut argv: Vec<String> = cmd[1..].to_vec();
    let hooks = if hooks_wanted {
        let writ_exe = std::env::current_exe().context("locate the writ binary")?;
        let policy_abs = canonical(policy).unwrap_or_else(|| absolute(policy, &cwd));
        let ledger_abs = absolute(ledger, &cwd);
        let ledger_dir = ledger_abs
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| cwd.clone());
        std::fs::create_dir_all(&ledger_dir)
            .with_context(|| format!("create {}", ledger_dir.display()))?;
        // `writ check` runs as the agent's child, inside the boundary: it
        // must append to the ledger and take `<ledger>.lock`.
        if mode != Mode::Unconfined {
            push_grant(&mut grants, &ledger_dir, "writ ledger (hooks append here)");
        }
        let mut settings =
            crate::integrate::claude_code_hook_settings(&writ_exe, &policy_abs, &ledger_abs);
        // Claude Code honours the `disableAllHooks` value left after
        // settings precedence, and `--settings` outranks local, project and
        // user settings: pinning `false` here means an agent that writes
        // `disableAllHooks: true` into those files cannot switch writ's
        // hooks off (only managed settings could).
        if let Some(obj) = settings.as_object_mut() {
            obj.insert("disableAllHooks".into(), serde_json::Value::Bool(false));
        }
        let control = ControlDir::create()?;
        let file = control.0.join("claude-settings.json");
        std::fs::write(&file, serde_json::to_vec_pretty(&settings)?)
            .with_context(|| format!("write {}", file.display()))?;
        argv.splice(0..0, ["--settings".to_string(), file.display().to_string()]);
        _control = Some(control);
        Hooks::On(file)
    } else {
        _control = None;
        if profile.claude_code_hooks {
            Hooks::Off
        } else {
            Hooks::Unsupported
        }
    };

    let mut protect = profile.protect.clone();
    protect.extend(profile.workspace_protect.iter().map(|rel| cwd.join(rel)));
    let sandbox_profile = Profile {
        workspace: cwd.clone(),
        writable: grants.iter().skip(1).map(|g| g.path.clone()).collect(),
        protect,
        net,
        mode,
    };

    // Windows: say which paths get a persistent Low integrity label BEFORE
    // anything is labelled.
    let all: Vec<PathBuf> = grants.iter().map(|g| g.path.clone()).collect();
    let to_label =
        interactive::persistent_labels(&all, mode).map_err(|e| anyhow!("writ run: {e}"))?;
    if !to_label.is_empty() {
        eprintln!(
            "\x1b[33m  writ · these paths get a Low integrity label that PERSISTS after the run \
             (any Low integrity process of yours can then write there):\x1b[0m"
        );
        for p in &to_label {
            eprintln!("      {}", p.display());
        }
        eprintln!(
            "    undo later with: icacls \"<path>\" /setintegritylevel (OI)(CI)M   (add /T to \
             reset files the agent created)"
        );
    }

    let (child, report) =
        writ_sandbox::spawn_interactive(&cmd[0], &argv, &profile.env, &sandbox_profile)
            .map_err(|e| anyhow!("writ run: {e}\n  {}", refusal_hint(net)))?;
    print_boundary(&report, &grants, &hooks, &profile);
    if profile.suggest_caches && mode != Mode::Unconfined {
        let existing = agents::materialize(&agents::tool_caches(&ProcessEnv)).unwrap_or_default();
        let missing: Vec<_> = existing
            .iter()
            .filter(|c| canonical(&c.path).is_some_and(|p| !report.writable.contains(&p)))
            .collect();
        if !missing.is_empty() {
            eprintln!("    caches     : not writable (opt in if the agent's tools need them):");
            for c in missing {
                eprintln!(
                    "                 --allow-write {}  ({})",
                    c.path.display(),
                    c.why
                );
            }
        }
    }

    // No content capture by default (spec §9): the execution record stores
    // the exit status and a hash of what was captured — intentionally
    // nothing.
    let code = child.wait().map_err(|e| anyhow!(e.to_string()))?;
    let backend_label = if mode == Mode::Unconfined {
        "none (unconfined)"
    } else {
        backend
    };
    let mut writer = LedgerWriter::new(&mut *store);
    writer
        .record_execution(&outcome.record, backend_label, code, &[])
        .map_err(|e| anyhow!(e.to_string()))?;
    eprintln!("  writ · run complete · exit {code} · recorded");
    drop(_control);
    std::process::exit(code);
}

/// State exactly what is enforced (stderr; stdout belongs to the agent).
fn print_boundary(r: &InteractiveReport, grants: &[Grant], hooks: &Hooks, p: &AgentProfile) {
    let home = if cfg!(windows) {
        ProcessEnv.get("USERPROFILE")
    } else {
        ProcessEnv.get("HOME")
    }
    .and_then(|h| canonical(Path::new(&h)));
    let show = |path: &Path| agents::display_path(path, home.as_deref());
    if r.mode == Mode::Unconfined {
        eprintln!(
            "\x1b[31m  ⚠ UNCONFINED: no kernel boundary — the agent can write anywhere you can \
             and reach any host. Only the launch is governed.\x1b[0m"
        );
    } else {
        eprintln!("  writ · kernel boundary: {}", r.mechanism);
    }
    eprintln!("    profile    : {} ({})", p.name, p.display);
    eprintln!("    filesystem : {}", r.filesystem_line());
    eprintln!("    network    : {}", r.network_line());
    if r.mode != Mode::Unconfined {
        let mut first = true;
        let mut line = |text: String| {
            eprintln!(
                "    {} {text}",
                if first {
                    "writable   :"
                } else {
                    "            "
                }
            );
            first = false;
        };
        for w in &r.writable {
            let why = if *w == r.temp_dir {
                "private temp (TMPDIR/TEMP/TMP), removed after the run".to_string()
            } else {
                grants
                    .iter()
                    .find(|g| g.path == *w)
                    .map(|g| g.why.clone())
                    .unwrap_or_default()
            };
            line(format!("{}  ({why})", show(w)));
        }
    }
    match hooks {
        Hooks::On(file) => {
            eprintln!(
                "    hooks      : on — every Claude Code tool call is decided by `writ check` \
                 (--settings {}, outside the writable set; disableAllHooks pinned false, which \
                 outranks user/project/local settings)",
                file.display()
            );
            eprintln!(
                "                 not covered: managed settings (e.g. allowManagedHooksOnly) and \
                 a nested `claude` the agent starts itself (still inside this boundary)"
            );
        }
        Hooks::Off => eprintln!("    hooks      : off (--no-hooks) — only the launch is governed"),
        Hooks::Unsupported => eprintln!(
            "    hooks      : none — writ has no tool-call hook for this agent; only the launch \
             is governed"
        ),
    }
    if !r.protected.is_empty() && r.mode != Mode::Unconfined {
        let shown: Vec<String> = r.protected.iter().map(|p| show(p)).collect();
        match r.protection {
            writ_sandbox::Level::Enforced => eprintln!(
                "    protected  : {} — kernel-enforced read-only (no write, delete or replace)",
                shown.join(", ")
            ),
            _ => eprintln!(
                "    protected  : NO — the agent CAN rewrite {} (hooks planted there would run \
                 in later sessions outside writ); this kernel cannot exclude a file inside a \
                 writable directory",
                shown.join(", ")
            ),
        }
    }
    if !r.labelled.is_empty() {
        let shown: Vec<String> = r.labelled.iter().map(|p| show(p)).collect();
        eprintln!(
            "    labelled   : {} (Low integrity, persists; undo: icacls \"<path>\" \
             /setintegritylevel (OI)(CI)M)",
            shown.join(", ")
        );
    }
    for n in r.notes.iter().filter(|n| !n.starts_with("NOT protected")) {
        eprintln!("    note       : {n}");
    }
}
