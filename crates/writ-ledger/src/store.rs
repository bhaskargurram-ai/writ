//! Store selection: which backend holds the ledger at a given path.
//!
//! Detection is content-first, extension-second, and fails closed:
//!
//! - a file that starts with the 16-byte SQLite header is a SQLite ledger,
//!   whatever it is called;
//! - a non-empty file without that header is a JSONL ledger — unless its
//!   extension claims SQLite (`.db`, `.sqlite`, `.sqlite3`), which is
//!   ambiguous and therefore an error rather than a guess;
//! - a missing or empty file is decided by extension (SQLite for the three
//!   extensions above, JSONL otherwise), so `open_store` can create it.
//!
//! Without the `sqlite` cargo feature, a SQLite ledger is reported as an
//! error — it is never parsed as JSONL.

use std::fs::File;
use std::io::{ErrorKind, Read};
use std::path::Path;

use writ_core::error::{Result, WritError};
use writ_core::ledger::{LedgerRecord, LedgerStore};

use crate::file_store::{record_iter, FileLedgerStore};

/// The 16-byte header every SQLite 3 database file starts with.
pub(crate) const SQLITE_MAGIC: &[u8; 16] = b"SQLite format 3\0";

/// Which backend a ledger path refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreKind {
    /// [`FileLedgerStore`]: one JSON record per line (ADR-005 default).
    Jsonl,
    /// `SqliteLedgerStore` (WAL mode; needs the `sqlite` feature to open).
    Sqlite,
}

fn kind_by_extension(path: &Path) -> StoreKind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("db" | "sqlite" | "sqlite3") => StoreKind::Sqlite,
        _ => StoreKind::Jsonl,
    }
}

/// Decide which backend holds (or will hold) the ledger at `path`.
/// See the module docs for the rules.
pub fn detect_store_kind(path: impl AsRef<Path>) -> Result<StoreKind> {
    let path = path.as_ref();
    let mut head = Vec::with_capacity(SQLITE_MAGIC.len());
    match File::open(path) {
        Ok(f) => {
            f.take(SQLITE_MAGIC.len() as u64).read_to_end(&mut head)?;
        }
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(kind_by_extension(path)),
        Err(e) => return Err(e.into()),
    }
    if head.as_slice() == SQLITE_MAGIC {
        return Ok(StoreKind::Sqlite);
    }
    if head.is_empty() {
        return Ok(kind_by_extension(path));
    }
    if kind_by_extension(path) == StoreKind::Sqlite {
        return Err(WritError::Ledger(format!(
            "ledger {path:?} has a SQLite file extension but no SQLite header; \
             refusing to guess its format"
        )));
    }
    Ok(StoreKind::Jsonl)
}

#[cfg(not(feature = "sqlite"))]
pub(crate) fn sqlite_unavailable(path: &Path) -> WritError {
    WritError::Ledger(format!(
        "ledger {path:?} is a SQLite ledger, but this build of writ-ledger was \
         compiled without the `sqlite` feature"
    ))
}

/// Open (creating if absent) the ledger at `path` with the backend chosen
/// by [`detect_store_kind`].
pub fn open_store(path: impl AsRef<Path>) -> Result<Box<dyn LedgerStore>> {
    let path = path.as_ref();
    match detect_store_kind(path)? {
        StoreKind::Jsonl => Ok(Box::new(FileLedgerStore::open(path)?)),
        #[cfg(feature = "sqlite")]
        StoreKind::Sqlite => Ok(Box::new(crate::sqlite::SqliteLedgerStore::open(path)?)),
        #[cfg(not(feature = "sqlite"))]
        StoreKind::Sqlite => Err(sqlite_unavailable(path)),
    }
}

/// Read-only record stream over an existing ledger, in ledger order. Does
/// not create, repair or lock anything; used by `verify` and the query
/// helpers.
pub(crate) fn read_records(path: &Path) -> Result<Box<dyn Iterator<Item = Result<LedgerRecord>>>> {
    match detect_store_kind(path)? {
        StoreKind::Jsonl => Ok(Box::new(record_iter(File::open(path)?))),
        #[cfg(feature = "sqlite")]
        StoreKind::Sqlite => Ok(Box::new(crate::sqlite::read_only_records(path)?)),
        #[cfg(not(feature = "sqlite"))]
        StoreKind::Sqlite => Err(sqlite_unavailable(path)),
    }
}
