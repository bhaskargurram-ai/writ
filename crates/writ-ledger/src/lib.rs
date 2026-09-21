//! writ-ledger — the tamper-evident provenance ledger store (Contract 3).
//!
//! Append-only hash-chained provenance ledger with verify, receipts and
//! anchors. Implements the schema frozen in `writ_core::ledger`. The
//! default store is [`FileLedgerStore`] (ADR-005): one JSONL file, one
//! serialized `LedgerRecord` per line, durable on append (`flush` +
//! `sync_data`), crash-tolerant on open (a torn final line is ignored;
//! anything worse is reported, never silently dropped).
//!
//! No content is captured beyond what `LedgerRecord` already holds, and
//! this crate emits zero telemetry (Contract 3 invariants).

#![forbid(unsafe_code)]

pub mod file_store;
pub mod query;
pub mod verify;

pub use file_store::FileLedgerStore;
pub use query::{find_by_call_id, sessions, SessionSummary};
pub use verify::verify;

/// Reserved for `SqliteLedgerStore` (WAL mode) — intentionally empty.
///
/// ADR-005: the JSONL [`FileLedgerStore`] is the default store on every
/// platform; the SQLite backend ships behind this feature gate once
/// packaging includes a C toolchain. Both backends must pass the same
/// `writ_core::verify_chain` suite — the store is swappable, the evidence
/// format is not.
#[cfg(feature = "sqlite")]
pub mod sqlite {}
