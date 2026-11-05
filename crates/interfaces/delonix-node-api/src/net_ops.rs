//! `NetworkOps` — the node's **SDN** as this interface needs it (R3d-f,
//! ADR-0040 A5). The interface holds this port instead of naming the
//! `delonix-sdn` adapter directly: a bin composes an implementor and hands the
//! server one, so the forbidden interface→adapter edge stays in the bin — the
//! same shape as [`crate::vms::VmPort`], [`crate::store::StorePort`] and
//! [`crate::run_resolver::RunResolver`].
//!
//! This Engine has no network *service* yet (the slice is PRE-R3h), so the
//! implementor is backed by `delonix-sdn` DIRECTLY — the same way the bin backs
//! the record store with `delonix-state` — not by an engine network port. When
//! a network service lands, this port is where it is swapped in.
//!
//! The adapter's `delonix_sdn::Network` is crossed as the [`NetRecord`] summary
//! — the fields this interface renders — so the adapter type never leaks past
//! the port (the discipline the CRI's own ports use for their records). The
//! shared [`delonix_model::Error`] both the engine and this crate speak is the
//! error on every fallible method; the pure netns-publish rules and the SDN
//! provider report (a shared [`delonix_compute::capability::ProviderReport`])
//! cross as themselves.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use delonix_compute::capability::ProviderReport;
use delonix_compute::Container;
use delonix_model::Error;

/// A declared network as this interface reads it: the fields of `delonix-sdn`'s
/// `Network` the node API actually renders, crossed as a summary so the adapter
/// type does not leak past the port. `Debug`-derived on purpose — a
/// `GetNetwork`'s `etag` is a hash of this record, so the summary must carry
/// everything that distinguishes one network from another (a relabel, a changed
/// subnet) for the etag to move when the resource does.
#[derive(Clone, Debug)]
pub struct NetRecord {
    /// The network's name (its identity at the node).
    pub name: String,
    /// `"bridge"` (the default/user bridge), `"overlay"`, `"macvlan"`/`"ipvlan"`.
    pub driver: String,
    /// The IPv4 CIDR, e.g. `10.88.0.0/16`.
    pub subnet: String,
    /// The host bridge device name.
    pub bridge: String,
    /// The gateway address.
    pub gateway: String,
    /// The physical parent NIC (`macvlan`/`ipvlan` only).
    pub parent: Option<String>,
    /// The VXLAN Network Identifier (`overlay` only).
    pub vni: Option<u32>,
    /// This node's WireGuard tunnel IP (encrypted overlay only); its presence
    /// alone is what marks the overlay encrypted.
    pub wg_ip: Option<String>,
    /// The raw overlay peer entries, parsed with [`NetworkOps::parse_overlay_peer`].
    pub peers: Vec<String>,
    /// Free labels (a `BTreeMap`, so the `Debug` and thus the etag are
    /// deterministic, and a label selector reads it directly).
    pub labels: BTreeMap<String, String>,
    /// Free annotations.
    pub annotations: BTreeMap<String, String>,
}

/// The SDN use cases the node API reads and writes through (R3d-f). Object-safe
/// (`dyn NetworkOps`): every method names only shared types — [`NetRecord`], the
/// shared [`Error`], [`Container`] and [`ProviderReport`] — never a `delonix-sdn`
/// type, so the implementor lives in the composition and the interface stays
/// clean of the adapter.
pub trait NetworkOps: Send + Sync {
    // ---- the declarative network store ----

    /// Every declared network under `root`, as [`NetRecord`]s. The caller checks
    /// the store directory exists first (reading must not create it).
    fn list(&self, root: &Path) -> Result<Vec<NetRecord>, Error>;
    /// One declared network under `root`, by name.
    fn get(&self, root: &Path, name: &str) -> Result<NetRecord, Error>;
    /// Stamp `(label, Some(value))`/`None` pairs on a declared network's record.
    fn set_metadata(
        &self,
        root: &Path,
        name: &str,
        labels: &[(String, Option<String>)],
        annotations: &[(String, Option<String>)],
    ) -> Result<(), Error>;
    /// Refuse a name the engine would never accept (before any operation exists).
    fn validate_name(&self, name: &str) -> Result<(), Error>;
    /// Refuse a subnet the engine would never accept.
    fn validate_subnet(&self, cidr: &str) -> Result<(), Error>;

    // ---- the dataplane writes ----

    /// Create the bridge network `name` under `root`: its declarative record,
    /// its allocated subnet (or `subnet` when given) and its dataplane
    /// realization, rolling the record back if the realization fails.
    fn create_bridge(&self, root: &Path, name: &str, subnet: Option<&str>) -> Result<(), Error>;
    /// Remove the network `name` under `root` — its record and its dataplane.
    fn remove(&self, root: &Path, name: &str) -> Result<(), Error>;
    /// Who, among these container and `(vm name, its network)` records, is on
    /// network `name`/`subnet`, as `container <n>`/`vm <n>` labels.
    fn dependents_of(
        &self,
        name: &str,
        subnet: &str,
        containers: &[Container],
        vms: &[(String, String)],
    ) -> Vec<String>;

    // ---- the dataplane reads ----

    /// The names that have a dataplane record under `root` (what is realized).
    fn realized_names(&self, root: &Path) -> Vec<String>;
    /// Whether `ip`/`mac` on `network` answers within `timeout`. `None` means the
    /// node cannot ask at all (the holder is down, no `ip(8)`), which is not a
    /// "no".
    fn reachable(&self, network: &str, ip: &str, mac: &str, timeout: Duration) -> Option<bool>;

    // ---- the SDN provider reports ----

    /// This host's SDN provider report(s), MEASURED (the probe) — extended into
    /// the node's measured provider list, the way the VM backends come through
    /// [`crate::vms::VmPort::provider_reports`].
    fn provider_reports(&self) -> Vec<ProviderReport>;
    /// The same, DECLARED (host assumed complete) — what the published-matrix
    /// equality test reads.
    fn declared_provider_reports(&self) -> Vec<ProviderReport>;
    /// The ids registered as gateway providers (ADR-0059 D2): the node's
    /// perimeter-appliance report carries «configured» when its id is among them.
    fn gateway_provider_ids(&self) -> Vec<String>;

    // ---- the pure rules ----

    /// The reserved default network's name (`delonix0`/`docker0` style).
    fn default_net(&self) -> &'static str;
    /// Parse one overlay peer entry into `(node_ip, Some((wg_pubkey, wg_ip)))`
    /// (`None` for a flat VXLAN peer).
    fn parse_overlay_peer(&self, peer: &str) -> (String, Option<(String, String)>);
    /// Expand a publish spec that may carry a port RANGE into one spec per port,
    /// so a malformed spec is refused before an operation exists.
    fn expand_publish_range(&self, spec: &str) -> Result<Vec<String>, Error>;
    /// Canonicalise one publish spec to the form the container record stores.
    fn normalize_publish_spec(&self, spec: &str) -> Result<String, Error>;
}

/// A [`NetworkOps`] with no composed SDN — the empty backing [`crate::router`]
/// hands the server when no engine is injected. Reads are empty, writes are
/// accepted and discarded; the pure rules answer the safe nothing. The
/// production server never uses it — a bin hands in the `delonix-sdn`-backed
/// port — exactly as [`crate::store::NoStore`] is to [`crate::store::StorePort`].
/// The network/create paths that would rely on these answers are not exercised
/// through the default router (its tests drive node/provider reads and refusals).
pub(crate) struct NoNet;

impl NetworkOps for NoNet {
    fn list(&self, _root: &Path) -> Result<Vec<NetRecord>, Error> {
        Ok(Vec::new())
    }
    fn get(&self, _root: &Path, name: &str) -> Result<NetRecord, Error> {
        Err(Error::NotFound(name.to_string()))
    }
    fn set_metadata(
        &self,
        _root: &Path,
        _name: &str,
        _labels: &[(String, Option<String>)],
        _annotations: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        Ok(())
    }
    fn validate_name(&self, _name: &str) -> Result<(), Error> {
        Ok(())
    }
    fn validate_subnet(&self, _cidr: &str) -> Result<(), Error> {
        Ok(())
    }
    fn create_bridge(&self, _root: &Path, _name: &str, _subnet: Option<&str>) -> Result<(), Error> {
        Ok(())
    }
    fn remove(&self, _root: &Path, _name: &str) -> Result<(), Error> {
        Ok(())
    }
    fn dependents_of(
        &self,
        _name: &str,
        _subnet: &str,
        _containers: &[Container],
        _vms: &[(String, String)],
    ) -> Vec<String> {
        Vec::new()
    }
    fn realized_names(&self, _root: &Path) -> Vec<String> {
        Vec::new()
    }
    fn reachable(&self, _network: &str, _ip: &str, _mac: &str, _timeout: Duration) -> Option<bool> {
        None
    }
    fn provider_reports(&self) -> Vec<ProviderReport> {
        Vec::new()
    }
    fn declared_provider_reports(&self) -> Vec<ProviderReport> {
        Vec::new()
    }
    fn gateway_provider_ids(&self) -> Vec<String> {
        Vec::new()
    }
    fn default_net(&self) -> &'static str {
        // The reserved default's name — the one `delonix_sdn::DEFAULT_NET` value,
        // stated here because this backing names no adapter. The production
        // `delonix-sdn`-backed port returns the adapter's own constant.
        "bridge"
    }
    fn parse_overlay_peer(&self, peer: &str) -> (String, Option<(String, String)>) {
        (peer.to_string(), None)
    }
    fn expand_publish_range(&self, spec: &str) -> Result<Vec<String>, Error> {
        Ok(vec![spec.to_string()])
    }
    fn normalize_publish_spec(&self, spec: &str) -> Result<String, Error> {
        Ok(spec.to_string())
    }
}

/// A [`NetworkOps`] over the real `delonix-sdn` adapter, for the crate's own
/// tests only (R3d-f) — the same shape as [`crate::store::LocalStore`] and
/// [`crate::vms::LocalVmReads`]. The server never builds this; production hands
/// in a bin's `delonix-sdn`-backed port. `delonix-sdn` is a dev-dependency, so
/// this (and the matrix/parse tests) may name it while production code does not.
#[cfg(test)]
pub(crate) struct LocalNet;

#[cfg(test)]
fn to_record(n: &delonix_sdn::Network) -> NetRecord {
    NetRecord {
        name: n.name.clone(),
        driver: n.driver.clone(),
        subnet: n.subnet.clone(),
        bridge: n.bridge.clone(),
        gateway: n.gateway.clone(),
        parent: n.parent.clone(),
        vni: n.vni,
        wg_ip: n.wg_ip.clone(),
        peers: n.peers.clone(),
        labels: n.labels.clone(),
        annotations: n.annotations.clone(),
    }
}

#[cfg(test)]
impl NetworkOps for LocalNet {
    fn list(&self, root: &Path) -> Result<Vec<NetRecord>, Error> {
        Ok(delonix_sdn::NetworkStore::open(root)?
            .list()?
            .iter()
            .map(to_record)
            .collect())
    }
    fn get(&self, root: &Path, name: &str) -> Result<NetRecord, Error> {
        Ok(to_record(
            &delonix_sdn::NetworkStore::open(root)?.get(name)?,
        ))
    }
    fn set_metadata(
        &self,
        root: &Path,
        name: &str,
        labels: &[(String, Option<String>)],
        annotations: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        delonix_sdn::NetworkStore::open(root)?.set_metadata(name, labels, annotations)?;
        Ok(())
    }
    fn validate_name(&self, name: &str) -> Result<(), Error> {
        delonix_sdn::NetworkStore::validate_name(name)?;
        Ok(())
    }
    fn validate_subnet(&self, cidr: &str) -> Result<(), Error> {
        delonix_sdn::NetworkStore::validate_subnet(cidr)?;
        Ok(())
    }
    fn create_bridge(&self, root: &Path, name: &str, subnet: Option<&str>) -> Result<(), Error> {
        let store = delonix_sdn::NetworkStore::open(root)?;
        delonix_sdn::netops::create_bridge(&store, name, subnet, |_| Ok(None))?;
        Ok(())
    }
    fn remove(&self, root: &Path, name: &str) -> Result<(), Error> {
        let store = delonix_sdn::NetworkStore::open(root)?;
        delonix_sdn::netops::remove(&store, name)?;
        Ok(())
    }
    fn dependents_of(
        &self,
        name: &str,
        subnet: &str,
        containers: &[Container],
        vms: &[(String, String)],
    ) -> Vec<String> {
        delonix_sdn::netops::dependents_of(name, subnet, containers, vms)
    }
    fn realized_names(&self, root: &Path) -> Vec<String> {
        delonix_sdn::infra::network_list_in(root)
            .into_iter()
            .map(|d| d.name)
            .collect()
    }
    fn reachable(&self, network: &str, ip: &str, mac: &str, timeout: Duration) -> Option<bool> {
        delonix_sdn::infra::sdn_reachable(network, ip, mac, timeout)
    }
    fn provider_reports(&self) -> Vec<ProviderReport> {
        vec![delonix_sdn::provider_report::report(
            &delonix_sdn::provider_report::SdnHost::probe(),
        )]
    }
    fn declared_provider_reports(&self) -> Vec<ProviderReport> {
        vec![delonix_sdn::provider_report::report(
            &delonix_sdn::provider_report::SdnHost::ASSUMED,
        )]
    }
    fn gateway_provider_ids(&self) -> Vec<String> {
        delonix_sdn::gateway::gateway_provider_ids()
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn default_net(&self) -> &'static str {
        delonix_sdn::DEFAULT_NET
    }
    fn parse_overlay_peer(&self, peer: &str) -> (String, Option<(String, String)>) {
        delonix_sdn::parse_overlay_peer(peer)
    }
    fn expand_publish_range(&self, spec: &str) -> Result<Vec<String>, Error> {
        delonix_sdn::expand_publish_range(spec).map_err(Into::into)
    }
    fn normalize_publish_spec(&self, spec: &str) -> Result<String, Error> {
        delonix_sdn::normalize_publish_spec(spec).map_err(Into::into)
    }
}
