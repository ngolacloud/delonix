//! `CephStorage` — the node's **Ceph cluster** as this interface needs it
//! (R3d-j, ADR-0081, ADR-0040 A5). The interface holds this port instead of
//! naming the `delonix-provider-ceph` adapter directly: a bin composes an
//! implementor and hands the server one, so the forbidden interface→adapter edge
//! stays in the bin — the same shape as [`crate::net_ops::NetworkOps`] and
//! [`crate::image_repo::ImageRepo`].
//!
//! Four surfaces of one cluster cross this port, each as a port-owned summary so
//! the adapter's `CephCtx`/`ClusterHealth`/`Rgw`/`RgwError` never leak past it:
//! RADOS health ([`CephHealth`]), the RBD/CephFS pool listings, the object
//! gateway's tenants and buckets ([`UserRecord`]/[`BucketRecord`], over the
//! port-owned [`RgwFault`] the caller maps to a gRPC status), and the two
//! storage-pool drivers Ceph adds ([`StoragePoolDriver`], a shared
//! `delonix-storage` type — the registry [`crate::storage`] provisions through
//! holds them). The Ceph client identity crosses as [`CephIdentity`] (the
//! adapter's `CephCtx`). The implementor holds the `Run` the tools execute
//! through, exactly as the old in-crate `run()` did.
//!
//! This Engine has no storage *service* yet (the slice is PRE-R3h), so the
//! implementor is backed by `delonix-provider-ceph` DIRECTLY — the same way the
//! bin backs the record store with `delonix-state` and the SDN with
//! `delonix-sdn`. When a storage service lands, this port is where it is swapped
//! in.

use std::path::{Path, PathBuf};
use std::sync::Arc;

pub use delonix_storage::pool::StoragePoolDriver;

/// The Ceph client identity a surface is reached with: the `ceph.conf` (none =
/// the system default the tools find) and the cephx user. The adapter's
/// `CephCtx`, crossed port-owned.
#[derive(Clone, Debug)]
pub struct CephIdentity {
    pub conf: Option<PathBuf>,
    pub user: String,
}

/// The RADOS cluster's state, as one node sees it — the adapter's
/// `ClusterHealth`, crossed port-owned.
#[derive(Clone, Debug, Default)]
pub struct CephHealth {
    /// `HEALTH_OK`, `HEALTH_WARN`, `HEALTH_ERR`, or empty when unreadable.
    pub status: String,
    pub fsid: String,
    pub mons: u32,
    pub osds_up: u32,
    pub osds_in: u32,
    pub pools: u32,
}

/// A bucket's stats — the adapter's `BucketInfo`, crossed port-owned.
#[derive(Clone, Debug)]
pub struct BucketRecord {
    pub name: String,
    pub owner: String,
    pub objects: u64,
    pub size_bytes: u64,
}

/// An object-gateway tenant's S3 identity — the adapter's `RgwUser`, crossed
/// port-owned.
#[derive(Clone, Debug)]
pub struct UserRecord {
    pub uid: String,
    pub display_name: String,
    pub access_key: String,
    pub secret_key: String,
}

/// Why an object-gateway operation did not give a clean answer — the adapter's
/// `RgwError`, crossed port-owned so the caller ([`crate::storage`]) maps it to a
/// gRPC status without naming the adapter.
#[derive(Clone, Debug)]
pub enum RgwFault {
    /// The gateway tool is not installed / not reachable.
    Unavailable(String),
    /// The call could not be decided (not a clean yes or no).
    Undetermined(String),
    /// The bucket or user provably does not exist.
    NotFound(String),
}

/// The Ceph cluster use cases the node API reads and provisions through (R3d-j).
/// Object-safe (`dyn CephStorage`): every method names only shared/port-owned
/// types — the summaries above, [`StoragePoolDriver`] (a `delonix-storage`
/// type), the shared primitives — never a `delonix-provider-ceph` type, so the
/// implementor lives in the composition and the interface stays clean of the
/// adapter.
pub trait CephStorage: Send + Sync {
    /// Read `ceph status` for the cluster `ident` reaches. `None` when the tool
    /// is missing or the cluster cannot be reached — unknown is never a
    /// healthy-looking zero.
    fn cluster_health(&self, ident: &CephIdentity) -> Option<CephHealth>;
    /// The RBD images the RADOS pool `rados` holds, reached as `ident`.
    fn list_rbd_images(&self, ident: &CephIdentity, rados: &str) -> Result<Vec<String>, String>;
    /// The subvolumes the CephFS filesystem `fs` holds, reached as `ident`.
    fn list_cephfs_subvolumes(&self, ident: &CephIdentity, fs: &str)
        -> Result<Vec<String>, String>;
    /// The object-gateway buckets (all, or `uid`'s), reached with `conf`.
    fn list_buckets(&self, conf: Option<&Path>, uid: Option<&str>)
        -> Result<Vec<String>, RgwFault>;
    /// One bucket's stats, reached with `conf`.
    fn bucket_stats(&self, conf: Option<&Path>, bucket: &str) -> Result<BucketRecord, RgwFault>;
    /// Provision (or read back) a tenant's S3 identity, reached with `conf`.
    fn ensure_user(
        &self,
        conf: Option<&Path>,
        uid: &str,
        display_name: &str,
    ) -> Result<UserRecord, RgwFault>;
    /// The storage-pool drivers Ceph adds (`ceph-rbd`, `cephfs`), for the
    /// composition root to register; `mount_root` is where CephFS subvolumes are
    /// mounted. Registering does no I/O (ADR-0008).
    fn drivers(&self, mount_root: PathBuf) -> Vec<Arc<dyn StoragePoolDriver>>;
}

/// A [`CephStorage`] with no composed cluster — the empty backing
/// [`crate::router`] hands the server when no engine is injected. Health is
/// unreachable, the listings are empty, the gateway reads find nothing and
/// provisioning has no drivers. The production server never uses it — a bin
/// hands in the `delonix-provider-ceph`-backed port — exactly as
/// [`crate::net_ops::NoNet`] is to [`crate::net_ops::NetworkOps`]. The storage
/// paths are not exercised through the default router (its tests drive
/// node/provider reads and refusals).
pub(crate) struct NoCeph;

impl CephStorage for NoCeph {
    fn cluster_health(&self, _ident: &CephIdentity) -> Option<CephHealth> {
        None
    }
    fn list_rbd_images(&self, _ident: &CephIdentity, _rados: &str) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn list_cephfs_subvolumes(
        &self,
        _ident: &CephIdentity,
        _fs: &str,
    ) -> Result<Vec<String>, String> {
        Ok(Vec::new())
    }
    fn list_buckets(
        &self,
        _conf: Option<&Path>,
        _uid: Option<&str>,
    ) -> Result<Vec<String>, RgwFault> {
        Ok(Vec::new())
    }
    fn bucket_stats(&self, _conf: Option<&Path>, bucket: &str) -> Result<BucketRecord, RgwFault> {
        Err(RgwFault::NotFound(bucket.to_string()))
    }
    fn ensure_user(
        &self,
        _conf: Option<&Path>,
        _uid: &str,
        _display_name: &str,
    ) -> Result<UserRecord, RgwFault> {
        Err(RgwFault::Unavailable(
            "no object gateway is composed on this server".into(),
        ))
    }
    fn drivers(&self, _mount_root: PathBuf) -> Vec<Arc<dyn StoragePoolDriver>> {
        Vec::new()
    }
}

/// A [`CephStorage`] over the real `delonix-provider-ceph` adapter, for the
/// crate's own tests only (R3d-j) — the same shape as [`crate::net_ops::LocalNet`].
/// The server never builds this; production hands in a bin's
/// `delonix-provider-ceph`-backed port. `delonix-provider-ceph` is a
/// dev-dependency, so this (and the live-Ceph test) may name it while production
/// code does not. The Ceph tools run through the same `SystemRun` the local
/// drivers use.
#[cfg(test)]
pub(crate) struct LocalCeph;

#[cfg(test)]
fn ctx(ident: &CephIdentity) -> delonix_provider_ceph::ceph::CephCtx {
    delonix_provider_ceph::ceph::CephCtx {
        conf: ident.conf.clone(),
        user: ident.user.clone(),
    }
}

#[cfg(test)]
fn run() -> Arc<dyn delonix_storage::exec::Run> {
    Arc::new(delonix_volume::pool_exec::SystemRun)
}

#[cfg(test)]
fn gateway(conf: Option<&Path>) -> delonix_provider_ceph::rgw::Rgw {
    delonix_provider_ceph::rgw::Rgw::new(run(), conf.map(Path::to_path_buf))
}

#[cfg(test)]
fn fault(e: delonix_provider_ceph::RgwError) -> RgwFault {
    match e {
        delonix_provider_ceph::RgwError::Unavailable(m) => RgwFault::Unavailable(m),
        delonix_provider_ceph::RgwError::Undetermined(m) => RgwFault::Undetermined(m),
        delonix_provider_ceph::RgwError::NotFound(m) => RgwFault::NotFound(m),
    }
}

#[cfg(test)]
impl CephStorage for LocalCeph {
    fn cluster_health(&self, ident: &CephIdentity) -> Option<CephHealth> {
        delonix_provider_ceph::cluster_health(&*run(), &ctx(ident)).map(|h| CephHealth {
            status: h.status,
            fsid: h.fsid,
            mons: h.mons,
            osds_up: h.osds_up,
            osds_in: h.osds_in,
            pools: h.pools,
        })
    }
    fn list_rbd_images(&self, ident: &CephIdentity, rados: &str) -> Result<Vec<String>, String> {
        delonix_provider_ceph::rbd::list_images(&*run(), &ctx(ident), rados)
    }
    fn list_cephfs_subvolumes(
        &self,
        ident: &CephIdentity,
        fs: &str,
    ) -> Result<Vec<String>, String> {
        delonix_provider_ceph::cephfs::list_subvolumes(&*run(), &ctx(ident), fs)
    }
    fn list_buckets(
        &self,
        conf: Option<&Path>,
        uid: Option<&str>,
    ) -> Result<Vec<String>, RgwFault> {
        gateway(conf).list_buckets(uid).map_err(fault)
    }
    fn bucket_stats(&self, conf: Option<&Path>, bucket: &str) -> Result<BucketRecord, RgwFault> {
        let b = gateway(conf).bucket_stats(bucket).map_err(fault)?;
        Ok(BucketRecord {
            name: b.name,
            owner: b.owner,
            objects: b.objects,
            size_bytes: b.size_bytes,
        })
    }
    fn ensure_user(
        &self,
        conf: Option<&Path>,
        uid: &str,
        display_name: &str,
    ) -> Result<UserRecord, RgwFault> {
        let u = gateway(conf)
            .ensure_user(uid, display_name)
            .map_err(fault)?;
        Ok(UserRecord {
            uid: u.uid,
            display_name: u.display_name,
            access_key: u.access_key,
            secret_key: u.secret_key,
        })
    }
    fn drivers(&self, mount_root: PathBuf) -> Vec<Arc<dyn StoragePoolDriver>> {
        delonix_provider_ceph::drivers(run(), mount_root)
    }
}
