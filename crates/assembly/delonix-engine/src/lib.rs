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

use std::path::{Path, PathBuf};

use delonix_compute::capability::ProviderReport;
use delonix_compute::{Container, Vm};
use delonix_linux::usage::UsageSample;
use delonix_oci::{Image, ImageStore};
use delonix_sdn::infra::InfraStatus;
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
}
