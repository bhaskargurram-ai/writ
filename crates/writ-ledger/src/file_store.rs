//! ADR-005 default store: an append-only JSONL file, one serialized
//! [`LedgerRecord`] per line (e.g. `.writ/ledger.jsonl`).
//!
//! Durability: every append is flushed and `sync_data`'d before `Ok` is
//! returned — a decision record on disk is evidence even if the process
//! dies immediately after (ADR-003).
//!
//! Crash-tolerant tail: if the process died mid-write, the final line may
//! be torn. [`FileLedgerStore::open`] stops at the last valid record in
//! that case; an unparseable line anywhere else is a hard error, because
//! mid-file corruption is evidence of tampering, not of a crash.

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use writ_core::error::{Result, WritError};
use writ_core::ledger::{LedgerRecord, LedgerStore, GENESIS_HASH};

/// Append-only JSONL ledger. State (`len`, tip hash, tip record) is rebuilt
/// by scanning the file at [`open`](FileLedgerStore::open).
#[derive(Debug)]
pub struct FileLedgerStore {
    path: PathBuf,
    len: u64,
    tip_hash: String,
    tip_record: Option<LedgerRecord>,
}

impl FileLedgerStore {
    /// Open (creating file and parent dirs if absent) and rebuild in-memory
    /// state by scanning existing lines. A torn final line is truncated from
    /// the logical view; any other corruption is a `WritError::Ledger`.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        // Ensure the file exists without truncating it.
        OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&path)?;
        let mut store = FileLedgerStore {
            path,
            len: 0,
            tip_hash: GENESIS_HASH.to_string(),
            tip_record: None,
        };
        if let Some(torn_at) = store.rescan()? {
            // Truncate the torn tail so the next append does not strand the
            // partial bytes mid-file. Only unparseable bytes past the last
            // valid record are dropped — no record is ever mutated (ADR-003).
            OpenOptions::new()
                .write(true)
                .open(&store.path)?
                .set_len(torn_at)?;
        }
        Ok(store)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Rebuild `len` / tip state from disk. Cheap link checks only (index
    /// sequence + prev_hash chain); full hash verification is `verify()`.
    /// Returns the byte offset of a torn tail, if one was found.
    fn rescan(&mut self) -> Result<Option<u64>> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut offset = 0u64; // byte offset of the current line
        let mut torn_at = None;
        let mut expected = 0u64;
        let mut prev = GENESIS_HASH.to_string();
        let mut tip_record = None;
        loop {
            line.clear();
            let line_start = offset;
            let n = reader.read_line(&mut line)? as u64;
            if n == 0 {
                break; // EOF
            }
            offset += n;
            let trimmed = line.trim_end();
            if trimmed.is_empty() {
                continue; // tolerate stray blank lines
            }
            match serde_json::from_str::<LedgerRecord>(trimmed) {
                Ok(rec) => {
                    if rec.index != expected {
                        return Err(WritError::Ledger(format!(
                            "ledger {:?}: record index {} out of sequence (expected {})",
                            self.path, rec.index, expected
                        )));
                    }
                    if rec.prev_hash != prev {
                        return Err(WritError::Ledger(format!(
                            "ledger {:?}: record {} prev_hash does not match the chain tip",
                            self.path, rec.index
                        )));
                    }
                    prev = rec.record_hash.clone();
                    tip_record = Some(rec);
                    expected += 1;
                }
                Err(e) => {
                    // Only acceptable as a torn final write: nothing but
                    // whitespace may follow this line.
                    let mut rest = String::new();
                    reader.read_to_string(&mut rest)?;
                    if rest.trim().is_empty() {
                        torn_at = Some(line_start);
                        break; // crash-tolerant tail: stop at last valid record
                    }
                    return Err(WritError::Ledger(format!(
                        "ledger {:?}: corrupt record at index {} (mid-file corruption): {}",
                        self.path, expected, e
                    )));
                }
            }
        }
        self.len = expected;
        self.tip_hash = prev;
        self.tip_record = tip_record;
        Ok(torn_at)
    }
}

impl LedgerStore for FileLedgerStore {
    fn append(&mut self, record: &LedgerRecord) -> Result<()> {
        if record.index != self.len {
            return Err(WritError::Ledger(format!(
                "append rejected: record index {} but ledger length is {} \
                 (records are append-only and sequential)",
                record.index, self.len
            )));
        }
        if record.prev_hash != self.tip_hash {
            return Err(WritError::Ledger(format!(
                "append rejected: record {} prev_hash does not match the ledger tip",
                record.index
            )));
        }
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        let mut file = OpenOptions::new().append(true).open(&self.path)?;
        file.write_all(line.as_bytes())?;
        file.flush()?;
        // Durability before Ok: the record must survive a power loss.
        file.sync_data()?;
        self.tip_hash = record.record_hash.clone();
        self.tip_record = Some(record.clone());
        self.len += 1;
        Ok(())
    }

    fn tip(&self) -> Result<Option<LedgerRecord>> {
        Ok(self.tip_record.clone())
    }

    fn get(&self, index: u64) -> Result<Option<LedgerRecord>> {
        if index >= self.len {
            return Ok(None);
        }
        for (i, item) in record_iter(File::open(&self.path)?).enumerate() {
            if i as u64 == index {
                return item.map(Some);
            }
        }
        Ok(None)
    }

    fn len(&self) -> u64 {
        self.len
    }

    fn iter(&self) -> Box<dyn Iterator<Item = Result<LedgerRecord>> + '_> {
        match File::open(&self.path) {
            Ok(f) => Box::new(record_iter(f)),
            Err(e) => Box::new(std::iter::once(Err(WritError::Io(e)))),
        }
    }
}

/// Lazy line-by-line parse; never loads the whole file into memory.
/// Blank lines are skipped; each non-blank line is one record.
pub(crate) fn record_iter(file: File) -> impl Iterator<Item = Result<LedgerRecord>> {
    BufReader::new(file).lines().filter_map(|line| match line {
        Err(e) => Some(Err(WritError::Io(e))),
        Ok(s) if s.trim().is_empty() => None,
        Ok(s) => Some(serde_json::from_str(&s).map_err(WritError::from)),
    })
}
