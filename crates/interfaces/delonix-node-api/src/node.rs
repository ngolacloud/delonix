//! `GetNodeInfo`, `GetHealth` and `GetCapacity` (ADR-0042 step C): what this
//! node is, whether it can run workloads, and what room it has.
//!
//! Every answer is computed at call time from the same functions the CLI uses
//! (`delonix system info`, `provider ls`), never from a second copy of their
//! rules. A value that could not be measured is said so — `UNKNOWN` in a
//! condition, a name in `Capacity.unmeasured` — and never reported as zero or
//! as healthy: "unknown" and "no" have opposite remedies.

use std::path::{Path, PathBuf};

use delonix_compute::capability::{HealthStatus, ProviderKind, ProviderReport};
use delonix_model::records::Status;

use crate::proto::v1::{
    ApiRoot, Capacity, Condition, ConditionStatus, DeviceCapacity, FilesystemCapacity, Health,
    IpPoolCapacity, Link, NodeInfo,
};

/// The engine's state root: `$DELONIX_ROOT`, else the rootless (`$XDG_DATA_HOME`/
/// `$HOME/.local/share`) or root (`/var/lib/delonix`) default. This is the node's
/// own copy of the delonix-wide root convention (the same rule the CLI bins and
/// the adapter apply); R3d-d stopped resolving it through `delonix_state::Store`
/// so the interface no longer names the adapter for a value that is pure env/uid.
pub fn state_root() -> PathBuf {
    if let Some(root) = std::env::var_os("DELONIX_ROOT") {
        return PathBuf::from(root);
    }
    // SAFETY: geteuid() is always safe and does not fail.
    if unsafe { libc::geteuid() } != 0 {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(|| PathBuf::from("."));
        return base.join("delonix");
    }
    PathBuf::from("/var/lib/delonix")
}

/// The contract's API identity (ADR-0042 D1).
pub const API_VERSION: &str = "delonix.node.v1";

/// A `GET` link. Every link this socket offers today is a `GET`: the served
/// routes are read-only (`the_rest_routes_are_get_only`).
pub fn link(rel: &str, href: &str) -> Link {
    Link {
        rel: rel.to_string(),
        href: href.to_string(),
        method: "GET".to_string(),
    }
}

/// `GetApiRoot` (ADR-0042 D2): the version and a link to every resource this
/// socket serves — and only those: a link to an operation the server would
/// answer 501 is a promise it does not keep.
pub fn api_root() -> ApiRoot {
    ApiRoot {
        api_version: API_VERSION.to_string(),
        links: vec![
            link("self", "/v1"),
            link("node", "/v1/node"),
            link("health", "/v1/node/health"),
            link("capacity", "/v1/node/capacity"),
            link("providers", "/v1/providers"),
            link("networks", "/v1/namespaces/default/networks"),
            link("volumes", "/v1/namespaces/default/volumes"),
            link("operations", "/v1/operations"),
            link("openapi", "/openapi.json"),
            link("docs", "/docs"),
            link("redoc", "/redoc"),
        ],
    }
}

fn read_trimmed(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Backends whose VMs run ON this node. A remote backend's VMs (a Proxmox
/// node) are not this node's capacity, and reconciling them would be a round
/// trip per VM.
const LOCAL_VM_BACKENDS: &[&str] = &["libvirt", "cloud-hypervisor"];

fn compute(reports: &[ProviderReport], id: &str) -> Option<bool> {
    reports
        .iter()
        .find(|r| r.kind == ProviderKind::Compute && r.id == id)
        .map(|r| r.available)
}

/// The workload types this node can run right now, from the provider reports:
/// containers and pods with the Linux compute provider; VMs with any VM
/// backend available here; microVMs with Cloud Hypervisor.
pub fn supported_workload_types(reports: &[ProviderReport]) -> Vec<String> {
    let mut out = Vec::new();
    if compute(reports, "linux") == Some(true) {
        out.push("container".to_string());
        out.push("pod".to_string());
    }
    let vm = reports
        .iter()
        .any(|r| r.kind == ProviderKind::Compute && r.id != "linux" && r.available);
    if vm {
        out.push("vm".to_string());
    }
    if compute(reports, "cloud-hypervisor") == Some(true) {
        out.push("microvm".to_string());
    }
    out
}

/// `GetNodeInfo`. `record` is the node's scheduling state (ADR-0101): its labels,
/// taints, cordon flag and version come off the shared record, read by the service
/// layer; the rest are host probes computed at call time.
pub fn node_info(
    reports: &[ProviderReport],
    linux: &dyn crate::linux_lifecycle::LinuxLifecycle,
    record: &delonix_model::node_meta::NodeRecord,
) -> NodeInfo {
    NodeInfo {
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
        engine_commit: env!("DELONIX_GIT_HASH").to_string(),
        api_version: API_VERSION.to_string(),
        hostname: read_trimmed("/proc/sys/kernel/hostname").unwrap_or_default(),
        kernel: read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_default(),
        arch: std::env::consts::ARCH.to_string(),
        rootless: linux.is_rootless(),
        // The engine writes cgroupfs itself and never asks systemd for units —
        // the same answer the CRI gives the kubelet (`RuntimeConfig`).
        cgroup_driver: "cgroupfs".to_string(),
        cgroup_delegated: linux.cgroup_limits_apply(),
        supported_workload_types: supported_workload_types(reports),
        // Only the user's placement labels — the reconciler's own `delonix.io/*`
        // bookkeeping is filtered out, as A4 filters the version annotations.
        labels: delonix_model::node_meta::user_labels(&record.metadata.labels)
            .into_iter()
            .collect(),
        taints: record
            .spec
            .taints
            .iter()
            .map(crate::node_meta::taint_to_proto)
            .collect(),
        unschedulable: !record.spec.schedulable,
        unschedulable_reason: record.spec.unschedulable_reason.clone(),
        generation: record.generation(),
        observed_generation: record.observed_generation(),
        links: vec![
            link("self", "/v1/node"),
            link("root", "/v1"),
            link("health", "/v1/node/health"),
            link("capacity", "/v1/node/capacity"),
        ],
    }
}

fn condition(kind: &str, status: ConditionStatus, reason: &str, message: String) -> Condition {
    Condition {
        r#type: kind.to_string(),
        status: status as i32,
        reason: reason.to_string(),
        message,
        last_transition_time: None,
    }
}

fn from_health(status: &HealthStatus) -> ConditionStatus {
    match status {
        HealthStatus::Healthy => ConditionStatus::True,
        HealthStatus::Unavailable => ConditionStatus::False,
        HealthStatus::Unknown => ConditionStatus::Unknown,
    }
}

/// Whether the store can be written, asked without writing anything: a health
/// check that created the state root or left a file would change the node it
/// reports on. `access(W_OK)` on the root, or — before the first workload
/// creates it — on its nearest existing ancestor (the root is then created on
/// demand). `access` answers for the permissions and for a read-only mount
/// (`EROFS`).
pub fn store_writable(root: &Path) -> Condition {
    use std::os::unix::ffi::OsStrExt;
    let existing = root
        .ancestors()
        .find(|p| p.exists())
        .unwrap_or(Path::new("/"));
    let writable = std::ffi::CString::new(existing.as_os_str().as_bytes())
        .ok()
        // SAFETY: a valid NUL-terminated path; `access` only reads it.
        .map(|c| unsafe { libc::access(c.as_ptr(), libc::W_OK) } == 0);
    let not_yet = existing != root;
    match writable {
        Some(true) if not_yet => condition(
            "StoreWritable",
            ConditionStatus::True,
            "CreatedOnDemand",
            format!(
                "{} does not exist yet; {} is writable and it is created with the first workload",
                root.display(),
                existing.display()
            ),
        ),
        Some(true) => condition(
            "StoreWritable",
            ConditionStatus::True,
            "Writable",
            format!("{} is writable", root.display()),
        ),
        _ => condition(
            "StoreWritable",
            ConditionStatus::False,
            "NotWritable",
            format!(
                "{}: {}",
                existing.display(),
                std::io::Error::last_os_error()
            ),
        ),
    }
}

/// The conditions of `GetHealth`, given the measured parts. Pure, so the
/// worst-of rule and each reason are tested without the host.
pub fn health_from(store: Condition, cgroup_delegated: bool, reports: &[ProviderReport]) -> Health {
    let cgroup = if cgroup_delegated {
        condition(
            "CgroupDelegated",
            ConditionStatus::True,
            "Delegated",
            "memory, cpu and pids limits are enforced".to_string(),
        )
    } else {
        condition(
            "CgroupDelegated",
            ConditionStatus::False,
            "NotDelegated",
            "memory, cpu and pids limits are NOT enforced: run under systemd-run --user \
             --scope -p Delegate=yes"
                .to_string(),
        )
    };
    // The network provider's own health (`provider ls`), not a second probe.
    let network = match reports
        .iter()
        .find(|r| r.kind == ProviderKind::Network && r.id == "linux")
    {
        Some(r) => condition(
            "NetworkReady",
            from_health(&r.health.status),
            r.health.reason,
            r.health.message.clone(),
        ),
        None => condition(
            "NetworkReady",
            ConditionStatus::Unknown,
            "NoReport",
            "the node's network provider gave no report".to_string(),
        ),
    };
    // The workloads this node runs itself need the Linux compute provider; a
    // backend that is merely not installed (or a remote one not configured) is
    // named, and does not make the node unhealthy.
    let providers = match compute(reports, "linux") {
        Some(true) => {
            let mut down: Vec<&str> = reports
                .iter()
                .filter(|r| r.kind == ProviderKind::Compute && !r.available)
                .map(|r| r.id.as_str())
                .collect();
            down.sort_unstable();
            let message = if down.is_empty() {
                "every compute provider is available".to_string()
            } else {
                format!(
                    "the linux compute provider is available; not available here: {}",
                    down.join(", ")
                )
            };
            condition(
                "ProvidersAvailable",
                ConditionStatus::True,
                "LinuxAvailable",
                message,
            )
        }
        Some(false) => condition(
            "ProvidersAvailable",
            ConditionStatus::False,
            "LinuxUnavailable",
            reports
                .iter()
                .find(|r| r.kind == ProviderKind::Compute && r.id == "linux")
                .map(|r| r.health.message.clone())
                .unwrap_or_default(),
        ),
        None => condition(
            "ProvidersAvailable",
            ConditionStatus::Unknown,
            "NoReport",
            "the linux compute provider gave no report".to_string(),
        ),
    };
    let conditions = vec![network, store, cgroup, providers];
    Health {
        overall: worst(&conditions) as i32,
        conditions,
        links: vec![link("self", "/v1/node/health"), link("node", "/v1/node")],
    }
}

/// Overall = worst of the parts: any FALSE is FALSE, else any UNKNOWN is
/// UNKNOWN, else TRUE.
pub fn worst(conditions: &[Condition]) -> ConditionStatus {
    let has = |s: ConditionStatus| conditions.iter().any(|c| c.status == s as i32);
    if has(ConditionStatus::False) {
        ConditionStatus::False
    } else if has(ConditionStatus::Unknown) || conditions.is_empty() {
        ConditionStatus::Unknown
    } else {
        ConditionStatus::True
    }
}

/// `GetHealth`.
pub fn health(
    reports: &[ProviderReport],
    linux: &dyn crate::linux_lifecycle::LinuxLifecycle,
) -> Health {
    health_from(
        store_writable(&state_root()),
        linux.cgroup_limits_apply(),
        reports,
    )
}

/// A cgroup v2 limit file: `None` when unreadable, `Some(None)` for `max`.
fn cgroup_limit(path: &str) -> Option<Option<String>> {
    let v = read_trimmed(path)?;
    Some((v != "max").then_some(v))
}

/// `cpu.max` (`<quota> <period>` or `max <period>`) as millicores; `max` is
/// the whole host.
pub fn cpu_max_millis(raw: &str, total: i64) -> Option<i64> {
    let mut parts = raw.split_whitespace();
    let quota = parts.next()?;
    let period: i64 = parts.next().unwrap_or("100000").parse().ok()?;
    if quota == "max" {
        return Some(total);
    }
    let quota: i64 = quota.parse().ok()?;
    (period > 0).then(|| (quota * 1000 / period).min(total))
}

/// The mount point holding `path`: the highest ancestor on the same device.
fn mountpoint(path: &Path) -> PathBuf {
    use std::os::unix::fs::MetadataExt;
    let Ok(dev) = std::fs::metadata(path).map(|m| m.dev()) else {
        return path.to_path_buf();
    };
    let mut cur = path.to_path_buf();
    while let Some(parent) = cur.parent() {
        match std::fs::metadata(parent) {
            Ok(m) if m.dev() == dev => cur = parent.to_path_buf(),
            _ => break,
        }
    }
    cur
}

fn filesystem(role: &str, path: &Path) -> Option<FilesystemCapacity> {
    use std::os::unix::ffi::OsStrExt;
    let c = std::ffi::CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: `statvfs` is a plain C struct of integers; all-zero is a valid
    // value for it.
    let mut st: libc::statvfs = unsafe { std::mem::zeroed() };
    // SAFETY: `c` is a valid NUL-terminated path and `st` an out-param statvfs
    // fills on success; nothing is read from it on failure.
    if unsafe { libc::statvfs(c.as_ptr(), &mut st) } != 0 {
        return None;
    }
    let frsize = st.f_frsize as i64;
    Some(FilesystemCapacity {
        role: role.to_string(),
        mountpoint: mountpoint(path).display().to_string(),
        bytes_total: st.f_blocks as i64 * frsize,
        bytes_available: st.f_bavail as i64 * frsize,
        inodes_total: st.f_files as i64,
        inodes_available: st.f_favail as i64,
    })
}

/// `GetCapacity`.
pub fn capacity(
    vms: &dyn crate::vms::VmPort,
    store: &dyn crate::store::StorePort,
    linux: &dyn crate::linux_lifecycle::LinuxLifecycle,
    record: &delonix_model::node_meta::NodeRecord,
) -> Capacity {
    let mut c = Capacity::default();
    let mut unmeasured: Vec<&str> = Vec::new();
    let slice = linux.slice_path();
    let limit = |file: &str| {
        slice
            .as_deref()
            .and_then(|s| cgroup_limit(&format!("{s}/{file}")))
    };

    let ncpu = linux.host_ncpu() as i64;
    if ncpu > 0 {
        c.cpu_millis_total = ncpu * 1000;
        match slice
            .as_deref()
            .and_then(|s| read_trimmed(&format!("{s}/cpu.max")))
            .and_then(|raw| cpu_max_millis(&raw, c.cpu_millis_total))
        {
            Some(m) => c.cpu_millis_allocatable = m,
            None => unmeasured.push("cpu_millis_allocatable"),
        }
    } else {
        unmeasured.extend(["cpu_millis_total", "cpu_millis_allocatable"]);
    }

    let mem = linux.host_mem_bytes() as i64;
    if mem > 0 {
        c.memory_bytes_total = mem;
        match limit("memory.max") {
            Some(None) => c.memory_bytes_allocatable = mem,
            Some(Some(v)) => match v.parse::<i64>() {
                Ok(b) => c.memory_bytes_allocatable = b.min(mem),
                Err(_) => unmeasured.push("memory_bytes_allocatable"),
            },
            None => unmeasured.push("memory_bytes_allocatable"),
        }
    } else {
        unmeasured.extend(["memory_bytes_total", "memory_bytes_allocatable"]);
    }

    let pid_max = read_trimmed("/proc/sys/kernel/pid_max").and_then(|v| v.parse::<i64>().ok());
    match (limit("pids.max"), pid_max) {
        (Some(Some(v)), _) if v.parse::<i64>().is_ok() => {
            c.pids_allocatable = v.parse::<i64>().unwrap_or_default();
        }
        (Some(None), Some(max)) => c.pids_allocatable = max,
        _ => unmeasured.push("pids_allocatable"),
    }

    let root = state_root();
    for (role, dir) in [
        ("state", root.clone()),
        ("images", root.join("blobs")),
        ("volumes", root.join("volumes")),
        ("vm-images", root.join("vm-images")),
    ] {
        // A store not created yet has no filesystem to report: absent, not
        // unmeasured. One that exists and cannot be read IS unmeasured.
        if !dir.exists() {
            continue;
        }
        match filesystem(role, &dir) {
            Some(fs) => c.filesystems.push(fs),
            None => unmeasured.push(match role {
                "state" => "filesystems.state",
                "images" => "filesystems.images",
                "volumes" => "filesystems.volumes",
                _ => "filesystems.vm-images",
            }),
        }
    }

    match store.container_list() {
        Ok(cs) => {
            c.containers_running = cs
                .iter()
                .filter(|x| matches!(x.status, Status::Running))
                .count() as i32
        }
        Err(_) => unmeasured.push("containers_running"),
    }

    match store.vm_list() {
        Ok(records) => {
            c.vms_running = records
                .iter()
                .filter(|v| LOCAL_VM_BACKENDS.contains(&v.backend.as_str()))
                .filter(|v| {
                    // Reconciled one by one through the composed port, local
                    // backends only (a record can say Running for a VM whose
                    // process died).
                    vms.status(&v.name)
                        .map(|s| matches!(s.status, Status::Running))
                        .unwrap_or(false)
                })
                .count() as i32
        }
        Err(_) => unmeasured.push("vms_running"),
    }

    // Extended resources (ADR-0101, A6), each by the same doctrine as the axes above.

    // GPU: a cheap probe of the NVIDIA driver's own listing — no `nvidia-smi` in a
    // capacity read. Present ⇒ one device line per entry. Absent ⇒ unmeasured, because a
    // missing driver cannot be told apart from "no GPU" with certainty (a non-NVIDIA
    // accelerator would also be missed), and a silent zero would read as "no GPU here".
    match gpu_count() {
        Some(n) => c.devices.push(DeviceCapacity {
            kind: "nvidia.com/gpu".to_string(),
            total: n,
            allocatable: n,
        }),
        None => unmeasured.push("devices.gpu"),
    }

    // IPs: the node's declarative IPPool inventory. No pool declared ⇒ nothing reported
    // (absent, not unmeasured — the same rule the filesystems use just above).
    c.ip_pools = ip_pool_capacities(&root);

    // Overcommit: the operator's declaration on the node record (set via `kind: Node`).
    // Unset ⇒ both ratios are unmeasured — the engine has no node-level overcommit of
    // its own, so an un-declared one is unmeasured, not the misleading `1.0`.
    match &record.spec.overcommit {
        Some(oc) => c.overcommit = Some(crate::node_meta::overcommit_to_proto(oc)),
        None => unmeasured.extend(["overcommit.cpu", "overcommit.memory"]),
    }

    c.unmeasured = unmeasured.into_iter().map(str::to_string).collect();
    c.links = vec![link("self", "/v1/node/capacity"), link("node", "/v1/node")];
    c
}

/// The number of GPUs the NVIDIA driver lists under `/proc/driver/nvidia/gpus/`, or
/// `None` when the driver is not present (so the caller reports GPU as `unmeasured`
/// rather than `0`). One subdirectory per GPU; cheap, no subprocess.
fn gpu_count() -> Option<i64> {
    let entries = std::fs::read_dir("/proc/driver/nvidia/gpus").ok()?;
    let n = entries.flatten().filter(|e| e.path().is_dir()).count();
    Some(n as i64)
}

/// The node's IP inventory from the declarative IPPool ledger under `<root>/ippool/`.
/// One entry per pool: `addresses_total` counted with the shared `delonix_sdn::Cidr`
/// (so CIDR math never drifts from the binary's `expand`), `addresses_allocated` the
/// live lease count. An absent directory yields an empty list (absent, not unmeasured).
fn ip_pool_capacities(root: &Path) -> Vec<IpPoolCapacity> {
    let dir = root.join("ippool");
    let mut pools: Vec<IpPoolCapacity> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter_map(|e| {
            let v: serde_json::Value =
                serde_json::from_slice(&std::fs::read(e.path()).ok()?).ok()?;
            let name = v.get("name")?.as_str()?.to_string();
            let total: i64 = v
                .get("addresses")
                .and_then(|a| a.as_array())
                .map(|entries| {
                    entries
                        .iter()
                        .filter_map(|x| x.as_str())
                        .map(address_entry_count)
                        .sum()
                })
                .unwrap_or(0);
            let allocated = v
                .get("leases")
                .and_then(|l| l.as_object())
                .map_or(0, |m| m.len() as i64);
            Some(IpPoolCapacity {
                name,
                addresses_total: total,
                addresses_allocated: allocated,
            })
        })
        .collect();
    pools.sort_by(|a, b| a.name.cmp(&b.name));
    pools
}

/// The number of host addresses one IPPool `addresses` entry covers — a single IP, an
/// `a-b` range, or an `a.b.c.d/len` CIDR (network/broadcast excluded for a `/30` or wider,
/// kept for `/31` and `/32`). Mirrors the arithmetic of the binary's
/// `ippool::parse_entry`; computed with `std::net` so this interface crate takes no
/// adapter dependency. An entry that does not parse counts as 0 rather than failing the
/// whole report. Kept in lockstep with `bins/delonix/src/cmd/ippool.rs::parse_entry`.
fn address_entry_count(entry: &str) -> i64 {
    use std::net::Ipv4Addr;
    let v4 = |s: &str| s.trim().parse::<Ipv4Addr>().ok().map(u32::from);
    let (first, last) = if let Some((a, b)) = entry.split_once('-') {
        match (v4(a), v4(b)) {
            (Some(a), Some(b)) => (a, b),
            _ => return 0,
        }
    } else if let Some((addr, len)) = entry.split_once('/') {
        let (Some(base), Ok(len)) = (v4(addr), len.trim().parse::<u8>()) else {
            return 0;
        };
        if len > 32 {
            return 0;
        }
        // Mask the host bits off the base, then the broadcast is base | ~mask.
        let mask: u32 = if len == 0 { 0 } else { u32::MAX << (32 - len) };
        let network = base & mask;
        let broadcast = network | !mask;
        // A /30 or wider leaves out the network and broadcast addresses; a /31 and
        // /32 have neither to leave out.
        if len <= 30 {
            (network + 1, broadcast - 1)
        } else {
            (network, broadcast)
        }
    } else {
        match v4(entry) {
            Some(a) => (a, a),
            None => return 0,
        }
    };
    if first > last {
        0
    } else {
        i64::from(last - first) + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use delonix_compute::capability::ProviderHealth;

    fn report(
        kind: ProviderKind,
        id: &str,
        available: bool,
        status: HealthStatus,
    ) -> ProviderReport {
        ProviderReport {
            id: id.to_string(),
            kind,
            available,
            health: ProviderHealth {
                status,
                reason: if available { "Ready" } else { "Missing" },
                message: format!("{id} message"),
            },
            capabilities: Vec::new(),
        }
    }

    fn host(ch: bool) -> Vec<ProviderReport> {
        vec![
            report(ProviderKind::Compute, "linux", true, HealthStatus::Healthy),
            report(
                ProviderKind::Compute,
                "libvirt",
                true,
                HealthStatus::Healthy,
            ),
            report(
                ProviderKind::Compute,
                "cloud-hypervisor",
                ch,
                HealthStatus::Healthy,
            ),
            report(
                ProviderKind::Compute,
                "proxmox",
                false,
                HealthStatus::Unknown,
            ),
            report(ProviderKind::Network, "linux", true, HealthStatus::Healthy),
        ]
    }

    fn writable() -> Condition {
        condition(
            "StoreWritable",
            ConditionStatus::True,
            "Writable",
            String::new(),
        )
    }

    #[test]
    fn workload_types_follow_the_providers_available_here() {
        assert_eq!(
            supported_workload_types(&host(true)),
            vec!["container", "pod", "vm", "microvm"]
        );
        assert_eq!(
            supported_workload_types(&host(false)),
            vec!["container", "pod", "vm"]
        );
        let mut no_linux = host(false);
        no_linux[0].available = false;
        no_linux[1].available = false;
        assert!(supported_workload_types(&no_linux).is_empty());
    }

    #[test]
    fn health_is_the_worst_of_its_parts_and_names_what_is_missing() {
        let h = health_from(writable(), true, &host(false));
        assert_eq!(h.overall, ConditionStatus::True as i32);
        let reasons: Vec<&str> = h.conditions.iter().map(|c| c.r#type.as_str()).collect();
        assert_eq!(
            reasons,
            vec![
                "NetworkReady",
                "StoreWritable",
                "CgroupDelegated",
                "ProvidersAvailable"
            ]
        );
        let p = &h.conditions[3];
        assert!(
            p.message.contains("cloud-hypervisor, proxmox"),
            "{}",
            p.message
        );

        // Undelegated cgroups: FALSE, and the overall follows.
        let h = health_from(writable(), false, &host(false));
        assert_eq!(h.overall, ConditionStatus::False as i32);
        assert!(h.conditions[2].message.contains("Delegate=yes"));

        // A network provider that could not tell: UNKNOWN, never TRUE.
        let mut r = host(false);
        r[4].health.status = HealthStatus::Unknown;
        assert_eq!(
            health_from(writable(), true, &r).overall,
            ConditionStatus::Unknown as i32
        );

        // No linux compute provider at all.
        let mut r = host(false);
        r[0].available = false;
        let h = health_from(writable(), true, &r);
        assert_eq!(h.conditions[3].reason, "LinuxUnavailable");
        assert_eq!(h.overall, ConditionStatus::False as i32);
    }

    #[test]
    fn the_store_check_writes_nothing_and_says_what_it_found() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let c = store_writable(dir.path());
        assert_eq!(
            (c.status, c.reason.as_str()),
            (ConditionStatus::True as i32, "Writable")
        );
        let missing = dir.path().join("not/yet");
        let c = store_writable(&missing);
        assert_eq!(c.reason, "CreatedOnDemand");
        assert!(!missing.exists(), "a health check must not create the root");
        assert!(
            std::fs::read_dir(dir.path()).unwrap().next().is_none(),
            "nothing written"
        );
        // Read-only for the owner (root ignores modes: only checked as non-root).
        // Through the crate's own `LocalLinux` port, so this test names no linux
        // adapter directly either.
        use crate::linux_lifecycle::LinuxLifecycle;
        if !crate::linux_lifecycle::LocalLinux.is_rootless() {
            return;
        }
        let ro = dir.path().join("ro");
        std::fs::create_dir(&ro).unwrap();
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o555)).unwrap();
        let c = store_writable(&ro);
        std::fs::set_permissions(&ro, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(
            (c.status, c.reason.as_str()),
            (ConditionStatus::False as i32, "NotWritable")
        );
    }

    #[test]
    fn cpu_max_reads_as_millicores() {
        assert_eq!(cpu_max_millis("max 100000", 8000), Some(8000));
        assert_eq!(cpu_max_millis("150000 100000", 8000), Some(1500));
        assert_eq!(cpu_max_millis("900000 100000", 8000), Some(8000));
        assert_eq!(cpu_max_millis("garbage", 8000), None);
        assert_eq!(cpu_max_millis("100000 0", 8000), None);
    }

    #[test]
    fn worst_never_reads_nothing_as_healthy() {
        assert_eq!(worst(&[]), ConditionStatus::Unknown);
    }

    // --- A6 (ADR-0101): the scheduling surface on NodeInfo and Capacity. ---

    #[test]
    fn node_info_reflects_the_record_and_filters_reserved_labels() {
        use delonix_model::node_meta::{NodeRecord, Taint};
        let mut r = NodeRecord::default();
        r.metadata.labels.insert("zone".into(), "lis1".into());
        // A reconciler bookkeeping label must NOT surface to a control plane.
        r.metadata
            .labels
            .insert("delonix.io/stack".into(), "s".into());
        r.spec.taints.push(Taint {
            key: "dedicated".into(),
            value: "ml".into(),
            effect: "NoSchedule".into(),
        });
        r.spec.schedulable = false;
        r.spec.unschedulable_reason = "drain".into();
        r.stamp();

        let info = node_info(&host(true), &super::super::linux_lifecycle::NoLinux, &r);
        assert_eq!(info.labels.get("zone").map(String::as_str), Some("lis1"));
        assert!(
            !info.labels.contains_key("delonix.io/stack"),
            "reserved labels are hidden"
        );
        assert_eq!(info.taints.len(), 1);
        assert_eq!(info.taints[0].effect, "NoSchedule");
        assert!(info.unschedulable);
        assert_eq!(info.unschedulable_reason, "drain");
        assert_eq!(info.generation, 1);
        assert_eq!(info.observed_generation, 1, "node metadata is single-phase");
    }

    #[test]
    fn address_entry_count_covers_single_range_and_cidr() {
        assert_eq!(address_entry_count("10.0.0.5"), 1);
        assert_eq!(address_entry_count("10.0.0.10-10.0.0.19"), 10);
        // /24 excludes network + broadcast ⇒ 254 host addresses.
        assert_eq!(address_entry_count("10.0.0.0/24"), 254);
        // /30 ⇒ 2 host addresses; /31 and /32 keep both/one.
        assert_eq!(address_entry_count("10.0.0.0/30"), 2);
        assert_eq!(address_entry_count("10.0.0.0/31"), 2);
        assert_eq!(address_entry_count("10.0.0.5/32"), 1);
        assert_eq!(address_entry_count("not-an-ip"), 0);
        assert_eq!(address_entry_count("10.0.0.20-10.0.0.10"), 0);
    }

    #[test]
    fn ip_pool_capacities_reads_total_and_allocated_from_the_ledger() {
        let root = std::env::temp_dir().join(format!("delonix-node-ippool-{}", std::process::id()));
        let dir = root.join("ippool");
        std::fs::create_dir_all(&dir).unwrap();
        // One pool of 254 addresses with two leased.
        std::fs::write(
            dir.join("edge.json"),
            br#"{"name":"edge","addresses":["10.0.0.0/24"],"announce":"local","leases":{"a":"10.0.0.5","b":"10.0.0.6"}}"#,
        )
        .unwrap();
        let pools = ip_pool_capacities(&root);
        assert_eq!(pools.len(), 1);
        assert_eq!(pools[0].name, "edge");
        assert_eq!(pools[0].addresses_total, 254);
        assert_eq!(pools[0].addresses_allocated, 2);
        // No ippool directory ⇒ nothing reported (absent, not unmeasured).
        assert!(ip_pool_capacities(std::path::Path::new("/nonexistent-xyz")).is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }
}
