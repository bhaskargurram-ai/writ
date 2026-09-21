//! Acceptance tests for FileLedgerStore (Contract 3 / ADR-005).

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use writ_core::approver::{ApproverIdentity, ApproverKind};
use writ_core::error::WritError;
use writ_core::ledger::{LedgerStore, LedgerWriter, RecordKind, GENESIS_HASH};
use writ_core::{
    CallerIdentity, InterceptMode, Timestamp, ToolCall, Verdict,
};
use writ_ledger::{find_by_call_id, sessions, verify, FileLedgerStore};

/// Zero-dependency tempdir (this environment builds offline; no tempfile).
struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let id = COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "writ-ledger-test-{}-{}",
            std::process::id(),
            id
        ));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn call(call_id: &str, session_id: &str) -> ToolCall {
    ToolCall {
        call_id: call_id.into(),
        session_id: session_id.into(),
        caller: CallerIdentity {
            agent: "test-agent".into(),
            agent_version: None,
            user: None,
            non_human_id: None,
        },
        mode: InterceptMode::Mcp,
        tool: "bash".into(),
        args: serde_json::json!({"command": "ls -la"}),
        server: None,
        trust: None,
        captured_at: Timestamp::now(),
    }
}

fn allow() -> Verdict {
    Verdict::Allow {
        rule_id: Some("r-allow".into()),
    }
}

fn deny() -> Verdict {
    Verdict::Deny {
        rule_id: "r-deny".into(),
        reason: "destructive command".into(),
        location: Some("writ.yaml:14".into()),
    }
}

fn ledger_path(dir: &TempDir) -> PathBuf {
    // Nested path also exercises parent-dir creation in open().
    dir.path().join(".writ").join("ledger.jsonl")
}

/// Three records: decision(allow) + execution in s1, decision(deny) in s2.
fn populated(dir: &TempDir) -> PathBuf {
    let path = ledger_path(dir);
    let mut store = FileLedgerStore::open(&path).unwrap();
    let mut w = LedgerWriter::new(&mut store);
    let d = w.record_decision(&call("c1", "s1"), &allow(), None).unwrap();
    w.record_execution(&d, "local-os", 0, b"ok").unwrap();
    w.record_decision(&call("c2", "s2"), &deny(), None).unwrap();
    path
}

#[test]
fn roundtrip_append_iter_and_reopen() {
    let dir = TempDir::new();
    let path = populated(&dir);

    let mut store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 3);
    let tip = store.tip().unwrap().unwrap();
    assert_eq!(tip.index, 2);
    assert_eq!(tip.kind, RecordKind::Decision);

    let exec = store.get(1).unwrap().unwrap();
    assert_eq!(exec.kind, RecordKind::Execution);
    assert_eq!(exec.decision_index, Some(0));
    assert!(store.get(3).unwrap().is_none());

    let all: Vec<_> = store.iter().collect::<Result<_, _>>().unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].index, 0);
    assert_eq!(all[2].index, 2);
    // Chain links hold across a reopen.
    assert_eq!(all[1].prev_hash, all[0].record_hash);
    assert_eq!(all[2].prev_hash, all[1].record_hash);

    // Append continues seamlessly after reopen.
    let mut w = LedgerWriter::new(&mut store);
    w.record_decision(&call("c3", "s1"), &allow(), None).unwrap();
    assert_eq!(store.len(), 4);
    assert!(verify(&path).unwrap().intact);
}

#[test]
fn genesis_record_has_zero_prev_hash() {
    let dir = TempDir::new();
    let path = ledger_path(&dir);
    let mut store = FileLedgerStore::open(&path).unwrap();
    let mut w = LedgerWriter::new(&mut store);
    w.record_decision(&call("c1", "s1"), &allow(), None).unwrap();
    let first = store.get(0).unwrap().unwrap();
    assert_eq!(first.prev_hash, GENESIS_HASH);
    assert_eq!(first.prev_hash.len(), 64);
    assert!(first.prev_hash.chars().all(|c| c == '0'));
}

#[test]
fn rejects_out_of_order_append() {
    let dir = TempDir::new();
    let path = populated(&dir);
    let mut store = FileLedgerStore::open(&path).unwrap();
    let mut bad = store.tip().unwrap().unwrap();
    bad.index = 7; // len() is 3
    bad.record_hash = bad.compute_hash().unwrap();
    let err = store.append(&bad).unwrap_err();
    assert!(matches!(err, WritError::Ledger(_)), "got {err:?}");
    assert!(err.to_string().contains("index 7"));
    assert_eq!(store.len(), 3); // unchanged
}

#[test]
fn rejects_wrong_prev_hash() {
    let dir = TempDir::new();
    let path = populated(&dir);
    let mut store = FileLedgerStore::open(&path).unwrap();
    let mut bad = store.tip().unwrap().unwrap();
    bad.index = 3;
    bad.prev_hash = "f".repeat(64);
    bad.record_hash = bad.compute_hash().unwrap();
    let err = store.append(&bad).unwrap_err();
    assert!(matches!(err, WritError::Ledger(_)), "got {err:?}");
    assert!(err.to_string().contains("prev_hash"));
    assert_eq!(store.len(), 3);
}

#[test]
fn verify_reports_exact_index_on_valid_json_tamper() {
    let dir = TempDir::new();
    let path = populated(&dir);
    assert!(verify(&path).unwrap().intact);

    // Tamper with record 1 while keeping the line valid JSON: flip the
    // session_id. The stored record_hash no longer matches the payload.
    let mut lines: Vec<String> = {
        let mut s = String::new();
        std::fs::File::open(&path).unwrap().read_to_string(&mut s).unwrap();
        s.lines().map(|l| l.to_string()).collect()
    };
    let mut v: serde_json::Value = serde_json::from_str(&lines[1]).unwrap();
    v["session_id"] = serde_json::json!("evil-session");
    lines[1] = serde_json::to_string(&v).unwrap();
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();

    let report = verify(&path).unwrap();
    assert!(!report.intact);
    assert_eq!(report.broken_at, Some(1));
    assert_eq!(report.records, 1);
}

#[test]
fn verify_reports_exact_index_on_unparseable_tamper() {
    let dir = TempDir::new();
    let path = populated(&dir);

    // Flip a byte in the middle line so it is no longer valid JSON.
    let mut bytes = std::fs::read(&path).unwrap();
    let first_nl = bytes.iter().position(|&b| b == b'\n').unwrap();
    bytes[first_nl + 1] = b'X'; // first char of line 2 ('{' -> 'X')
    std::fs::write(&path, bytes).unwrap();

    let report = verify(&path).unwrap();
    assert!(!report.intact);
    assert_eq!(report.broken_at, Some(1));
    assert_eq!(report.records, 1);
}

#[test]
fn crash_tolerant_tail() {
    let dir = TempDir::new();
    let path = populated(&dir);

    // Simulate a crash mid-write: a torn final line.
    {
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"schema_version\":1,\"kind\":\"deci").unwrap();
        f.flush().unwrap();
    }

    // open() stops at the last valid record instead of failing.
    let mut store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 3);
    assert_eq!(store.tip().unwrap().unwrap().index, 2);

    // The torn tail was truncated at open, so the ledger keeps accepting
    // records and the chain still verifies end-to-end.
    let mut w = LedgerWriter::new(&mut store);
    w.record_decision(&call("c9", "s9"), &allow(), None).unwrap();
    assert_eq!(store.len(), 4);
    let report = verify(&path).unwrap();
    assert!(report.intact);
    assert_eq!(report.records, 4);

    // And a subsequent reopen sees all four valid records.
    let store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 4);
}

#[test]
fn mid_file_corruption_is_a_hard_error_on_open() {
    let dir = TempDir::new();
    let path = populated(&dir);
    let mut bytes = std::fs::read(&path).unwrap();
    let first_nl = bytes.iter().position(|&b| b == b'\n').unwrap();
    bytes[first_nl + 1] = b'X'; // corrupt line 2 (records 3+ follow)
    std::fs::write(&path, bytes).unwrap();

    let err = FileLedgerStore::open(&path).unwrap_err();
    assert!(matches!(err, WritError::Ledger(_)), "got {err:?}");
    assert!(err.to_string().contains("mid-file corruption"));
}

#[test]
fn sessions_and_find_by_call_id() {
    let dir = TempDir::new();
    let path = ledger_path(&dir);
    let human = ApproverIdentity {
        kind: ApproverKind::Tui,
        id: "bhaskar".into(),
    };
    {
        let mut store = FileLedgerStore::open(&path).unwrap();
        let mut w = LedgerWriter::new(&mut store);
        // s1: allowed + executed, plus a human-approved ask resolution.
        let d1 = w.record_decision(&call("c1", "s1"), &allow(), None).unwrap();
        w.record_execution(&d1, "local-os", 0, b"ok").unwrap();
        w.record_decision(&call("c2", "s1"), &allow(), Some(human)).unwrap();
        // s2: one denial.
        w.record_decision(&call("c3", "s2"), &deny(), None).unwrap();
    }

    let summaries = sessions(&path).unwrap();
    assert_eq!(summaries.len(), 2);
    let s1 = &summaries[0];
    assert_eq!(s1.session_id, "s1");
    assert_eq!(s1.records, 3);
    assert_eq!(s1.denied, 0);
    assert!(s1.approved_by_human);
    let s2 = &summaries[1];
    assert_eq!(s2.session_id, "s2");
    assert_eq!(s2.records, 1);
    assert_eq!(s2.denied, 1);
    assert!(!s2.approved_by_human);

    // find_by_call_id returns the decision and its linked execution.
    let recs = find_by_call_id(&path, "c1").unwrap();
    assert_eq!(recs.len(), 2);
    assert_eq!(recs[0].kind, RecordKind::Decision);
    assert_eq!(recs[1].kind, RecordKind::Execution);
    assert_eq!(recs[1].decision_index, Some(recs[0].index));
    assert!(find_by_call_id(&path, "nope").unwrap().is_empty());
}

#[test]
fn empty_ledger_verifies_intact() {
    let dir = TempDir::new();
    let path = ledger_path(&dir);
    let store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 0);
    assert!(store.tip().unwrap().is_none());
    let report = verify(&path).unwrap();
    assert!(report.intact);
    assert_eq!(report.records, 0);
    assert_eq!(report.broken_at, None);
}

