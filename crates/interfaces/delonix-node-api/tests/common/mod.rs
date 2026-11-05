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

use delonix_node_api::{RunResolver, StorePort, VmPort};

/// A [`VmPort`] over the `delonix-vm` adapter at a state root — the tests'
/// stand-in for the engine the bin composes.
pub struct AdapterVms(pub PathBuf);

/// The adapter-backed port over this host's real state root, as an `Arc` for
/// `serve_blocking`. The reads are read-only; the tests that use it are chosen
/// to refuse or answer `NotFound` before anything is written.
pub fn backing() -> Arc<dyn VmPort> {
    Arc::new(AdapterVms(delonix_node_api::node::state_root()))
}

/// A [`StorePort`] over the `delonix-state` adapter at a state root (R3d-d) —
/// the tests' stand-in for the engine-backed record store the bin composes.
/// Production reaches the record store only through this port; the node-api
/// crate itself names no store adapter, so this lives in the test harness over
/// the `delonix-state` dev-dependency, exactly as [`AdapterVms`] does for VMs.
pub struct AdapterStore(pub PathBuf);

/// The adapter-backed record-store port over this host's real state root, as an
/// `Arc` for `serve_blocking`.
pub fn backing_store() -> Arc<dyn StorePort> {
    Arc::new(AdapterStore(delonix_node_api::node::state_root()))
}

/// A [`RunResolver`] over the real host adapters at a state root (R3d-e) — the
/// tests' stand-in for the engine-composed resolver the bin injects, mirroring
/// `delonix-node-api-bin`'s `HostResolver`. Production reaches the create
/// resolution only through this port; the node-api crate itself names no OCI/
/// volume/linux adapter on the create path, so this lives in the test harness
/// over the crate's own deps, exactly as [`AdapterVms`]/[`AdapterStore`] do.
pub struct AdapterResolver(pub PathBuf);

/// The adapter-backed create resolver over this host's real state root, as an
/// `Arc` for `serve_blocking`.
pub fn backing_resolver() -> Arc<dyn RunResolver> {
    Arc::new(AdapterResolver(delonix_node_api::node::state_root()))
}

impl RunResolver for AdapterResolver {
    fn resolve_run(
        &self,
        root: &std::path::Path,
        o: &delonix_compute::RunOpts,
        id: String,
        name: String,
        namespace: String,
    ) -> Result<delonix_compute::run::Resolved, delonix_model::Error> {
        use delonix_oci::rootfs_user::UserLookupError;
        let images = delonix_oci::ImageStore::open(&self.0)?;
        let announce_pull = |_r: &str| {};
        let unpacking =
            |prepare: &mut dyn FnMut() -> delonix_model::Result<std::path::PathBuf>| prepare();
        let user_error = |e: UserLookupError| {
            delonix_model::Error::Invalid(match e {
                UserLookupError::EmptyUser => "spec.user: empty user".to_string(),
                UserLookupError::NoSuchUser(u) => {
                    format!("spec.user: user '{u}' does not exist in the image (/etc/passwd)")
                }
                UserLookupError::NoSuchGroup(g) => {
                    format!("spec.user: group '{g}' does not exist in the image (/etc/group)")
                }
            })
        };
        let apparmor_disabled = |p: &str| {
            delonix_linux::Error::ApparmorUnavailable(format!(
                "spec.security.apparmor_profile '{p}': this host has no AppArmor to apply it"
            ))
        };
        delonix_compute::run::resolve_run(
            o,
            id,
            name,
            namespace,
            false,
            &delonix_oci::run_images::HostImages {
                store: &images,
                announce_pull: &announce_pull,
                unpacking: &unpacking,
                user_error: &user_error,
            },
            &delonix_volume::HostVolumes {
                root: root.to_path_buf(),
            },
            &delonix_linux::cdi::HostDevices,
            &delonix_linux::run_host::HostRuntime {
                state_root: root.to_path_buf(),
                apparmor_disabled: &apparmor_disabled,
            },
        )
    }
}

impl StorePort for AdapterStore {
    fn container_list(&self) -> Result<Vec<delonix_compute::Container>, delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.list())
            .map_err(Into::into)
    }
    fn container_load(
        &self,
        id_or_name: &str,
    ) -> Result<delonix_compute::Container, delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.load(id_or_name))
            .map_err(Into::into)
    }
    fn container_save(&self, c: &delonix_compute::Container) -> Result<(), delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.save(c))
            .map_err(Into::into)
    }
    fn container_update(
        &self,
        id: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Container) -> bool,
    ) -> Result<delonix_compute::Container, delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.update(id, |c| f(c)))
            .map_err(Into::into)
    }
    fn container_remove(&self, id: &str) -> Result<(), delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.remove(id))
            .map_err(Into::into)
    }
    fn container_remove_unless(
        &self,
        id: &str,
        keep: &mut dyn FnMut(&delonix_compute::Container) -> bool,
    ) -> Result<Option<delonix_compute::Container>, delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.remove_unless(id, |c| keep(c)))
            .map_err(Into::into)
    }
    fn container_claim_name(
        &self,
        name: &str,
    ) -> Result<Box<dyn std::any::Any + Send>, delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.claim_name(name))
            .map(|c| Box::new(c) as Box<dyn std::any::Any + Send>)
            .map_err(Into::into)
    }
    fn vm_list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error> {
        delonix_state::JsonStore::<delonix_compute::Vm>::open(self.0.join("vms"))
            .and_then(|s| s.list())
            .map_err(Into::into)
    }
    fn vm_update(
        &self,
        name: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Vm) -> bool,
    ) -> Result<delonix_compute::Vm, delonix_model::Error> {
        delonix_state::JsonStore::<delonix_compute::Vm>::open(self.0.join("vms"))
            .and_then(|s| s.update(name, |vm| f(vm)))
            .map_err(Into::into)
    }
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
