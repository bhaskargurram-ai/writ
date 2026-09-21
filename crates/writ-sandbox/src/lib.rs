//! writ-sandbox — `SandboxBackend` adapters (spec §8).
//!
//! Writ's opinion is about *which* call runs, not *how* it is contained.
//! `local-os` is the default workstation backend; container/VM/cluster
//! adapters are detected if present (see [`detect::detect_backends`]).

#![forbid(unsafe_code)]

pub mod detect;
pub mod local_os;
pub mod platform;

pub use detect::{detect_backends, BackendInfo};
pub use local_os::LocalOsBackend;
pub use platform::{kernel_hardening, KernelHardening};

use writ_core::sandbox::SandboxBackend;

/// Name -> backend for everything available on this machine.
pub struct SandboxRegistry {
    pub backends: Vec<(&'static str, Box<dyn SandboxBackend>)>,
}

impl SandboxRegistry {
    pub fn detect() -> Self {
        let backends: Vec<(&'static str, Box<dyn SandboxBackend>)> =
            vec![("local-os", Box::new(LocalOsBackend::new()))];
        // docker/microsandbox/firecracker/k8s adapters register here as their
        // waves land; detect::detect_backends() reports them honestly.
        SandboxRegistry { backends }
    }

    pub fn get(&mut self, name: &str) -> Option<&mut Box<dyn SandboxBackend>> {
        self.backends.iter_mut().find(|(n, _)| *n == name).map(|(_, b)| b)
    }
}
