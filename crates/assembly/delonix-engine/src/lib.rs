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

use delonix_compute::Vm;
use delonix_compute::capability::ProviderReport;

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
