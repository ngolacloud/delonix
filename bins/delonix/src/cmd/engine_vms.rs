//! The engine ports the `dash`/`system` commands drive, backed by the composed
//! engine (R3f).
//!
//! The resource summary moved off `delonix-mgmt` into the `delonix-engine`
//! assembly: `delonix` is a bin, the one layer allowed to name the assembly, so
//! it mounts the `Engine` and either calls it directly (one-shot `dash_summary`/
//! `container_net`) or hands `delonix-mgmt` an [`delonix_mgmt::EngineOps`] port
//! for the bounded, circuit-broken collection the TUI's background refresh uses.
//! `Engine::mount` only stores the root, so building one per call is free.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use delonix_engine::Engine;
use delonix_model::dash::DashSummary;
use delonix_model::Error;
use delonix_telemetry::workload::WorkloadSeries;
use serde_json::Value;

/// The composed engine as a [`delonix_mgmt::EngineOps`] port. Only the summary
/// methods are exercised by `dash`/`system`; the image methods round out the
/// trait so one port serves all of `delonix-mgmt`'s bounded collectors.
struct EnginePorts(Engine);

impl delonix_mgmt::EngineOps for EnginePorts {
    fn list_images(&self) -> Result<Value, Error> {
        self.0
            .images()
            .list()
            .map(|imgs| serde_json::to_value(imgs).unwrap_or(Value::Null))
    }
    fn remove_image(&self, reference: &str) -> Result<String, Error> {
        self.0.images().remove(reference)
    }
    fn image_sbom(&self, reference: &str) -> Result<Value, Error> {
        self.0.images().sbom(reference)
    }
    fn dash_summary(&self, include_network: bool, include_storage: bool) -> DashSummary {
        self.0.dash_summary(include_network, include_storage)
    }
    fn container_net(&self) -> HashMap<String, (u64, u64)> {
        self.0.container_net()
    }
    fn workload_series(&self) -> Vec<WorkloadSeries> {
        self.0.workload_series()
    }
    fn list_volumes(&self) -> Result<Value, Error> {
        self.0
            .volumes()
            .list()
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
    }
    fn get_volume(&self, name: &str) -> Result<Value, Error> {
        self.0
            .volumes()
            .inspect(name)
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
    }
    fn create_volume(
        &self,
        name: &str,
        driver: &str,
        device: Option<String>,
        options: Option<String>,
    ) -> Result<Value, Error> {
        self.0
            .volumes()
            .create_with(name, driver, device, options)
            .map(|v| serde_json::to_value(v).unwrap_or(Value::Null))
    }
    fn remove_volume(&self, name: &str) -> Result<(), Error> {
        self.0.volumes().remove(name)
    }
}

/// A composed `Engine` rooted at `root` (infallible — only stores the path), for
/// the one-shot `dash_summary`/`container_net`/`workload_series` calls.
pub fn engine(root: &Path) -> Engine {
    Engine::mount(root.to_path_buf()).expect("Engine::mount only stores the root")
}

/// The engine as an owned [`delonix_mgmt::EngineOps`] port, for the bounded,
/// circuit-broken collectors in `delonix-mgmt` (the TUI's background refresh).
pub fn ports(root: &Path) -> Arc<dyn delonix_mgmt::EngineOps> {
    Arc::new(EnginePorts(engine(root)))
}
