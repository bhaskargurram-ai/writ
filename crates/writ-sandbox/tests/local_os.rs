//! Acceptance tests for the local-os backend (plan: A8).

use std::collections::BTreeMap;
use writ_core::sandbox::{ExecRequest, SandboxBackend, SandboxSpec};
use writ_sandbox::{detect_backends, kernel_hardening, LocalOsBackend};

/// Std-only unique temp dir (no tempfile dep: windows-sys cannot build on
/// this environment's binutils — ADR-006). Auto-cleans on Drop.
struct TestDir(std::path::PathBuf);

impl TestDir {
    fn new() -> Self {
        let unique = format!(
            "writ-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let p = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&p).unwrap();
        TestDir(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spec() -> (TestDir, SandboxSpec) {
    let dir = TestDir::new();
    let spec = SandboxSpec {
        workspace: dir.path().to_path_buf(),
        allowed_hosts: vec![],
        env: BTreeMap::new(),
    };
    (dir, spec)
}

fn req(program: &str, args: Vec<String>, timeout_ms: u64) -> ExecRequest {
    ExecRequest {
        program: program.into(),
        args,
        cwd: None,
        env: BTreeMap::new(),
        timeout_ms: Some(timeout_ms),
    }
}

#[cfg(windows)]
fn echo_req(msg: &str) -> ExecRequest {
    req("cmd", vec!["/c".into(), format!("echo {msg}")], 10_000)
}

#[cfg(not(windows))]
fn echo_req(msg: &str) -> ExecRequest {
    req("sh", vec!["-c".into(), format!("echo {msg}")], 10_000)
}

#[test]
fn exec_captures_output_and_exit_code() {
    let (_d, s) = spec();
    let mut b = LocalOsBackend::new();
    let id = b.prepare(&s).unwrap();
    let out = b.exec(&id, &echo_req("hello-writ")).unwrap();
    assert_eq!(out.exit_code, 0);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("hello-writ"), "stdout was: {stdout}");
    b.teardown(id).unwrap();
}

#[test]
fn timeout_kills_sleeper() {
    let (_d, s) = spec();
    let mut b = LocalOsBackend::new();
    let id = b.prepare(&s).unwrap();
    #[cfg(windows)]
    let r = req(
        "ping",
        vec!["-n".into(), "30".into(), "127.0.0.1".into()],
        300,
    ); // ping: timeout refuses piped stdin
    #[cfg(not(windows))]
    let r = req("sh", vec!["-c".into(), "sleep 30".into()], 300);
    let err = b.exec(&id, &r).unwrap_err();
    assert!(err.to_string().contains("timed out"), "{err}");
}

#[test]
fn cwd_escape_is_rejected() {
    let (d, s) = spec();
    let mut b = LocalOsBackend::new();
    let id = b.prepare(&s).unwrap();
    let mut r = echo_req("x");
    r.cwd = Some(d.path().parent().unwrap().to_path_buf());
    let err = b.exec(&id, &r).unwrap_err();
    assert!(err.to_string().contains("escapes workspace"), "{err}");
}

#[test]
fn child_env_is_clean() {
    std::env::set_var("WRIT_TEST_LEAK", "1");
    let (_d, s) = spec();
    let mut b = LocalOsBackend::new();
    let id = b.prepare(&s).unwrap();
    #[cfg(windows)]
    let r = req(
        "cmd",
        vec!["/c".into(), "echo [%WRIT_TEST_LEAK%]".into()],
        10_000,
    );
    #[cfg(not(windows))]
    let r = req(
        "sh",
        vec!["-c".into(), "echo [$WRIT_TEST_LEAK]".into()],
        10_000,
    );
    let out = b.exec(&id, &r).unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("[1]"), "parent env leaked: {stdout}");
}

#[test]
fn detect_reports_local_os_and_kernel_honesty() {
    let backends = detect_backends();
    assert!(backends.iter().any(|b| b.name == "local-os" && b.available));
    assert!(backends.iter().all(|b| !b.notes.is_empty()));
    assert!(
        !kernel_hardening().enforced,
        "wave 1 must not claim kernel enforcement"
    );
}
