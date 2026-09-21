//! `writ verify` backend: whole-file chain verification.
//!
//! Wraps [`writ_core::verify_chain`] so every store shares one evidentiary
//! code path (ADR-002: verify-forever). Adds one thing the raw iterator
//! cannot express: a line so tampered it no longer parses. That is reported
//! as a verification failure at that record's position, not as an opaque
//! error — the exact broken index is the product's promise (spec §9).

use std::cell::Cell;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;

use writ_core::error::{Result, WritError};
use writ_core::ledger::{verify_chain, VerifyReport};

use crate::file_store::record_iter;

/// Verify the hash chain of the JSONL ledger at `path`.
///
/// Returns `VerifyReport { intact: false, broken_at: Some(i), records: i }`
/// when record `i` fails its hash, its chain link, or fails to parse at all.
pub fn verify(path: impl AsRef<Path>) -> Result<VerifyReport> {
    let file = File::open(path.as_ref())?;
    // Position (record ordinal) of the first unparseable line, if any.
    let parse_failure: Rc<Cell<Option<u64>>> = Rc::new(Cell::new(None));
    let flag = Rc::clone(&parse_failure);
    let mut ordinal = 0u64;
    let records = record_iter(file).map(move |item| {
        item.map_err(|e| {
            // record_iter only errors on IO or JSON parse; either way the
            // record at this ordinal cannot be verified.
            if flag.get().is_none() {
                flag.set(Some(ordinal));
            }
            match e {
                WritError::Serde(s) => WritError::Ledger(format!(
                    "record {} is not a valid LedgerRecord: {}",
                    ordinal, s
                )),
                other => other,
            }
        })
        .inspect(|_| {
            ordinal += 1;
        })
    });
    match verify_chain(records) {
        Ok(report) => Ok(report),
        Err(e) => match parse_failure.get() {
            // Tampered/unparseable line: surface position as a broken chain.
            Some(i) => Ok(VerifyReport {
                records: i,
                intact: false,
                broken_at: Some(i),
            }),
            None => Err(e),
        },
    }
}

