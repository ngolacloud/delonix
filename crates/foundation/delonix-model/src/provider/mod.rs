//! The provider skeleton (ADR-0040 D3): what every provider IS, independent of
//! what it provides. Identity (`ProviderId`), the capability report it answers
//! (`capability`), and the base `Provider` trait live in the foundation so that
//! any context can name a provider port without depending on another context —
//! the move that lets `delonix-networking` declare `SegmentProvider`/`DnsProvider`
//! and friends without reaching into `delonix-compute` (ADR-0040 R2).
//!
//! The VM-shaped and system-container-shaped extensions of this trait
//! (`VmProvider`, `SystemContainerProvider`, `Extensions`) stay in the compute
//! context: they speak VM/container types, which are compute's domain.

pub mod capability;

use capability::{ProviderHealth, ProviderReport};

/// A provider's stable identity: the name a composition root seeds the registry
/// with, and the key of the registry. Fixed once, at construction; never
/// re-derived by comparing it against a literal inside a lifecycle method
/// (ADR-0044 D3 rule 3: no provider-name matching outside a composition root).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderId(pub &'static str);

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

/// ADR-0040 D3's skeleton: identity, capabilities, health.
pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;

    /// The provider's answer to every catalog entry, on THIS host when the
    /// provider is local (it probes) and as declared when it is remote (it
    /// never connects to answer this). ADR-0050's report, verbatim: a caller
    /// that needs `vm.snapshot.memory` finds it here and refuses before any
    /// effect when it is not usable — the provider never has to guess.
    fn capabilities(&self) -> ProviderReport;

    /// The report's health, for a caller that asks only "can I select you".
    fn health(&self) -> ProviderHealth {
        self.capabilities().health
    }
}

/// The capabilities a caller required that a provider's report does not deliver,
/// each with the reason (unusable with its detail, or absent from the report).
/// Empty means the provider can honour the request. Pure over the report — no
/// host, no registry — so any context can refuse before an effect.
pub fn unmet(report: &ProviderReport, required: &[capability::Capability]) -> Vec<String> {
    required
        .iter()
        .filter_map(
            |c| match report.capabilities.iter().find(|r| r.capability == *c) {
                Some(r) if r.state.is_usable() => None,
                Some(r) => {
                    let detail = r.state.detail();
                    Some(if detail.is_empty() {
                        format!("{}: {}", c.name(), r.state.label())
                    } else {
                        format!("{}: {} — {}", c.name(), r.state.label(), detail)
                    })
                }
                None => Some(format!("{}: not in this provider's report", c.name())),
            },
        )
        .collect()
}
