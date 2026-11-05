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

use delonix_node_api::image_repo::StoredImage;
use delonix_node_api::linux_lifecycle::{ContainerUsage, LimitUpdate, ResourceUpdate};
use delonix_node_api::net_ops::NetRecord;
use delonix_node_api::storage_backend::{
    BucketRecord, CephHealth, CephIdentity, RgwFault, StoragePoolDriver, UserRecord,
};
use delonix_node_api::store::PortRepo;
use delonix_node_api::volume_store::{OwnedVolumeRecord, VolumeRecord, VolumeUsage};
use delonix_node_api::{
    CephStorage, ImageRepo, LinuxLifecycle, NetworkOps, ProviderReports, RunResolver, StorePort,
    VmPort, VolumeStore,
};

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

/// A [`NetworkOps`] over the real `delonix-sdn` adapter (R3d-f) — the tests'
/// stand-in for the `delonix-sdn`-backed port the bin composes. Production
/// reaches the SDN only through this port; the node-api crate names no SDN
/// adapter, so this lives in the test harness over the `delonix-sdn`
/// dev-dependency, exactly as [`AdapterVms`]/[`AdapterStore`] do. The provider
/// report a `ListProviders` test asserts comes from the real registry, the one
/// thing only this backing can supply.
pub struct AdapterNet;

/// The adapter-backed network port, as an `Arc` for `serve_blocking`.
pub fn backing_net() -> Arc<dyn NetworkOps> {
    Arc::new(AdapterNet)
}

/// An [`ImageRepo`] over the real `delonix-oci` adapter at a state root (R3d-g)
/// — the tests' stand-in for the `delonix-oci`-backed port the bin composes.
/// Production reaches the OCI image store only through this port; the node-api
/// crate names no OCI adapter, so this lives in the test harness over the
/// `delonix-oci` dev-dependency, exactly as [`AdapterNet`]/[`AdapterStore`] do.
/// It mirrors `delonix-node-api-bin`'s `OciImages`.
pub struct AdapterImages;

/// The adapter-backed image port, as an `Arc` for `serve_blocking`.
pub fn backing_images() -> Arc<dyn ImageRepo> {
    Arc::new(AdapterImages)
}

/// A [`LinuxLifecycle`] over the real `delonix-linux` adapter (R3d-i) — the tests'
/// stand-in for the `delonix-linux`-backed port the bin composes. Production
/// reaches the container lifecycle edge only through this port; the node-api crate
/// names no linux adapter on this path, so this lives in the test harness over the
/// `delonix-linux` dependency, exactly as [`AdapterImages`]/[`AdapterNet`] do. It
/// mirrors `delonix-node-api-bin`'s `HostLinux`. The `stop`/`remove` verbs drive
/// over the record store they are handed, bridged by [`PortRepo`].
pub struct AdapterLinux;

/// The adapter-backed container lifecycle port, as an `Arc` for `serve_blocking`.
pub fn backing_linux() -> Arc<dyn LinuxLifecycle> {
    Arc::new(AdapterLinux)
}

impl LinuxLifecycle for AdapterLinux {
    fn reconcile_status(&self, c: &mut delonix_compute::Container) {
        delonix_linux::reconcile_status(c);
    }
    fn container_usage(&self, c: &delonix_compute::Container) -> ContainerUsage {
        let u = delonix_linux::container_usage(c);
        ContainerUsage {
            cpu_usage_usec: u.cpu_usage_usec,
            mem_bytes: u.mem_bytes,
        }
    }
    fn stop(
        &self,
        store: &dyn StorePort,
        c: &mut delonix_compute::Container,
        grace_secs: u64,
    ) -> Result<(), delonix_model::Error> {
        delonix_linux::stop(&PortRepo(store), c, grace_secs).map_err(Into::into)
    }
    fn send_signal(
        &self,
        c: &delonix_compute::Container,
        signal: i32,
    ) -> Result<(), delonix_model::Error> {
        delonix_linux::send_signal(c, signal).map_err(Into::into)
    }
    fn remove(
        &self,
        store: &dyn StorePort,
        c: &delonix_compute::Container,
        force: bool,
    ) -> Result<(), delonix_model::Error> {
        delonix_linux::remove(&PortRepo(store), c, force).map_err(Into::into)
    }
    fn update_kube_resources(
        &self,
        c: &delonix_compute::Container,
        u: &ResourceUpdate,
    ) -> Result<LimitUpdate, delonix_model::Error> {
        let du = delonix_linux::ResourceUpdate {
            memory: u.memory.as_deref(),
            cpus: u.cpus.as_deref(),
            cpu_weight: u.cpu_weight.as_deref(),
            cpuset: u.cpuset.as_deref(),
            cpuset_mems: u.cpuset_mems.as_deref(),
            hugepage_limits: &u.hugepage_limits,
            unified: &u.unified,
            oom_score_adj: u.oom_score_adj,
        };
        delonix_linux::update_kube_resources(c, &du)
            .map(|l| match l {
                delonix_linux::LimitUpdate::Applied => LimitUpdate::Applied,
                delonix_linux::LimitUpdate::Deferred => LimitUpdate::Deferred,
                delonix_linux::LimitUpdate::NotEnforced => LimitUpdate::NotEnforced,
            })
            .map_err(Into::into)
    }
    fn is_rootless(&self) -> bool {
        delonix_linux::is_rootless()
    }
    fn cgroup_limits_apply(&self) -> bool {
        delonix_linux::cgroup_limits_apply()
    }
    fn host_ncpu(&self) -> u64 {
        delonix_linux::host_ncpu()
    }
    fn host_mem_bytes(&self) -> u64 {
        delonix_linux::host_mem_bytes()
    }
    fn slice_path(&self) -> Option<String> {
        delonix_linux::slice_path()
    }
}

fn stored_image(store: &delonix_oci::ImageStore, img: &delonix_oci::Image) -> StoredImage {
    let mut size: u64 = 0;
    for layer in &img.layers {
        if let Ok(meta) = std::fs::metadata(store.cas().path(layer)) {
            size += meta.len();
        }
    }
    StoredImage {
        id: img.id.clone(),
        repo_tags: img.repo_tags.clone(),
        created_unix: img.created_unix,
        size_bytes: size as i64,
    }
}

impl ImageRepo for AdapterImages {
    fn list(&self, root: &std::path::Path) -> Result<Vec<StoredImage>, delonix_model::Error> {
        let store = delonix_oci::ImageStore::open(root)?;
        Ok(store
            .list()?
            .iter()
            .map(|img| stored_image(&store, img))
            .collect())
    }
    fn remove(&self, root: &std::path::Path, reference: &str) -> Result<(), delonix_model::Error> {
        delonix_oci::ImageStore::open(root)?.remove(reference)?;
        Ok(())
    }
    fn strip_digest<'a>(&self, digest: &'a str) -> &'a str {
        delonix_oci::cas::strip(digest)
    }
}

/// A [`ProviderReports`] over the four real adapters (R3d-h) — the tests'
/// stand-in for the adapter-backed port the bin composes. Production reaches the
/// Proxmox/Linux/volume/OPNsense reports only through this port; the node-api
/// crate names no provider/host adapter in production, so this lives in the test
/// harness over the `delonix-provider-proxmox`/`delonix-provider-opnsense`/
/// `delonix-linux`/`delonix-volume` dev-dependencies, exactly as
/// [`AdapterNet`]/[`AdapterImages`] do. It mirrors `delonix-node-api-bin`'s
/// `HostProviders`, so the provider list a `ListProviders` test asserts carries
/// the real Proxmox/Linux/volume/OPNsense ids.
pub struct AdapterProviders;

/// The adapter-backed capability-report port, as an `Arc` for `serve_blocking`.
pub fn backing_providers() -> Arc<dyn ProviderReports> {
    Arc::new(AdapterProviders)
}

impl ProviderReports for AdapterProviders {
    fn proxmox_compute_report(
        &self,
        configured: bool,
    ) -> delonix_compute::capability::ProviderReport {
        delonix_provider_proxmox::capability_report(configured)
    }
    fn proxmox_network_report(
        &self,
        configured: bool,
    ) -> delonix_compute::capability::ProviderReport {
        delonix_provider_proxmox::network_capability_report(configured)
    }
    fn linux_compute_report(&self, measured: bool) -> delonix_compute::capability::ProviderReport {
        let host = if measured {
            delonix_linux::provider_report::LinuxHost::probe()
        } else {
            delonix_linux::provider_report::LinuxHost::ASSUMED
        };
        delonix_linux::provider_report::report(&host)
    }
    fn volume_storage_report(&self, measured: bool) -> delonix_compute::capability::ProviderReport {
        let host = if measured {
            delonix_volume::provider_report::StorageHost::probe()
        } else {
            delonix_volume::provider_report::StorageHost::ASSUMED
        };
        delonix_volume::provider_report::report(&host)
    }
    fn opnsense_report(&self, is_gateway: bool) -> delonix_compute::capability::ProviderReport {
        delonix_provider_opnsense::capability_report(is_gateway)
    }
    fn opnsense_id(&self) -> &'static str {
        delonix_provider_opnsense::ID
    }
}

/// A [`VolumeStore`] over the real `delonix-volume` adapter (R3d-j) — the tests'
/// stand-in for the `delonix-volume`-backed port the bin composes. Production
/// reaches the volume store only through this port; the node-api crate names no
/// volume adapter on this path, so this lives in the test harness, exactly as
/// [`AdapterImages`]/[`AdapterNet`] do. It mirrors `delonix-node-api-bin`'s
/// `HostVolumes`.
pub struct AdapterVolumes;

/// The adapter-backed volume port, as an `Arc` for `serve_blocking`.
pub fn backing_volumes() -> Arc<dyn VolumeStore> {
    Arc::new(AdapterVolumes)
}

fn volume_record(v: &delonix_volume::Volume) -> VolumeRecord {
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

impl VolumeStore for AdapterVolumes {
    fn list_all(
        &self,
        root: &std::path::Path,
    ) -> Result<Vec<OwnedVolumeRecord>, delonix_model::Error> {
        Ok(delonix_volume::VolumeStore::open(root)?
            .list_all()?
            .into_iter()
            .map(|o| OwnedVolumeRecord {
                namespace: o.namespace,
                volume: volume_record(&o.volume),
            })
            .collect())
    }
    fn measure(&self, path: &std::path::Path) -> VolumeUsage {
        let u = delonix_volume::measure(path);
        VolumeUsage {
            bytes: u.bytes,
            complete: u.is_complete(),
        }
    }
    fn is_network_driver(&self, driver: &str) -> bool {
        delonix_volume::is_network_driver(driver)
    }
    fn validate_name(&self, name: &str) -> Result<(), delonix_model::Error> {
        delonix_volume::VolumeStore::validate_name(name)?;
        Ok(())
    }
    fn create_local(&self, root: &std::path::Path, name: &str) -> Result<(), delonix_model::Error> {
        delonix_volume::VolumeStore::open(root)?.create_with(name, "local", None, None)?;
        Ok(())
    }
    fn register_in_pool(
        &self,
        root: &std::path::Path,
        name: &str,
        mountpoint: &std::path::Path,
        size_bytes: u64,
        alert_pct: Option<u8>,
        pool: &str,
    ) -> Result<(), delonix_model::Error> {
        delonix_volume::VolumeStore::open(root)?
            .register_in_pool(name, mountpoint, size_bytes, alert_pct, pool)?;
        Ok(())
    }
    fn set_quota(
        &self,
        root: &std::path::Path,
        name: &str,
        quota: Option<u64>,
        alert_pct: Option<u8>,
    ) -> Result<(), delonix_model::Error> {
        delonix_volume::VolumeStore::open(root)?.set_quota(name, quota, alert_pct, false)?;
        Ok(())
    }
    fn set_metadata(
        &self,
        root: &std::path::Path,
        name: &str,
        labels: &[(String, Option<String>)],
        annotations: &[(String, Option<String>)],
    ) -> Result<(), delonix_model::Error> {
        delonix_volume::VolumeStore::open(root)?.set_metadata(name, labels, annotations)?;
        Ok(())
    }
    fn remove(
        &self,
        root: &std::path::Path,
        namespace: Option<&str>,
        name: &str,
    ) -> Result<(), delonix_model::Error> {
        let store = match namespace {
            Some(ns) => delonix_volume::VolumeStore::open_scoped(root, ns),
            None => delonix_volume::VolumeStore::open(root),
        }?;
        store.remove_with(name, None)?;
        Ok(())
    }
}

/// A [`CephStorage`] over the real `delonix-provider-ceph` adapter (R3d-j) — the
/// tests' stand-in for the `delonix-provider-ceph`-backed port the bin composes.
/// Production reaches the Ceph cluster only through this port; the node-api crate
/// names no Ceph adapter, so this lives in the test harness over the
/// `delonix-provider-ceph` dev-dependency. It mirrors `delonix-node-api-bin`'s
/// `CephStore`. The integration tests that use it drive refusals (no cluster is
/// present), so no live cluster is required.
pub struct AdapterCeph;

/// The adapter-backed Ceph port, as an `Arc` for `serve_blocking`.
pub fn backing_ceph() -> Arc<dyn CephStorage> {
    Arc::new(AdapterCeph)
}

fn ceph_ctx(ident: &CephIdentity) -> delonix_provider_ceph::ceph::CephCtx {
    delonix_provider_ceph::ceph::CephCtx {
        conf: ident.conf.clone(),
        user: ident.user.clone(),
    }
}

fn ceph_run() -> Arc<delonix_volume::pool_exec::SystemRun> {
    Arc::new(delonix_volume::pool_exec::SystemRun)
}

fn rgw_gateway(conf: Option<&std::path::Path>) -> delonix_provider_ceph::rgw::Rgw {
    delonix_provider_ceph::rgw::Rgw::new(ceph_run(), conf.map(std::path::Path::to_path_buf))
}

fn rgw_fault(e: delonix_provider_ceph::RgwError) -> RgwFault {
    match e {
        delonix_provider_ceph::RgwError::Unavailable(m) => RgwFault::Unavailable(m),
        delonix_provider_ceph::RgwError::Undetermined(m) => RgwFault::Undetermined(m),
        delonix_provider_ceph::RgwError::NotFound(m) => RgwFault::NotFound(m),
    }
}

impl CephStorage for AdapterCeph {
    fn cluster_health(&self, ident: &CephIdentity) -> Option<CephHealth> {
        delonix_provider_ceph::cluster_health(&*ceph_run(), &ceph_ctx(ident)).map(|h| CephHealth {
            status: h.status,
            fsid: h.fsid,
            mons: h.mons,
            osds_up: h.osds_up,
            osds_in: h.osds_in,
            pools: h.pools,
        })
    }
    fn list_rbd_images(&self, ident: &CephIdentity, rados: &str) -> Result<Vec<String>, String> {
        delonix_provider_ceph::rbd::list_images(&*ceph_run(), &ceph_ctx(ident), rados)
    }
    fn list_cephfs_subvolumes(
        &self,
        ident: &CephIdentity,
        fs: &str,
    ) -> Result<Vec<String>, String> {
        delonix_provider_ceph::cephfs::list_subvolumes(&*ceph_run(), &ceph_ctx(ident), fs)
    }
    fn list_buckets(
        &self,
        conf: Option<&std::path::Path>,
        uid: Option<&str>,
    ) -> Result<Vec<String>, RgwFault> {
        rgw_gateway(conf).list_buckets(uid).map_err(rgw_fault)
    }
    fn bucket_stats(
        &self,
        conf: Option<&std::path::Path>,
        bucket: &str,
    ) -> Result<BucketRecord, RgwFault> {
        let b = rgw_gateway(conf).bucket_stats(bucket).map_err(rgw_fault)?;
        Ok(BucketRecord {
            name: b.name,
            owner: b.owner,
            objects: b.objects,
            size_bytes: b.size_bytes,
        })
    }
    fn ensure_user(
        &self,
        conf: Option<&std::path::Path>,
        uid: &str,
        display_name: &str,
    ) -> Result<UserRecord, RgwFault> {
        let u = rgw_gateway(conf)
            .ensure_user(uid, display_name)
            .map_err(rgw_fault)?;
        Ok(UserRecord {
            uid: u.uid,
            display_name: u.display_name,
            access_key: u.access_key,
            secret_key: u.secret_key,
        })
    }
    fn drivers(&self, mount_root: std::path::PathBuf) -> Vec<Arc<dyn StoragePoolDriver>> {
        delonix_provider_ceph::drivers(ceph_run(), mount_root)
    }
}

fn net_record(n: &delonix_sdn::Network) -> NetRecord {
    NetRecord {
        name: n.name.clone(),
        driver: n.driver.clone(),
        subnet: n.subnet.clone(),
        bridge: n.bridge.clone(),
        gateway: n.gateway.clone(),
        parent: n.parent.clone(),
        vni: n.vni,
        wg_ip: n.wg_ip.clone(),
        peers: n.peers.clone(),
        labels: n.labels.clone(),
        annotations: n.annotations.clone(),
    }
}

impl NetworkOps for AdapterNet {
    fn list(&self, root: &std::path::Path) -> Result<Vec<NetRecord>, delonix_model::Error> {
        Ok(delonix_sdn::NetworkStore::open(root)?
            .list()?
            .iter()
            .map(net_record)
            .collect())
    }
    fn get(&self, root: &std::path::Path, name: &str) -> Result<NetRecord, delonix_model::Error> {
        Ok(net_record(
            &delonix_sdn::NetworkStore::open(root)?.get(name)?,
        ))
    }
    fn set_metadata(
        &self,
        root: &std::path::Path,
        name: &str,
        labels: &[(String, Option<String>)],
        annotations: &[(String, Option<String>)],
    ) -> Result<(), delonix_model::Error> {
        delonix_sdn::NetworkStore::open(root)?.set_metadata(name, labels, annotations)?;
        Ok(())
    }
    fn validate_name(&self, name: &str) -> Result<(), delonix_model::Error> {
        delonix_sdn::NetworkStore::validate_name(name)?;
        Ok(())
    }
    fn validate_subnet(&self, cidr: &str) -> Result<(), delonix_model::Error> {
        delonix_sdn::NetworkStore::validate_subnet(cidr)?;
        Ok(())
    }
    fn create_bridge(
        &self,
        root: &std::path::Path,
        name: &str,
        subnet: Option<&str>,
    ) -> Result<(), delonix_model::Error> {
        let store = delonix_sdn::NetworkStore::open(root)?;
        delonix_sdn::netops::create_bridge(&store, name, subnet, |_| Ok(None))?;
        Ok(())
    }
    fn remove(&self, root: &std::path::Path, name: &str) -> Result<(), delonix_model::Error> {
        let store = delonix_sdn::NetworkStore::open(root)?;
        delonix_sdn::netops::remove(&store, name)?;
        Ok(())
    }
    fn dependents_of(
        &self,
        name: &str,
        subnet: &str,
        containers: &[delonix_compute::Container],
        vms: &[(String, String)],
    ) -> Vec<String> {
        delonix_sdn::netops::dependents_of(name, subnet, containers, vms)
    }
    fn realized_names(&self, root: &std::path::Path) -> Vec<String> {
        delonix_sdn::infra::network_list_in(root)
            .into_iter()
            .map(|d| d.name)
            .collect()
    }
    fn reachable(
        &self,
        network: &str,
        ip: &str,
        mac: &str,
        timeout: std::time::Duration,
    ) -> Option<bool> {
        delonix_sdn::infra::sdn_reachable(network, ip, mac, timeout)
    }
    fn provider_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        vec![delonix_sdn::provider_report::report(
            &delonix_sdn::provider_report::SdnHost::probe(),
        )]
    }
    fn declared_provider_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        vec![delonix_sdn::provider_report::report(
            &delonix_sdn::provider_report::SdnHost::ASSUMED,
        )]
    }
    fn gateway_provider_ids(&self) -> Vec<String> {
        delonix_sdn::gateway::gateway_provider_ids()
            .into_iter()
            .map(String::from)
            .collect()
    }
    fn default_net(&self) -> &'static str {
        delonix_sdn::DEFAULT_NET
    }
    fn parse_overlay_peer(&self, peer: &str) -> (String, Option<(String, String)>) {
        delonix_sdn::parse_overlay_peer(peer)
    }
    fn expand_publish_range(&self, spec: &str) -> Result<Vec<String>, delonix_model::Error> {
        delonix_sdn::expand_publish_range(spec).map_err(Into::into)
    }
    fn normalize_publish_spec(&self, spec: &str) -> Result<String, delonix_model::Error> {
        delonix_sdn::normalize_publish_spec(spec).map_err(Into::into)
    }
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
        delonix_vm::start(&self.0, name)
            .map(|_| ())
            .map_err(Into::into)
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
