//! writ-ledger — the tamper-evident provenance ledger store (Contract 3).
//!
//! Append-only hash-chained provenance ledger with verify, receipts and
//! anchors. Implements the schema frozen in `writ_core::ledger`. Two
//! stores, one evidence format:
//!
//! - [`FileLedgerStore`] (ADR-005 default, every platform): one JSONL file,
//!   one serialized `LedgerRecord` per line, durable on append (`flush` +
//!   `sync_data`), crash-tolerant on open (a torn final line is ignored;
//!   anything worse is reported, never silently dropped).
//! - `SqliteLedgerStore` (`sqlite` cargo feature): the same serialized
//!   records, one per row, in a WAL-mode SQLite database with transactional
//!   append. See the `sqlite` module docs for schema and concurrency.
//!
//! Hashes cover the record, not the container, so both stores pass the same
//! `writ_core::verify_chain` suite and a record moved between them verifies
//! identically. [`verify()`], [`sessions`] and [`find_by_call_id`] take a path
//! and pick the store with [`detect_store_kind`]; [`open_store`] does the
//! same for writers.
//!
//! No content is captured beyond what `LedgerRecord` already holds, and
//! this crate emits zero telemetry (Contract 3 invariants).

#![forbid(unsafe_code)]

pub mod file_store;
pub mod query;
#[cfg(feature = "sqlite")]
pub mod sqlite;
pub mod store;
pub mod verify;

pub use file_store::FileLedgerStore;
pub use query::{find_by_call_id, find_by_call_id_in, sessions, sessions_in, SessionSummary};
#[cfg(feature = "sqlite")]
pub use sqlite::{verify_sqlite, SqliteLedgerStore};
pub use store::{detect_store_kind, open_store, StoreKind};
pub use verify::{verify, verify_jsonl};
