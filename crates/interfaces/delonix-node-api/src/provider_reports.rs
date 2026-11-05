//! `ProviderReports` — the per-adapter **capability reports** this node composes
//! into its provider list, as this interface needs them (R3d-h, ADR-0040 A5).
//! The interface holds this port instead of naming the `delonix-provider-proxmox`,
//! `delonix-provider-opnsense`, `delonix-linux` and `delonix-volume` adapters in
//! [`crate::providers`]: a bin composes an implementor and hands the server one,
//! so the forbidden interface→adapter edge stays in the bin — the same shape as
//! [`crate::vms::VmPort`], [`crate::store::StorePort`],
//! [`crate::run_resolver::RunResolver`], [`crate::net_ops::NetworkOps`] and
//! [`crate::image_repo::ImageRepo`].
//!
//! Only the per-adapter REPORT CONSTRUCTION moves behind the port; the
//! composition — the `proxmox_configured` derivation, the ordering of the pushes
//! and the interleaving with the VM/network ports — stays in
//! [`crate::providers::measured_reports`]/[`crate::providers::declared_reports`],
//! which name no adapter.
//!
//! Every method names only shared types: the return is the shared
//! [`delonix_compute::capability::ProviderReport`] both the engine and this crate
//! speak, and the parameters are primitives (`bool`, `&'static str`) — never an
//! adapter type — so the implementor lives in the composition and the interface
//! stays clean of the adapters.
//!
//! This Engine has no provider-report *service* yet (PRE-later-slice), so the
//! implementor is backed by the four adapters DIRECTLY — the same way the bin
//! backs the record store with `delonix-state`, the SDN with `delonix-sdn` and
//! the image store with `delonix-oci`. When a report service lands, this port is
//! where it is swapped in.

use delonix_compute::capability::{
    CapabilityState, HealthStatus, ProviderHealth, ProviderKind, ProviderReport,
};

/// The per-adapter capability reports the node API composes into its provider
/// list (R3d-h). Object-safe (`dyn ProviderReports`): every method names only
/// shared types — the shared [`ProviderReport`] and primitives — never a
/// provider/host adapter type, so the implementor lives in the composition and
/// the interface names none of the four adapters.
pub trait ProviderReports: Send + Sync {
    /// Proxmox's COMPUTE report. `configured` is `true` when a Proxmox target is
    /// registered in this process (declared always passes `true`; measured passes
    /// `false` only when the VM registry carried no `proxmox` backend).
    fn proxmox_compute_report(&self, configured: bool) -> ProviderReport;

    /// Proxmox's own per-VM-firewall NETWORK report (ADR-0052) — the second
    /// network provider, with the same `configured` flag as the compute backend.
    fn proxmox_network_report(&self, configured: bool) -> ProviderReport;

    /// The Linux provider's COMPUTE report. `measured` `true` probes this host;
    /// `false` is the declared, host-assumed view the published matrix reads.
    fn linux_compute_report(&self, measured: bool) -> ProviderReport;

    /// The Linux provider's STORAGE report. `measured` `true` probes this host;
    /// `false` is the declared, host-assumed view.
    fn volume_storage_report(&self, measured: bool) -> ProviderReport;

    /// The OPNsense perimeter appliance report (ADR-0059 D2), declared.
    /// `is_gateway` is `true` when the network port's registry view names
    /// OPNsense a gateway provider.
    fn opnsense_report(&self, is_gateway: bool) -> ProviderReport;

    /// The OPNsense provider id, so [`crate::providers`] can keep the
    /// `net.gateway_provider_ids().any(|id| id == ...)` check without naming the
    /// `delonix-provider-opnsense` `ID` constant.
    fn opnsense_id(&self) -> &'static str;
}

/// A [`ProviderReports`] with no composed adapters — the benign backing
/// [`crate::router`] hands the server when no engine is injected. Each report is
/// a placeholder (unknown health, every catalog entry `not-implemented`) and the
/// id answers the stable OPNsense id. The production server never uses it — a bin
/// hands in the adapter-backed port — exactly as [`crate::image_repo::NoImages`]
/// is to [`crate::image_repo::ImageRepo`] and [`crate::net_ops::NoNet`] is to
/// [`crate::net_ops::NetworkOps`]. The provider list is exercised through
/// `serve_blocking`/`router_with` with the real adapter backing, not through the
/// default [`crate::router`].
pub(crate) struct NoProviderReports;

impl NoProviderReports {
    /// A placeholder report of the given id/kind: unavailable, unknown health,
    /// every applicable catalog entry `not-implemented`. Stated here because this
    /// backing names no adapter — the production port returns the adapters' own
    /// reports.
    fn placeholder(id: &str, kind: ProviderKind) -> ProviderReport {
        ProviderReport::build(
            id,
            kind,
            false,
            ProviderHealth {
                status: HealthStatus::Unknown,
                reason: "NoBacking",
                message: "no provider backing composed (default router)".to_string(),
            },
            |_| CapabilityState::NotImplemented,
        )
    }
}

impl ProviderReports for NoProviderReports {
    fn proxmox_compute_report(&self, _configured: bool) -> ProviderReport {
        Self::placeholder("proxmox", ProviderKind::Compute)
    }
    fn proxmox_network_report(&self, _configured: bool) -> ProviderReport {
        Self::placeholder("proxmox", ProviderKind::Network)
    }
    fn linux_compute_report(&self, _measured: bool) -> ProviderReport {
        Self::placeholder("linux", ProviderKind::Compute)
    }
    fn volume_storage_report(&self, _measured: bool) -> ProviderReport {
        Self::placeholder("linux", ProviderKind::Storage)
    }
    fn opnsense_report(&self, _is_gateway: bool) -> ProviderReport {
        Self::placeholder("opnsense", ProviderKind::Gateway)
    }
    fn opnsense_id(&self) -> &'static str {
        "opnsense"
    }
}

// The real adapter-backed implementor lives in the composition
// (`delonix-node-api-bin`'s `HostProviders`) and, for the integration tests, in
// the harness's `tests/common::AdapterProviders` over the four provider/host
// dev-dependencies. The node-api crate names no provider/host adapter in
// production — not even here; the `#[cfg(test)] LocalProviders` below names them
// only to back this crate's own unit tests, where they are dev-dependencies.

/// The in-crate test backing that names the four real adapters, so the
/// [`crate::providers`] unit tests compose the IDENTICAL reports the production
/// path does — analogous to [`crate::net_ops::LocalNet`]. Under `#[cfg(test)]`
/// the adapters are dev-dependencies, so naming them here is allowed.
#[cfg(test)]
pub(crate) struct LocalProviders;

#[cfg(test)]
impl ProviderReports for LocalProviders {
    fn proxmox_compute_report(&self, configured: bool) -> ProviderReport {
        delonix_provider_proxmox::capability_report(configured)
    }
    fn proxmox_network_report(&self, configured: bool) -> ProviderReport {
        delonix_provider_proxmox::network_capability_report(configured)
    }
    fn linux_compute_report(&self, measured: bool) -> ProviderReport {
        let host = if measured {
            delonix_linux::provider_report::LinuxHost::probe()
        } else {
            delonix_linux::provider_report::LinuxHost::ASSUMED
        };
        delonix_linux::provider_report::report(&host)
    }
    fn volume_storage_report(&self, measured: bool) -> ProviderReport {
        let host = if measured {
            delonix_volume::provider_report::StorageHost::probe()
        } else {
            delonix_volume::provider_report::StorageHost::ASSUMED
        };
        delonix_volume::provider_report::report(&host)
    }
    fn opnsense_report(&self, is_gateway: bool) -> ProviderReport {
        delonix_provider_opnsense::capability_report(is_gateway)
    }
    fn opnsense_id(&self) -> &'static str {
        delonix_provider_opnsense::ID
    }
}
