//! Acceptance tests for FileLedgerStore (Contract 3 / ADR-005): the shared
//! store-generic suite plus JSONL-specific crash-tolerance tests.

#[macro_use]
mod common;

use std::fs::OpenOptions;
use std::io::Write;

use common::{allow, call, ledger_path, populated, FileBackend, TempDir};
use writ_core::error::WritError;
use writ_core::ledger::{LedgerStore, LedgerWriter};
use writ_ledger::{
    detect_store_kind, is_append_race, retry_append, verify, FileLedgerStore, StoreKind,
};

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

#[test]
fn stale_writer_cannot_fork_the_chain() {
    let dir = TempDir::new();
    let path = populated::<FileBackend>(&dir);
    let mut a = FileLedgerStore::open(&path).unwrap();
    let mut b = FileLedgerStore::open(&path).unwrap();

    // b prepares a record on the tip it has seen (index 3)...
    let tip = b.tip().unwrap().unwrap();
    let mut stale = tip.clone();
    stale.index = 3;
    stale.call_id = "b-call".into();
    stale.prev_hash = tip.record_hash.clone();
    stale.record_hash = stale.compute_hash().unwrap();

    // ...but a appends first.
    LedgerWriter::new(&mut a)
        .record_decision(&call("a-call", "s1"), &allow(), None)
        .unwrap();

    // b sees a's record on disk, and its stale append is rejected as a race.
    assert_eq!(b.len(), 4);
    assert_eq!(b.tip().unwrap().unwrap().call_id, "a-call");
    let err = b.append(&stale).unwrap_err();
    assert!(is_append_race(&err), "got {err:?}");
    assert_eq!(b.len(), 4);

    // Retrying through LedgerWriter rebuilds on the new tip and succeeds.
    LedgerWriter::new(&mut b)
        .record_decision(&call("b-call", "s1"), &allow(), None)
        .unwrap();
    assert_eq!(a.len(), 5);
    assert_eq!(a.tip().unwrap().unwrap().call_id, "b-call");
    assert_eq!(a.get(4).unwrap().unwrap().call_id, "b-call");
    let report = verify(&path).unwrap();
    assert!(report.intact);
    assert_eq!(report.records, 5);
}

#[test]
fn hash_mismatch_is_not_a_race() {
    let dir = TempDir::new();
    let path = populated::<FileBackend>(&dir);
    let mut store = FileLedgerStore::open(&path).unwrap();
    let mut bad = store.tip().unwrap().unwrap();
    bad.index = 3;
    bad.prev_hash = bad.record_hash.clone();
    bad.record_hash = "0".repeat(64);
    let err = store.append(&bad).unwrap_err();
    assert!(!is_append_race(&err), "got {err:?}");
    // retry_append gives up immediately on a non-race error.
    let mut calls = 0;
    let res: Result<(), WritError> = retry_append(|| {
        calls += 1;
        store.append(&bad)
    });
    assert!(res.is_err());
    assert_eq!(calls, 1);
}

#[test]
fn concurrent_writers_produce_one_linear_chain() {
    const THREADS: usize = 8;
    const PER_THREAD: usize = 25;
    let dir = TempDir::new();
    let path = ledger_path::<FileBackend>(&dir);
    drop(FileLedgerStore::open(&path).unwrap());

    // Every thread has its own store (its own file handles and lock-file
    // handle), exactly like separate processes.
    let handles: Vec<_> = (0..THREADS)
        .map(|t| {
            let path = path.clone();
            std::thread::spawn(move || {
                let mut store = FileLedgerStore::open(&path).unwrap();
                for i in 0..PER_THREAD {
                    let c = call(&format!("t{t}-c{i}"), &format!("t{t}"));
                    retry_append(|| {
                        LedgerWriter::new(&mut store).record_decision(&c, &allow(), None)
                    })
                    .unwrap_or_else(|e| panic!("thread {t}: {e}"));
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }

    let report = verify(&path).unwrap();
    assert!(report.intact, "{report:?}");
    assert_eq!(report.records, (THREADS * PER_THREAD) as u64);
    let store = FileLedgerStore::open(&path).unwrap();
    let mut ids: Vec<String> = store.iter().map(|r| r.unwrap().call_id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), THREADS * PER_THREAD, "every call recorded once");
}

#[test]
fn torn_tail_is_repaired_by_the_next_append_and_hidden_from_readers() {
    let dir = TempDir::new();
    let path = populated::<FileBackend>(&dir);
    let mut store = FileLedgerStore::open(&path).unwrap();

    // Another writer "crashes" mid-line after this store was opened.
    {
        let mut f = OpenOptions::new().append(true).open(&path).unwrap();
        f.write_all(b"{\"schema_version\":1,\"ind").unwrap();
    }
    // Readers see only committed records.
    assert_eq!(store.len(), 3);
    assert_eq!(store.iter().count(), 3);
    assert!(store.get(3).unwrap().is_none());

    // The next append (under the lock) drops the debris and extends the chain.
    LedgerWriter::new(&mut store)
        .record_decision(&call("c4", "s1"), &allow(), None)
        .unwrap();
    let report = verify(&path).unwrap();
    assert!(report.intact, "{report:?}");
    assert_eq!(report.records, 4);
}

#[test]
fn final_record_without_newline_is_kept() {
    let dir = TempDir::new();
    let path = populated::<FileBackend>(&dir);
    // Strip the trailing newline from the last (complete) record.
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.trim_end()).unwrap();

    let mut store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(store.len(), 3);
    LedgerWriter::new(&mut store)
        .record_decision(&call("c4", "s1"), &allow(), None)
        .unwrap();
    let report = verify(&path).unwrap();
    assert!(report.intact, "{report:?}");
    assert_eq!(report.records, 4);
}

#[test]
fn lock_file_sits_next_to_the_ledger() {
    let dir = TempDir::new();
    let path = populated::<FileBackend>(&dir);
    let store = FileLedgerStore::open(&path).unwrap();
    assert_eq!(
        store.lock_path().file_name().unwrap().to_str().unwrap(),
        "ledger.jsonl.lock"
    );
    assert!(store.lock_path().exists(), "created by the first append");
}
