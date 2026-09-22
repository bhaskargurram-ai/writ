//! Small Unix process helpers shared by the Linux and macOS paths.

/// SIGKILL every process in process group `pgid`. Callers must only use
/// this while the group leader is still unreaped (alive or zombie), so the
/// pgid cannot have been recycled for an unrelated group.
pub(crate) fn kill_process_group(pgid: u32) {
    let Ok(pgid) = libc::pid_t::try_from(pgid) else {
        return;
    };
    if pgid <= 1 {
        return; // never signal init or "every process" (pgid 0/-1 semantics)
    }
    // SAFETY: killpg with a positive pgid and a valid signal number has no
    // memory-safety preconditions; ESRCH (group already gone) is ignored.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}
