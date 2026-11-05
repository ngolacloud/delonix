//! `delonix-engine` — the engine's single composition root (ADR-0040 D1/A5, the
//! assembly layer). It decides which adapter and which provider serve each port
//! from the node's configuration and capabilities, and exposes one `Engine` with
//! a service per context. An interface receives the `Engine` already mounted and
//! never learns what is underneath (A5).
//!
//! R3a establishes the type and the VM service over the existing composition
//! (`delonix-vm` still seeds the VM backend registry); R3b–e move the container,
//! network, storage, … services here and rewire the interfaces off the adapters
//! onto `Engine`, at which point `delonix-vm` is absorbed and its provider
//! dependencies become this crate's (assembly may depend on a provider; an
//! adapter may not).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use delonix_compute::capability::ProviderReport;
use delonix_compute::{Container, Vm};
use delonix_linux::pressure::{self, CgroupSeries, Why};
use delonix_linux::usage::UsageSample;
use delonix_model::dash::DashSummary;
use delonix_model::records::Status;
use delonix_oci::{Image, ImageStore};
use delonix_sdn::infra::InfraStatus;
use delonix_telemetry::workload::{
    CpuThrottle, MemoryEvents, Pressure as WorkloadPressure, Stall, Unavailable, WorkloadKind,
    WorkloadSeries,
};
use delonix_volume::Volume;
use serde_json::json;

// The VM service's result is the compute VM error (re-exported by delonix-vm).
// As more contexts' services land here, the engine presents each through its own
// context's `Result`; a single DX_* surface over all of them is a later slice.
pub use delonix_vm::{Destroyed, Result};

/// The mounted engine: the state root it operates under and the composed set of
/// providers behind its services. Built once by the bin (or a test) and shared,
/// read-only, by every surface.
pub struct Engine {
    root: PathBuf,
}

impl Engine {
    /// Mount the engine over a `DELONIX_ROOT`. The default composition is today's
    /// — the VM backend registry is seeded by `delonix-vm` from this build's
    /// features; a later slice moves that seeding here, behind a `NodeConfig`.
    pub fn mount(root: impl Into<PathBuf>) -> Result<Self> {
        Ok(Self { root: root.into() })
    }

    /// The state root this engine operates under.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The virtual-machine service (ADR-0044): create, observe and end VMs through
    /// whichever backend the composition selected.
    pub fn vms(&self) -> VmService<'_> {
        VmService { engine: self }
    }

    /// Every registered VM provider's capability report, on this host — what a
    /// caller checks before selecting one (ADR-0050). The composition's own view,
    /// so a surface never reaches a provider crate to ask.
    pub fn provider_reports(&self) -> Vec<ProviderReport> {
        delonix_vm::provider_reports()
    }

    /// The container context's read use cases: the container store and the host
    /// metrics that describe a running container's process (R3e). An interface
    /// calls `engine.containers()` instead of the `delonix-state`/`delonix-linux`
    /// adapters it used to name directly.
    pub fn containers(&self) -> ContainerService<'_> {
        ContainerService { engine: self }
    }

    /// The storage context's named-volume reads (R3e): what the interfaces used to
    /// reach through the `delonix-volume` adapter.
    pub fn volumes(&self) -> VolumeService<'_> {
        VolumeService { engine: self }
    }

    /// The network context's read use cases (R3e): the SDN infra status and a
    /// running container's cumulative byte counters, formerly the interfaces'
    /// direct `delonix-sdn` dependency.
    pub fn networks(&self) -> NetworkService<'_> {
        NetworkService { engine: self }
    }

    /// The image context's read use cases (R3f): the local image store and the
    /// SBOM a scanner derives from an image's layers. What `delonix-mgmt`'s image
    /// endpoints used to reach through the `delonix-oci`/`delonix-scanner`
    /// adapters directly. (Pull and build are not here: they run the engine CLI,
    /// not a library call — the interface keeps shelling out for those.)
    pub fn images(&self) -> ImageService<'_> {
        ImageService { engine: self }
    }

    /// This host's resource snapshot, the capacity/pressure advice that follows,
    /// and whether this node should run a local model — the body `delonix system
    /// resources` and the MCP `resources.get` tool both answer with (R3e). Built
    /// here, where the `delonix-linux` advice rules and their rendered findings
    /// are nameable, so an interface serves one composed JSON value instead of
    /// reaching the adapter and re-deriving the same finding ids a second way.
    ///
    /// The shape is the MCP tool's published contract (ADR-0025): the snapshot
    /// keys (`memory_bytes`, …), the `pressure` array, and each advice line with
    /// its stable `DLX-RES-nnn` id and its sentence rendered in English. Changing
    /// a key here changes that contract.
    pub fn host_resources(&self) -> serde_json::Value {
        use delonix_linux::resource_advice as advice;
        let snap = advice::collect(self.root());
        let findings = advice::advise(&snap);
        let inference = advice::local_inference(&snap);
        // Built before the `json!`: the macro reads a `[` in value position as a
        // JSON array literal and chokes on the `.iter()` that follows.
        let pressure: Vec<_> = [
            ("cpu", snap.psi_cpu),
            ("memory", snap.psi_memory),
            ("io", snap.psi_io),
        ]
        .iter()
        .map(|(r, p)| {
            json!({
                "resource": r,
                "avg10": p.map(|p| p.avg10),
                "avg60": p.map(|p| p.avg60),
                "avg300": p.map(|p| p.avg300),
            })
        })
        .collect();
        json!({
            "host": {
                "cpus": snap.cpus,
                "memory_bytes": snap.mem_total,
                "memory_available_bytes": snap.mem_available,
                "swap_bytes": snap.swap_total,
                "swap_used_bytes": snap.swap_used,
                "state_root_free_bytes": snap.disk_free,
                "cpu_temperature_c": snap.cpu_temp_c,
                "gpu": snap.gpu.as_ref().map(|g| json!({
                    "name": g.name,
                    "vram_total_mib": g.vram_total_mib,
                    "vram_free_mib": g.vram_free_mib,
                    "cdi_spec": g.cdi_spec,
                    "drives_display": g.drives_display,
                })),
            },
            "control": {
                "rootless": snap.rootless,
                "cgroup_base": snap.cgroup_base,
                "delegated": snap.delegated,
                "aggregate_slice": snap.aggregate_slice,
            },
            "pressure": pressure,
            "advice": findings.iter().map(|f| json!({
                "id": f.id,
                "subject": f.subject,
                "severity": f.severity.as_str(),
                "class": f.class.as_str(),
                // Rendered in English here: an MCP client gets the stable `id`
                // for machines and a finished sentence for humans. The template
                // and its holes are the CLI's business, because only the CLI
                // knows which language to render them in.
                "finding": f.finding.render(),
                "action": f.action.as_ref().map(|a| a.render()).unwrap_or_default(),
            })).collect::<Vec<_>>(),
            // Whether this very node should be the one running the model that
            // is reading this. It usually should not, and the reasons say why.
            "local_inference": {
                "verdict": inference.verdict.as_str(),
                "largest_model_b_q4": inference.largest_model_b,
                "reasons": inference.reasons,
            },
        })
    }

    /// A point-in-time [`DashSummary`] — engine-wide counts, cgroup memory, and
    /// (optionally) per-container network totals and the per-area disk walk
    /// (R3f-B). The single place that aggregates "how much is this node using",
    /// composed from the engine's own services instead of an interface reaching
    /// the stores directly; `delonix-mgmt`'s `/metrics`+`/v1/dash` and
    /// `delonix-mcp`'s `runtime.info`/`metrics.query` both answer with it.
    ///
    /// Best-effort throughout: a store that doesn't open counts as empty/zero
    /// (this must never fail a scrape or a dashboard render over one missing
    /// store). `include_network` enters each running container's netns (one
    /// `nsenter` per container); `include_storage` walks several store areas (over
    /// a minute on a heavy host) — a tight-budget caller passes `false` and shows
    /// the last background value.
    pub fn dash_summary(&self, include_network: bool, include_storage: bool) -> DashSummary {
        let root = self.root();
        let mut containers_running = 0u64;
        let mut containers_total = 0u64;
        let mut running_ids: Vec<String> = Vec::new();
        if let Ok(store) = delonix_state::Store::open(root.join("containers")) {
            if let Ok(list) = store.list() {
                containers_total = list.len() as u64;
                for mut c in list {
                    delonix_linux::reconcile_status(&mut c);
                    if c.status == Status::Running {
                        containers_running += 1;
                        running_ids.push(c.id.clone());
                    }
                }
            }
        }

        let vm_list = self.vms().list().unwrap_or_default();
        let vms_running = vm_list
            .iter()
            .filter(|v| v.status == Status::Running)
            .count() as u64;
        let vms_total = vm_list.len() as u64;

        let networks_total = delonix_sdn::NetworkStore::open(root)
            .and_then(|s| s.list())
            .map(|l| l.len() as u64)
            .unwrap_or(0);
        let volumes_total = delonix_volume::VolumeStore::open(root)
            .and_then(|s| s.list())
            .map(|l| l.len() as u64)
            .unwrap_or(0);
        let images_total = self.images().count();
        let secrets_total = delonix_state::SecretStore::open(root)
            .map(|s| s.list().len() as u64)
            .unwrap_or(0);

        let (memory_bytes_limit, memory_bytes_used, ..) = delonix_linux::slice_budget();

        let (network_rx_bytes, network_tx_bytes, network_unmeasured_containers) = if include_network
        {
            let mut rx = 0u64;
            let mut tx = 0u64;
            let mut unmeasured = 0u64;
            for id in &running_ids {
                match delonix_sdn::infra::container_net_bytes(id) {
                    Some((r, t)) => {
                        rx += r;
                        tx += t;
                    }
                    // `--net host`/`--net none` containers have no netns to
                    // inspect — count them, don't silently treat as zero traffic.
                    None => unmeasured += 1,
                }
            }
            (Some(rx), Some(tx), unmeasured)
        } else {
            (None, None, 0)
        };

        let (
            storage_bytes_images,
            storage_bytes_volumes,
            storage_bytes_vm_images,
            storage_bytes_containers,
        ) = if include_storage {
            (
                Some(dir_size(&root.join("blobs")) + dir_size(&root.join("layers"))),
                Some(dir_size(&root.join("volumes"))),
                Some(dir_size(&root.join("vm-images"))),
                Some(dir_size(&root.join("containers"))),
            )
        } else {
            (None, None, None, None)
        };

        DashSummary {
            containers_running,
            containers_total,
            vms_running,
            vms_total,
            networks_total,
            volumes_total,
            images_total,
            secrets_total,
            memory_bytes_used,
            memory_bytes_limit,
            network_rx_bytes,
            network_tx_bytes,
            network_unmeasured_containers,
            storage_bytes_images,
            storage_bytes_volumes,
            storage_bytes_vm_images,
            storage_bytes_containers,
        }
    }

    /// Per-container cumulative network rx/tx bytes, keyed by container id, for
    /// every RUNNING container (R3f-B). One `nsenter`+`cat` per container — never
    /// on a tick budget; the interface serves this from a slow background refresh.
    pub fn container_net(&self) -> HashMap<String, (u64, u64)> {
        let mut out = HashMap::new();
        if let Ok(store) = delonix_state::Store::open(self.root().join("containers")) {
            if let Ok(list) = store.list() {
                for mut c in list {
                    delonix_linux::reconcile_status(&mut c);
                    if c.status == Status::Running {
                        if let Some(bytes) = delonix_sdn::infra::container_net_bytes(&c.id) {
                            out.insert(c.id.clone(), bytes);
                        }
                    }
                }
            }
        }
        out
    }

    /// Per-workload contention series (CPU throttling, pressure, OOM) for every
    /// running container and VM, read from cgroup v2 with no new privilege (R3f-B).
    /// A container is read from its own live cgroup; a VM only when its VMM is
    /// alone in its cgroup. Per-CONTAINER network is left `None` here — the
    /// interface overlays it from its background-refreshed cache to keep the
    /// inline scrape path free of netns enters — while a Cloud Hypervisor VM's tap
    /// bytes are one read away and attached here.
    pub fn workload_series(&self) -> Vec<WorkloadSeries> {
        let mut out = Vec::new();
        if let Ok(store) = delonix_state::Store::open(self.root().join("containers")) {
            if let Ok(list) = store.list() {
                for mut c in list {
                    delonix_linux::reconcile_status(&mut c);
                    if c.status == Status::Running {
                        let dir = PathBuf::from(delonix_linux::live_cgroup(&c));
                        out.push(workload_series_one(
                            WorkloadKind::Container,
                            &c.id,
                            &c.name,
                            Ok(dir),
                        ));
                    }
                }
            }
        }
        for vm in self.vms().list().unwrap_or_default() {
            if vm.status != Status::Running {
                continue;
            }
            let cgroup = match vm.pid {
                Some(pid) => {
                    pressure::vmm_cgroup(Path::new("/proc"), Path::new("/sys/fs/cgroup"), pid)
                }
                None => Err(Why::CgroupMissing),
            };
            let mut ws = workload_series_one(WorkloadKind::Vm, &vm.name, &vm.name, cgroup);
            if vm.backend == "cloud-hypervisor" {
                if let Some((rx, tx)) = delonix_sdn::infra::vm_tap_net_bytes(&vm.tap) {
                    ws.net_rx_bytes = Some(rx);
                    ws.net_tx_bytes = Some(tx);
                }
            }
            out.push(ws);
        }
        out
    }
}

/// Recursive disk usage of a directory, `du`-style (allocated blocks, hardlink-
/// deduplicated). Zero on any unreadable path — a summary must never fail over
/// one missing directory.
fn dir_size(p: &Path) -> u64 {
    delonix_volume::measure(p).bytes
}

/// Reads one workload's series from `cgroup` (or records why there is none) and
/// converts them to the exporter's [`WorkloadSeries`]. Every group that could not
/// be read becomes an [`Unavailable`] entry instead of a value.
fn workload_series_one(
    kind: WorkloadKind,
    id: &str,
    name: &str,
    // Fully qualified: this crate's bare `Result` is `delonix_vm::Result` (a
    // one-argument alias re-exported at the top), not `std::result::Result`.
    cgroup: std::result::Result<PathBuf, Why>,
) -> WorkloadSeries {
    let read = match cgroup {
        Ok(dir) => pressure::read_cgroup(&dir),
        Err(why) => CgroupSeries::unavailable(why),
    };
    let mut unavailable = Vec::new();
    let cpu_usage_usec = take("cpu_usage", read.cpu_usage_usec, &mut unavailable);
    let memory_current_bytes = take("memory_current", read.memory_current, &mut unavailable);
    let cpu_throttle =
        take("cpu_throttle", read.cpu_throttle, &mut unavailable).map(|c| CpuThrottle {
            nr_periods: c.nr_periods,
            nr_throttled: c.nr_throttled,
            throttled_usec: c.throttled_usec,
        });
    let cpu_pressure = take("cpu_pressure", read.cpu_pressure, &mut unavailable).map(to_pressure);
    let memory_pressure =
        take("memory_pressure", read.memory_pressure, &mut unavailable).map(to_pressure);
    let io_pressure = take("io_pressure", read.io_pressure, &mut unavailable).map(to_pressure);
    let memory_events =
        take("memory_events", read.memory_events, &mut unavailable).map(|m| MemoryEvents {
            oom: m.oom,
            oom_kill: m.oom_kill,
        });
    WorkloadSeries {
        kind,
        id: id.to_string(),
        name: name.to_string(),
        cpu_usage_usec,
        memory_current_bytes,
        // Network is NOT a cgroup read: it needs a netns enter, too costly for the
        // inline scrape path. The interface fills container net from its cache;
        // left `None` here (a Cloud Hypervisor VM's tap is attached by the caller).
        net_rx_bytes: None,
        net_tx_bytes: None,
        cpu_throttle,
        cpu_pressure,
        memory_pressure,
        io_pressure,
        memory_events,
        unavailable,
    }
}

/// A group that could not be read becomes `None` plus its reason.
fn take<T>(
    group: &'static str,
    r: std::result::Result<T, Why>,
    unavailable: &mut Vec<Unavailable>,
) -> Option<T> {
    match r {
        Ok(v) => Some(v),
        Err(why) => {
            unavailable.push(Unavailable {
                group,
                reason: why.as_str(),
            });
            None
        }
    }
}

fn to_pressure(p: pressure::Pressure) -> WorkloadPressure {
    let stall = |l: pressure::StallLine| Stall {
        avg10_percent: l.avg10_percent,
        total_usec: l.total_usec,
    };
    WorkloadPressure {
        some: stall(p.some),
        full: p.full.map(stall),
    }
}

/// The compute context's VM use cases, bound to a mounted [`Engine`]. The surface
/// an interface calls (`engine.vms().list()`) instead of the free functions or a
/// provider crate — the composition, not the caller, chose the backend.
pub struct VmService<'a> {
    engine: &'a Engine,
}

impl VmService<'_> {
    /// Every VM the node has.
    pub fn list(&self) -> Result<Vec<Vm>> {
        delonix_vm::list(self.engine.root())
    }

    /// One VM's full record by name.
    pub fn status(&self, name: &str) -> Result<Vm> {
        delonix_vm::status(self.engine.root(), name)
    }

    /// Whether a VM of this name exists.
    pub fn exists(&self, name: &str) -> bool {
        delonix_vm::exists(self.engine.root(), name)
    }

    /// Start a stopped VM; returns its observed state.
    pub fn start(&self, name: &str) -> Result<Vm> {
        delonix_vm::start(self.engine.root(), name)
    }

    /// Stop a running VM, keeping its disk.
    pub fn stop(&self, name: &str) -> Result<()> {
        delonix_vm::stop(self.engine.root(), name)
    }

    /// The snapshot names a VM carries.
    pub fn snapshots(&self, name: &str) -> Result<Vec<String>> {
        delonix_vm::snapshots(self.engine.root(), name)
    }

    /// End a VM; `purge_disks` releases its storage.
    pub fn destroy(&self, name: &str, force: bool, purge_disks: bool) -> Result<Destroyed> {
        delonix_vm::destroy(self.engine.root(), name, force, purge_disks)
    }

    /// Pause a running VM (freeze its vCPUs), keeping it resident.
    pub fn pause(&self, name: &str) -> Result<()> {
        delonix_vm::pause(self.engine.root(), name)
    }

    /// Resume a paused VM.
    pub fn unpause(&self, name: &str) -> Result<()> {
        delonix_vm::unpause(self.engine.root(), name)
    }

    /// Take a named snapshot of a VM.
    pub fn snapshot(&self, name: &str, snap: &str) -> Result<()> {
        delonix_vm::snapshot(self.engine.root(), name, snap)
    }

    /// Restore a VM to one of its snapshots.
    pub fn restore(&self, name: &str, snap: &str) -> Result<()> {
        delonix_vm::restore(self.engine.root(), name, snap)
    }

    /// Delete one of a VM's snapshots.
    pub fn delete_snapshot(&self, name: &str, snap: &str) -> Result<()> {
        delonix_vm::delete_snapshot(self.engine.root(), name, snap)
    }
}

/// The container context's reads, bound to a mounted [`Engine`] (R3e). The store
/// queries and the host metrics arrive through here instead of an interface
/// naming `delonix-state`/`delonix-linux`. Errors are the shared
/// [`delonix_model::Error`] every surface already maps.
pub struct ContainerService<'a> {
    engine: &'a Engine,
}

impl ContainerService<'_> {
    fn store(&self) -> delonix_model::Result<delonix_state::Store> {
        delonix_state::Store::open(self.engine.root().join("containers")).map_err(Into::into)
    }

    /// Every container this node has.
    pub fn list(&self) -> delonix_model::Result<Vec<Container>> {
        self.store()?.list().map_err(Into::into)
    }

    /// One container's full record, by name or id (prefix).
    pub fn load(&self, id_or_name: &str) -> delonix_model::Result<Container> {
        self.store()?.load(id_or_name).map_err(Into::into)
    }

    /// **Safe read-modify-write** of one container under its exclusive `flock`
    /// (R3h): re-reads the record ALREADY under the lock, applies `f`, writes —
    /// one critical section between processes. `f` returns `false` to abort the
    /// write (nothing changes); the returned container is the final state (or the
    /// one read, if it aborted). NOT `load` + mutate + `save`, which loses writes
    /// when the concurrent CRI and the CLI touch the same container at once —
    /// the only write the CRI issues to the container store goes through here.
    pub fn update<F>(&self, id_or_name: &str, f: F) -> delonix_model::Result<Container>
    where
        F: FnOnce(&mut Container) -> bool,
    {
        self.store()?
            .update(id_or_name, f)
            .map_err(delonix_model::Error::from)
    }

    /// Whether the container store is reachable — the openability a health or
    /// doctor check reports, with the open error kept for its detail line.
    pub fn store_reachable(&self) -> delonix_model::Result<()> {
        self.store().map(|_| ())
    }

    /// Reconcile a container's recorded status against its live process, in place.
    pub fn reconcile_status(&self, c: &mut Container) {
        delonix_linux::reconcile_status(c);
    }

    /// The cgroup path a running container's usage is read from.
    pub fn live_cgroup(&self, c: &Container) -> String {
        delonix_linux::live_cgroup(c)
    }

    /// One cumulative-usage sample of the cgroup at `dir`.
    pub fn cgroup_sample(&self, dir: &Path) -> UsageSample {
        delonix_linux::usage::cgroup_sample(dir)
    }

    /// One cumulative-usage sample of a process by pid (the VMM basis a VM row
    /// reports from).
    pub fn process_sample(&self, pid: i32) -> UsageSample {
        delonix_linux::usage::process_sample(pid)
    }

    /// The Unix start time of a pid, or `None` if it cannot be read.
    pub fn process_started_at_unix(&self, pid: i32) -> Option<u64> {
        delonix_linux::usage::process_started_at_unix(pid)
    }

    /// Record a published-port `spec` on the container whose IP is `ip` (R3f-D).
    /// Silent when no record owns the IP — the ingress addresses workloads by IP
    /// and nothing guarantees one of ours holds it (a VM, a foreign netns); this
    /// records what it can, it is not a second admission gate.
    pub fn record_published_port(&self, ip: &str, spec: &str) -> delonix_model::Result<()> {
        let store = self.store()?;
        for mut c in store.list().map_err(delonix_model::Error::from)? {
            if c.ip.as_deref() == Some(ip) {
                if !c.ports.iter().any(|p| p == spec) {
                    c.ports.push(spec.to_string());
                    store.save(&c).map_err(delonix_model::Error::from)?;
                }
                return Ok(());
            }
        }
        Ok(())
    }

    /// Drop a publication from whichever record claims it (R3f-D). `ip` narrows
    /// the search when known (the rollback path); the DELETE route knows only the
    /// host port and matches on that alone (a host port is published once).
    pub fn forget_published_port(
        &self,
        ip: Option<&str>,
        spec_or_port: &str,
    ) -> delonix_model::Result<()> {
        let store = self.store()?;
        let wanted_port = delonix_sdn::parse_publish(spec_or_port)
            .map(|(hp, _, _)| hp)
            .unwrap_or_else(|_| spec_or_port.to_string());
        for mut c in store.list().map_err(delonix_model::Error::from)? {
            if ip.is_some() && c.ip.as_deref() != ip {
                continue;
            }
            let before = c.ports.len();
            c.ports.retain(|s| {
                delonix_sdn::parse_publish(s)
                    .map(|(hp, _, _)| hp != wanted_port)
                    .unwrap_or(true)
            });
            if c.ports.len() != before {
                store.save(&c).map_err(delonix_model::Error::from)?;
            }
        }
        Ok(())
    }
}

/// The storage context's named-volume reads, bound to a mounted [`Engine`] (R3e).
pub struct VolumeService<'a> {
    engine: &'a Engine,
}

impl VolumeService<'_> {
    /// Every named volume this node has.
    pub fn list(&self) -> delonix_model::Result<Vec<Volume>> {
        delonix_volume::VolumeStore::open(self.engine.root())
            .and_then(|s| s.list())
            .map_err(Into::into)
    }

    /// Whether the volume store is reachable (the health/doctor openability check).
    pub fn store_reachable(&self) -> delonix_model::Result<()> {
        delonix_volume::VolumeStore::open(self.engine.root())
            .map(|_| ())
            .map_err(Into::into)
    }

    /// One named volume's full record.
    pub fn inspect(&self, name: &str) -> delonix_model::Result<Volume> {
        delonix_volume::VolumeStore::open(self.engine.root())
            .and_then(|s| s.inspect(name))
            .map_err(Into::into)
    }

    /// Create a named volume (`driver` defaults are the caller's; `device`/
    /// `options` are the nfs/mount parameters). Returns the created record.
    pub fn create_with(
        &self,
        name: &str,
        driver: &str,
        device: Option<String>,
        options: Option<String>,
    ) -> delonix_model::Result<Volume> {
        delonix_volume::VolumeStore::open(self.engine.root())
            .and_then(|s| s.create_with(name, driver, device, options))
            .map_err(Into::into)
    }

    /// Remove a named volume.
    pub fn remove(&self, name: &str) -> delonix_model::Result<()> {
        delonix_volume::VolumeStore::open(self.engine.root())
            .and_then(|s| s.remove(name))
            .map_err(Into::into)
    }
}

/// The network context's read use cases, bound to a mounted [`Engine`] (R3e).
pub struct NetworkService<'a> {
    engine: &'a Engine,
}

impl NetworkService<'_> {
    /// The SDN/holder infrastructure status (bridges, firewall, ingress).
    pub fn status(&self) -> InfraStatus {
        let _ = self.engine;
        delonix_sdn::infra::status()
    }

    /// A running container's cumulative received/transmitted bytes across its own
    /// network namespace, or `None` when it has none to read (host/none network,
    /// or the holder is unreachable).
    pub fn container_net_bytes(&self, id: &str) -> Option<(u64, u64)> {
        let _ = self.engine;
        delonix_sdn::infra::container_net_bytes(id)
    }

    /// The IPv4 a MAC was leased on a network, or `None` if the network has none
    /// (R3f-E). Deterministic, no state.
    pub fn dhcp_ip_for_mac(&self, net: &str, mac: &str) -> Option<String> {
        let _ = self.engine;
        delonix_sdn::infra::dhcp_ip_for_mac(net, mac)
    }

    /// The IPv6 equivalent (v6 is off by default in this engine).
    pub fn dhcp_ip6_for_mac(&self, net: &str, mac: &str) -> Option<String> {
        let _ = self.engine;
        delonix_sdn::infra::dhcp_ip6_for_mac(net, mac)
    }

    /// The IP a container has on the default network (derived from its id).
    pub fn container_ip(&self, id: &str) -> String {
        let _ = self.engine;
        delonix_sdn::infra::container_ip(id)
    }

    /// Every DECLARED network on this node.
    pub fn network_list(&self) -> Vec<delonix_sdn::infra::NetDef> {
        let _ = self.engine;
        delonix_sdn::infra::network_list()
    }

    /// One declared network by name, or `None`.
    pub fn network_get(&self, name: &str) -> Option<delonix_sdn::infra::NetDef> {
        let _ = self.engine;
        delonix_sdn::infra::network_get(name)
    }

    /// Whether `ip` parses as a valid address — the input guard a DNAT rule needs
    /// before a published port reaches the mechanism.
    pub fn valid_container_ip(&self, ip: &str) -> bool {
        let _ = self.engine;
        delonix_sdn::Cidr::parse_addr(ip).is_some()
    }

    /// Rate-limit a running container's bandwidth (tc/tbf).
    pub fn set_net_rate(
        &self,
        id: &str,
        rate_bit: u64,
        burst_bytes: u64,
    ) -> delonix_model::Result<()> {
        let _ = self.engine;
        delonix_sdn::infra::set_net_rate(id, rate_bit, burst_bytes)
            .map_err(delonix_model::Error::from)
    }

    /// Publish a container port through the ingress (a DNAT rule + holder fwd).
    pub fn publish_port(&self, container_ip: &str, spec: &str) -> delonix_model::Result<()> {
        let _ = self.engine;
        delonix_sdn::infra::publish_port(container_ip, spec).map_err(delonix_model::Error::from)
    }

    /// Withdraw a published host port. Best-effort (no result): a 200 means "ran",
    /// not "removed" — the record drop is the durable half (see `ContainerService`).
    pub fn unpublish_port(&self, host_port: &str) {
        let _ = self.engine;
        delonix_sdn::infra::unpublish_port(host_port)
    }
}

/// The image context's reads, bound to a mounted [`Engine`] (R3f). Errors are the
/// shared [`delonix_model::Error`]; the native `delonix-oci` error maps into it in
/// the adapter, so every surface sees one error vocabulary.
pub struct ImageService<'a> {
    engine: &'a Engine,
}

impl ImageService<'_> {
    // `delonix_oci::Error` converts into BOTH `delonix_model::Error` and
    // `delonix_scanner::Error` (the scanner builds on the image store), so the
    // bare `Into::into` is ambiguous here — name the target explicitly.
    fn store(&self) -> delonix_model::Result<ImageStore> {
        ImageStore::open(self.engine.root()).map_err(delonix_model::Error::from)
    }

    /// Every image in the local store.
    pub fn list(&self) -> delonix_model::Result<Vec<Image>> {
        self.store()?.list().map_err(delonix_model::Error::from)
    }

    /// How many images are stored — best-effort (`0` on an unreadable store, the
    /// same degradation the dashboard summary expects).
    pub fn count(&self) -> u64 {
        self.store()
            .and_then(|s| s.list().map_err(delonix_model::Error::from))
            .map(|l| l.len() as u64)
            .unwrap_or(0)
    }

    /// Remove an image (tag or id); returns the human line the store reports
    /// (`untagged: …` / `deleted: …`).
    pub fn remove(&self, reference: &str) -> delonix_model::Result<String> {
        self.store()?
            .remove(reference)
            .map_err(delonix_model::Error::from)
    }

    /// The SBOM (installed packages) a scanner reads from an image's layers,
    /// already serialized — the `Package` type lives in the scanner adapter and
    /// the caller only serves the list. An image that exists but exposes no
    /// readable package manager is an empty list, not an error (the store's
    /// `resolve` is what distinguishes "no such image").
    pub fn sbom(&self, reference: &str) -> delonix_model::Result<serde_json::Value> {
        let store = self.store()?;
        let img = store
            .resolve(reference)
            .map_err(delonix_model::Error::from)?;
        let pkgs = delonix_scanner::extract_sbom(&store, &img).unwrap_or_default();
        Ok(serde_json::to_value(pkgs).unwrap_or(serde_json::Value::Null))
    }

    /// Pull an image from its registry, optionally with `(username, password)`
    /// the caller resolved (a Pod's `imagePullSecrets`); returns the stored id
    /// (R3h). The CRI `PullImage` path, composed away from `delonix-oci`.
    pub fn pull(
        &self,
        reference: &str,
        creds: Option<(String, String)>,
    ) -> delonix_model::Result<String> {
        let store = self.store()?;
        delonix_oci::pull_from_registry_with_creds(&store, reference, creds)
            .map(|img| img.id)
            .map_err(delonix_model::Error::from)
    }

    /// The CRI-shaped summary of every local image — `id`, the OCI config `user`,
    /// the reference list (`repo_tags`, which the CRI boundary later splits into
    /// tags/digests) and the on-disk layer size (R3h). The `Image` type stays in
    /// the adapter; an interface serves these fields without naming it.
    pub fn image_summaries(&self) -> delonix_model::Result<serde_json::Value> {
        let imgs = self.store()?.list().map_err(delonix_model::Error::from)?;
        Ok(serde_json::Value::Array(
            imgs.iter().map(image_summary).collect(),
        ))
    }

    /// The summary of one image by reference, or `None` if it is not stored.
    pub fn image_status(&self, reference: &str) -> delonix_model::Result<Option<serde_json::Value>> {
        Ok(self
            .store()?
            .resolve(reference)
            .ok()
            .map(|img| image_summary(&img)))
    }

    /// Total on-disk bytes of all image layers (the CRI `ImageFsInfo` usage).
    /// Best-effort — `0` on an unreadable store, as the CRI caller expects.
    pub fn image_fs_used_bytes(&self) -> u64 {
        self.store()
            .and_then(|s| s.list().map_err(delonix_model::Error::from))
            .map(|imgs| imgs.iter().map(image_layer_size).sum())
            .unwrap_or(0)
    }
}

/// `{id, user, repo_tags, size}` of one image — the fields the CRI `Image`
/// message is built from, in a form an interface can name (R3h).
fn image_summary(img: &Image) -> serde_json::Value {
    json!({
        "id": img.id,
        "user": img.config.user,
        "repo_tags": img.repo_tags,
        "size": image_layer_size(img),
    })
}

/// On-disk size of an image's layer blobs, summed from the CAS. Mirrors the
/// CRI's historical path exactly: `<default_root>/blobs/sha256/<hex>` per layer,
/// `fs::metadata().len()`, skipping anything unreadable.
fn image_layer_size(img: &Image) -> u64 {
    let root = ImageStore::default_root().join("blobs").join("sha256");
    img.layers
        .iter()
        .map(|l| {
            let hex = l.strip_prefix("sha256:").unwrap_or(l);
            std::fs::metadata(root.join(hex)).map(|m| m.len()).unwrap_or(0)
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mounted_engine_exposes_its_root_and_the_vm_service() {
        let eng = Engine::mount("/tmp/delonix-engine-test-root").expect("mount");
        assert_eq!(eng.root(), Path::new("/tmp/delonix-engine-test-root"));
        // `list` over a root with no vms dir is an empty list, not an error:
        // proves the VM service is wired to the composition without a host VM.
        assert!(eng.vms().list().map(|v| v.is_empty()).unwrap_or(true));
    }

    // --- resource summary (moved from delonix-mgmt's dashstats in R3f-B) ---

    #[test]
    fn dash_summary_over_an_empty_root_does_not_panic_and_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        let eng = Engine::mount(dir.path().to_path_buf()).unwrap();
        let s = eng.dash_summary(true, true);
        assert_eq!(s.containers_running, 0);
        assert_eq!(s.containers_total, 0);
        assert_eq!(s.vms_total, 0);
        assert_eq!(s.storage_bytes_images, Some(0));
        assert_eq!(s.network_rx_bytes, Some(0));
    }

    #[test]
    fn dash_summary_without_network_or_storage_leaves_those_none() {
        let dir = tempfile::tempdir().unwrap();
        let eng = Engine::mount(dir.path().to_path_buf()).unwrap();
        let s = eng.dash_summary(false, false);
        assert_eq!(s.network_rx_bytes, None);
        assert_eq!(s.network_tx_bytes, None);
        assert_eq!(s.storage_bytes_images, None);
        assert_eq!(s.storage_bytes_containers, None);
    }

    #[test]
    fn workload_series_on_an_empty_root_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        let eng = Engine::mount(dir.path().to_path_buf()).unwrap();
        assert!(eng.workload_series().is_empty());
    }

    /// The container write path the CRI depends on (R3h): `update` applies the
    /// closure to the record and persists it, and a closure that returns `false`
    /// changes nothing. The locking itself is proved by the concurrent-update test
    /// in `delonix-state`; here the point is that the engine's port forwards to
    /// that read-modify-write faithfully, including the abort case the CRI's
    /// reconcile relies on.
    #[test]
    fn container_update_persists_a_change_and_an_aborting_closure_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let eng = Engine::mount(dir.path().to_path_buf()).unwrap();
        // Seed one record straight into the store (the engine's port has no
        // `create`; the CRI seeds through its own path, delonix-state here).
        let store = delonix_state::Store::open(dir.path().join("containers")).unwrap();
        let mut c = Container::new(
            "cri-abc".to_string(),
            "cri-abc".to_string(),
            "img".to_string(),
            vec!["/sh".to_string()],
            "64m".to_string(),
        );
        c.pid = Some(7);
        store.save(&c).unwrap();

        // A writing closure is applied and persisted.
        let after = eng
            .containers()
            .update("cri-abc", |c| {
                c.pid = Some(42);
                true
            })
            .unwrap();
        assert_eq!(after.pid, Some(42));
        assert_eq!(eng.containers().load("cri-abc").unwrap().pid, Some(42));

        // A closure returning `false` aborts the WRITE: nothing is persisted. The
        // record on disk is still the one the closure read (42), even though the
        // returned value carries the discarded in-place mutation — the on-disk
        // invariant is what the CRI's reconcile depends on.
        let _ = eng
            .containers()
            .update("cri-abc", |c| {
                c.pid = Some(99);
                false
            })
            .unwrap();
        assert_eq!(eng.containers().load("cri-abc").unwrap().pid, Some(42));
    }

    /// `dir_size` descends the whole tree and counts what `du` counts, delegating
    /// to `delonix_volume::measure` (allocated blocks, `(dev, ino)` dedup), so the
    /// assertion is on behaviour, not a filesystem-specific byte count.
    #[test]
    fn dir_size_sums_recursively_and_zeroes_a_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a"), b"12345").unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("b"), b"1234567890").unwrap();
        let total = dir_size(dir.path());
        let only_sub = dir_size(&dir.path().join("sub"));
        assert!(total > 0);
        assert!(total > only_sub);
        assert_eq!(dir_size(&dir.path().join("does-not-exist")), 0);
    }

    /// REGRESSION: the disk tiles must not count an inode once per hardlink.
    #[test]
    fn dir_size_does_not_count_hardlinks_more_than_once() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("original"), vec![0u8; 64 * 1024]).unwrap();
        let one = dir_size(dir.path());
        for i in 0..5 {
            std::fs::hard_link(dir.path().join("original"), dir.path().join(format!("l{i}")))
                .unwrap();
        }
        assert_eq!(dir_size(dir.path()), one, "6 names of one inode counted as 6 files");
    }

    const PSI: &str = "some avg10=12.50 avg60=3.00 avg300=1.00 total=4500000\n\
        full avg10=2.25 avg60=0.50 avg300=0.10 total=500000\n";

    fn write_cgroup_file(dir: &Path, name: &str, body: &str) {
        std::fs::write(dir.join(name), body).unwrap();
    }

    fn fixture_cgroup() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let p = d.path();
        write_cgroup_file(p, "cgroup.controllers", "cpu memory io pids\n");
        write_cgroup_file(
            p,
            "cpu.stat",
            "usage_usec 9500000\nnr_periods 200\nnr_throttled 50\nthrottled_usec 1500000\n",
        );
        write_cgroup_file(p, "cpu.pressure", PSI);
        write_cgroup_file(p, "memory.pressure", PSI);
        write_cgroup_file(p, "io.pressure", PSI);
        write_cgroup_file(p, "memory.events.local", "low 0\nhigh 0\nmax 4\noom 2\noom_kill 1\n");
        write_cgroup_file(p, "memory.current", "4194304\n");
        d
    }

    #[test]
    fn the_exporter_renders_a_fixture_cgroup_with_the_workload_labels() {
        let d = fixture_cgroup();
        let ws =
            workload_series_one(WorkloadKind::Container, "c1", "web", Ok(d.path().to_path_buf()));
        assert!(ws.unavailable.is_empty(), "{:?}", ws.unavailable);
        let out = delonix_telemetry::workload::render(&[ws]);
        let l = r#"kind="container",id="c1",name="web""#;
        for want in [
            format!("delonix_workload_cpu_usage_seconds_total{{{l}}} 9.5"),
            format!("delonix_workload_memory_current_bytes{{{l}}} 4194304"),
            format!("delonix_workload_cpu_throttled_seconds_total{{{l}}} 1.5"),
            format!("delonix_workload_memory_oom_kills_total{{{l}}} 1"),
        ] {
            assert!(out.contains(&want), "missing `{want}` in:\n{out}");
        }
        assert!(!out.contains("series_unavailable"), "{out}");
    }

    #[test]
    fn a_workload_without_a_readable_cgroup_exports_only_reasons() {
        let ws = workload_series_one(WorkloadKind::Vm, "vm1", "vm1", Err(Why::SharedCgroup));
        assert_eq!(ws.unavailable.len(), 7);
        assert!(ws.unavailable.iter().all(|u| u.reason == "shared_cgroup"));
        let out = delonix_telemetry::workload::render(&[ws]);
        assert_eq!(out.matches("series_unavailable{").count(), 7, "{out}");
        assert!(!out.contains("delonix_workload_cpu_"), "{out}");
    }
}
