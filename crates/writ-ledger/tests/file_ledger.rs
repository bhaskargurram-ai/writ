//! Acceptance tests for FileLedgerStore (Contract 3 / ADR-005): the shared
//! store-generic suite plus JSONL-specific crash-tolerance tests.

#[macro_use]
mod common;

use std::fs::OpenOptions;
use std::io::Write;

use common::{allow, call, populated, FileBackend, TempDir};
use writ_core::ledger::{LedgerStore, LedgerWriter};
use writ_ledger::{detect_store_kind, verify, FileLedgerStore, StoreKind};

ledger_suite!(FileBackend);

#[test]
fn crash_tolerant_tail() {
    let dir = TempDir::new();
    let path = populated::<FileBackend>(&dir);

    // Simulate a crash mid-write: a torn final line.
    {
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"schema_version\":1,\"kind\":\"deci")
            .unwrap();
        f.flush().unwrap();
    }

    // open() stops at the last valid record instead of failing.
    let mut store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 3);
    assert_eq!(store.tip().unwrap().unwrap().index, 2);

    // The torn tail was truncated at open, so the ledger keeps accepting
    // records and the chain still verifies end-to-end.
    let mut w = LedgerWriter::new(&mut store);
    w.record_decision(&call("c9", "s9"), &allow(), None)
        .unwrap();
    assert_eq!(store.len(), 4);
    let report = verify(&path).unwrap();
    assert!(report.intact);
    assert_eq!(report.records, 4);

    // And a subsequent reopen sees all four valid records.
    let store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 4);
}

#[test]
fn store_kind_detection() {
    let dir = TempDir::new();
    let jsonl = populated::<FileBackend>(&dir);
    assert_eq!(detect_store_kind(&jsonl).unwrap(), StoreKind::Jsonl);
    // Missing files are decided by extension.
    for (name, kind) in [
        ("x.jsonl", StoreKind::Jsonl),
        ("x.log", StoreKind::Jsonl),
        ("x.db", StoreKind::Sqlite),
        ("x.SQLite", StoreKind::Sqlite),
        ("x.sqlite3", StoreKind::Sqlite),
    ] {
        assert_eq!(detect_store_kind(dir.path().join(name)).unwrap(), kind);
    }
    // Content wins over the name: a SQLite header is SQLite whatever the
    // file is called.
    let disguised = dir.path().join("disguised.jsonl");
    std::fs::write(&disguised, b"SQLite format 3\0rest-of-header").unwrap();
    assert_eq!(detect_store_kind(&disguised).unwrap(), StoreKind::Sqlite);
}

#[test]
fn jsonl_content_under_a_sqlite_name_is_refused() {
    let dir = TempDir::new();
    let jsonl = populated::<FileBackend>(&dir);
    let misnamed = dir.path().join("ledger.db");
    std::fs::copy(&jsonl, &misnamed).unwrap();
    let err = detect_store_kind(&misnamed).unwrap_err();
    assert!(err.to_string().contains("refusing to guess"), "got {err}");
    assert!(verify(&misnamed).is_err());
    assert!(writ_ledger::open_store(&misnamed).is_err());
}

#[cfg(not(feature = "sqlite"))]
#[test]
fn sqlite_ledgers_fail_closed_without_the_feature() {
    let dir = TempDir::new();
    let db = dir.path().join("ledger.db");
    let err = writ_ledger::open_store(&db).err().expect("must not open");
    assert!(err.to_string().contains("`sqlite` feature"), "got {err}");
    assert!(!db.exists(), "nothing may be created");

    std::fs::write(&db, b"SQLite format 3\0rest-of-header").unwrap();
    for err in [
        verify(&db).unwrap_err(),
        writ_ledger::sessions(&db).unwrap_err(),
        writ_ledger::find_by_call_id(&db, "c1").unwrap_err(),
    ] {
        assert!(err.to_string().contains("`sqlite` feature"), "got {err}");
    }
}
