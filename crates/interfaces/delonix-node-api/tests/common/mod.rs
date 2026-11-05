//! A `VmPort` the integration tests hand `serve_blocking`.
//!
//! Production reaches the VM use cases through the engine-backed port the bin
//! injects (ADR-0040 A5); the node-api crate itself names no VM adapter. These
//! tests stand in for that composition with an adapter-backed port: `delonix-vm`
//! is a dev-dependency, so a test may seed VM backends and read them back, and
//! the provider list a `ListProviders` test asserts (libvirt, cloud-hypervisor)
//! comes from the real registry — the one thing only this backing can supply.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

use delonix_node_api::VmPort;

/// A [`VmPort`] over the `delonix-vm` adapter at a state root — the tests'
/// stand-in for the engine the bin composes.
pub struct AdapterVms(pub PathBuf);

/// The adapter-backed port over this host's real state root, as an `Arc` for
/// `serve_blocking`. The reads are read-only; the tests that use it are chosen
/// to refuse or answer `NotFound` before anything is written.
pub fn backing() -> Arc<dyn VmPort> {
    Arc::new(AdapterVms(delonix_node_api::node::state_root()))
}

impl VmPort for AdapterVms {
    fn list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error> {
        delonix_vm::list(&self.0).map_err(Into::into)
    }
    fn status(&self, name: &str) -> Result<delonix_compute::Vm, delonix_model::Error> {
        delonix_vm::status(&self.0, name).map_err(Into::into)
    }
    fn exists(&self, name: &str) -> bool {
        delonix_vm::exists(&self.0, name)
    }
    fn start(&self, name: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::start(&self.0, name).map(|_| ()).map_err(Into::into)
    }
    fn stop(&self, name: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::stop(&self.0, name).map_err(Into::into)
    }
    fn pause(&self, name: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::pause(&self.0, name).map_err(Into::into)
    }
    fn unpause(&self, name: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::unpause(&self.0, name).map_err(Into::into)
    }
    fn snapshots(&self, name: &str) -> Result<Vec<String>, delonix_model::Error> {
        delonix_vm::snapshots(&self.0, name).map_err(Into::into)
    }
    fn snapshot(&self, name: &str, snap: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::snapshot(&self.0, name, snap).map_err(Into::into)
    }
    fn restore(&self, name: &str, snap: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::restore(&self.0, name, snap).map_err(Into::into)
    }
    fn delete_snapshot(&self, name: &str, snap: &str) -> Result<(), delonix_model::Error> {
        delonix_vm::delete_snapshot(&self.0, name, snap).map_err(Into::into)
    }
    fn destroy(
        &self,
        name: &str,
        force: bool,
        purge_disks: bool,
    ) -> Result<(), delonix_model::Error> {
        delonix_vm::destroy(&self.0, name, force, purge_disks)
            .map(|_| ())
            .map_err(Into::into)
    }
    fn create(
        &self,
        cfg: &delonix_compute::vm_backend::VmConfig,
    ) -> Result<(), delonix_model::Error> {
        delonix_vm::create_with(&self.0, cfg, &|_stage| {})
            .map(|_| ())
            .map_err(Into::into)
    }
    fn image_defaults(&self, reference: &str) -> Option<delonix_compute::ports::ImageDefaults> {
        delonix_vm::image_defaults(&self.0, reference)
    }
    fn store_image_disk(&self, reference: &str) -> String {
        delonix_vm::store_image_disk(&self.0, reference)
    }
    fn refuse_undeclared_network(
        &self,
        cfg: &delonix_compute::vm_backend::VmConfig,
    ) -> Result<(), delonix_model::Error> {
        delonix_vm::refuse_undeclared_network(cfg).map_err(Into::into)
    }
    fn ip_is_predicted(&self, vm: &delonix_compute::Vm) -> bool {
        delonix_vm::ip_is_predicted(vm)
    }
    fn provider_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        delonix_vm::provider_reports()
    }
    fn declared_vm_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        use delonix_vm::capabilities as vmc;
        vec![
            vmc::cloud_hypervisor_report(&vmc::CloudHypervisorHost::ASSUMED),
            vmc::libvirt_report(&vmc::LibvirtHost::ASSUMED),
        ]
    }
}
