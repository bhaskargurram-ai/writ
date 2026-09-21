//! Read-only query helpers for the CLI (`writ log`, `writ show`).
//!
//! All helpers stream the JSONL file line by line; none load the ledger
//! into memory beyond the records they return.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

use serde::{Deserialize, Serialize};
use writ_core::approver::ApproverKind;
use writ_core::error::Result;
use writ_core::ledger::LedgerRecord;
use writ_core::verdict::Verdict;

use crate::file_store::record_iter;

/// Per-session rollup for `writ log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    /// Total ledger records (decisions + executions) in the session.
    pub records: u64,
    /// Decision records whose verdict was `Deny`.
    pub denied: u64,
    /// True if any record carries a human approver (TUI or RBAC; an
    /// out-of-band webhook is automation, not a human at the keyboard).
    pub approved_by_human: bool,
}

/// Summarize every session in the ledger at `path`, in first-seen order.
pub fn sessions(path: impl AsRef<Path>) -> Result<Vec<SessionSummary>> {
    let file = File::open(path.as_ref())?;
    let mut order: Vec<String> = Vec::new();
    let mut by_id: HashMap<String, SessionSummary> = HashMap::new();
    for item in record_iter(file) {
        let rec = item?;
        let summary = by_id.entry(rec.session_id.clone()).or_insert_with(|| {
            order.push(rec.session_id.clone());
            SessionSummary {
                session_id: rec.session_id.clone(),
                records: 0,
                denied: 0,
                approved_by_human: false,
            }
        });
        summary.records += 1;
        if matches!(rec.verdict, Some(Verdict::Deny { .. })) {
            summary.denied += 1;
        }
        if let Some(approver) = &rec.approver {
            if matches!(approver.kind, ApproverKind::Tui | ApproverKind::Rbac) {
                summary.approved_by_human = true;
            }
        }
    }
    Ok(order
        .into_iter()
        .map(|id| by_id.remove(&id).expect("inserted above"))
        .collect())
}

/// All records for one call — normally the `Decision` plus its linked
/// `Execution` (ADR-003) — in ledger order.
pub fn find_by_call_id(
    path: impl AsRef<Path>,
    call_id: &str,
) -> Result<Vec<LedgerRecord>> {
    let file = File::open(path.as_ref())?;
    let mut found = Vec::new();
    for item in record_iter(file) {
        let rec = item?;
        if rec.call_id == call_id {
            found.push(rec);
        }
    }
    Ok(found)
}
