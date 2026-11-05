//! `delonix-node-api` — the node API (`delonix.node.v1`, gRPC and HTTP/JSON on a
//! unix socket, the calling uid only). `delonix serve node-api` runs this binary.
//!
//! A listening socket passed by systemd socket activation (`LISTEN_FDS`) is
//! served as is; otherwise `--addr` beats `DELONIX_NODE_API_ADDR`, which beats
//! `unix:///run/delonix-node.sock`. `--idle-timeout` (or
//! `DELONIX_NODE_API_IDLE_TIMEOUT`) makes the server exit once idle — the
//! default when socket-activated, so nothing stays resident (ADR-0040 P5).

use std::os::fd::RawFd;
use std::process::ExitCode;
use std::sync::Arc;

use delonix_compute::Vm;
use delonix_engine::Engine;
use delonix_model::Error;
use delonix_node_api::image_repo::StoredImage;
use delonix_node_api::net_ops::NetRecord;
use delonix_node_api::{
    activation, ImageRepo, Listen, NetworkOps, ProviderReports, RunResolver, StorePort, VmPort,
};

/// The composition's VM reads, handed to the node API as its `VmPort` port
/// (ADR-0040 A5). The bin is the one place that names `delonix-engine`: it mounts
/// the `Engine` and the server receives it already composed, never learning what
/// is underneath, so no `interface -> delonix-vm` edge survives on the VM read
/// path. The native VM error maps to the shared engine error the interface speaks.
struct EngineVms(Engine);

impl VmPort for EngineVms {
    fn list(&self) -> Result<Vec<Vm>, Error> {
        self.0.vms().list().map_err(Into::into)
    }

    fn status(&self, name: &str) -> Result<Vm, Error> {
        self.0.vms().status(name).map_err(Into::into)
    }

    fn exists(&self, name: &str) -> bool {
        self.0.vms().exists(name)
    }

    fn start(&self, name: &str) -> Result<(), Error> {
        self.0.vms().start(name).map(|_| ()).map_err(Into::into)
    }

    fn stop(&self, name: &str) -> Result<(), Error> {
        self.0.vms().stop(name).map_err(Into::into)
    }

    fn pause(&self, name: &str) -> Result<(), Error> {
        self.0.vms().pause(name).map_err(Into::into)
    }

    fn unpause(&self, name: &str) -> Result<(), Error> {
        self.0.vms().unpause(name).map_err(Into::into)
    }

    fn snapshots(&self, name: &str) -> Result<Vec<String>, Error> {
        self.0.vms().snapshots(name).map_err(Into::into)
    }

    fn snapshot(&self, name: &str, snap: &str) -> Result<(), Error> {
        self.0.vms().snapshot(name, snap).map_err(Into::into)
    }

    fn restore(&self, name: &str, snap: &str) -> Result<(), Error> {
        self.0.vms().restore(name, snap).map_err(Into::into)
    }

    fn delete_snapshot(&self, name: &str, snap: &str) -> Result<(), Error> {
        self.0.vms().delete_snapshot(name, snap).map_err(Into::into)
    }

    fn destroy(&self, name: &str, force: bool, purge_disks: bool) -> Result<(), Error> {
        self.0
            .vms()
            .destroy(name, force, purge_disks)
            .map(|_| ())
            .map_err(Into::into)
    }

    fn create(&self, cfg: &delonix_compute::vm_backend::VmConfig) -> Result<(), Error> {
        self.0.vms().create(cfg).map(|_| ()).map_err(Into::into)
    }

    fn image_defaults(&self, reference: &str) -> Option<delonix_compute::ports::ImageDefaults> {
        self.0.vms().image_defaults(reference)
    }

    fn store_image_disk(&self, reference: &str) -> String {
        self.0.vms().store_image_disk(reference)
    }

    fn refuse_undeclared_network(
        &self,
        cfg: &delonix_compute::vm_backend::VmConfig,
    ) -> Result<(), Error> {
        self.0
            .vms()
            .refuse_undeclared_network(cfg)
            .map_err(Into::into)
    }

    fn ip_is_predicted(&self, vm: &Vm) -> bool {
        self.0.vms().ip_is_predicted(vm)
    }

    fn provider_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        self.0.provider_reports()
    }

    fn declared_vm_reports(&self) -> Vec<delonix_compute::capability::ProviderReport> {
        self.0.declared_vm_reports()
    }
}

/// The composition's record store, handed to the node API as its `StorePort`
/// (ADR-0040 A5, R3d-d). The bin names the `delonix-state` adapter directly — a
/// bin may name an adapter (A1) — so the `delonix-node-api` interface no longer
/// opens the store itself. Rooted at the node's state root, it serves the
/// container store, the VM record store and the operation-log's atomic write; the
/// native state error maps to the shared engine error the interface speaks.
struct StateStore(std::path::PathBuf);

impl StorePort for StateStore {
    fn container_list(&self) -> Result<Vec<delonix_compute::Container>, Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.list())
            .map_err(Into::into)
    }
    fn container_load(&self, id_or_name: &str) -> Result<delonix_compute::Container, Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.load(id_or_name))
            .map_err(Into::into)
    }
    fn container_save(&self, c: &delonix_compute::Container) -> Result<(), Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.save(c))
            .map_err(Into::into)
    }
    fn container_update(
        &self,
        id: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Container) -> bool,
    ) -> Result<delonix_compute::Container, Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.update(id, |c| f(c)))
            .map_err(Into::into)
    }
    fn container_remove(&self, id: &str) -> Result<(), Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.remove(id))
            .map_err(Into::into)
    }
    fn container_remove_unless(
        &self,
        id: &str,
        keep: &mut dyn FnMut(&delonix_compute::Container) -> bool,
    ) -> Result<Option<delonix_compute::Container>, Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.remove_unless(id, |c| keep(c)))
            .map_err(Into::into)
    }
    fn container_claim_name(&self, name: &str) -> Result<Box<dyn std::any::Any + Send>, Error> {
        delonix_state::Store::open(self.0.join("containers"))
            .and_then(|s| s.claim_name(name))
            .map(|c| Box::new(c) as Box<dyn std::any::Any + Send>)
            .map_err(Into::into)
    }
    fn vm_list(&self) -> Result<Vec<Vm>, Error> {
        delonix_state::JsonStore::<Vm>::open(self.0.join("vms"))
            .and_then(|s| s.list())
            .map_err(Into::into)
    }
    fn vm_update(&self, name: &str, f: &mut dyn FnMut(&mut Vm) -> bool) -> Result<Vm, Error> {
        delonix_state::JsonStore::<Vm>::open(self.0.join("vms"))
            .and_then(|s| s.update(name, |vm| f(vm)))
            .map_err(Into::into)
    }
}

/// The composition's container-create resolver, handed to the node API as its
/// `RunResolver` port (ADR-0040 A5, R3d-e). The bin composes the four host
/// adapters a `CreateContainer` resolution needs — the OCI image store, the
/// volume root, the CDI device view and the Linux runtime — so the
/// `delonix-node-api` interface no longer names them. Rooted at the node's state
/// root; it opens the OCI image store itself, exactly as the interface's create
/// handler did before the port existed. The host adapters' errors map to the
/// shared engine error the interface speaks.
struct HostResolver(std::path::PathBuf);

impl RunResolver for HostResolver {
    fn resolve_run(
        &self,
        root: &std::path::Path,
        o: &delonix_compute::RunOpts,
        id: String,
        name: String,
        namespace: String,
    ) -> Result<delonix_compute::run::Resolved, Error> {
        use delonix_oci::rootfs_user::UserLookupError;
        let images = delonix_oci::ImageStore::open(&self.0)?;
        let announce_pull = |_r: &str| {};
        // No progress line: run the unpacking straight through.
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

/// The composition's SDN, handed to the node API as its `NetworkOps` port
/// (ADR-0040 A5, R3d-f). The bin names the `delonix-sdn` adapter directly — a
/// bin may name an adapter (A1) — so the `delonix-node-api` interface no longer
/// names it. This Engine has no network service yet (PRE-R3h), so the port is
/// backed by `delonix-sdn` directly, exactly as `StateStore` backs the record
/// store with `delonix-state`; the adapter's `Network` is summarised into the
/// interface's [`NetRecord`], and the native SDN error maps to the shared engine
/// error the interface speaks.
struct SdnNet;

/// `delonix-sdn`'s `Network` as the interface's summary record.
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

impl NetworkOps for SdnNet {
    fn list(&self, root: &std::path::Path) -> Result<Vec<NetRecord>, Error> {
        Ok(delonix_sdn::NetworkStore::open(root)?
            .list()?
            .iter()
            .map(net_record)
            .collect())
    }
    fn get(&self, root: &std::path::Path, name: &str) -> Result<NetRecord, Error> {
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
    ) -> Result<(), Error> {
        delonix_sdn::NetworkStore::open(root)?.set_metadata(name, labels, annotations)?;
        Ok(())
    }
    fn validate_name(&self, name: &str) -> Result<(), Error> {
        delonix_sdn::NetworkStore::validate_name(name)?;
        Ok(())
    }
    fn validate_subnet(&self, cidr: &str) -> Result<(), Error> {
        delonix_sdn::NetworkStore::validate_subnet(cidr)?;
        Ok(())
    }
    fn create_bridge(
        &self,
        root: &std::path::Path,
        name: &str,
        subnet: Option<&str>,
    ) -> Result<(), Error> {
        let store = delonix_sdn::NetworkStore::open(root)?;
        delonix_sdn::netops::create_bridge(&store, name, subnet, |_| Ok(None))?;
        Ok(())
    }
    fn remove(&self, root: &std::path::Path, name: &str) -> Result<(), Error> {
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
    fn expand_publish_range(&self, spec: &str) -> Result<Vec<String>, Error> {
        delonix_sdn::expand_publish_range(spec).map_err(Into::into)
    }
    fn normalize_publish_spec(&self, spec: &str) -> Result<String, Error> {
        delonix_sdn::normalize_publish_spec(spec).map_err(Into::into)
    }
}

/// The composition's OCI image store, handed to the node API as its `ImageRepo`
/// port (ADR-0040 A5, R3d-g). The bin names the `delonix-oci` adapter directly —
/// a bin may name an adapter (A1) — so the `delonix-node-api` interface no longer
/// names it. This Engine has no image service yet (PRE-R3h), so the port is
/// backed by `delonix-oci` directly, exactly as `StateStore`/`SdnNet` back the
/// record store and the SDN; the adapter's `Image` is summarised into the
/// interface's [`StoredImage`] (its layer blobs summed to the on-disk size), and
/// the native OCI error maps to the shared engine error the interface speaks.
struct OciImages;

/// `delonix-oci`'s `Image` as the interface's summary record, with the on-disk
/// size summed from its layer blobs in the CAS (`stat`, never reading them).
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

impl ImageRepo for OciImages {
    fn list(&self, root: &std::path::Path) -> Result<Vec<StoredImage>, Error> {
        let store = delonix_oci::ImageStore::open(root)?;
        Ok(store
            .list()?
            .iter()
            .map(|img| stored_image(&store, img))
            .collect())
    }
    fn remove(&self, root: &std::path::Path, reference: &str) -> Result<(), Error> {
        delonix_oci::ImageStore::open(root)?.remove(reference)?;
        Ok(())
    }
    fn strip_digest<'a>(&self, digest: &'a str) -> &'a str {
        delonix_oci::cas::strip(digest)
    }
}

/// The composition's per-adapter capability reports, handed to the node API as
/// its `ProviderReports` port (ADR-0040 A5, R3d-h). The bin names the
/// `delonix-provider-proxmox`, `delonix-provider-opnsense`, `delonix-linux` and
/// `delonix-volume` adapters directly — a bin may name an adapter (A1) — so the
/// `delonix-node-api` interface no longer names them in `providers.rs`. This
/// Engine has no report service yet, so the port is backed by the four adapters
/// directly, exactly as `StateStore`/`SdnNet`/`OciImages` back the record store,
/// the SDN and the image store.
struct HostProviders;

impl ProviderReports for HostProviders {
    fn proxmox_compute_report(&self, configured: bool) -> delonix_compute::capability::ProviderReport {
        delonix_provider_proxmox::capability_report(configured)
    }
    fn proxmox_network_report(&self, configured: bool) -> delonix_compute::capability::ProviderReport {
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

/// The `--version` line: binary name and the crate version, nothing else.
fn version_line() -> String {
    format!("delonix-node-api {}", env!("CARGO_PKG_VERSION"))
}

/// True when the first argument asks for the version. Answered before any
/// effect, including the dispatch version handshake.
fn wants_version(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("--version" | "-V"))
}

/// What the arguments say. Each `None` falls back to its environment variable,
/// then to the default ([`resolve`]).
#[derive(Debug, Default, PartialEq)]
struct Args {
    addr: Option<String>,
    idle_timeout: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Args, Error> {
    let mut out = Args::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let (flag, inline) = match a.split_once('=') {
            Some((f, v)) if f.starts_with("--") => (f, Some(v.to_string())),
            _ => (a.as_str(), None),
        };
        let slot = match flag {
            "--addr" => &mut out.addr,
            "--idle-timeout" => &mut out.idle_timeout,
            _ => return Err(Error::Invalid(format!("unknown argument: {a}"))),
        };
        let value = match inline {
            Some(v) => v,
            None => it
                .next()
                .cloned()
                .ok_or_else(|| Error::Invalid(format!("{flag} needs a value")))?,
        };
        *slot = Some(value);
    }
    Ok(out)
}

/// The default address when nothing is inherited and nothing is given.
const DEFAULT_ADDR: &str = "unix:///run/delonix-node.sock";

/// Where to listen and when to exit, from the arguments, the environment and
/// an inherited socket (`fd`, ADR-0040 P5).
///
/// - An inherited socket wins over `DELONIX_NODE_API_ADDR` and the default;
///   an EXPLICIT `--addr` together with one is refused (DX-1904): the two name
///   different sockets, and serving one silently would leave the other's
///   clients unanswered.
/// - The idle timeout: `--idle-timeout`, else `DELONIX_NODE_API_IDLE_TIMEOUT`,
///   else [`activation::DEFAULT_ACTIVATED_IDLE`] when activated and «never»
///   when this process binds its own socket. `0` is «never».
fn resolve(
    args: &Args,
    env_addr: Option<String>,
    env_idle: Option<String>,
    fd: Option<RawFd>,
    root: std::path::PathBuf,
) -> Result<(Listen, Option<activation::IdleExit>), Error> {
    let listen = match (fd, &args.addr) {
        (Some(fd), Some(addr)) => {
            return Err(Error::coded(
                1904,
                Error::Invalid(format!(
                    "--addr {addr} was given, but the service manager passed a listening socket \
                     (descriptor {fd}); drop --addr from the unit's ExecStart= — the socket is \
                     the .socket unit's ListenStream="
                )),
            ))
        }
        (Some(fd), None) => Listen::Inherited(fd),
        (None, addr) => Listen::Path(
            addr.clone()
                .or(env_addr)
                .unwrap_or_else(|| DEFAULT_ADDR.to_string()),
        ),
    };
    let timeout = match args.idle_timeout.clone().or(env_idle) {
        Some(t) => Some(delonix_host::dispatch::parse_idle_timeout(&t)?),
        None if fd.is_some() => Some(activation::DEFAULT_ACTIVATED_IDLE),
        None => None,
    };
    let idle = timeout.map(|timeout| activation::IdleExit { timeout, root });
    Ok((listen, idle))
}

fn run() -> Result<(), Error> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    delonix_host::dispatch::check_version("delonix-node-api", env!("CARGO_PKG_VERSION"))
        .map_err(Error::Unavailable)?;
    let parsed = parse_args(&args)?;
    let root = delonix_node_api::node::state_root();
    let (listen, idle) = resolve(
        &parsed,
        std::env::var("DELONIX_NODE_API_ADDR").ok(),
        std::env::var("DELONIX_NODE_API_IDLE_TIMEOUT").ok(),
        activation::from_env()?,
        root.clone(),
    )?;
    // Mount the engine and hand the server its VM port (ADR-0040 A5). The read
    // path resolves off the engine; the write half of the VM lifecycle still
    // reaches `delonix-vm` inside the interface until a later slice moves it.
    let store: Arc<dyn StorePort> = Arc::new(StateStore(root.clone()));
    // The container-create host adapters, composed behind the `RunResolver` port
    // (R3d-e) so the interface no longer names them.
    let run_resolver: Arc<dyn RunResolver> = Arc::new(HostResolver(root.clone()));
    // R3d-e: the bin installs the SDN VM-network + linux process-leaves providers
    // into the shared `vm_registry` (moved out of the interface's `compose()`),
    // the same registrations the CLI's `main` makes — without them a networked
    // `CreateVirtualMachine` on Cloud Hypervisor is refused.
    delonix_compute::vm_registry::set_network(Box::new(delonix_sdn::vm_network::HostVmNetwork {
        state_root: root.clone(),
    }));
    delonix_compute::vm_registry::set_process_leaves(Box::new(delonix_linux::HostProcessLeaves));
    let engine = Engine::mount(root)?;
    let vms: Arc<dyn VmPort> = Arc::new(EngineVms(engine));
    // The SDN, behind the `NetworkOps` port (R3d-f): this Engine has no network
    // service yet, so the bin backs it with `delonix-sdn` directly — the same
    // shape as the record store — and the interface names no SDN adapter.
    let net: Arc<dyn NetworkOps> = Arc::new(SdnNet);
    // The OCI image store, behind the `ImageRepo` port (R3d-g): this Engine has
    // no image service yet, so the bin backs it with `delonix-oci` directly —
    // the same shape as the record store and the SDN — and the interface names
    // no OCI adapter.
    let images: Arc<dyn ImageRepo> = Arc::new(OciImages);
    // The per-adapter capability reports, behind the `ProviderReports` port
    // (R3d-h): this Engine has no report service yet, so the bin backs it with
    // the Proxmox/OPNsense/Linux/volume adapters directly — the same shape as the
    // record store, the SDN and the image store — and the interface names none of
    // the four adapters in `providers.rs`.
    let providers: Arc<dyn ProviderReports> = Arc::new(HostProviders);
    delonix_node_api::serve(
        listen,
        idle,
        vms,
        store,
        run_resolver,
        net,
        images,
        providers,
    )
}

fn main() -> ExitCode {
    // `--version` must not initialise telemetry (no exporter, no side effects).
    if wants_version(&std::env::args().skip(1).collect::<Vec<_>>()) {
        println!("{}", version_line());
        return ExitCode::SUCCESS;
    }
    delonix_telemetry::telemetry::init();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!(
                "delonix-node-api: {}: {e}",
                delonix_model::codes::label(e.number())
            );
            ExitCode::from(delonix_model::exitcode::for_error(&e).clamp(1, 255) as u8)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn version_flags_are_recognised() {
        assert!(wants_version(&args(&["--version"])));
        assert!(wants_version(&args(&["-V"])));
        assert!(!wants_version(&args(&[])));
        assert!(!wants_version(&args(&["--addr", "--version"])));
    }

    fn resolve_plain(a: &[&str], env: Option<&str>, fd: Option<RawFd>) -> Result<Listen, Error> {
        let parsed = parse_args(&args(a))?;
        resolve(&parsed, env.map(String::from), None, fd, "/r".into()).map(|(l, _)| l)
    }

    fn path_of(l: Listen) -> String {
        match l {
            Listen::Path(p) => p,
            other => panic!("expected a path, got {other:?}"),
        }
    }

    #[test]
    fn the_flag_beats_the_variable_which_beats_the_default() {
        assert_eq!(
            path_of(resolve_plain(&["--addr", "unix:///a"], Some("unix:///b"), None).unwrap()),
            "unix:///a"
        );
        assert_eq!(
            path_of(resolve_plain(&["--addr=unix:///a"], None, None).unwrap()),
            "unix:///a"
        );
        assert_eq!(
            path_of(resolve_plain(&[], Some("unix:///b"), None).unwrap()),
            "unix:///b"
        );
        assert_eq!(
            path_of(resolve_plain(&[], None, None).unwrap()),
            "unix:///run/delonix-node.sock"
        );
        assert!(parse_args(&args(&["--port"])).is_err());
        assert!(parse_args(&args(&["--addr"])).is_err());
    }

    #[test]
    fn an_inherited_socket_beats_the_variable_and_refuses_an_explicit_addr() {
        assert!(matches!(
            resolve_plain(&[], Some("unix:///b"), Some(3)).unwrap(),
            Listen::Inherited(3)
        ));
        let e = resolve_plain(&["--addr", "unix:///a"], None, Some(3)).unwrap_err();
        assert_eq!(e.number(), 1904, "{e}");
    }

    #[test]
    fn the_idle_timeout_defaults_only_when_activated() {
        let r = |a: &[&str], env: Option<&str>, fd| {
            resolve(
                &parse_args(&args(a)).unwrap(),
                None,
                env.map(String::from),
                fd,
                "/r".into(),
            )
            .map(|(_, i)| i.map(|i| i.timeout))
        };
        use std::time::Duration;
        assert_eq!(r(&[], None, None).unwrap(), None);
        assert_eq!(
            r(&[], None, Some(3)).unwrap(),
            Some(activation::DEFAULT_ACTIVATED_IDLE)
        );
        assert_eq!(
            r(&["--idle-timeout", "2s"], Some("9"), None).unwrap(),
            Some(Duration::from_secs(2))
        );
        assert_eq!(
            r(&[], Some("9"), None).unwrap(),
            Some(Duration::from_secs(9))
        );
        assert_eq!(
            r(&["--idle-timeout=0"], None, Some(3)).unwrap(),
            Some(Duration::ZERO)
        );
        assert!(r(&["--idle-timeout", "soon"], None, None).is_err());
    }
}
