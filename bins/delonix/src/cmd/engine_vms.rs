//! The VM port the `dashstats` collector needs, backed by the composed engine.
//!
//! `dashstats::collect`/`collect_with_timeout`/`collect_workload_series` take a
//! [`delonix_mgmt::VmView`] (ADR-0040 A5): the interface no longer reaches the
//! `delonix-vm` adapter directly. `delonix` is a bin, so it is the one layer
//! allowed to name the `delonix-engine` assembly — it mounts the `Engine` and
//! hands the collector this port, exactly like `delonix-mgmt-bin`/`delonix-mcp-bin`
//! do for their servers. `Engine::mount` only stores the root, so building one
//! per call is free.

use std::path::Path;

use delonix_compute::Vm;
use delonix_engine::Engine;
use delonix_model::Error;

/// The composition's VM service as a [`delonix_mgmt::VmView`].
pub struct EngineVms(Engine);

impl delonix_mgmt::VmView for EngineVms {
    fn list(&self) -> Result<Vec<Vm>, Error> {
        self.0.vms().list().map_err(Into::into)
    }

    fn status(&self, name: &str) -> Result<Vm, Error> {
        self.0.vms().status(name).map_err(Into::into)
    }
}

/// A VM port rooted at `root`. `Engine::mount` is infallible (it only stores the
/// path), so the dashstats callers can take this by value without error handling.
pub fn vm_view(root: &Path) -> EngineVms {
    EngineVms(Engine::mount(root.to_path_buf()).expect("Engine::mount only stores the root"))
}
