//! `StorePort` — the node's **record store** as this interface needs it (R3d-d,
//! ADR-0040 A5). The interface holds this port instead of opening the
//! `delonix-state` adapter directly: `delonix-node-api-bin` mounts the node's
//! engine and hands the server an implementor, so the forbidden
//! interface→adapter edge stays in the bin.
//!
//! It covers the two record stores this crate reads and writes — the container
//! store (`delonix_compute::Container`) and the VM record store
//! (`delonix_compute::Vm`) — in shared types, with the shared
//! [`delonix_model::Error`] both the engine and this crate speak. The container
//! surface is the full flock-guarded one the lifecycle verbs need
//! (`stop`/`remove` go through `delonix_linux` over a [`delonix_model::ports::StateRepository`],
//! which [`PortRepo`] bridges to this object-safe port). The per-call `root` the
//! service still threads is used only for filesystem checks and the operation-log
//! path, never to open a store here.

use delonix_compute::Container;
use delonix_model::ports::StateRepository;

/// The record-store use cases the node API reads and writes through (R3d-d).
/// Object-safe: the lifecycle closures cross as `&mut dyn FnMut` (a `dyn StorePort`
/// cannot carry a generic method), and the implementor forwards them to the
/// engine's generic, flock-guarded store.
pub trait StorePort: Send + Sync {
    /// Every container record the node has.
    fn container_list(&self) -> Result<Vec<Container>, delonix_model::Error>;
    /// One container record by id or name.
    fn container_load(&self, id_or_name: &str) -> Result<Container, delonix_model::Error>;
    /// Persist a container record (create or replace).
    fn container_save(&self, c: &Container) -> Result<(), delonix_model::Error>;
    /// Locked read-modify-write of one container: `f` mutates the record re-read
    /// under the lock and returns `false` to abort the write; the final (or read)
    /// record comes back.
    fn container_update(
        &self,
        id: &str,
        f: &mut dyn FnMut(&mut Container) -> bool,
    ) -> Result<Container, delonix_model::Error>;
    /// Remove a container record by id.
    fn container_remove(&self, id: &str) -> Result<(), delonix_model::Error>;
    /// Locked remove-unless: checks `keep` and removes as ONE critical section (the
    /// same lock `container_update` takes). `None` once removed, or the record as
    /// found when `keep` kept it.
    fn container_remove_unless(
        &self,
        id: &str,
        keep: &mut dyn FnMut(&Container) -> bool,
    ) -> Result<Option<Container>, delonix_model::Error>;
    /// Take the cross-process **named lock** for `name` (the sharded lock the CLI
    /// and CRI also take), serialising check-then-create so two racing same-name
    /// creates cannot both win. The returned value is the opaque RAII guard: hold
    /// it for the whole create, drop it to release. Its concrete type
    /// (`delonix_state`'s `NameClaim`) stays in the adapter — the interface only
    /// keeps it alive.
    fn container_claim_name(
        &self,
        name: &str,
    ) -> Result<Box<dyn std::any::Any + Send>, delonix_model::Error>;
    /// Every VM record the node has.
    fn vm_list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error>;
    /// Locked read-modify-write of one VM record (`f` returns `false` to abort).
    fn vm_update(
        &self,
        name: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Vm) -> bool,
    ) -> Result<delonix_compute::Vm, delonix_model::Error>;
}

/// Bridges an object-safe [`StorePort`] to the generic
/// [`delonix_model::ports::StateRepository`] the `delonix_linux` lifecycle verbs
/// (`stop`/`remove`) take, so the interface can drive them without naming a store
/// adapter. The generic `update`/`remove_unless` closures are forwarded to the
/// port's `&mut dyn FnMut` methods (each applied once). Public so the
/// composition's `delonix-linux`-backed [`crate::linux_lifecycle::LinuxLifecycle`]
/// (the bin's `HostLinux`, the harness's `AdapterLinux`) can bridge the store it
/// is handed to the adapter's `stop`/`remove` without the interface naming a
/// store adapter.
pub struct PortRepo<'a>(pub &'a dyn StorePort);

impl StateRepository<Container> for PortRepo<'_> {
    fn get(&self, id: &str) -> delonix_model::Result<Container> {
        self.0.container_load(id)
    }
    fn list(&self) -> delonix_model::Result<Vec<Container>> {
        self.0.container_list()
    }
    fn set(&self, _id: &str, value: &Container) -> delonix_model::Result<()> {
        self.0.container_save(value)
    }
    fn update<F>(&self, id: &str, f: F) -> delonix_model::Result<Container>
    where
        F: FnOnce(&mut Container) -> bool,
    {
        let mut f = Some(f);
        self.0
            .container_update(id, &mut |c| (f.take().expect("update applies f once"))(c))
    }
    fn remove(&self, id: &str) -> delonix_model::Result<()> {
        self.0.container_remove(id)
    }
    fn remove_unless<F>(&self, id: &str, keep: F) -> delonix_model::Result<Option<Container>>
    where
        F: FnOnce(&Container) -> bool,
    {
        let mut keep = Some(keep);
        self.0
            .container_remove_unless(id, &mut |c| (keep.take().expect("keep applies once"))(c))
    }
}

/// An empty [`StorePort`] for the default `router` (no composed engine): reads are
/// empty, writes are accepted and discarded. The production server never uses it —
/// `delonix-node-api-bin` hands in the engine-backed port — exactly as `NoVms` is
/// to [`crate::vms::VmPort`].
pub(crate) struct NoStore;

impl StorePort for NoStore {
    fn container_list(&self) -> Result<Vec<Container>, delonix_model::Error> {
        Ok(Vec::new())
    }
    fn container_load(&self, id_or_name: &str) -> Result<Container, delonix_model::Error> {
        Err(delonix_model::Error::NotFound(id_or_name.to_string()))
    }
    fn container_save(&self, _c: &Container) -> Result<(), delonix_model::Error> {
        Ok(())
    }
    fn container_update(
        &self,
        id: &str,
        _f: &mut dyn FnMut(&mut Container) -> bool,
    ) -> Result<Container, delonix_model::Error> {
        Err(delonix_model::Error::NotFound(id.to_string()))
    }
    fn container_remove(&self, _id: &str) -> Result<(), delonix_model::Error> {
        Ok(())
    }
    fn container_remove_unless(
        &self,
        _id: &str,
        _keep: &mut dyn FnMut(&Container) -> bool,
    ) -> Result<Option<Container>, delonix_model::Error> {
        Ok(None)
    }
    fn container_claim_name(
        &self,
        _name: &str,
    ) -> Result<Box<dyn std::any::Any + Send>, delonix_model::Error> {
        Ok(Box::new(()))
    }
    fn vm_list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error> {
        Ok(Vec::new())
    }
    fn vm_update(
        &self,
        name: &str,
        _f: &mut dyn FnMut(&mut delonix_compute::Vm) -> bool,
    ) -> Result<delonix_compute::Vm, delonix_model::Error> {
        Err(delonix_model::Error::VmNotFound(name.to_string()))
    }
}

/// A [`StorePort`] over a real on-disk `delonix-state` root, for the crate's own
/// tests only (R3d-d) — the same shape as [`crate::vms::LocalVmReads`]. The server
/// never builds this; production hands in the engine-backed port from the bin.
#[cfg(test)]
pub(crate) struct LocalStore(pub std::path::PathBuf);

#[cfg(test)]
impl LocalStore {
    fn containers(&self) -> Result<delonix_state::Store, delonix_model::Error> {
        delonix_state::Store::open(self.0.join("containers")).map_err(Into::into)
    }
    fn vms(&self) -> Result<delonix_state::JsonStore<delonix_compute::Vm>, delonix_model::Error> {
        delonix_state::JsonStore::<delonix_compute::Vm>::open(self.0.join("vms"))
            .map_err(Into::into)
    }
}

#[cfg(test)]
impl StorePort for LocalStore {
    fn container_list(&self) -> Result<Vec<Container>, delonix_model::Error> {
        self.containers()?.list().map_err(Into::into)
    }
    fn container_load(&self, id_or_name: &str) -> Result<Container, delonix_model::Error> {
        self.containers()?.load(id_or_name).map_err(Into::into)
    }
    fn container_save(&self, c: &Container) -> Result<(), delonix_model::Error> {
        self.containers()?.save(c).map_err(Into::into)
    }
    fn container_update(
        &self,
        id: &str,
        f: &mut dyn FnMut(&mut Container) -> bool,
    ) -> Result<Container, delonix_model::Error> {
        self.containers()?.update(id, |c| f(c)).map_err(Into::into)
    }
    fn container_remove(&self, id: &str) -> Result<(), delonix_model::Error> {
        self.containers()?.remove(id).map_err(Into::into)
    }
    fn container_remove_unless(
        &self,
        id: &str,
        keep: &mut dyn FnMut(&Container) -> bool,
    ) -> Result<Option<Container>, delonix_model::Error> {
        self.containers()?
            .remove_unless(id, |c| keep(c))
            .map_err(Into::into)
    }
    fn container_claim_name(
        &self,
        name: &str,
    ) -> Result<Box<dyn std::any::Any + Send>, delonix_model::Error> {
        self.containers()?
            .claim_name(name)
            .map(|c| Box::new(c) as Box<dyn std::any::Any + Send>)
            .map_err(Into::into)
    }
    fn vm_list(&self) -> Result<Vec<delonix_compute::Vm>, delonix_model::Error> {
        self.vms()?.list().map_err(Into::into)
    }
    fn vm_update(
        &self,
        name: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Vm) -> bool,
    ) -> Result<delonix_compute::Vm, delonix_model::Error> {
        self.vms()?.update(name, |vm| f(vm)).map_err(Into::into)
    }
}
