//! Kernel-level hardening status per platform.
//!
//! HONESTY REQUIREMENT (spec §6, §15): nothing here may claim enforcement
//! that is not implemented and tested. `writ doctor` surfaces these notes
//! verbatim so users know exactly which guarantees hold.

/// What the current platform's kernel boundary enforces.
#[derive(Debug, Clone)]
pub struct KernelHardening {
    pub platform: &'static str,
    /// True only when syscall-level enforcement is active and tested.
    pub enforced: bool,
    /// The mechanism that WILL enforce the boundary, per spec §6B.
    pub mechanism: &'static str,
    /// Shown by `writ doctor`.
    pub notes: &'static str,
}

#[cfg(target_os = "linux")]
pub fn kernel_hardening() -> KernelHardening {
    KernelHardening {
        platform: "linux",
        enforced: false,
        mechanism: "Landlock (filesystem) + seccomp (syscalls)",
        notes: "wave 1: process spawn + workspace-boundary checks only. \
                Landlock/seccomp land in wave 2 — until then the kernel floor \
                is NOT enforced on this platform.",
    }
}

#[cfg(target_os = "macos")]
pub fn kernel_hardening() -> KernelHardening {
    KernelHardening {
        platform: "macos",
        enforced: false,
        mechanism: "Seatbelt (sandbox-exec profile)",
        notes: "wave 1: process spawn + workspace-boundary checks only. \
                Seatbelt profile lands in wave 2 — until then the kernel floor \
                is NOT enforced on this platform.",
    }
}

#[cfg(target_os = "windows")]
pub fn kernel_hardening() -> KernelHardening {
    KernelHardening {
        platform: "windows",
        enforced: false,
        mechanism: "restricted tokens + job objects",
        notes: "wave 1: process spawn + workspace-boundary checks only. \
                Restricted-token/job-object confinement lands in wave 2 — \
                until then the kernel floor is NOT enforced on this platform.",
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
pub fn kernel_hardening() -> KernelHardening {
    KernelHardening {
        platform: "other",
        enforced: false,
        mechanism: "none",
        notes: "unsupported platform: no kernel boundary is enforced at all.",
    }
}
