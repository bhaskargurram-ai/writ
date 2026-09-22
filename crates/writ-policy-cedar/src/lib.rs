//! writ-policy-cedar — the Cedar engine backend (Contract 2, spec §7).
//!
//! Stub for the Wave-3 task: the engine type is registered and fails
//! closed on every evaluation. The real implementation must produce
//! verdicts identical to the native engine on the shared fixture corpus
//! (`crates/writ-policy/fixtures/`, INTERFACES.md Contract 2).

use writ_core::call::ToolCallContext;
use writ_core::error::Result;
use writ_core::policy::{PolicyEngine, PolicyMeta};
use writ_core::verdict::{DefaultVerdict, Verdict};

/// The Cedar engine (`name() == "cedar"`). Stub: fails closed.
#[derive(Debug, Clone)]
pub struct CedarPolicyEngine;

impl CedarPolicyEngine {
    /// Compile a policy from writ.yaml source.
    pub fn from_source(_source: &str) -> Result<Self> {
        Ok(CedarPolicyEngine)
    }

    /// The policy file's declared version and default verdict.
    pub fn meta(&self) -> PolicyMeta {
        PolicyMeta {
            version: 1,
            default: DefaultVerdict::Ask,
        }
    }
}

impl PolicyEngine for CedarPolicyEngine {
    fn name(&self) -> &'static str {
        "cedar"
    }

    fn evaluate(&self, _ctx: &ToolCallContext) -> Verdict {
        Verdict::Deny {
            rule_id: "engine-cedar-unimplemented".to_string(),
            reason:
                "The Cedar engine backend is not implemented yet; the call is denied (fail closed)."
                    .to_string(),
            location: None,
        }
    }

    fn reload(&mut self, _source: &str) -> Result<()> {
        Ok(())
    }

    fn rule_count(&self) -> usize {
        0
    }
}
