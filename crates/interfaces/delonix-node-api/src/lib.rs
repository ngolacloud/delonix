//! **The node API of the Delonix Runtime** — `delonix.node.v1` served as gRPC and
//! HTTP/JSON on a local unix socket, or — reachable from another host — on a TCP
//! socket with mutual TLS (ADR-0040 D4, ADR-0042 D1, ADR-0096).
//!
//! What is served today, and it is said here so nobody reads the socket as the
//! whole contract: `NodeService.ListProviders` (ADR-0050 D5) — the same
//! `ProviderInfo` per provider that `delonix provider ls -o json` prints — the
//! entry point `GetApiRoot` (`GET /v1`: a link to every resource served; each
//! resource carries its own `links`, mirrored in an RFC 8288 `Link` header) and
//! `GetNodeInfo`, `GetHealth` and `GetCapacity` (ADR-0042 step C), computed from
//! the same functions `delonix system info` reads. `GET /openapi.json` serves
//! the published document, and `GET /docs` (Swagger UI) and `GET /redoc`
//! render it from UI files embedded in the binary ([`docs`]). `WatchEvents`
//! streams the engine's event log ([`events`]); every service of the
//! contract this socket does not implement at all (`ContainerService`,
//! `PodService`) is not registered either.
//!
//! Errors: gRPC carries `google.rpc.Status`; the REST encoding answers every
//! error with an RFC 9457 `application/problem+json` document built by the
//! engine's `codes::problem` — the same `DX-` codes the CLI exits with — which
//! is what the published OpenAPI declares (ADR-0042 D2).
//!
//! Step E, first wave: `NetworkService.GetNetwork` and `ListNetworks`
//! ([`networks`]) — the resource's `etag` is the REST `ETag` (a matching
//! `If-None-Match` is 304), a list takes `label_selector` ([`selector`]) and
//! pages by name, with the next page as a link.
//!
//! Step E, the first mutations: `CreateNetwork` and `DeleteNetwork`
//! ([`network_ops`]) answer an `Operation` persisted under the state root
//! before the work starts ([`operations`]), read back with
//! `OperationService.GetOperation` and `ListOperations`. `request_id` (REST:
//! `Idempotency-Key`) makes a retry the first answer; `etag` (REST:
//! `If-Match`) makes a stale delete 412.
//!
//! `VolumeService` ([`volumes`] reads, [`volume_ops`] writes): `CreateVolume`
//! of a local volume (quota, alert, labels) or a Ceph-provisioned pool volume
//! (`spec.provision`), and `DeleteVolume` of a local volume, a share's record
//! or a pool volume (its driver releases the data), on the same ledger; a
//! remote share, a share and `destroy_remote` answer `UNIMPLEMENTED` with the
//! CLI command that does them. `OperationService.WatchOperation`
//! streams an operation until it ends (gRPC only: the REST encoding does not
//! carry streams), and `CancelOperation` answers `FAILED_PRECONDITION`
//! (DX-5002) — no served mutation has a cancellation point, and an operation
//! is never reported cancelled for work that ran.
//!
//! `VirtualMachineService` ([`vms`] reads, [`vm_ops`] writes), full: create/
//! get/list/delete, start/stop (the same `Operation` ledger as networks),
//! the synchronous pause/resume, and the snapshot lifecycle. `Console` (a
//! live byte pipe into the guest) is `UNIMPLEMENTED`, the same family as
//! `ContainerService.Exec`, which this socket does not serve at all yet.
//!
//! The REST routes are not written by hand: `build.rs` generates the route
//! table and the dispatchers from the `google.api.http` annotations
//! ([`transcode`]), for every RPC of the contract. A route of a service this
//! engine does not serve yet answers 501 (DX-6001), a path the contract does
//! not have 404 (DX-4001).
//!
//! Both encodings come from the same `proto/` files: the gRPC stubs and the
//! proto3 JSON (`pbjson`, proto field names — what the published OpenAPI
//! declares). The HTTP route for a `google.api.http` annotation is written by
//! hand for now (the four GET routes of `NodeService`); a generic transcoder
//! over the annotations is a later slice of ADR-0042 step C.
//!
//! `delonix serve node-api` runs the `delonix-node-api` binary, which calls
//! [`serve`]. On the unix socket the file is `0600` and every accepted
//! connection is checked with `SO_PEERCRED` against this process's uid — the
//! same discipline as the CRI, the management API and the holder's control
//! socket. On the TCP socket ([`serve_tls_listener`], ADR-0096) the transport
//! authenticates instead: a client certificate that chains to the configured
//! client CA is required and verified at the handshake, cleartext is never
//! served, and the caller's identity in that certificate drives authorization
//! from A3 onward (today a valid client is served every operation).

// A `tonic::Status` is large by nature and it is the error every service
// method returns; the CRI crate silences this lint for the same reason.
#![allow(clippy::result_large_err)]

use delonix_model::Error;

/// The generated contract: prost messages, tonic server and client, and the
/// proto3 JSON `Serialize`/`Deserialize` of every message.
pub mod proto {
    pub mod v1 {
        tonic::include_proto!("delonix.node.v1");
        include!(concat!(env!("OUT_DIR"), "/delonix.node.v1.serde.rs"));
    }
}

pub mod activation;
pub mod container_exec;
pub mod container_launch;
pub mod container_logs;
pub mod container_ops;
pub mod container_update;
pub mod containers;
pub mod docs;
pub mod events;
pub mod image_ops;
pub mod image_repo;
pub mod images;
pub mod linux_lifecycle;
pub mod net_ops;
pub mod network_ops;
pub mod networks;
pub mod node;
pub mod operations;
pub mod page;
pub mod pod_ops;
pub mod pods;
pub mod provider_reports;
pub mod providers;
pub mod run_resolver;
pub mod selector;
mod service;
pub mod stacks;
pub mod storage;
pub mod storage_backend;
pub mod store;
pub mod transcode;
pub mod vm_ops;
pub mod vms;
pub mod volume_ops;
pub mod volume_store;
pub mod volumes;

pub use image_repo::ImageRepo;
pub use linux_lifecycle::LinuxLifecycle;
pub use net_ops::NetworkOps;
pub use provider_reports::ProviderReports;
pub use run_resolver::RunResolver;
pub use service::{
    capacity, health, link_header, list_providers, node_info, openapi_document, router,
    router_with, NodeApi, OPENAPI_YAML,
};
pub use storage_backend::CephStorage;
pub use store::StorePort;
pub use vms::VmPort;
pub use volume_store::VolumeStore;

/// The node API's composition root: the ports this process registers before it
/// serves anything, once.
///
/// The SHARED VM registry providers a networked Cloud Hypervisor create needs —
/// the node's VM network and the cgroup leaf provider — are NOT installed here
/// any more (R3d-e): they name the `delonix-sdn`/`delonix-linux` adapters, so
/// `delonix-node-api-bin` installs them into `delonix_compute::vm_registry`
/// before it serves, the same registrations the CLI's `main` makes. What stays
/// here names no adapter: the Ceph storage-pool drivers (ADR-0081), through
/// which `CreateVolume` with a `spec.provision` naming a `ceph-rbd`/`cephfs`
/// pool allocates. Idempotent; the first registration in a process wins.
pub fn compose(ceph: &dyn CephStorage) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        // The Ceph storage-pool drivers (ADR-0081): `CreateVolume` with a
        // `spec.provision` naming a `ceph-rbd`/`cephfs` pool allocates through
        // them. The drivers come from the `CephStorage` port the bin composes
        // (R3d-j) — the interface names no adapter. Registering does no I/O.
        storage::register_pool_drivers(ceph);
    });
}

/// Where the server listens.
#[derive(Debug, Clone)]
pub enum Listen {
    /// A socket this process binds: a unix path (`unix:///path` or a bare path)
    /// or a TCP address (`tcp://host:port`). A `tcp://` address requires TLS
    /// material (ADR-0096); a unix path must not carry any.
    Path(String),
    /// A listening socket the service manager passed (`LISTEN_FDS`), already
    /// bound, with its mode set by the `.socket` unit ([`activation`]). Its
    /// family (unix vs TCP) is classified at serve time
    /// ([`activation::classify_listening_stream`]): a TCP socket takes the
    /// mutual-TLS path, a unix socket the `SO_PEERCRED` path.
    Inherited(std::os::fd::RawFd),
}

/// The TLS material the TCP transport serves with (ADR-0096): the server's own
/// certificate chain and key, and the CA that client certificates must chain to
/// (require-and-verify client authentication). All three are PEM, already read
/// from disk by the caller — the engine does not own this material, it consumes
/// what the operator (or, from A2, the node-identity flow) supplies. The unix
/// transport takes none of this; supplying it for a unix listener is refused.
#[derive(Clone)]
pub struct Tls {
    /// The server certificate chain (PEM, leaf first).
    pub cert_pem: String,
    /// The server private key (PEM).
    pub key_pem: String,
    /// The CA bundle a client certificate must chain to (PEM).
    pub client_ca_pem: String,
}

impl std::fmt::Debug for Tls {
    // Never print the key material, even at debug.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tls")
            .field("cert_pem", &"<pem>")
            .field("key_pem", &"<redacted>")
            .field("client_ca_pem", &"<pem>")
            .finish()
    }
}

/// Builds the rustls `ServerConfig` for the TCP transport: the server's cert
/// chain + key, and **require-and-verify** client authentication against the
/// configured client CA (`WebPkiClientVerifier`). A client with no certificate,
/// or one that does not chain to the CA, is rejected at the handshake. ALPN
/// offers `h2` then `http/1.1`, so the same listener serves gRPC and HTTP/JSON,
/// as the unix listener does over cleartext HTTP/2. PEM is parsed via the
/// `rustls-pki-types` re-export, as `ingress_proxy.rs` does (no `rustls-pemfile`,
/// RUSTSEC-2025-0134).
fn build_server_config(
    tls: &Tls,
) -> Result<std::sync::Arc<tokio_rustls::rustls::ServerConfig>, Error> {
    use std::sync::Arc;
    use tokio_rustls::rustls::pki_types::pem::{Error as PemError, PemObject};
    use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};
    use tokio_rustls::rustls::server::WebPkiClientVerifier;
    use tokio_rustls::rustls::{RootCertStore, ServerConfig};

    // The provider must be installed before a `ServerConfig` is built; doing it
    // here is idempotent and needs no coordination with the caller.
    let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();

    let tls_err = |what: &str, e: String| Error::Invalid(format!("node API TLS: {what}: {e}"));

    let certs: Vec<CertificateDer<'static>> =
        CertificateDer::pem_slice_iter(tls.cert_pem.as_bytes())
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| tls_err("server certificate (PEM)", e.to_string()))?;
    if certs.is_empty() {
        return Err(Error::Invalid(
            "node API TLS: server certificate PEM has no CERTIFICATE".into(),
        ));
    }
    let key = PrivateKeyDer::from_pem_slice(tls.key_pem.as_bytes()).map_err(|e| match e {
        PemError::NoItemsFound => {
            Error::Invalid("node API TLS: server key PEM has no PRIVATE KEY".into())
        }
        other => tls_err("server key (PEM)", other.to_string()),
    })?;

    let mut roots = RootCertStore::empty();
    let mut added = 0usize;
    for ca in CertificateDer::pem_slice_iter(tls.client_ca_pem.as_bytes()) {
        let ca = ca.map_err(|e| tls_err("client CA (PEM)", e.to_string()))?;
        roots
            .add(ca)
            .map_err(|e| tls_err("client CA", e.to_string()))?;
        added += 1;
    }
    if added == 0 {
        return Err(Error::Invalid(
            "node API TLS: client CA PEM has no CERTIFICATE — a TCP listener must verify clients"
                .into(),
        ));
    }
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| tls_err("client verifier", e.to_string()))?;
    let mut cfg = ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(certs, key)
        .map_err(|e| tls_err("mismatched server cert/key", e.to_string()))?;
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(cfg))
}

/// Serves the node API on a unix socket, blocking the calling thread. `addr` is a
/// path or `unix:///path`, `vms` is the VM port the server reads through (same
/// shape as `delonix_mgmt::serve_blocking`, which already takes its VM view). A
/// test passes an adapter-backed port for the VM reads and provider reports it
/// drives; the bin's production entry is [`serve`], which injects the engine.
#[allow(clippy::too_many_arguments)]
pub fn serve_blocking(
    addr: &str,
    vms: std::sync::Arc<dyn VmPort>,
    store: std::sync::Arc<dyn StorePort>,
    run_resolver: std::sync::Arc<dyn RunResolver>,
    net: std::sync::Arc<dyn NetworkOps>,
    images: std::sync::Arc<dyn ImageRepo>,
    providers: std::sync::Arc<dyn ProviderReports>,
    linux: std::sync::Arc<dyn LinuxLifecycle>,
    volumes: std::sync::Arc<dyn VolumeStore>,
    ceph: std::sync::Arc<dyn CephStorage>,
) -> Result<(), Error> {
    serve_with(
        Listen::Path(addr.to_string()),
        None,
        None,
        vms,
        store,
        run_resolver,
        net,
        images,
        providers,
        linux,
        volumes,
        ceph,
    )
}

/// Serves the node API, blocking the calling thread, until `idle` (if given)
/// finds it idle — then returns `Ok`, which a socket-activated service reports
/// as a clean exit (ADR-0040 P5). With `idle` `None`, or a zero timeout, it
/// serves until killed. `vms` is the composition's VM port (ADR-0040 A5):
/// `delonix-node-api-bin` mounts the engine and passes an implementor.
#[allow(clippy::too_many_arguments)]
pub fn serve(
    listen: Listen,
    idle: Option<activation::IdleExit>,
    tls: Option<Tls>,
    vms: std::sync::Arc<dyn VmPort>,
    store: std::sync::Arc<dyn StorePort>,
    run_resolver: std::sync::Arc<dyn RunResolver>,
    net: std::sync::Arc<dyn NetworkOps>,
    images: std::sync::Arc<dyn ImageRepo>,
    providers: std::sync::Arc<dyn ProviderReports>,
    linux: std::sync::Arc<dyn LinuxLifecycle>,
    volumes: std::sync::Arc<dyn VolumeStore>,
    ceph: std::sync::Arc<dyn CephStorage>,
) -> Result<(), Error> {
    serve_with(
        listen,
        idle,
        tls,
        vms,
        store,
        run_resolver,
        net,
        images,
        providers,
        linux,
        volumes,
        ceph,
    )
}

/// A listener taken before any thread exists, with its transport decided
/// (ADR-0096): a local unix socket (`SO_PEERCRED`, cleartext) or a TCP socket
/// with the mutual-TLS acceptor's config.
enum Prepared {
    Unix(std::os::unix::net::UnixListener),
    Tcp(
        std::net::TcpListener,
        std::sync::Arc<tokio_rustls::rustls::ServerConfig>,
    ),
}

#[allow(clippy::too_many_arguments)]
fn serve_with(
    listen: Listen,
    idle: Option<activation::IdleExit>,
    tls: Option<Tls>,
    vms: std::sync::Arc<dyn VmPort>,
    store: std::sync::Arc<dyn StorePort>,
    run_resolver: std::sync::Arc<dyn RunResolver>,
    net: std::sync::Arc<dyn NetworkOps>,
    images: std::sync::Arc<dyn ImageRepo>,
    providers: std::sync::Arc<dyn ProviderReports>,
    linux: std::sync::Arc<dyn LinuxLifecycle>,
    volumes: std::sync::Arc<dyn VolumeStore>,
    ceph: std::sync::Arc<dyn CephStorage>,
) -> Result<(), Error> {
    compose(ceph.as_ref());
    delonix_node::alloc_tuning::limit_malloc_arenas();
    let runtime_err = |context: &'static str| {
        move |e: std::io::Error| Error::Runtime {
            context,
            message: e.to_string(),
        }
    };
    // TLS material supplied for a unix listener is refused, not ignored: it
    // would read as protection that is not there (ADR-0096 decision 3).
    let tls_on_unix = || {
        Error::Invalid(
            "node API TLS material was given, but the listener is a local unix socket — \
             TLS applies to the TCP transport only (tcp://host:port)"
                .into(),
        )
    };
    // A TCP listener without TLS material is refused — cleartext is never served
    // on TCP (ADR-0096 decision 3), fail-closed.
    let tcp_without_tls = |why: &str| {
        Error::Invalid(format!(
            "node API: {why}, which requires mutual TLS, but no TLS material was given \
             (--tls-cert/--tls-key/--tls-client-ca or DELONIX_NODE_API_TLS_*) — refusing to \
             serve the node contract in cleartext over TCP"
        ))
    };

    // The transport is decided, and the listener taken/bound, before any thread
    // exists.
    let prepared = match listen {
        Listen::Inherited(fd) => {
            let kind = activation::classify_listening_stream(fd)?;
            activation::set_cloexec(fd)?;
            use std::os::fd::FromRawFd;
            match kind {
                activation::ActivatedKind::Unix => {
                    if tls.is_some() {
                        return Err(tls_on_unix());
                    }
                    // SAFETY: `fd` was passed by the service manager for this
                    // process alone (`LISTEN_PID`), was just classified as a
                    // listening unix socket, and nothing else here owns it.
                    let l = unsafe { std::os::unix::net::UnixListener::from_raw_fd(fd) };
                    l.set_nonblocking(true)
                        .map_err(runtime_err("inherited socket"))?;
                    Prepared::Unix(l)
                }
                activation::ActivatedKind::Tcp => {
                    let tls = tls.ok_or_else(|| tcp_without_tls("the inherited socket is TCP"))?;
                    let cfg = build_server_config(&tls)?;
                    // SAFETY: as above, just classified as a listening TCP
                    // socket, owned by nothing else in this process.
                    let l = unsafe { std::net::TcpListener::from_raw_fd(fd) };
                    l.set_nonblocking(true)
                        .map_err(runtime_err("inherited socket"))?;
                    Prepared::Tcp(l, cfg)
                }
            }
        }
        Listen::Path(addr) => {
            if let Some(hostport) = addr.strip_prefix("tcp://") {
                let tls =
                    tls.ok_or_else(|| tcp_without_tls(&format!("the address {addr} is TCP")))?;
                let cfg = build_server_config(&tls)?;
                let l = std::net::TcpListener::bind(hostport).map_err(runtime_err("bind"))?;
                l.set_nonblocking(true).map_err(runtime_err("bind"))?;
                Prepared::Tcp(l, cfg)
            } else {
                if tls.is_some() {
                    return Err(tls_on_unix());
                }
                let path = addr.strip_prefix("unix://").unwrap_or(&addr).to_string();
                let _ = std::fs::remove_file(&path); // an old socket of a previous run
                let l =
                    std::os::unix::net::UnixListener::bind(&path).map_err(runtime_err("bind"))?;
                // 0600 on the file AND `SO_PEERCRED` per connection: the file
                // mode depends on the ambient umask at bind time, the credential
                // check does not.
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
                l.set_nonblocking(true).map_err(runtime_err("bind"))?;
                Prepared::Unix(l)
            }
        }
    };

    let idle = idle.filter(|i| !i.timeout.is_zero());
    let rt = tokio::runtime::Builder::new_multi_thread()
        // A local control-plane API: a couple of workers, the same reasoning as
        // the CRI and the management API.
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| Error::Runtime {
            context: "tokio",
            message: e.to_string(),
        })?;
    rt.block_on(async move {
        let router = router_with(
            vms,
            store,
            run_resolver,
            net,
            images,
            providers,
            linux,
            volumes,
            ceph,
        );
        match prepared {
            Prepared::Unix(l) => {
                let uds = tokio::net::UnixListener::from_std(l).map_err(runtime_err("listener"))?;
                tracing::info!(
                    socket = ?uds.local_addr().ok().and_then(|a| a.as_pathname().map(|p| p.display().to_string())),
                    idle_timeout = ?idle.as_ref().map(|i| i.timeout),
                    "delonix-node-api (node API) listening (unix, SO_PEERCRED)"
                );
                serve_listener(uds, router, idle).await
            }
            Prepared::Tcp(l, cfg) => {
                let tcp = tokio::net::TcpListener::from_std(l).map_err(runtime_err("listener"))?;
                let acceptor = tokio_rustls::TlsAcceptor::from(cfg);
                tracing::info!(
                    addr = ?tcp.local_addr().ok().map(|a| a.to_string()),
                    idle_timeout = ?idle.as_ref().map(|i| i.timeout),
                    "delonix-node-api (node API) listening (tcp, mutual TLS)"
                );
                serve_tls_listener(tcp, acceptor, router, idle).await
            }
        }
    })
}

/// Serves the router over a `UnixListener`: accept loop + hyper-util's `auto`
/// connection builder, which speaks HTTP/1.1 for the JSON routes and HTTP/2
/// (prior knowledge, no TLS on a local socket) for gRPC on the same listener.
///
/// With `idle`, returns `Ok` once no connection has been open for its timeout
/// and no operation of this process is `RUNNING` ([`activation`]). Every
/// accepted connection, open or closed, restarts the clock.
pub async fn serve_listener(
    uds: tokio::net::UnixListener,
    app: axum::Router,
    idle: Option<activation::IdleExit>,
) -> Result<(), Error> {
    use delonix_host::peer_cred::peer_uid;
    use hyper::body::Incoming;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tower::Service;
    // SAFETY: geteuid() has no preconditions.
    let own_uid = unsafe { libc::geteuid() };
    let mut make = app.into_make_service();
    let open = Arc::new(AtomicUsize::new(0));
    let last = Arc::new(Mutex::new(std::time::Instant::now()));
    // How often the idle condition is looked at: a fraction of the timeout,
    // never busier than 50 ms nor lazier than 1 s.
    let tick = idle.as_ref().map(|i| {
        (i.timeout / 4).clamp(
            std::time::Duration::from_millis(50),
            std::time::Duration::from_secs(1),
        )
    });
    loop {
        let accepted = match (&idle, tick) {
            (Some(i), Some(tick)) => {
                tokio::select! {
                    r = uds.accept() => r,
                    _ = tokio::time::sleep(tick) => {
                        if idle_reached(i, &last, &open) {
                            tracing::info!("delonix-node-api idle, exiting");
                            return Ok(());
                        }
                        continue;
                    }
                }
            }
            _ => uds.accept().await,
        };
        // Transient accept errors (EMFILE, ECONNABORTED) are retried, never
        // fatal — see the management API for the incident that taught it.
        let (socket, _) = match accepted {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!(error = %e, "accept() failed, retrying");
                continue;
            }
        };
        if let Ok(mut t) = last.lock() {
            *t = std::time::Instant::now();
        }
        if peer_uid(&socket) != Some(own_uid) {
            continue;
        }
        let tower_service = match make.call(&socket).await {
            Ok(svc) => svc,
            Err(never) => match never {},
        };
        open.fetch_add(1, Ordering::SeqCst);
        let (open, last) = (open.clone(), last.clone());
        tokio::spawn(async move {
            let io = TokioIo::new(socket);
            let hyper_service = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                tower_service.clone().call(req)
            });
            let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(io, hyper_service)
                .await;
            // The clock restarts when the LAST connection closes, not when it
            // opened: a long stream ending is activity.
            if let Ok(mut t) = last.lock() {
                *t = std::time::Instant::now();
            }
            open.fetch_sub(1, Ordering::SeqCst);
        });
    }
}

/// The idle-exit condition the unix and TLS accept loops share: no connection
/// open, quiet for at least the timeout, and no operation of THIS process still
/// `RUNNING` in the ledger. Shared so the two transports cannot drift on the
/// one subtle decision (an open stream is an open connection — `open` is held
/// for the connection's whole life).
fn idle_reached(
    idle: &activation::IdleExit,
    last: &std::sync::Mutex<std::time::Instant>,
    open: &std::sync::atomic::AtomicUsize,
) -> bool {
    use std::sync::atomic::Ordering;
    let quiet = last.lock().map(|t| t.elapsed()).unwrap_or_default();
    open.load(Ordering::SeqCst) == 0
        && quiet >= idle.timeout
        && !operations::running_owned_by(&idle.root, std::process::id() as i32)
}

/// Serves the router over a `TcpListener` with **mutual TLS** (ADR-0096): the
/// same accept loop and `auto` connection builder as [`serve_listener`], but
/// each accepted TCP connection is first put through the TLS handshake
/// (`acceptor.accept`). The acceptor's `ServerConfig` requires and verifies a
/// client certificate, so a client with none — or one that does not chain to
/// the configured client CA — fails the handshake and is dropped here, never
/// reaching the router. ALPN picks `h2` (gRPC) or `http/1.1` (JSON) on the same
/// listener. The identity in the client cert is not read yet (A3); a valid
/// client is served every operation, as the unix path serves the owning uid.
///
/// The idle exit is the same as the unix path ([`idle_reached`]): a handshake
/// in progress or a live connection holds `open`, so the server does not exit
/// under one.
pub async fn serve_tls_listener(
    tcp: tokio::net::TcpListener,
    acceptor: tokio_rustls::TlsAcceptor,
    app: axum::Router,
    idle: Option<activation::IdleExit>,
) -> Result<(), Error> {
    use hyper::body::Incoming;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tower::Service;
    let mut make = app.into_make_service();
    let open = Arc::new(AtomicUsize::new(0));
    let last = Arc::new(Mutex::new(std::time::Instant::now()));
    let tick = idle.as_ref().map(|i| {
        (i.timeout / 4).clamp(
            std::time::Duration::from_millis(50),
            std::time::Duration::from_secs(1),
        )
    });
    loop {
        let accepted = match (&idle, tick) {
            (Some(i), Some(tick)) => {
                tokio::select! {
                    r = tcp.accept() => r,
                    _ = tokio::time::sleep(tick) => {
                        if idle_reached(i, &last, &open) {
                            tracing::info!("delonix-node-api idle, exiting");
                            return Ok(());
                        }
                        continue;
                    }
                }
            }
            _ => tcp.accept().await,
        };
        // Transient accept errors (EMFILE, ECONNABORTED) are retried, never
        // fatal — the same posture as the unix loop.
        let (socket, _peer) = match accepted {
            Ok(x) => x,
            Err(e) => {
                tracing::warn!(error = %e, "accept() failed, retrying");
                continue;
            }
        };
        if let Ok(mut t) = last.lock() {
            *t = std::time::Instant::now();
        }
        // The router service is produced before the handshake (the TLS stream
        // exists only after it); `IntoMakeService` ignores the target, so the
        // not-yet-wrapped socket is a fine stand-in.
        let tower_service = match make.call(&socket).await {
            Ok(svc) => svc,
            Err(never) => match never {},
        };
        open.fetch_add(1, Ordering::SeqCst);
        let (open, last, acceptor) = (open.clone(), last.clone(), acceptor.clone());
        tokio::spawn(async move {
            match acceptor.accept(socket).await {
                Ok(tls) => {
                    let io = TokioIo::new(tls);
                    let hyper_service =
                        hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                            tower_service.clone().call(req)
                        });
                    let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                        .serve_connection_with_upgrades(io, hyper_service)
                        .await;
                }
                // A client with no certificate, or one from an untrusted CA,
                // fails the handshake here (fail-closed, ADR-0096): logged at
                // debug and dropped, never reaching the contract.
                Err(e) => {
                    tracing::debug!(error = %e, "TLS handshake refused");
                }
            }
            if let Ok(mut t) = last.lock() {
                *t = std::time::Instant::now();
            }
            open.fetch_sub(1, Ordering::SeqCst);
        });
    }
}
