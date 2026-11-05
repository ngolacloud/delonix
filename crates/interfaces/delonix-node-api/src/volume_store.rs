//! `VolumeStore` — the node's **volume store** as this interface needs it
//! (R3d-j, ADR-0040 A5). The interface holds this port instead of naming the
//! `delonix-volume` adapter directly: a bin composes an implementor and hands
//! the server one, so the forbidden interface→adapter edge stays in the bin —
//! the same shape as [`crate::vms::VmPort`], [`crate::store::StorePort`],
//! [`crate::run_resolver::RunResolver`], [`crate::net_ops::NetworkOps`] and
//! [`crate::image_repo::ImageRepo`].
//!
//! This Engine has no volume *service* yet (the slice is PRE-R3h), so the
//! implementor is backed by `delonix-volume` DIRECTLY — the same way the bin
//! backs the record store with `delonix-state` and the SDN with `delonix-sdn` —
//! not by an engine volume port. When a volume service lands, this port is where
//! it is swapped in.
//!
//! The adapter's `delonix_volume::Volume`/`OwnedVolume` are crossed as the
//! [`VolumeRecord`]/[`OwnedVolumeRecord`] summaries — the fields the
//! `VolumeService` read and write halves render and act on — so the adapter type
//! never leaks past the port (the discipline the CRI's own ports use for their
//! records). The shared [`delonix_model::Error`] both the engine and this crate
//! speak is the error on every fallible method; a measured [`VolumeUsage`] (the
//! adapter's `Usage`) and the pure network-driver rule cross as themselves.

use std::collections::BTreeMap;
use std::path::Path;

use delonix_model::Error;

/// A volume as this interface reads and writes it: the fields of
/// `delonix-volume`'s `Volume` the node API actually renders and acts on,
/// crossed as a summary so the adapter type does not leak past the port.
/// `Debug`-derived on purpose — a `GetVolume`'s `etag` is a hash of this record,
/// so the summary must carry everything that distinguishes one volume from
/// another (a relabel, a changed quota) for the etag to move when the resource
/// does.
#[derive(Clone, Debug)]
pub struct VolumeRecord {
    /// The volume name (its identity in its store).
    pub name: String,
    /// The data directory on the host (`.../_data`), or the mapped device for a
    /// pool volume.
    pub mountpoint: String,
    /// Creation instant (Unix seconds).
    pub created_unix: u64,
    /// Driver: `local` (default) or a network share (`nfs`, `cifs`, `davfs`).
    pub driver: String,
    /// For a network share: the export/device (`server:/path`, `//server/share`,
    /// a URL).
    pub device: Option<String>,
    /// Mount options (`mount -o ...`), e.g. `vers=4,ro`.
    pub options: Option<String>,
    /// Size quota in bytes. `None` = no limit.
    pub quota_bytes: Option<u64>,
    /// Usage percentage above which an alert is raised.
    pub alert_pct: Option<u8>,
    /// Free labels (a `BTreeMap`, so the `Debug` and thus the etag are
    /// deterministic, and a label selector reads it directly).
    pub labels: BTreeMap<String, String>,
    /// Free annotations.
    pub annotations: BTreeMap<String, String>,
    /// The volume this one is carved out of, for a share.
    pub parent: Option<String>,
    /// The storage pool this volume was allocated in (ADR-0067); its data goes
    /// back through the pool's driver when the volume is removed.
    pub pool: Option<String>,
}

/// A [`VolumeRecord`] together with the namespace that owns it — `None` is the
/// unscoped root (NOT the `default` namespace). The adapter's `OwnedVolume`,
/// crossed as a summary.
#[derive(Clone, Debug)]
pub struct OwnedVolumeRecord {
    /// `None` = the unscoped root (no owner).
    pub namespace: Option<String>,
    pub volume: VolumeRecord,
}

/// What a volume's usage walk found: the bytes seen, and whether every directory
/// was readable (the adapter's `Usage`, crossed). When `complete` is `false` the
/// bytes are a floor, not the real usage.
#[derive(Clone, Copy, Debug)]
pub struct VolumeUsage {
    /// Bytes of every file the walk could actually stat.
    pub bytes: u64,
    /// `true` when every directory in the tree was readable, so `bytes` is the
    /// real usage rather than a floor.
    pub complete: bool,
}

/// The volume store use cases the node API reads and writes through (R3d-j).
/// Object-safe (`dyn VolumeStore`): every method names only shared types —
/// [`VolumeRecord`], [`OwnedVolumeRecord`], [`VolumeUsage`], the shared [`Error`]
/// and primitives — never a `delonix-volume` type, so the implementor lives in
/// the composition and the interface stays clean of the adapter.
pub trait VolumeStore: Send + Sync {
    // ---- reads ----

    /// Every volume under `root`, scoped or unscoped, in no particular order
    /// (the caller sorts). The caller checks the store directory exists first
    /// (reading must not create it).
    fn list_all(&self, root: &Path) -> Result<Vec<OwnedVolumeRecord>, Error>;
    /// Walk `path` and report its usage, saying whether the walk was complete.
    fn measure(&self, path: &Path) -> VolumeUsage;

    // ---- pure rules ----

    /// Whether `driver` mounts a network share (as opposed to `local`/loopback).
    fn is_network_driver(&self, driver: &str) -> bool;
    /// Refuse a name the store would never accept (before any operation exists).
    fn validate_name(&self, name: &str) -> Result<(), Error>;

    // ---- writes ----

    /// Create a `local` volume `name` in the unscoped store under `root`.
    fn create_local(&self, root: &Path, name: &str) -> Result<(), Error>;
    /// Register a volume `name` whose data a storage pool allocated at
    /// `mountpoint`, stamping the pool so a delete releases it (ADR-0067).
    fn register_in_pool(
        &self,
        root: &Path,
        name: &str,
        mountpoint: &Path,
        size_bytes: u64,
        alert_pct: Option<u8>,
        pool: &str,
    ) -> Result<(), Error>;
    /// Set (monitored) quota and alert threshold on volume `name` under `root`.
    fn set_quota(
        &self,
        root: &Path,
        name: &str,
        quota: Option<u64>,
        alert_pct: Option<u8>,
    ) -> Result<(), Error>;
    /// Stamp `(label, Some(value))`/`None` pairs on volume `name`'s record.
    fn set_metadata(
        &self,
        root: &Path,
        name: &str,
        labels: &[(String, Option<String>)],
        annotations: &[(String, Option<String>)],
    ) -> Result<(), Error>;
    /// Remove volume `name` from the store under `root` (its data first, record
    /// last) — scoped to `namespace` when given, else the unscoped root.
    fn remove(&self, root: &Path, namespace: Option<&str>, name: &str) -> Result<(), Error>;
}

/// A [`VolumeStore`] with no composed store — the empty backing [`crate::router`]
/// hands the server when no engine is injected. Reads are empty, writes are
/// accepted and discarded; the pure rules answer the safe nothing. The
/// production server never uses it — a bin hands in the `delonix-volume`-backed
/// port — exactly as [`crate::store::NoStore`] is to [`crate::store::StorePort`].
/// The volume read/create/delete paths are not exercised through the default
/// router (its tests drive node/provider reads and refusals).
pub(crate) struct NoVolumes;

impl VolumeStore for NoVolumes {
    fn list_all(&self, _root: &Path) -> Result<Vec<OwnedVolumeRecord>, Error> {
        Ok(Vec::new())
    }
    fn measure(&self, _path: &Path) -> VolumeUsage {
        VolumeUsage {
            bytes: 0,
            complete: true,
        }
    }
    fn is_network_driver(&self, driver: &str) -> bool {
        // The one `delonix_volume::is_network_driver` rule, stated here because
        // this backing names no adapter: the drivers that mount a network share.
        // The production `delonix-volume`-backed port returns the adapter's own.
        matches!(driver, "nfs" | "cifs" | "smb" | "webdav" | "dav")
    }
    fn validate_name(&self, _name: &str) -> Result<(), Error> {
        Ok(())
    }
    fn create_local(&self, _root: &Path, _name: &str) -> Result<(), Error> {
        Ok(())
    }
    fn register_in_pool(
        &self,
        _root: &Path,
        _name: &str,
        _mountpoint: &Path,
        _size_bytes: u64,
        _alert_pct: Option<u8>,
        _pool: &str,
    ) -> Result<(), Error> {
        Ok(())
    }
    fn set_quota(
        &self,
        _root: &Path,
        _name: &str,
        _quota: Option<u64>,
        _alert_pct: Option<u8>,
    ) -> Result<(), Error> {
        Ok(())
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
    fn remove(&self, _root: &Path, _namespace: Option<&str>, _name: &str) -> Result<(), Error> {
        Ok(())
    }
}

/// A [`VolumeStore`] over the real `delonix-volume` adapter, for the crate's own
/// tests only (R3d-j) — the same shape as [`crate::store::LocalStore`],
/// [`crate::vms::LocalVmReads`] and [`crate::net_ops::LocalNet`]. The server
/// never builds this; production hands in a bin's `delonix-volume`-backed port.
/// `delonix-volume` is a (production) dependency the interface keeps for
/// `providers.rs`, so this and the integration harness may name it directly.
#[cfg(test)]
pub(crate) struct LocalVolumes;

#[cfg(test)]
fn to_record(v: &delonix_volume::Volume) -> VolumeRecord {
    VolumeRecord {
        name: v.name.clone(),
        mountpoint: v.mountpoint.clone(),
        created_unix: v.created_unix,
        driver: v.driver.clone(),
        device: v.device.clone(),
        options: v.options.clone(),
        quota_bytes: v.quota_bytes,
        alert_pct: v.alert_pct,
        labels: v.labels.clone(),
        annotations: v.annotations.clone(),
        parent: v.parent.clone(),
        pool: v.pool.clone(),
    }
}

#[cfg(test)]
impl VolumeStore for LocalVolumes {
    fn list_all(&self, root: &Path) -> Result<Vec<OwnedVolumeRecord>, Error> {
        Ok(delonix_volume::VolumeStore::open(root)?
            .list_all()?
            .into_iter()
            .map(|o| OwnedVolumeRecord {
                namespace: o.namespace,
                volume: to_record(&o.volume),
            })
            .collect())
    }
    fn measure(&self, path: &Path) -> VolumeUsage {
        let u = delonix_volume::measure(path);
        VolumeUsage {
            bytes: u.bytes,
            complete: u.is_complete(),
        }
    }
    fn is_network_driver(&self, driver: &str) -> bool {
        delonix_volume::is_network_driver(driver)
    }
    fn validate_name(&self, name: &str) -> Result<(), Error> {
        delonix_volume::VolumeStore::validate_name(name)?;
        Ok(())
    }
    fn create_local(&self, root: &Path, name: &str) -> Result<(), Error> {
        delonix_volume::VolumeStore::open(root)?.create_with(name, "local", None, None)?;
        Ok(())
    }
    fn register_in_pool(
        &self,
        root: &Path,
        name: &str,
        mountpoint: &Path,
        size_bytes: u64,
        alert_pct: Option<u8>,
        pool: &str,
    ) -> Result<(), Error> {
        delonix_volume::VolumeStore::open(root)?
            .register_in_pool(name, mountpoint, size_bytes, alert_pct, pool)?;
        Ok(())
    }
    fn set_quota(
        &self,
        root: &Path,
        name: &str,
        quota: Option<u64>,
        alert_pct: Option<u8>,
    ) -> Result<(), Error> {
        delonix_volume::VolumeStore::open(root)?.set_quota(name, quota, alert_pct, false)?;
        Ok(())
    }
    fn set_metadata(
        &self,
        root: &Path,
        name: &str,
        labels: &[(String, Option<String>)],
        annotations: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        delonix_volume::VolumeStore::open(root)?.set_metadata(name, labels, annotations)?;
        Ok(())
    }
    fn remove(&self, root: &Path, namespace: Option<&str>, name: &str) -> Result<(), Error> {
        let store = match namespace {
            Some(ns) => delonix_volume::VolumeStore::open_scoped(root, ns),
            None => delonix_volume::VolumeStore::open(root),
        }?;
        store.remove_with(name, None)?;
        Ok(())
    }
}
