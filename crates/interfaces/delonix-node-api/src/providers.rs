//! The providers of this node as the contract carries them (ADR-0050 D5).
//!
//! **The composition is written here a second time**, and the second copy is
//! guarded: `delonix provider ls` composes the same list in the CLI
//! (`cmd/provider.rs`), which this crate cannot depend on. The test
//! `the_declared_providers_are_the_published_matrix` reads the ids and kinds
//! the CLI published in `docs/providers/capability-matrix.md` and fails when
//! this list disagrees — so a provider added to one side and not the other is
//! a red test, not a server that quietly lists fewer providers than the CLI.

use crate::net_ops::NetworkOps;
use crate::proto::v1::{Capability, Condition, ConditionStatus, ProviderInfo};
use crate::provider_reports::ProviderReports;
use crate::vms::VmPort;
use delonix_compute::capability::{HealthStatus, ProviderReport, CATALOG_VERSION};

/// Every provider this node knows, MEASURED on this host: the VM backends of
/// the registry (plus Proxmox unconfigured when nobody registered it), the
/// Linux provider three times — compute, network, storage — and Proxmox's own
/// per-VM firewall as the second network provider. The VM backends come through
/// the composed VM port, the SDN provider through the composed network port, and
/// the Proxmox/Linux/volume/OPNsense reports through the composed
/// [`ProviderReports`] port (ADR-0040 A5), not the `delonix-vm`/`delonix-sdn`/
/// `delonix-provider-proxmox`/`delonix-linux`/`delonix-volume`/
/// `delonix-provider-opnsense` adapters directly. The composition — ordering and
/// the `proxmox_configured` flag — lives here; only the report construction is
/// behind the port.
pub fn measured_reports(
    vms: &dyn VmPort,
    net: &dyn NetworkOps,
    providers: &dyn ProviderReports,
) -> Vec<ProviderReport> {
    let mut out = vms.provider_reports();
    let proxmox_configured = out.iter().any(|r| r.id == "proxmox");
    if !proxmox_configured {
        out.push(providers.proxmox_compute_report(false));
    }
    out.push(providers.linux_compute_report(true));
    out.extend(net.provider_reports());
    // The node's own per-VM firewall (ADR-0052): the second network provider,
    // with the same configured flag as the VM backend — one target, two ports.
    out.push(providers.proxmox_network_report(proxmox_configured));
    out.push(providers.volume_storage_report(true));
    // The perimeter appliance (ADR-0059 D2), declared, same flag rule as Proxmox.
    // Whether it is a gateway comes through the network port's registry view.
    out.push(providers.opnsense_report(
        net.gateway_provider_ids()
            .iter()
            .any(|id| id == providers.opnsense_id()),
    ));
    out
}

/// The same set, DECLARED: every host assumed complete, Proxmox assumed
/// configured. Pure — what the equality test against the published matrix reads.
/// The VM providers' declared reports come through the composed VM port, the SDN
/// provider through the composed network port, and the Proxmox/Linux/volume/
/// OPNsense reports through the composed [`ProviderReports`] port, not the
/// adapters directly.
pub fn declared_reports(
    vms: &dyn VmPort,
    net: &dyn NetworkOps,
    providers: &dyn ProviderReports,
) -> Vec<ProviderReport> {
    let mut out = vms.declared_vm_reports();
    out.extend([
        providers.proxmox_compute_report(true),
        providers.linux_compute_report(false),
    ]);
    out.extend(net.declared_provider_reports());
    out.extend([
        providers.proxmox_network_report(true),
        providers.volume_storage_report(false),
        providers.opnsense_report(true),
    ]);
    out
}

/// One report as the contract's `ProviderInfo`: the same fields
/// `delonix provider ls -o json` prints, under the contract's names.
pub fn provider_info(r: &ProviderReport) -> ProviderInfo {
    let status = match r.health.status {
        HealthStatus::Healthy => ConditionStatus::True,
        HealthStatus::Unavailable => ConditionStatus::False,
        HealthStatus::Unknown => ConditionStatus::Unknown,
    };
    ProviderInfo {
        id: r.id.to_string(),
        kind: r.kind.as_str().to_string(),
        available: r.available,
        capabilities: r
            .capabilities
            .iter()
            .map(|c| Capability {
                name: c.capability.name().to_string(),
                supported: c.state.is_usable(),
                detail: c.state.detail().to_string(),
                state: c.state.label().to_string(),
            })
            .collect(),
        health: Some(Condition {
            r#type: "Healthy".to_string(),
            status: status as i32,
            reason: r.health.reason.to_string(),
            message: r.health.message.clone(),
            last_transition_time: None,
        }),
        catalog_version: CATALOG_VERSION.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
    }

    /// A `delonix-vm`-backed port for the pure declared matrix; the root is
    /// unused (the declared reports are host-assumed, no probe).
    fn vms() -> crate::vms::LocalVmReads {
        crate::vms::LocalVmReads(std::path::PathBuf::from("/"))
    }

    /// The crate's `delonix-sdn`-backed network port (dev-dependency): the SDN
    /// provider's declared report must be the real one for the matrix test.
    const NET: crate::net_ops::LocalNet = crate::net_ops::LocalNet;

    /// The crate's adapter-backed [`ProviderReports`] port (dev-dependencies):
    /// the Proxmox/Linux/volume/OPNsense reports must be the real ones for the
    /// matrix test, so the composed (kind, id) set equals the published one.
    const LOCAL_PROVIDERS: crate::provider_reports::LocalProviders =
        crate::provider_reports::LocalProviders;

    /// The CLI publishes its composition as the matrix; this crate's must be
    /// the same set of (kind, id), or the socket lists fewer providers than
    /// `provider ls` for no reason anyone wrote down.
    #[test]
    fn the_declared_providers_are_the_published_matrix() {
        let md = std::fs::read_to_string(repo().join("docs/providers/capability-matrix.md"))
            .expect("docs/providers/capability-matrix.md");
        let mut published = std::collections::BTreeSet::new();
        let mut kind = String::new();
        for line in md.lines() {
            if let Some(k) = line.strip_prefix("## ") {
                kind = k.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("- **") {
                if let Some((label, _)) = rest.split_once("**") {
                    // A gateway is a column of the network table, labelled
                    // `<id> (gateway)` (ADR-0059 D2): its own kind wins.
                    let (id, k) = match label.strip_suffix(')').and_then(|l| l.split_once(" (")) {
                        Some((id, k)) => (id, k.to_string()),
                        None => (label, kind.clone()),
                    };
                    published.insert((k, id.to_string()));
                }
            }
        }
        let ours: std::collections::BTreeSet<(String, String)> = declared_reports(&vms(), &NET, &LOCAL_PROVIDERS)
            .iter()
            .map(|r| (r.kind.as_str().to_string(), r.id.to_string()))
            .collect();
        assert_eq!(
            ours, published,
            "node-api composes a different provider list than the CLI published"
        );
    }

    /// Every field of the CLI's JSON has a home in the contract, and the
    /// boolean is the reading of the state, never a second opinion.
    #[test]
    fn provider_info_carries_the_state_and_the_catalog_version() {
        let r = declared_reports(&vms(), &NET, &LOCAL_PROVIDERS).remove(1); // libvirt
        let info = provider_info(&r);
        assert_eq!(info.id, "libvirt");
        assert_eq!(info.kind, "compute");
        assert_eq!(info.catalog_version, CATALOG_VERSION);
        assert_eq!(info.capabilities.len(), r.capabilities.len());
        for (c, d) in info.capabilities.iter().zip(&r.capabilities) {
            assert_eq!(c.supported, d.state.is_usable(), "{}", c.name);
            assert_eq!(c.state, d.state.label(), "{}", c.name);
        }
        let h = info.health.expect("health condition");
        assert_eq!(h.r#type, "Healthy");
        assert!(!h.reason.is_empty());
    }

    /// The JSON on the socket uses the proto field names the OpenAPI declares
    /// (`naming=proto`), not camelCase.
    #[test]
    fn the_json_uses_proto_field_names() {
        let info = provider_info(&declared_reports(&vms(), &NET, &LOCAL_PROVIDERS)[0]);
        let v = serde_json::to_value(&info).unwrap();
        assert!(v.get("catalog_version").is_some(), "{v}");
        assert!(v["capabilities"][0].get("state").is_some());
        assert!(v["health"]["status"].is_string(), "enum as its name: {v}");
    }
}
