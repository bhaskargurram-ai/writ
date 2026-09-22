//! `local-os` backend — the default workstation sandbox (spec §8).
//!
//! Wave 1: process-level containment done portably (clean environment,
//! workspace-bound cwd, timeouts, dead-lock-free output capture via reader
//! threads). Kernel-level enforcement (Landlock/seccomp on Linux, Seatbelt
//! on macOS, restricted tokens on Windows) is cfg-gated in `platform` and
//! reports itself honestly as not-yet-enforced — see docs/THREAT_MODEL.md.
//! Overclaiming what a sandbox enforces is the cardinal sin here.

use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use writ_core::error::{Result, WritError};
use writ_core::sandbox::{
    Artifacts, ExecOutput, ExecRequest, SandboxBackend, SandboxId, SandboxSpec,
};

/// Minimal OS baseline a child process needs to function (Windows refuses to
/// start cmd.exe without SystemRoot, PATHEXT, etc.). Least privilege means
/// "nothing but the baseline and what the run declares" — not "nothing at
/// all", which is just broken.
fn baseline_env() -> std::collections::BTreeMap<String, String> {
    #[cfg(windows)]
    const KEYS: &[&str] = &[
        "SystemRoot",
        "windir",
        "COMSPEC",
        "PATHEXT",
        "SystemDrive",
        "PATH",
        "TEMP",
        "TMP",
    ];
    #[cfg(not(windows))]
    const KEYS: &[&str] = &["PATH", "HOME", "LANG", "TERM"];
    KEYS.iter()
        .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
        .collect()
}

pub struct LocalOsBackend {
    specs: HashMap<SandboxId, SandboxSpec>,
    next_id: u64,
}

impl Default for LocalOsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalOsBackend {
    pub fn new() -> Self {
        LocalOsBackend {
            specs: HashMap::new(),
            next_id: 0,
        }
    }

    /// Reject an ExecRequest whose cwd escapes the sandbox workspace.
    /// Canonicalize both sides so `..` tricks fail closed.
    fn check_cwd(spec: &SandboxSpec, req: &ExecRequest) -> Result<()> {
        let Some(cwd) = &req.cwd else { return Ok(()) };
        let ws = spec.workspace.canonicalize().map_err(|e| {
            WritError::Sandbox(format!("workspace not canonicalizable: {e}"))
        })?;
        let target = cwd.canonicalize().map_err(|e| {
            WritError::Sandbox(format!(
                "cwd {} not canonicalizable (fail closed): {e}",
                cwd.display()
            ))
        })?;
        if !target.starts_with(&ws) {
            return Err(WritError::Sandbox(format!(
                "cwd {} escapes workspace {}",
                target.display(),
                ws.display()
            )));
        }
        Ok(())
    }
}

impl SandboxBackend for LocalOsBackend {
    fn name(&self) -> &'static str {
        "local-os"
    }

    fn prepare(&mut self, spec: &SandboxSpec) -> Result<SandboxId> {
        std::fs::create_dir_all(&spec.workspace)?;
        self.next_id += 1;
        let id = SandboxId(format!("local-{}-{}", std::process::id(), self.next_id));
        self.specs.insert(id.clone(), spec.clone());
        Ok(id)
    }

    fn exec(&mut self, id: &SandboxId, req: &ExecRequest) -> Result<ExecOutput> {
        let spec = self
            .specs
            .get(id)
            .ok_or_else(|| WritError::Sandbox(format!("unknown sandbox {id:?}")))?
            .clone();
        Self::check_cwd(&spec, req)?;

        let mut cmd = Command::new(&req.program);
        cmd.args(&req.args)
            // Least privilege: OS baseline (programs must resolve and start)
            // plus what the run explicitly declares — nothing else.
            .env_clear()
            .envs(baseline_env())
            .envs(&spec.env)
            .envs(&req.env)
            .current_dir(req.cwd.clone().unwrap_or_else(|| spec.workspace.clone()))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let start = Instant::now();
        let mut child = cmd.spawn().map_err(|e| {
            WritError::Sandbox(format!("spawn {} failed: {e}", req.program))
        })?;

        // Drain pipes on reader threads: a chatty child must never deadlock
        // against a full pipe buffer while we poll try_wait.
        let mut out_pipe = child.stdout.take().expect("piped stdout");
        let mut err_pipe = child.stderr.take().expect("piped stderr");
        let t_out = thread::spawn(move || {
            let mut b = Vec::new();
            let _ = out_pipe.read_to_end(&mut b);
            b
        });
        let t_err = thread::spawn(move || {
            let mut b = Vec::new();
            let _ = err_pipe.read_to_end(&mut b);
            b
        });

        let timeout = Duration::from_millis(req.timeout_ms.unwrap_or(600_000));
        let status = loop {
            match child.try_wait()? {
                Some(s) => break s,
                None if start.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = t_out.join();
                    let _ = t_err.join();
                    return Err(WritError::Sandbox(format!(
                        "execution timed out after {}ms and was killed",
                        timeout.as_millis()
                    )));
                }
                None => thread::sleep(Duration::from_millis(5)),
            }
        };

        Ok(ExecOutput {
            stdout: t_out.join().unwrap_or_default(),
            stderr: t_err.join().unwrap_or_default(),
            exit_code: status.code().unwrap_or(-1),
            duration_ms: start.elapsed().as_millis() as u64,
        })
    }

    fn collect(&mut self, _id: &SandboxId) -> Result<Artifacts> {
        // Wave 1: file-diff collection is best-effort and not yet implemented.
        Ok(Artifacts::default())
    }

    fn teardown(&mut self, id: SandboxId) -> Result<()> {
        self.specs.remove(&id);
        Ok(())
    }
}
