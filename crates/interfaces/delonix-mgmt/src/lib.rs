//! **Local management API of the Delonix Runtime** (HTTP+JSON over a unix socket).
//!
//! A local process on the same host drives the engine through this socket
//! **without a direct link to the crates** — it speaks only HTTP. The engine
//! names no consumer of it: an earlier version of this doc named a type in
//! another repository that never existed there (measured 2026-09-15). New local
//! clients belong on the node contract of ADR-0040/0041 (both Proposed), not on
//! these routes. It complements
//! the CRI (`delonix-cri`, which serves the kubelet): this serves the product's
//! *management* (volumes/containers/…).
//!
//! Exposed surfaces: **volumes** (CRUD), **containers** (list/get + run/rm/
//! action/logs/exec + partial reconfig), **images** (list/rmi/pull/build/scan/
//! sbom), **networks** (create/rm) and **VMs** (only stop/rm — divergent subsystem).
//! The READ contract is each resource's own serde type (`Volume`,
//! `Container`, `Image`, `Package`); the MUTATIONS return `{ok, output}`.

pub mod dashstats;

pub use dashstats::VmView;

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use delonix_host::peer_cred::peer_uid;
use delonix_model::dash::DashSummary;
use delonix_model::Error;
use delonix_telemetry::workload::WorkloadSeries;

/// Shared state of the handlers.
#[derive(Clone)]
struct AppState {
    /// The root of the runtime state (`$DELONIX_ROOT`).
    base: PathBuf,
    /// The runtime CLI binary (`delonix`) for the MUTATIONS. Unlike the
    /// reads (library calls to the Store), a container mutation
    /// (rm/stop/start/…) must reuse the engine's REAL path — kill the process,
    /// clean up cgroups/namespaces, unpublish ports, disconnect networks — which
    /// lives in the CLI. Calling the CLI itself guarantees full parity, rather than
    /// reimplementing that cleanup here. The shell-out is debt, not design: it
    /// exists because that logic lives in a crate with no `[lib]` target, and
    /// ADR-0041 (Proposed) plans moving these mutations onto library calls.
    bin: PathBuf,
    /// The composed engine's use cases this API serves, as a port (ADR-0040 A5,
    /// R3f). The bin mounts a `delonix-engine::Engine` and hands in an
    /// implementor, so the image/volume/container/network/summary endpoints
    /// resolve through the composition instead of this interface naming the
    /// `delonix-oci`/`delonix-volume`/`delonix-state`/`delonix-sdn`/`delonix-linux`
    /// adapters directly (the edge the arch gate ratchets down). VMs reach the
    /// engine through this same port for the summary; the standalone `VmView` is
    /// now only the reexport `delonix-mcp` reuses, no longer held here.
    engine: Arc<dyn EngineOps>,
}

/// The composed engine's use cases the management API serves, beyond VMs (R3f).
/// Grows one group of methods per adapter edge lifted off this interface onto the
/// `delonix-engine` assembly. Reads return a domain value where one exists below
/// the interface layer, or a pretty-printable `serde_json::Value` where the only
/// type is the adapter's own (the handler serializes it either way); mutations
/// return what the endpoint echoes. Every result carries the shared
/// [`Error`](delonix_model::Error) the HTTP layer already maps.
pub trait EngineOps: Send + Sync {
    // --- images (delonix-oci / delonix-scanner) ---
    /// Every local image, serialized (`Image` is the adapter's type).
    fn list_images(&self) -> Result<serde_json::Value, Error>;
    /// Remove an image by tag or id; returns the store's `untagged:`/`deleted:` line.
    fn remove_image(&self, reference: &str) -> Result<String, Error>;
    /// The SBOM of an image, serialized (`Package` is the scanner's type).
    fn image_sbom(&self, reference: &str) -> Result<serde_json::Value, Error>;

    // --- resource summary (delonix-state/linux/sdn/volume/oci, R3f-B) ---
    /// A fresh [`DashSummary`] — engine-wide counts, cgroup memory, and
    /// optionally per-container network totals and the per-area disk walk.
    fn dash_summary(&self, include_network: bool, include_storage: bool) -> DashSummary;
    /// Per-container cumulative rx/tx bytes, keyed by id, for the slow refresh.
    fn container_net(&self) -> std::collections::HashMap<String, (u64, u64)>;
    /// Per-workload contention series for the Prometheus workload exporter
    /// (per-container network is left `None` — the caller overlays its cache).
    fn workload_series(&self) -> Vec<WorkloadSeries>;

    // --- volumes (delonix-volume, R3f-C) ---
    /// Every named volume, serialized (`Volume` is the adapter's type).
    fn list_volumes(&self) -> Result<serde_json::Value, Error>;
    /// One named volume by name, serialized.
    fn get_volume(&self, name: &str) -> Result<serde_json::Value, Error>;
    /// Create a named volume; returns the created record, serialized.
    fn create_volume(
        &self,
        name: &str,
        driver: &str,
        device: Option<String>,
        options: Option<String>,
    ) -> Result<serde_json::Value, Error>;
    /// Remove a named volume.
    fn remove_volume(&self, name: &str) -> Result<(), Error>;

    // --- containers (delonix-state, R3f-D) ---
    /// Every container record (`Container` is a context type the interface names).
    fn list_containers(&self) -> Result<Vec<delonix_compute::Container>, Error>;
    /// One container record by id or name.
    fn get_container(&self, id: &str) -> Result<delonix_compute::Container, Error>;
    /// Record a published-port spec on the container holding `ip` (best-effort).
    fn record_published_port(&self, ip: &str, spec: &str) -> Result<(), Error>;
    /// Drop a publication from whichever record claims it.
    fn forget_published_port(&self, ip: Option<&str>, spec_or_port: &str) -> Result<(), Error>;

    // --- network (delonix-sdn, R3f-E) ---
    /// The IPv4 a MAC was leased on a network, or `None`.
    fn dhcp_ip(&self, net: &str, mac: &str) -> Option<String>;
    /// The IPv6 equivalent.
    fn dhcp_ip6(&self, net: &str, mac: &str) -> Option<String>;
    /// The IP a container has on the default network.
    fn container_ip(&self, id: &str) -> String;
    /// Every declared network, serialized (`NetDef` is the adapter's type).
    fn network_list(&self) -> serde_json::Value;
    /// One declared network by name, serialized, or `None`.
    fn network_get(&self, name: &str) -> Option<serde_json::Value>;
    /// The realized SDN infra status, serialized.
    fn network_status(&self) -> serde_json::Value;
    /// Whether `ip` is a valid container IP (the publish-port input guard).
    fn valid_container_ip(&self, ip: &str) -> bool;
    /// Rate-limit a running container's bandwidth.
    fn set_net_rate(&self, id: &str, rate_bit: u64, burst_bytes: u64) -> Result<(), Error>;
    /// Publish a container port through the ingress.
    fn publish_port(&self, container_ip: &str, spec: &str) -> Result<(), Error>;
    /// Withdraw a published host port (best-effort).
    fn unpublish_port(&self, host_port: &str);
}

/// Starts the management API listening on a unix socket (blocking). `addr` accepts
/// a path or `unix:///path`. Same pattern as `delonix-cri::serve_blocking`.
pub fn serve_blocking(base: PathBuf, engine: Arc<dyn EngineOps>, addr: &str) -> Result<(), Error> {
    // The CLI the mutations run: never this process's own executable, which is the
    // API server (`delonix serve api` runs `delonix-mgmt`) and would serve again
    // instead of running the command.
    let bin = delonix_host::dispatch::cli_bin();
    serve_blocking_with(base, bin, engine, addr)
}

/// Like [`serve_blocking`], but with the CLI binary explicit (for tests).
pub fn serve_blocking_with(
    base: PathBuf,
    bin: PathBuf,
    engine: Arc<dyn EngineOps>,
    addr: &str,
) -> Result<(), Error> {
    let path = addr.strip_prefix("unix://").unwrap_or(addr).to_string();
    delonix_node::alloc_tuning::limit_malloc_arenas();
    let rt = tokio::runtime::Builder::new_multi_thread()
        // A local control-plane API on a unix socket — see the CRI server for
        // the reasoning. The expensive endpoint here (`/v1/dash`) is a disk
        // walk, which is I/O-bound too.
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| Error::Runtime {
            context: "tokio",
            message: e.to_string(),
        })?;
    rt.block_on(async move {
        let _ = std::fs::remove_file(&path); // clean up an old socket
        let uds = tokio::net::UnixListener::bind(&path).map_err(|e| Error::Runtime {
            context: "bind",
            message: e.to_string(),
        })?;
        // SECURITY: this is the highest-privilege surface in the runtime — every
        // route (including `/v1/containers/:id/exec`, arbitrary code execution
        // inside any container) was reachable by ANY local process, gated only by
        // the ambient umask at bind time. Mirror the holder's control socket
        // (`delonix-sdn::infra::holder_main`): 0600 file mode + `SO_PEERCRED` on
        // every accepted connection, checked in `serve_over_uds` below.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        eprintln!("delonix-mgmt (management API) listening on unix://{path}");
        tokio::spawn(spawn_expensive_metrics_refresh(
            base.clone(),
            Arc::clone(&engine),
        ));

        let state = AppState { base, bin, engine };

        // Prometheus `/metrics` over TCP (OPTIONAL): a dedicated HTTP listener
        // enabled by `DELONIX_MGMT_METRICS_ADDR` (e.g. `0.0.0.0:9100`). The rich
        // `/metrics` of this crate — the one that recomputes the resource gauges
        // (containers/vms/memory/network/storage) and the per-workload series on
        // every scrape — otherwise lives ONLY on the management unix socket, which
        // Prometheus cannot scrape without a proxy; and the TCP `/metrics` of
        // `delonix-cri` carries only the CRI event counters (its resource gauges
        // read zero, nothing recomputes them in that process). This listener
        // closes that gap: a single TCP endpoint with the WHOLE registry, gauges
        // and all. It serves EXCLUSIVELY the `/metrics` route — never the
        // management routes (exec/run/rm are arbitrary code execution and stay on
        // the euid-checked unix socket). Off by default → no cost, no exposure.
        if let Some(maddr) = std::env::var_os("DELONIX_MGMT_METRICS_ADDR") {
            let maddr = maddr.to_string_lossy().into_owned();
            let metrics_state = state.clone();
            tokio::spawn(async move {
                match tokio::net::TcpListener::bind(&maddr).await {
                    Ok(l) => {
                        eprintln!("delonix-mgmt: /metrics (Prometheus, full registry) at http://{maddr}/metrics");
                        let _ = axum::serve(l, metrics_only_router(metrics_state)).await;
                    }
                    Err(e) => {
                        eprintln!("delonix-mgmt: /metrics (TCP) bind failed on {maddr}: {e}");
                    }
                }
            });
        }

        serve_over_uds(uds, router(state)).await
    })
}

/// Periodically re-publishes the EXPENSIVE `dashstats` gauges (per-container
/// network reads + the full disk-usage walk) in the background, decoupled
/// from any single `/metrics` request. MEASURED on a real host (49
/// containers, several full `kindest/node` rootfs copies): the storage walk
/// alone took over a minute — computing it inline inside a request handler
/// would blow past Prometheus's default 10s scrape timeout on every scrape.
/// The cheap fields (container/VM counts, cgroup memory) are still collected
/// fresh on every `/metrics` request (see `metrics()`) — only this slow half
/// is decoupled. Runs for the lifetime of the process; best-effort (a
/// collection panic/slow disk on one tick never stops the next one).
async fn spawn_expensive_metrics_refresh(base: PathBuf, engine: Arc<dyn EngineOps>) {
    let _ = base; // the engine port already carries the state root
                  // BUG FOUND (code review): this used to `.await` the collection with no
                  // ceiling at all — a genuinely stuck disk/netns operation (not just "slow",
                  // an actual hang) would freeze this loop, and every expensive gauge, for the
                  // remaining lifetime of the process. Bounded with `collect_with_timeout`; see
                  // its own doc comment for why the worker thread is leaked rather than
                  // cancelled on a timeout.
    const COLLECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);
    loop {
        // Per-container network bytes for the per-workload series
        // (`workload_network_{receive,transmit}_bytes_total`). Its OWN bounded
        // read (own latch in `dashstats`), so a wedged netns here never blocks
        // the `DashSummary` collection below, and it is kept OFF the inline
        // scrape path (each read is an `nsenter` per container). Served stale
        // from the cache between cycles — never read on a scrape.
        {
            let engine = Arc::clone(&engine);
            let net = tokio::task::spawn_blocking(move || {
                dashstats::collect_container_net_with_timeout(engine, COLLECT_TIMEOUT)
            })
            .await;
            if let Ok(dashstats::Bounded::Done(map)) = net {
                dashstats::publish_container_net_cache(map);
            }
        }
        let engine = Arc::clone(&engine);
        let summary = tokio::task::spawn_blocking(move || {
            dashstats::collect_with_timeout(engine, true, true, COLLECT_TIMEOUT)
        })
        .await;
        match summary {
            Ok(dashstats::Bounded::Done(summary)) => dashstats::publish_to_metrics(&summary),
            Ok(dashstats::Bounded::TimedOut) => eprintln!(
                "delonix-mgmt: expensive metrics collection did not finish within {}s — \
                 network/storage gauges stay at their last known value this cycle",
                COLLECT_TIMEOUT.as_secs()
            ),
            // Distinct from a timeout, and the distinction is the whole point:
            // nothing was attempted this cycle because the PREVIOUS collection
            // is still wedged in the same underlying operation (a hung network
            // volume, a stuck netns read). Saying "did not finish within 120s"
            // here would be a lie — we never started. See `run_bounded`.
            Ok(dashstats::Bounded::Skipped) => eprintln!(
                "delonix-mgmt: skipping expensive metrics collection — a previous one is \
                 still stuck (hung volume or netns read?); not starting another until it \
                 returns. Network/storage gauges stay at their last known value."
            ),
            Err(_) => {} // the spawn_blocking task itself panicked — already logged by tokio.
        }
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

/// Serves an axum `Router` over a `UnixListener` (`axum::serve` only accepts TCP;
/// here we use the accept loop + hyper-util, the pattern from axum's unix example).
async fn serve_over_uds(uds: tokio::net::UnixListener, app: Router) -> Result<(), Error> {
    use hyper::body::Incoming;
    use hyper_util::rt::{TokioExecutor, TokioIo};
    use tower::Service;
    // SAFETY: geteuid() has no preconditions.
    let own_uid = unsafe { libc::geteuid() };
    let mut make = app.into_make_service();
    loop {
        // BUG FIXED: `?` used to propagate ANY accept() error out of this loop,
        // tearing down the whole management API process — including on
        // EMFILE/ENFILE/ECONNABORTED, all transient and self-clearing conditions
        // `accept(2)` explicitly documents as "retry", not "give up". A brief fd
        // exhaustion elsewhere on the host used to kill every in-flight request
        // this server was handling. Now it's logged and the loop keeps accepting.
        let (socket, _) = match uds.accept().await {
            Ok(x) => x,
            Err(e) => {
                eprintln!("delonix-mgmt: accept() error (transient, retrying): {e}");
                continue;
            }
        };
        // Reject any peer that isn't this process's own euid BEFORE it ever reaches
        // the router — a mismatched umask/`/run` placement must not turn into a
        // full control-plane RCE for any local user.
        if peer_uid(&socket) != Some(own_uid) {
            continue;
        }
        // `into_make_service` is infallible → the connection service never fails here.
        let tower_service = match make.call(&socket).await {
            Ok(svc) => svc,
            Err(never) => match never {},
        };
        tokio::spawn(async move {
            let io = TokioIo::new(socket);
            let hyper_service = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                tower_service.clone().call(req)
            });
            let _ = hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
                .serve_connection_with_upgrades(io, hyper_service)
                .await;
        });
    }
}

/// A router that serves ONLY `GET /metrics`, for the optional TCP listener
/// gated by `DELONIX_MGMT_METRICS_ADDR`. Deliberately NOT [`router`]: the TCP
/// surface must carry the Prometheus endpoint and nothing else — the management
/// routes (exec/run/rm are arbitrary code execution) stay on the euid-checked
/// unix socket. Same handler as the socket's `/metrics`, so the registry and the
/// per-scrape recompute are identical on both surfaces.
fn metrics_only_router(state: AppState) -> Router {
    Router::new()
        .route("/metrics", get(metrics))
        .with_state(state)
}

/// The API routes. Exposed for tests (no socket, via `oneshot`).
fn router(state: AppState) -> Router {
    Router::new()
        .route("/_ping", get(ping))
        // `GET /metrics` — the SAME shared Prometheus registry that `delonix-cri`
        // exposes (defined in `delonix-telemetry::metrics`), refreshed with a
        // fresh `dashstats::collect` snapshot on every scrape (see `metrics()`).
        // The Grafana-native path: point a Prometheus server at this endpoint.
        .route("/metrics", get(metrics))
        // `GET /v1/dash` — the SAME snapshot as JSON, for anything that isn't
        // Prometheus (a custom SRE tool, `curl`, a JSON-datasource panel).
        .route("/v1/dash", get(dash_summary))
        .route("/v1/volumes", get(list_volumes).post(create_volume))
        .route("/v1/volumes/:name", get(get_volume).delete(delete_volume))
        // Containers: read (list/get) via library; mutation (below) via CLI.
        // `POST` = `run` (receives the spec in JSON and rebuilds the CLI args).
        .route("/v1/containers", get(list_containers).post(run_container))
        .route(
            "/v1/containers/:id",
            get(get_container).delete(delete_container),
        )
        // Container mutation: rm/start/stop/restart/pause/unpause — shell-out to the
        // runtime CLI (full cleanup parity), not a call to the Store.
        .route("/v1/containers/:id/action", post(container_action_ep))
        // Logs (request/response, not streaming) + non-interactive exec.
        .route("/v1/containers/:id/logs", get(container_logs_ep))
        .route("/v1/containers/:id/exec", post(container_exec_ep))
        // Images: list + rmi. The reference (`nginx:latest`, `library/nginx`,
        // `sha256:…`) does NOT fit in a path segment (it has `/` and `:`) → it goes
        // by query (`?ref=…`). No traversal risk: `ImageStore::remove`
        // resolves by linear scan (compares tags/id prefix) and the file
        // it deletes uses the sanitized `id`, never the raw `ref`.
        .route("/v1/images", get(list_images).delete(delete_image))
        // Pull (optionally with a CVE scan afterwards) — shell-out to the CLI.
        .route("/v1/images/pull", post(pull_image))
        // Build from a pasted Delonixfile (materializes + `delonix build`).
        .route("/v1/images/build", post(build_image))
        // CVE scan (text, via CLI) + SBOM (structured, via library).
        .route("/v1/images/scan", get(scan_image))
        .route("/v1/images/sbom", get(sbom_image))
        // Networks: create/rm (network lifecycle) — shell-out to the CLI. publish/
        // unpublish (DNAT) are separate routes below, over the library.
        .route("/v1/networks", get(list_networks).post(create_network))
        .route(
            "/v1/networks/:name",
            get(get_network).delete(delete_network),
        )
        .route("/v1/net/status", get(net_status))
        // Port publication (DNAT + the slirp's hostfwd). Unlike network
        // create/rm, this does NOT go through the binary: `publish_port` takes
        // the container's IP, while the CLI takes the NAME — a caller of this
        // API has the IP and no way to resolve the name on this side without a
        // second round-trip. Calling the library is the short path and the honest one.
        .route("/v1/net/publish", post(publish_port))
        .route(
            "/v1/net/publish/:host_port",
            axum::routing::delete(unpublish_port),
        )
        // Per-workload firewall, egress policy and network attach: REFUSED
        // (`net_route_withdrawn`) — they wrote the dataplane without the
        // container's record. Kept registered so they answer 501 with the code, not 404.
        .route(
            "/v1/net/firewall/:ip",
            put(net_route_withdrawn).delete(net_route_withdrawn),
        )
        .route("/v1/net/egress", put(net_route_withdrawn))
        .route("/v1/net/egress/:bridge", put(net_route_withdrawn))
        .route("/v1/containers/:id/rate", put(set_net_rate))
        // Addresses and network attach: the questions a client asks before
        // publishing a port or writing a rule.
        .route("/v1/net/dhcp/:net/:mac", get(dhcp_ip))
        .route("/v1/net/dhcp6/:net/:mac", get(dhcp_ip6))
        .route("/v1/net/container-ip/:id", get(container_ip))
        .route("/v1/net/attach-extra", post(net_route_withdrawn))
        .route(
            "/v1/net/attach-extra/:id/:idx/:ip",
            axum::routing::delete(net_route_withdrawn),
        )
        .route(
            "/v1/net/attach/:id/:ip",
            axum::routing::delete(net_route_withdrawn),
        )
        // Hot reconfig of a container: ONLY the subset that the runtime's `container
        // update` supports (publish-add/publish-rm). Any other field is refused here
        // (`ReconfigBody` denies unknown fields) — never silently ignored.
        .route("/v1/containers/:id/reconfig", post(reconfig_container))
        // VMs (delonix-vm subsystem): ONLY stop/rm. `vm create`/`vm start` exist
        // (`delonix_vm::create`/`start`) but are not routed here; ADR-0041 (Proposed)
        // proposes adding them to the node contract instead of to these routes.
        .route("/v1/vms/:name/action", post(vm_action_ep))
        .with_state(state)
}

async fn ping() -> &'static str {
    "delonix-mgmt ok"
}

/// `GET /metrics` — OpenMetrics body of the SHARED Prometheus registry in
/// `delonix-telemetry` (the same one `delonix-cri` serves). Refreshes only
/// the CHEAP gauges (container/VM counts, cgroup memory) inline on every
/// request — Prometheus's default scrape timeout (10s) does not leave room
/// for the per-container netns reads, let alone the disk-usage walk (measured
/// over a minute on a host with heavy containers). Those two are published by
/// `spawn_expensive_metrics_refresh`'s background loop instead — the gauges it
/// owns are stale by up to its refresh interval, never absent.
///
/// The per-workload contention series (CPU throttling, pressure, OOM) are also
/// refreshed inline: a few small cgroup file reads per running workload, and the
/// whole set is replaced so a stopped workload stops being exported. The
/// per-workload NETWORK series (`workload_network_{receive,transmit}_bytes`) are
/// the exception: they need a netns enter per container, too costly inline, so
/// they are attached from the cache the background refresh fills — stale by up
/// to its interval, never read on the scrape itself.
async fn metrics(State(s): State<AppState>) -> impl IntoResponse {
    let engine = Arc::clone(&s.engine);
    let collected = tokio::task::spawn_blocking(move || {
        (
            engine.dash_summary(false, false),
            dashstats::workload_series_with_net_cache(&*engine),
        )
    })
    .await;
    if let Ok((summary, workloads)) = collected {
        dashstats::publish_to_metrics(&summary);
        dashstats::publish_workloads_to_metrics(workloads);
    }
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        delonix_telemetry::metrics::encode(),
    )
}

/// `GET /v1/dash` — the same [`dashstats::DashSummary`] as JSON, WITH the
/// expensive fields (unlike `/metrics`, which stays fast for Prometheus's
/// scrape timeout) — this is an on-demand call, not a periodic scrape, so it
/// can afford the full network+storage collection. Can take well over a
/// minute on a host with many/large containers (see `dashstats::collect`'s
/// doc comment) — a client with a request timeout shorter than that should
/// use `/metrics` instead, which is always fast.
async fn dash_summary(State(s): State<AppState>) -> Response {
    let engine = Arc::clone(&s.engine);
    match tokio::task::spawn_blocking(move || engine.dash_summary(true, true)).await {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// Safe volume name at the API BOUNDARY (defense against path traversal). It is
/// deliberately STRICTER than the `VolumeStore`: that one accepts `..` (only `.`
/// characters) and `inspect`/`remove` don't even validate the name — a `remove("..")`
/// coming from the URL path would delete the parent directory. Here any lone
/// `..`/`/`/`.` is rejected.
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains("..")
        && !name.contains('/')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// Standard 400 error for an invalid volume name.
fn invalid_name() -> Response {
    err_response(Error::Invalid("invalid volume name".to_string()))
}

/// Maps an engine `Error` to (HTTP code, JSON body) — the client
/// reconstructs its own `RuntimeError` from the code + message.
///
/// The body carries `code` (the stable `DX_*` identity, see
/// [`Error::code`]) beside the message. **Additive on purpose**: the field was
/// added, `error` was not touched. A client outside this repo already reads
/// `.error`, and the discipline ADR-0005 fixed for `-o json` — fields may be
/// ADDED, never removed nor retyped — is the same one that applies here. The
/// nested `{"error": {"code", "message"}}` envelope the CLI restructuring
/// specifies is a DIFFERENT surface (the CLI's own `-o json`), and building it
/// there costs nothing here.
///
/// Why the message alone was not enough: it is the half that gets translated,
/// so a client that classifies by matching text works on the machine it was
/// written on and stops classifying on a node with another locale. That is the
/// same reason the exit codes exist.
fn err_response(e: Error) -> Response {
    // Read BEFORE the match: it destructures `e` by value.
    let dx = e.code();
    let (code, msg) = match e.into_root() {
        Error::NotFound(m) => (StatusCode::NOT_FOUND, m),
        Error::Invalid(m) => (StatusCode::BAD_REQUEST, m),
        Error::Conflict(m) => (StatusCode::CONFLICT, m),
        // The same two classes the exit codes publish, in the transport that
        // already has words for them. Without these they fell into the catch-all
        // 500, which tells a caller «this server is broken» when what happened
        // is «this host lacks a tool» or «it is still coming up».
        Error::Unavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
        Error::Timeout(m) => (StatusCode::GATEWAY_TIMEOUT, m),
        other => (StatusCode::INTERNAL_SERVER_ERROR, other.to_string()),
    };
    (code, Json(serde_json::json!({ "error": msg, "code": dx }))).into_response()
}

/// Runs a synchronous engine-port call on a blocking thread (the engine's reads
/// and store mutations are synchronous; they must not block the async executor),
/// mapping a join failure to the shared error. The port pattern (R3f) replacing
/// the per-adapter `with_*_store` helpers below as each edge moves to the engine.
async fn blocking<T, F>(f: F) -> Result<T, Error>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, Error> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Error::Runtime {
            context: "join",
            message: e.to_string(),
        })?
}

async fn list_volumes(State(s): State<AppState>) -> Response {
    let engine = s.engine.clone();
    match blocking(move || engine.list_volumes()).await {
        Ok(vols) => Json(vols).into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_volume(State(s): State<AppState>, Path(name): Path<String>) -> Response {
    if !valid_name(&name) {
        return invalid_name();
    }
    let engine = s.engine.clone();
    match blocking(move || engine.get_volume(&name)).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/volumes`.
#[derive(serde::Deserialize)]
struct CreateVolumeBody {
    name: String,
    #[serde(default)]
    driver: Option<String>,
    #[serde(default)]
    device: Option<String>,
    #[serde(default)]
    options: Option<String>,
}

async fn create_volume(State(s): State<AppState>, Json(b): Json<CreateVolumeBody>) -> Response {
    if !valid_name(&b.name) {
        return invalid_name();
    }
    let driver = b.driver.unwrap_or_else(|| "local".to_string());
    let engine = s.engine.clone();
    match blocking(move || engine.create_volume(&b.name, &driver, b.device, b.options)).await {
        Ok(v) => (StatusCode::CREATED, Json(v)).into_response(),
        Err(e) => err_response(e),
    }
}

async fn delete_volume(State(s): State<AppState>, Path(name): Path<String>) -> Response {
    if !valid_name(&name) {
        return invalid_name();
    }
    let engine = s.engine.clone();
    match blocking(move || engine.remove_volume(&name)).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => err_response(e),
    }
}

// ---- Containers (read) -----------------------------------------------------

async fn list_containers(State(s): State<AppState>) -> Response {
    let engine = s.engine.clone();
    match blocking(move || engine.list_containers()).await {
        Ok(cs) => Json(cs).into_response(),
        Err(e) => err_response(e),
    }
}

async fn get_container(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    // Same boundary defense as volumes: `Store::load` does `root.join(id)`
    // before falling into the scan by name/prefix — a `..` in the path would escape.
    if !valid_name(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    let engine = s.engine.clone();
    match blocking(move || engine.get_container(&id)).await {
        Ok(c) => Json(c).into_response(),
        Err(e) => err_response(e),
    }
}

/// Safe argument to pass to the CLI: besides `valid_name` (no `..`/`/`), it refuses
/// a leading `-` — otherwise the CLI's `clap` would interpret the id as a flag (e.g. an
/// id `--rm`). The CLI args do not suffer shell injection (`Command::args`, not a
/// string), but they can be read as options — hence the barrier against `-`.
///
/// Public so the OTHER surface that spawns the same CLI with a caller-supplied
/// name — `delonix-mcp` (`logs.query`, `container.restart`) — validates with the
/// exact same rule instead of a second copy that can drift (G-017).
pub fn valid_arg(s: &str) -> bool {
    valid_name(s) && !s.starts_with('-')
}

/// Runs the runtime CLI (`delonix …`) with `DELONIX_ROOT` at the base, and returns
/// `(success, combined output)`. Blocking → runs in `spawn_blocking`.
async fn run_cli(bin: PathBuf, base: PathBuf, args: Vec<String>) -> Result<(bool, String), Error> {
    tokio::task::spawn_blocking(move || {
        let out = std::process::Command::new(&bin)
            .env("DELONIX_ROOT", &base)
            .args(&args)
            .output()
            .map_err(|e| Error::Runtime {
                context: "cli",
                message: e.to_string(),
            })?;
        let mut s = String::from_utf8_lossy(&out.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&out.stderr));
        Ok((out.status.success(), s.trim().to_string()))
    })
    .await
    .map_err(|e| Error::Runtime {
        context: "join",
        message: e.to_string(),
    })?
}

/// Query of `DELETE /v1/containers/:id?force=<bool>`.
#[derive(serde::Deserialize)]
struct ForceQuery {
    #[serde(default)]
    force: bool,
}

async fn delete_container(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<ForceQuery>,
) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    let mut args = vec!["container".to_string(), "rm".to_string()];
    if q.force {
        args.push("-f".to_string());
    }
    args.push(id);
    match run_cli(s.bin, s.base, args).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/containers/:id/action`.
#[derive(serde::Deserialize)]
struct ActionBody {
    action: String,
}

async fn container_action_ep(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<ActionBody>,
) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    // Only known actions (allowlist) reach the CLI. `remove` = `rm -f`.
    let sub = match b.action.as_str() {
        "start" | "stop" | "restart" | "pause" | "unpause" => b.action.clone(),
        "remove" | "rm" => {
            match run_cli(
                s.bin,
                s.base,
                vec![
                    "container".to_string(),
                    "rm".to_string(),
                    "-f".to_string(),
                    id,
                ],
            )
            .await
            {
                Ok((ok, out)) => {
                    return Json(serde_json::json!({ "ok": ok, "output": out })).into_response()
                }
                Err(e) => return err_response(e),
            }
        }
        other => return err_response(Error::Invalid(format!("unknown action: {other}"))),
    };
    match run_cli(s.bin, s.base, vec!["container".to_string(), sub, id]).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn container_logs_ep(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    // `logs` request/response (not streaming); the output comes as-is, even if the
    // container does not exist (a client is expected to ignore the `ok` here).
    match run_cli(
        s.bin,
        s.base,
        vec!["container".to_string(), "logs".to_string(), id],
    )
    .await
    {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/containers/:id/exec`.
#[derive(serde::Deserialize)]
struct ExecBody {
    cmd: String,
}

async fn container_exec_ep(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<ExecBody>,
) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    // `exec <id> sh -c <cmd>`: the `cmd` is passed as ONE argument to `sh -c` INSIDE
    // the container — runs in the container, never in the host's shell (it is
    // `Command::args`, no shell of ours). Exec is, by nature, arbitrary exec in the container.
    let args = vec![
        "container".to_string(),
        "exec".to_string(),
        id,
        "sh".to_string(),
        "-c".to_string(),
        b.cmd,
    ];
    match run_cli(s.bin, s.base, args).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/containers` (run). The contract is the field names: a client
/// serializes this shape and the engine deserializes it.
///
/// `deny_unknown_fields`, like `ReconfigBody`: a client that sends `cpus`, `user` or a
/// misspelled `volume` gets a 422 naming the field, instead of a container that runs
/// WITHOUT what it asked for and an `ok: true` to go with it. A new field enters this
/// struct and `build_run_args` together, never the body alone.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RunSpecBody {
    image: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    ports: Vec<String>,
    #[serde(default)]
    env: Vec<String>,
    #[serde(default)]
    network: String,
    #[serde(default)]
    memory: String,
    #[serde(default)]
    restart: String,
    #[serde(default)]
    command: Vec<String>,
    #[serde(default)]
    volumes: Vec<String>,
    #[serde(default)]
    knows: Vec<String>,
    #[serde(default)]
    knows_none: bool,
}

/// Rebuilds the `delonix container run -d …` args from the spec — a PURE function
/// (testable without a kernel). The runtime binary spells the network flag `--net`;
/// a client coming from the Docker vocabulary would write `--network`.
fn build_run_args(spec: RunSpecBody) -> Vec<String> {
    let mut args: Vec<String> = vec!["container".into(), "run".into(), "-d".into()];
    if !spec.name.is_empty() {
        args.push("--name".into());
        args.push(spec.name);
    }
    if !spec.network.is_empty() && spec.network != "none" {
        // The runtime CLI uses `--net` (not `--network`). Form `--net=<v>` so the
        // value never escapes into a new token.
        args.push(format!("--net={}", spec.network));
    }
    for p in &spec.ports {
        if p.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ':' | '/'))
        {
            args.push("-p".into());
            args.push(p.clone());
        }
    }
    for e in spec.env {
        args.push("-e".into());
        args.push(e);
    }
    for v in spec.volumes {
        if !v.is_empty() && !v.contains("..") {
            args.push("-v".into());
            args.push(v);
        }
    }
    if spec.knows_none {
        args.push("--knows-none".into());
    } else {
        for k in spec.knows {
            if !k.is_empty() {
                args.push("--knows".into());
                args.push(k);
            }
        }
    }
    if !spec.memory.is_empty() {
        args.push("-m".into());
        args.push(spec.memory);
    }
    if !spec.restart.is_empty() {
        args.push("--restart".into());
        args.push(spec.restart);
    }
    args.push(spec.image);
    args.extend(spec.command);
    args
}

async fn run_container(State(s): State<AppState>, Json(spec): Json<RunSpecBody>) -> Response {
    // `image` is required and a value starting with `-` would be read by clap as
    // a flag (it is the final POSITIONAL argument) — refuse at the boundary. Same for
    // `name` (value of `--name`). The remaining fields either have their own charset
    // (ports) or are option values with no positional ambiguity.
    if spec.image.is_empty() || spec.image.starts_with('-') {
        return err_response(Error::Invalid("invalid image".to_string()));
    }
    if spec.name.starts_with('-') {
        return err_response(Error::Invalid("invalid name".to_string()));
    }
    match run_cli(s.bin, s.base, build_run_args(spec)).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

// ---- Images (list + rmi) ---------------------------------------------------

async fn list_images(State(s): State<AppState>) -> Response {
    let engine = s.engine.clone();
    match blocking(move || engine.list_images()).await {
        Ok(imgs) => Json(imgs).into_response(),
        Err(e) => err_response(e),
    }
}

/// Query of `DELETE /v1/images?ref=…`. `ref` is a reserved word in Rust.
#[derive(serde::Deserialize)]
struct RefQuery {
    #[serde(rename = "ref")]
    reference: String,
}

async fn delete_image(State(s): State<AppState>, Query(q): Query<RefQuery>) -> Response {
    if q.reference.is_empty() {
        return err_response(Error::Invalid("empty image reference".to_string()));
    }
    let engine = s.engine.clone();
    match blocking(move || engine.remove_image(&q.reference)).await {
        // `remove` returns "untagged: …" or "deleted: …" — return it as-is.
        Ok(result) => Json(serde_json::json!({ "result": result })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/images/pull`.
#[derive(serde::Deserialize)]
struct PullBody {
    #[serde(rename = "ref")]
    reference: String,
    /// Also runs a CVE scan after the pull (and appends the output).
    #[serde(default)]
    scan_after: bool,
}

async fn pull_image(State(s): State<AppState>, Json(b): Json<PullBody>) -> Response {
    // The reference is the POSITIONAL argument of `image pull` — a leading `-` would
    // be read as a flag. Refuse at the boundary. (Valid refs have `/`/`:`, never `-`
    // at the start.)
    if b.reference.is_empty() || b.reference.starts_with('-') {
        return err_response(Error::Invalid("invalid image reference".to_string()));
    }
    let (ok, mut out) = match run_cli(
        s.bin.clone(),
        s.base.clone(),
        vec!["image".into(), "pull".into(), b.reference.clone()],
    )
    .await
    {
        Ok(r) => r,
        Err(e) => return err_response(e),
    };
    // Optional auto-scan after the pull (only if the pull succeeded).
    if ok && b.scan_after {
        if let Ok((_so, sout)) = run_cli(
            s.bin,
            s.base,
            vec!["image".into(), "scan".into(), b.reference],
        )
        .await
        {
            out.push_str("\n--- scan (CVE) ---\n");
            out.push_str(&sout);
        }
    }
    Json(serde_json::json!({ "ok": ok, "output": out })).into_response()
}

/// Body of `POST /v1/images/build`.
#[derive(serde::Deserialize)]
struct BuildBody {
    /// Content of the Delonixfile (pasted; the `RUN`s run during the build).
    delonixfile: String,
    /// Tag of the resulting image (`repo:tag`).
    tag: String,
}

/// Per-process monotonic counter to name unique build work dirs.
static BUILD_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

async fn build_image(State(s): State<AppState>, Json(b): Json<BuildBody>) -> Response {
    // `-t <tag>`: a value starting with `-` would be read as a flag. Refuse at the boundary.
    if b.tag.is_empty() || b.tag.starts_with('-') {
        return err_response(Error::Invalid("invalid tag".to_string()));
    }
    if b.delonixfile.trim().is_empty() {
        return err_response(Error::Invalid("empty Delonixfile".to_string()));
    }
    // UNIQUE work dir per-build (the context where `COPY` resolves): `pid-seq` isolates
    // concurrent builds — without it, two parallel builds would share the
    // `Delonixfile`/context (TOCTOU: one builds the other's Delonixfile). Cleaned up at
    // the end. The name derives only from `s.base`+pid+counter — never from user input.
    let seq = BUILD_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = s
        .base
        .join("_mgmt_build")
        .join(format!("{}-{}", std::process::id(), seq));
    let file = dir.join("Delonixfile");
    let (dir_w, file_w, content) = (dir.clone(), file.clone(), b.delonixfile);
    let prep = tokio::task::spawn_blocking(move || {
        std::fs::create_dir_all(&dir_w)?;
        std::fs::write(&file_w, content)
    })
    .await;
    match prep {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            return err_response(Error::Runtime {
                context: "build-prep",
                message: e.to_string(),
            })
        }
        Err(e) => {
            return err_response(Error::Runtime {
                context: "join",
                message: e.to_string(),
            })
        }
    }
    let args = vec![
        "build".to_string(),
        "-t".to_string(),
        b.tag,
        "-f".to_string(),
        file.to_string_lossy().into_owned(),
        dir.to_string_lossy().into_owned(),
    ];
    let result = run_cli(s.bin, s.base, args).await;
    // Clean up the work dir (best-effort) — don't leave Delonixfiles/contexts piling up.
    let dir_c = dir.clone();
    let _ = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(&dir_c)).await;
    match result {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn scan_image(State(s): State<AppState>, Query(q): Query<RefQuery>) -> Response {
    // `image scan <ref>` (text). The ref is positional — a leading `-` would become a flag.
    if q.reference.is_empty() || q.reference.starts_with('-') {
        return err_response(Error::Invalid("invalid image reference".to_string()));
    }
    match run_cli(
        s.bin,
        s.base,
        vec!["image".into(), "scan".into(), q.reference],
    )
    .await
    {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn sbom_image(State(s): State<AppState>, Query(q): Query<RefQuery>) -> Response {
    if q.reference.is_empty() {
        return err_response(Error::Invalid("empty image reference".to_string()));
    }
    // SBOM is a LIBRARY call (it reads the layers from the CAS, runs nothing) — like
    // the volume/container reads. 404 if the image doesn't exist locally. The
    // engine resolves the image and extracts the SBOM; this interface no longer
    // names the image store or the scanner (R3f).
    let engine = s.engine.clone();
    let out = blocking(move || engine.image_sbom(&q.reference)).await;
    match out {
        Ok(pkgs) => Json(pkgs).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/networks`.
#[derive(serde::Deserialize)]
struct NetworkBody {
    name: String,
}

/// `GET /v1/net/dhcp/:net/:mac` — the IP DHCP gives a MAC on this network.
///
/// Deterministic from the MAC (`<prefix>.254.<10 + fnv32(mac)%240>`), not read
/// from the ARP table: `ip neigh` only shows the address after recent traffic,
/// and returned "none" for a live but quiet VM — a seriously reported case.
/// Here the IP exists as soon as the network exists, which is what you need to
/// open an SSH session.
///
/// `None` → 404: the network does not exist. That is not the same as "the VM
/// has not booted yet", so we do not answer 200 with an empty body.
async fn dhcp_ip(State(s): State<AppState>, Path((net, mac)): Path<(String, String)>) -> Response {
    if !valid_arg(&net) || !valid_mac(&mac) {
        return err_response(Error::Invalid("invalid network or MAC".to_string()));
    }
    let engine = s.engine.clone();
    match tokio::task::spawn_blocking(move || engine.dhcp_ip(&net, &mac)).await {
        Ok(Some(ip)) => Json(serde_json::json!({ "ip": ip })).into_response(),
        Ok(None) => err_response(Error::NotFound("network".to_string())),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// `GET /v1/net/dhcp6/:net/:mac` — the IPv6 equivalent.
///
/// Its own route rather than a field on the one above: v6 is OFF by a security
/// decision in this engine, and whoever wants it has to ask for it. An optional
/// field on a shared response would make it look like it comes for free.
async fn dhcp_ip6(State(s): State<AppState>, Path((net, mac)): Path<(String, String)>) -> Response {
    if !valid_arg(&net) || !valid_mac(&mac) {
        return err_response(Error::Invalid("invalid network or MAC".to_string()));
    }
    let engine = s.engine.clone();
    match tokio::task::spawn_blocking(move || engine.dhcp_ip6(&net, &mac)).await {
        Ok(Some(ip)) => Json(serde_json::json!({ "ip": ip })).into_response(),
        Ok(None) => err_response(Error::NotFound("ipv6 lease".to_string())),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// `GET /v1/net/container-ip/:id` — the IP a container has on the default network.
///
/// Derived from the id, touching no state: it is the question a client asks
/// before publishing a port or writing a rule, and asking it over HTTP saves it
/// from reimplementing the formula — which would have to yield exactly the same
/// result as this one, always.
async fn container_ip(State(s): State<AppState>, Path(id): Path<String>) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    let engine = s.engine.clone();
    match tokio::task::spawn_blocking(move || engine.container_ip(&id)).await {
        Ok(ip) => Json(serde_json::json!({ "ip": ip })).into_response(),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// The network MUTATIONS this API used to serve, withdrawn (P0-6 of the NaaS
/// audit, `docs/discovery/62_NAAS_FASE0_AUDITORIA.md`; decided by the owner).
///
/// `PUT|DELETE /v1/net/firewall/:ip`, `PUT /v1/net/egress[/:bridge]`,
/// `POST|DELETE /v1/net/attach-extra…` and `DELETE /v1/net/attach/:id/:ip` wrote
/// nftables and the holder's netns straight from an IP and an id, WITHOUT the
/// container's record: the next reapply from the record (a `start`, a respawn, an
/// `ingress` edit) silently undid them, and a firewall written on the primary IP
/// left a multi-homed container's other addresses open. They were a second writer
/// of state the record owns.
///
/// Refused rather than widened: this surface is frozen (ADR-0041 D4 — bug and
/// security fixes only), and making each route load and save the record would be
/// the translation layer ADR-0040 removes. `501` with `DX-6302`, so a client learns
/// the route is gone and where to go (the CLI, which goes through the record)
/// instead of reading a `200` for a rule that will not survive.
async fn net_route_withdrawn() -> Response {
    let e = Error::coded(
        6302,
        Error::Unavailable(
            "this management API route no longer changes the network: it wrote the dataplane \
             without the container's record. Use the CLI (`delonix net ingress`/`net egress`, \
             `delonix network connect`/`disconnect`)"
                .to_string(),
        ),
    );
    let body = serde_json::json!({
        "error": e.to_string(),
        "code": e.code(),
        "number": delonix_model::codes::label(e.number()),
    });
    (StatusCode::NOT_IMPLEMENTED, Json(body)).into_response()
}

/// A MAC is `aa:bb:cc:dd:ee:ff` — hexadecimal and colons, nothing else.
///
/// Validated separately from `valid_arg` because the former ACCEPTS colons and
/// the latter does not; without this, a legitimate MAC would be refused and the
/// alternative (loosening `valid_arg`) would widen the filter on every other argument.
fn valid_mac(mac: &str) -> bool {
    !mac.is_empty()
        && mac.len() <= 17
        && mac
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == ':' || c == '-')
}

/// Body of `PUT /v1/containers/:id/rate`.
#[derive(serde::Deserialize)]
struct RateBody {
    /// Rate in bits/second.
    rate_bit: u64,
    /// TBF bucket, in bytes.
    burst_bytes: u64,
}

/// `PUT /v1/containers/:id/rate` — rate-limits a running container's bandwidth.
///
/// Under `/v1/containers` and not under `/v1/net`: whoever reads it is changing a
/// property OF THAT workload, and that is where they look for it. The mechanism
/// is a network one; the resource is not.
async fn set_net_rate(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<RateBody>,
) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    let engine = s.engine.clone();
    match blocking(move || engine.set_net_rate(&id, b.rate_bit, b.burst_bytes)).await {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/net/publish`.
#[derive(serde::Deserialize)]
struct PublishBody {
    /// IP of the container that receives the traffic.
    container_ip: String,
    /// `hostPort:contPort[/tcp|udp]` — the same form as the CLI's `-p`.
    spec: String,
}

/// `POST /v1/net/publish` — publishes a port through the ingress.
///
/// Exists so a local client can publish a port without linking the engine's
/// crates: it is the same `delonix-sdn` operation, behind HTTP.
///
/// `container_ip` is validated before it reaches the mechanism: it is what ends
/// up in a DNAT rule, and an arbitrary string there is an arbitrary rule.
async fn publish_port(State(s): State<AppState>, Json(b): Json<PublishBody>) -> Response {
    if !s.engine.valid_container_ip(&b.container_ip) {
        return err_response(Error::Invalid(format!(
            "invalid container IP: '{}'",
            b.container_ip
        )));
    }
    if !valid_arg(&b.spec) {
        return err_response(Error::Invalid("invalid publish spec".to_string()));
    }
    // BUG FIXED HERE: this route used to call `infra::publish_port` and STOP —
    // it wrote the actual state and skipped the desired one. A port published
    // this way lived only in the slirp's `hostfwd` table and in the nft rules
    // inside the infra netns, both of which die with the ingress. The ONLY
    // durable copy of a publication is the container record's `ports`, and it is
    // exclusively from there that `cmd_start` and `reconcile_after_respawn`
    // replay them — so a port published here worked until the next ingress
    // restart and then vanished with nothing left to rebuild it from.
    //
    // Measured on a live host: 18 container records all with `ports: []` while
    // 127.0.0.1:8077 and :8079 were serving HTTP — publications no record knew
    // about. The engine could not have restored them; it did not know they were
    // wanted.
    //
    // Persist FIRST, publish second. The other order loses the record when the
    // process dies between the two, and an unpublished-but-recorded port is the
    // recoverable failure (the next start republishes it) while a
    // published-but-unrecorded one is exactly the bug being fixed.
    let ip = b.container_ip.clone();
    let spec = b.spec.clone();
    if let Err(e) = record_published_port(s.engine.clone(), ip.clone(), spec.clone()).await {
        return err_response(e);
    }
    let engine = s.engine.clone();
    let r =
        tokio::task::spawn_blocking(move || engine.publish_port(&b.container_ip, &b.spec)).await;
    match r {
        Ok(Ok(())) => Json(serde_json::json!({ "ok": true })).into_response(),
        Ok(Err(e)) => {
            // The publication failed, so the record must not keep claiming it —
            // otherwise the next start would replay a port the operator never got.
            let _ = forget_published_port(s.engine.clone(), Some(ip), spec).await;
            err_response(e)
        }
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// Adds `spec` to the `ports` of the container that holds `container_ip`.
///
/// Silent when no container matches: the ingress addresses workloads by IP and
/// nothing guarantees one of OUR records owns it (a VM, a foreign netns). The
/// publication itself still proceeds — refusing it would break a caller that
/// works today, and this function exists to record what it can, not to become a
/// second admission gate.
async fn record_published_port(
    engine: Arc<dyn EngineOps>,
    ip: String,
    spec: String,
) -> Result<(), Error> {
    blocking(move || engine.record_published_port(&ip, &spec)).await
}

/// Drops a publication from whichever record claims it.
///
/// `ip` narrows the search when the caller knows it (the rollback path); the
/// DELETE route knows only the host port, so it matches on that alone — which is
/// correct, because a host port can only be published once at a time.
async fn forget_published_port(
    engine: Arc<dyn EngineOps>,
    ip: Option<String>,
    spec_or_port: String,
) -> Result<(), Error> {
    blocking(move || engine.forget_published_port(ip.as_deref(), &spec_or_port)).await
}

/// `DELETE /v1/net/publish/:host_port` — withdraws a port publication.
///
/// **Best-effort, and the API does not pretend otherwise**: `unpublish_port`
/// returns no result — it removes the rule if it is there and stays quiet if it
/// is not. A 200 here means "the operation ran", not "there was something to
/// remove". Inventing a 404 would require a read the mechanism does not offer,
/// and a guessed 404 is worse than the truth.
async fn unpublish_port(State(s): State<AppState>, Path(host_port): Path<String>) -> Response {
    if host_port.is_empty() || !host_port.chars().all(|c| c.is_ascii_digit()) {
        return err_response(Error::Invalid(format!("invalid host port: '{host_port}'")));
    }
    // Mirror of the publish path: drop the desired state too, or the next start
    // would faithfully republish a port the operator has just asked to remove.
    let _ = forget_published_port(s.engine.clone(), None, host_port.clone()).await;
    let engine = s.engine.clone();
    let r = tokio::task::spawn_blocking(move || engine.unpublish_port(&host_port)).await;
    match r {
        Ok(()) => Json(serde_json::json!({ "ok": true })).into_response(),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// `GET /v1/networks` — the networks DECLARED on this node.
///
/// Read through the library, like the rest of this module's `GET`s: going
/// through the binary just to read would pay a `fork`+`exec` for a file read.
///
/// **Declared, not realized.** A network shows up here from `network create`,
/// but the bridge is only born in the holder's netns on the first attach — see
/// `/v1/net/status` to learn what is actually up. The distinction belongs to the
/// model, not to this route, and hiding it here would make the API lie about it.
async fn list_networks(State(s): State<AppState>) -> Response {
    let engine = s.engine.clone();
    match tokio::task::spawn_blocking(move || engine.network_list()).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// `GET /v1/networks/:name` — a network by name, or 404.
///
/// The 404 is the right answer and not a detail: whoever calls this is deciding
/// whether to create the network or reuse it, and an empty body with a 200 would
/// make them create over a network that already exists.
async fn get_network(State(s): State<AppState>, Path(name): Path<String>) -> Response {
    if !valid_arg(&name) {
        return err_response(Error::Invalid("invalid network name".to_string()));
    }
    let engine = s.engine.clone();
    let found = tokio::task::spawn_blocking(move || engine.network_get(&name)).await;
    match found {
        Ok(Some(def)) => Json(def).into_response(),
        Ok(None) => err_response(Error::NotFound("network".to_string())),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

/// `GET /v1/net/status` — the state of the REALIZED network infra on this node.
///
/// The counterpart to `list_networks`: that one says what is declared, this one
/// says what is up (holder, slirp, bridge, ref-count). A client that reads only
/// the first concludes the network exists when it does not yet.
async fn net_status(State(s): State<AppState>) -> Response {
    let engine = s.engine.clone();
    match tokio::task::spawn_blocking(move || engine.network_status()).await {
        Ok(st) => Json(st).into_response(),
        Err(e) => err_response(Error::Runtime {
            context: "join",
            message: e.to_string(),
        }),
    }
}

async fn create_network(State(s): State<AppState>, Json(b): Json<NetworkBody>) -> Response {
    if !valid_arg(&b.name) {
        return err_response(Error::Invalid("invalid network name".to_string()));
    }
    match run_cli(
        s.bin,
        s.base,
        vec!["network".into(), "create".into(), b.name],
    )
    .await
    {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

async fn delete_network(State(s): State<AppState>, Path(name): Path<String>) -> Response {
    if !valid_arg(&name) {
        return err_response(Error::Invalid("invalid network name".to_string()));
    }
    match run_cli(s.bin, s.base, vec!["network".into(), "rm".into(), name]).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/containers/:id/reconfig`. Only the subset that the runtime's
/// `container update` supports.
///
/// `deny_unknown_fields` because the ENGINE refuses what it does not do. This used
/// to say another program rejected the rest before calling — so `{"memory": "1G"}`
/// sent by any other client was accepted, ignored and answered with success: a field
/// the client wrote and the engine dropped. An engine that relies on its caller to
/// validate its own contract has no contract.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ReconfigBody {
    #[serde(default)]
    publish_add: Vec<String>,
    #[serde(default)]
    publish_rm: Vec<String>,
}

async fn reconfig_container(
    State(s): State<AppState>,
    Path(id): Path<String>,
    Json(b): Json<ReconfigBody>,
) -> Response {
    if !valid_arg(&id) {
        return err_response(Error::Invalid("invalid container id".to_string()));
    }
    let mut args = vec!["container".to_string(), "update".to_string(), id];
    // The ports have their own charset (digits/`:`/`/`) — they can't become a flag.
    for p in &b.publish_add {
        if p.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ':' | '/'))
        {
            args.push("--publish-add".into());
            args.push(p.clone());
        }
    }
    for p in &b.publish_rm {
        if p.chars()
            .all(|c| c.is_ascii_digit() || matches!(c, ':' | '/'))
        {
            args.push("--publish-rm".into());
            args.push(p.clone());
        }
    }
    match run_cli(s.bin, s.base, args).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

/// Body of `POST /v1/vms/:name/action`. Only `stop`/`rm` — `delonix_vm::start`
/// exists but is not routed (see the router note).
#[derive(serde::Deserialize)]
struct VmActionBody {
    action: String,
}

async fn vm_action_ep(
    State(s): State<AppState>,
    Path(name): Path<String>,
    Json(b): Json<VmActionBody>,
) -> Response {
    if !valid_arg(&name) {
        return err_response(Error::Invalid("invalid VM name".to_string()));
    }
    let sub = match b.action.as_str() {
        "stop" => "stop",
        "rm" | "remove" => "rm",
        other => return err_response(Error::Invalid(format!("unsupported VM action: {other}"))),
    };
    match run_cli(s.bin, s.base, vec!["vm".into(), sub.into(), name]).await {
        Ok((ok, out)) => Json(serde_json::json!({ "ok": ok, "output": out })).into_response(),
        Err(e) => err_response(e),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn run_refuses_a_field_the_engine_does_not_apply() {
        // The shape that failed silently: a sensible-looking field the run path never
        // reads. Before, this deserialized, the container ran without a CPU limit, and
        // the answer was `ok: true`.
        let err = serde_json::from_str::<super::RunSpecBody>(r#"{"image":"alpine","cpus":"2"}"#);
        assert!(
            err.is_err(),
            "an unknown field must be refused, not dropped"
        );
        let msg = err.err().map(|e| e.to_string()).unwrap_or_default();
        assert!(
            msg.contains("cpus"),
            "the refusal must name the field: {msg}"
        );
        let ok = serde_json::from_str::<super::RunSpecBody>(
            r#"{"image":"alpine","name":"web","ports":["8080:80"],"knows_none":true}"#,
        );
        assert!(ok.is_ok());
    }

    #[test]
    fn reconfig_refuses_a_field_the_engine_does_not_apply() {
        // Accepted-and-ignored is the failure this guards: the client believes the
        // limit changed.
        let err = serde_json::from_str::<super::ReconfigBody>(r#"{"memory":"1G"}"#);
        assert!(
            err.is_err(),
            "an unknown field must be refused, not dropped"
        );
        let ok = serde_json::from_str::<super::ReconfigBody>(r#"{"publish_add":["8080:80"]}"#);
        assert!(ok.is_ok());
    }

    /// Serialises the two tests below, which both write the PROCESS-GLOBAL
    /// `DELONIX_ROOT` while cargo runs them on parallel threads.
    ///
    /// The mechanism is not a guess — the doc-comment on the first of them
    /// already states it: those routes read through `infra`, which resolves the
    /// root from the ENVIRONMENT and not from the `AppState`, so each test
    /// points the global at a temp dir of its own. What that reasoning did not
    /// cover is the two of them doing it AT THE SAME TIME. Two tests writing
    /// one process-global is a hazard on its own terms, and that is the whole
    /// justification for this lock.
    ///
    /// **It is not claimed to fix the flake that led here, because that flake
    /// was never reproduced.** What was observed: under
    /// `cargo test --workspace`, the first test failed twice with the LIST
    /// returning one network and the GET of that network returning 404 — a list
    /// from one root, a lookup in another, which is the shape the shared global
    /// would produce. Run alone it passes 29/29, ten times over; with a 300 ms
    /// window opened right after the `set_var`, it still passes five times
    /// over, with and without this lock. So: plausible mechanism, documented in
    /// this very file, and no measurement. This comment will not pretend
    /// otherwise.
    ///
    /// A `tokio::sync::Mutex` and not a `std` one: the guard is held across
    /// `.await`, which `clippy::await_holding_lock` refuses — and this repo runs
    /// clippy with `-D warnings`.
    static ROOT_ENV: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use delonix_state::Store;
    use http_body_util::BodyExt;
    use tower::ServiceExt; // oneshot

    /// A `VmView` with no VMs — these handler tests exercise the volume/
    /// container/validation routes, none of which read VMs; the VM port is
    /// proven through the engine in the cross-process e2e.
    /// The [`EngineOps`] the tests run against: a REAL `delonix-engine::Engine`
    /// mounted over the test's state root (a dev-dependency — the arch gate
    /// ignores dev/build deps, so this interface→assembly edge is test-only and
    /// uncounted). Every endpoint is thus exercised against the same composition
    /// the bin wires in production, with no re-implemented fake to drift.
    struct RealEngine(delonix_engine::Engine);

    impl EngineOps for RealEngine {
        fn list_images(&self) -> Result<serde_json::Value, Error> {
            self.0
                .images()
                .list()
                .map(|imgs| serde_json::to_value(imgs).unwrap_or(serde_json::Value::Null))
        }
        fn remove_image(&self, reference: &str) -> Result<String, Error> {
            self.0.images().remove(reference)
        }
        fn image_sbom(&self, reference: &str) -> Result<serde_json::Value, Error> {
            self.0.images().sbom(reference)
        }
        fn dash_summary(&self, include_network: bool, include_storage: bool) -> DashSummary {
            self.0.dash_summary(include_network, include_storage)
        }
        fn container_net(&self) -> std::collections::HashMap<String, (u64, u64)> {
            self.0.container_net()
        }
        fn workload_series(&self) -> Vec<WorkloadSeries> {
            self.0.workload_series()
        }
        fn list_volumes(&self) -> Result<serde_json::Value, Error> {
            self.0
                .volumes()
                .list()
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
        fn get_volume(&self, name: &str) -> Result<serde_json::Value, Error> {
            self.0
                .volumes()
                .inspect(name)
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
        fn create_volume(
            &self,
            name: &str,
            driver: &str,
            device: Option<String>,
            options: Option<String>,
        ) -> Result<serde_json::Value, Error> {
            self.0
                .volumes()
                .create_with(name, driver, device, options)
                .map(|v| serde_json::to_value(v).unwrap_or(serde_json::Value::Null))
        }
        fn remove_volume(&self, name: &str) -> Result<(), Error> {
            self.0.volumes().remove(name)
        }
        fn list_containers(&self) -> Result<Vec<delonix_compute::Container>, Error> {
            self.0.containers().list()
        }
        fn get_container(&self, id: &str) -> Result<delonix_compute::Container, Error> {
            self.0.containers().load(id)
        }
        fn record_published_port(&self, ip: &str, spec: &str) -> Result<(), Error> {
            self.0.containers().record_published_port(ip, spec)
        }
        fn forget_published_port(&self, ip: Option<&str>, spec_or_port: &str) -> Result<(), Error> {
            self.0.containers().forget_published_port(ip, spec_or_port)
        }
        fn dhcp_ip(&self, net: &str, mac: &str) -> Option<String> {
            self.0.networks().dhcp_ip_for_mac(net, mac)
        }
        fn dhcp_ip6(&self, net: &str, mac: &str) -> Option<String> {
            self.0.networks().dhcp_ip6_for_mac(net, mac)
        }
        fn container_ip(&self, id: &str) -> String {
            self.0.networks().container_ip(id)
        }
        fn network_list(&self) -> serde_json::Value {
            serde_json::to_value(self.0.networks().network_list())
                .unwrap_or(serde_json::Value::Null)
        }
        fn network_get(&self, name: &str) -> Option<serde_json::Value> {
            self.0
                .networks()
                .network_get(name)
                .map(|d| serde_json::to_value(d).unwrap_or(serde_json::Value::Null))
        }
        fn network_status(&self) -> serde_json::Value {
            serde_json::to_value(self.0.networks().status()).unwrap_or(serde_json::Value::Null)
        }
        fn valid_container_ip(&self, ip: &str) -> bool {
            self.0.networks().valid_container_ip(ip)
        }
        fn set_net_rate(&self, id: &str, rate_bit: u64, burst_bytes: u64) -> Result<(), Error> {
            self.0.networks().set_net_rate(id, rate_bit, burst_bytes)
        }
        fn publish_port(&self, container_ip: &str, spec: &str) -> Result<(), Error> {
            self.0.networks().publish_port(container_ip, spec)
        }
        fn unpublish_port(&self, host_port: &str) {
            self.0.networks().unpublish_port(host_port)
        }
    }

    fn test_state() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let engine = delonix_engine::Engine::mount(dir.path().to_path_buf())
            .expect("Engine::mount only stores the root");
        (
            AppState {
                base: dir.path().to_path_buf(),
                // `/bin/false`: the real mutations are proven in the cross-process e2e;
                // the unit tests cover only the validation (refusal BEFORE the exec).
                bin: PathBuf::from("/bin/false"),
                engine: Arc::new(RealEngine(engine)),
            },
            dir,
        )
    }

    async fn body_json(resp: Response) -> serde_json::Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    /// Builds a saved container record with an IP, for the publish-persistence tests.
    fn ctr_with_ip(base: &std::path::Path, id: &str, ip: &str) -> Store {
        let store = Store::open(base.join("containers")).unwrap();
        let mut c = delonix_compute::Container::new(
            id.to_string(),
            id.to_string(),
            "alpine:3.20".to_string(),
            vec!["sleep".to_string()],
            "256m".to_string(),
        );
        c.ip = Some(ip.to_string());
        c.network = Some("net".to_string());
        store.save(&c).unwrap();
        store
    }

    /// THE REGRESSION GUARD. Publishing through the API used to touch only the
    /// slirp/nft (the ACTUAL state) and leave `ports` empty, so nothing could
    /// replay the publication after an ingress restart. Measured on a live host:
    /// 18 records with `ports: []` while two of those ports were serving HTTP.
    #[tokio::test]
    async fn publish_through_api_lands_in_record() {
        let (st, _d) = test_state();
        let store = ctr_with_ip(&st.base, "aaaa1111bbbb2222", "10.210.0.5");

        record_published_port(
            st.engine.clone(),
            "10.210.0.5".to_string(),
            "18099:80".to_string(),
        )
        .await
        .unwrap();

        let c = store.list().unwrap().pop().unwrap();
        assert_eq!(
            c.ports,
            vec!["18099:80".to_string()],
            "the publication never reached the record — nothing could replay it on a restart"
        );
    }

    /// Publishing the same spec twice must not stack duplicates: the replay on
    /// start walks `ports` and would try to publish the same host port twice,
    /// the second failing as already in use.
    #[tokio::test]
    async fn publishing_twice_does_not_duplicate() {
        let (st, _d) = test_state();
        let store = ctr_with_ip(&st.base, "cccc3333dddd4444", "10.210.0.6");
        for _ in 0..3 {
            record_published_port(
                st.engine.clone(),
                "10.210.0.6".to_string(),
                "18100:80".to_string(),
            )
            .await
            .unwrap();
        }
        assert_eq!(store.list().unwrap().pop().unwrap().ports.len(), 1);
    }

    /// An IP that belongs to no record of ours (a VM, a foreign netns) must not
    /// be an error: the ingress addresses workloads by IP and this function
    /// records what it can — it is not a second admission gate.
    #[tokio::test]
    async fn unknown_ip_does_not_fail_nor_touch_anyone() {
        let (st, _d) = test_state();
        let store = ctr_with_ip(&st.base, "eeee5555ffff6666", "10.210.0.7");
        record_published_port(
            st.engine.clone(),
            "10.210.99.99".to_string(),
            "18101:80".to_string(),
        )
        .await
        .expect("a foreign IP is not an error");
        assert!(store.list().unwrap().pop().unwrap().ports.is_empty());
    }

    /// Un-publishing must drop the DESIRED state too, or the next start would
    /// faithfully republish a port the operator just removed.
    #[tokio::test]
    async fn unpublish_removes_from_record() {
        let (st, _d) = test_state();
        let store = ctr_with_ip(&st.base, "7777aaaa8888bbbb", "10.210.0.8");
        record_published_port(
            st.engine.clone(),
            "10.210.0.8".to_string(),
            "18102:80".to_string(),
        )
        .await
        .unwrap();

        let resp = router(st)
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/net/publish/18102")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(
            store.list().unwrap().pop().unwrap().ports.is_empty(),
            "the record kept asking for a port that was already withdrawn"
        );
    }

    #[tokio::test]
    async fn ping_responds() {
        let (st, _d) = test_state();
        let resp = router(st)
            .oneshot(
                Request::builder()
                    .uri("/_ping")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn metrics_exposes_the_shared_registry() {
        let (st, _d) = test_state();
        let resp = router(st)
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "application/openmetrics-text; version=1.0.0; charset=utf-8"
        );
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body = String::from_utf8_lossy(&bytes);
        // The shared registry's `delonix_build_info` must always be present.
        assert!(body.contains("delonix_build_info"), "body: {body}");
    }

    /// The TCP `/metrics` listener (`DELONIX_MGMT_METRICS_ADDR`) must carry the
    /// Prometheus endpoint and NOTHING else — the management routes
    /// (exec/run/rm) stay on the euid-checked unix socket. Exercises the exact
    /// router the production listener builds via `metrics_only_router`.
    #[tokio::test]
    async fn metrics_only_router_serves_metrics_but_not_management_routes() {
        let (st, _d) = test_state();
        let app = metrics_only_router(st);

        // `/metrics` is served, with the full registry.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&bytes).contains("delonix_build_info"));

        // Management routes are ABSENT: the arbitrary-code-execution surface
        // must never ride on the TCP metrics port.
        for uri in ["/v1/containers", "/v1/volumes", "/v1/dash"] {
            let resp = app
                .clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::NOT_FOUND,
                "{uri} must not be reachable on the metrics-only router"
            );
        }
    }

    #[tokio::test]
    async fn volume_lifecycle() {
        let (st, _d) = test_state();
        let app = router(st);

        // Empty list initially.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/volumes")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);

        // Create a volume.
        let create = Request::builder()
            .method("POST")
            .uri("/v1/volumes")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"data"}"#))
            .unwrap();
        let resp = app.clone().oneshot(create).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let v = body_json(resp).await;
        assert_eq!(v["name"], "data");
        assert_eq!(v["driver"], "local");

        // Shows up in the listing and in the individual GET.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/volumes/data")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await["name"], "data");

        // Delete.
        let del = Request::builder()
            .method("DELETE")
            .uri("/v1/volumes/data")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(del).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NO_CONTENT);

        // GET of a nonexistent volume → 404.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/volumes/none")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn valid_name_rejects_traversal() {
        assert!(valid_name("dados"));
        assert!(valid_name("bd-1.snap_2"));
        // Traversal / separators / dot-segments → rejected.
        for bad in ["", ".", "..", "../x", "a/b", "a..b", "..\u{0000}", "/etc"] {
            assert!(!valid_name(bad), "should reject {bad:?}");
        }
    }

    #[tokio::test]
    async fn delete_with_dot_dot_gives_400_and_deletes_nothing() {
        let (st, _d) = test_state();
        // A DELETE with `..` in the path must be refused at the boundary (it doesn't
        // reach the store's remove_dir_all — otherwise it would delete the parent dir).
        let del = Request::builder()
            .method("DELETE")
            .uri("/v1/volumes/..")
            .body(Body::empty())
            .unwrap();
        let resp = router(st).oneshot(del).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn containers_empty_list_and_get_nonexistent_gives_404() {
        let (st, _d) = test_state();
        let app = router(st);

        // No containers created → empty list.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/containers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);

        // GET of a nonexistent container → 404.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/containers/none")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn containers_returns_populated_container() {
        use delonix_compute::Container;
        let (st, dir) = test_state();
        // Persist a real container in the store (`<base>/containers`), as the CLI does.
        let store = Store::open(dir.path().join("containers")).unwrap();
        let c = Container::new(
            "abc123def456".to_string(),
            "web".to_string(),
            "nginx:latest".to_string(),
            vec![
                "nginx".to_string(),
                "-g".to_string(),
                "daemon off;".to_string(),
            ],
            "512m".to_string(),
        );
        store.save(&c).unwrap();

        let app = router(st);
        // Shows up in the listing, with the fields intact.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/containers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let arr = body_json(resp).await;
        let arr = arr.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "web");
        assert_eq!(arr[0]["image"], "nginx:latest");

        // GET by exact id → the same container.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/containers/abc123def456")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let got = body_json(resp).await;
        assert_eq!(got["id"], "abc123def456");
        assert_eq!(got["name"], "web");
        assert_eq!(got["command"][0], "nginx");
    }

    #[tokio::test]
    async fn images_list_and_rmi() {
        use delonix_oci::{Image, ImageConfig, ImageStore};
        let (st, dir) = test_state();
        let store = ImageStore::open(dir.path()).unwrap();
        store
            .save(&Image {
                id: "sha256:aabbccddeeff00112233".to_string(),
                repo_tags: vec!["nginx:latest".to_string()],
                layers: vec![],
                config: ImageConfig::default(),
                created_unix: 1,
            })
            .unwrap();

        let app = router(st);
        // The list shows the image.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/images")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let arr = body_json(resp).await;
        assert_eq!(arr.as_array().unwrap().len(), 1);
        assert_eq!(arr[0]["repo_tags"][0], "nginx:latest");

        // rmi by tag (ref goes by query, with `:` and potentially `/`).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/images?ref=nginx:latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_json(resp).await["result"]
            .as_str()
            .unwrap()
            .contains("deleted"));

        // No longer exists → rmi again gives 404.
        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/v1/images?ref=nginx:latest")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn container_mutation_validates_id_before_exec() {
        let (st, _d) = test_state();
        let app = router(st);
        // `..` and a leading `-` (which the CLI would read as a flag) → 400, no exec. (An
        // `a/b` doesn't even reach the handler — it becomes 2 segments and the router gives 404.)
        for bad in ["..", "-rf"] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri(format!("/v1/containers/{bad}?force=true"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::BAD_REQUEST,
                "delete should reject {bad:?}"
            );
        }
        // Unknown action → 400 (allowlist).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/containers/web/action")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"action":"detonate"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // run: empty image or one starting with `-` (would become a positional flag) → 400.
        for bad in [
            r#"{"image":""}"#,
            r#"{"image":"-rm"}"#,
            r#"{"image":"x","name":"-p"}"#,
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/containers")
                        .header("content-type", "application/json")
                        .body(Body::from(bad))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::BAD_REQUEST,
                "run should reject {bad}"
            );
        }
        // pull: empty ref or one starting with `-` → 400 (before any exec).
        for bad in [r#"{"ref":""}"#, r#"{"ref":"-x"}"#] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/images/pull")
                        .header("content-type", "application/json")
                        .body(Body::from(bad))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::BAD_REQUEST,
                "pull should reject {bad}"
            );
        }
        // build: invalid tag or empty Delonixfile → 400 (before writing/exec).
        for bad in [
            r#"{"delonixfile":"FROM x","tag":""}"#,
            r#"{"delonixfile":"FROM x","tag":"-t"}"#,
            r#"{"delonixfile":"","tag":"ok:1"}"#,
        ] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/images/build")
                        .header("content-type", "application/json")
                        .body(Body::from(bad))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::BAD_REQUEST,
                "build should reject {bad}"
            );
        }
        // network create: invalid name (`-`/empty) → 400.
        for bad in [r#"{"name":""}"#, r#"{"name":"-net"}"#] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/networks")
                        .header("content-type", "application/json")
                        .body(Body::from(bad))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                resp.status(),
                StatusCode::BAD_REQUEST,
                "network create should reject {bad}"
            );
        }
    }

    #[test]
    fn build_run_args_uses_net_not_network_and_respects_filters() {
        let spec = RunSpecBody {
            image: "nginx:latest".into(),
            name: "web".into(),
            ports: vec!["8080:80".into(), "bad;port".into()], // 2nd is filtered out
            env: vec!["K=v".into()],
            network: "my-net".into(),
            memory: "256m".into(),
            restart: "always".into(),
            command: vec!["nginx".into(), "-g".into(), "daemon off;".into()],
            volumes: vec!["data:/var".into(), "bad/../x:/y".into()], // 2nd filtered out
            knows: vec!["db".into()],
            knows_none: false,
        };
        let args = build_run_args(spec);
        // Network flag is `--net=…` (the runtime's), NEVER `--network`.
        assert!(args.contains(&"--net=my-net".to_string()), "args: {args:?}");
        assert!(
            !args.iter().any(|a| a.starts_with("--network")),
            "must not use --network"
        );
        // Filters preserved: invalid port and volume with `..` fall out.
        assert!(args.contains(&"8080:80".to_string()));
        assert!(!args.iter().any(|a| a.contains("bad;port")));
        assert!(args.contains(&"data:/var".to_string()));
        assert!(!args.iter().any(|a| a.contains("..")));
        // The image comes before the command (final positional), and the command after.
        let img = args.iter().position(|a| a == "nginx:latest").unwrap();
        let cmd = args.iter().position(|a| a == "daemon off;").unwrap();
        assert!(img < cmd, "image before the command");
        assert!(args.contains(&"--knows".to_string()) && args.contains(&"db".to_string()));
    }

    #[test]
    fn build_run_args_knows_none_takes_precedence() {
        let spec = RunSpecBody {
            image: "x".into(),
            name: String::new(),
            ports: vec![],
            env: vec![],
            network: String::new(),
            memory: String::new(),
            restart: String::new(),
            command: vec![],
            volumes: vec![],
            knows: vec!["db".into()],
            knows_none: true,
        };
        let args = build_run_args(spec);
        assert!(args.contains(&"--knows-none".to_string()));
        assert!(
            !args.contains(&"--knows".to_string()),
            "knows-none excludes knows"
        );
    }

    #[tokio::test]
    async fn container_get_with_dot_dot_gives_400() {
        let (st, _d) = test_state();
        // `..` in the id path must be refused at the boundary (`Store::load` does
        // `root.join(id)` before the scan — a `..` would escape the root).
        let resp = router(st)
            .oneshot(
                Request::builder()
                    .uri("/v1/containers/..")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn invalid_name_gives_400() {
        let (st, _d) = test_state();
        let create = Request::builder()
            .method("POST")
            .uri("/v1/volumes")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"name":"invalid name!!"}"#))
            .unwrap();
        let resp = router(st).oneshot(create).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    /// The new network routes: list, get, and the realized-infra status.
    ///
    /// `DELONIX_ROOT` is PROCESS-wide, and `test_state` already gives a temp root
    /// per test — but these routes read through `infra`, which resolves the root
    /// from the environment and not from `AppState`. So the root is pointed here, once.
    #[tokio::test]
    async fn networks_list_get_and_status() {
        let (st, d) = test_state();
        let _root = ROOT_ENV.lock().await;
        std::env::set_var("DELONIX_ROOT", d.path());
        let app = router(st);

        // No declared networks → empty list (and not a 500).
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/networks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(body_json(resp).await.as_array().unwrap().len(), 0);

        // A network that does not exist → 404, not 200 with an empty body:
        // whoever calls this is deciding between creating and reusing.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/networks/does-not-exist")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // An invalid name is refused BEFORE touching the disk.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/networks/..")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // Declare a network through the library and confirm both routes see it.
        let def = delonix_sdn::infra::network_create("api-test").expect("create network");
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/networks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let list = body_json(resp).await;
        assert_eq!(list.as_array().unwrap().len(), 1);

        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/networks/api-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let j = body_json(resp).await;
        assert_eq!(j["name"], "api-test");
        assert_eq!(j["bridge"], def.bridge);
        assert_eq!(j["prefix"], def.prefix);

        // The infra status responds even with the holder down — it is the "is it
        // up?" question, and an error response there is indistinguishable from an
        // unreachable node.
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/v1/net/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
    /// Port publication: what is refused BEFORE reaching the mechanism.
    ///
    /// The happy path is deliberately not tested here — publishing requires the
    /// holder up, slirp and nft, and a test needing that stops running in CI and
    /// becomes decoration. What is tested is the boundary: what the API lets
    /// through into a DNAT rule.
    #[tokio::test]
    async fn publish_rejects_invalid_ip_and_spec() {
        let (st, _d) = test_state();
        let app = router(st);

        // An IP that is not an IP would end up in an arbitrary DNAT rule.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/net/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"container_ip":"not-an-ip","spec":"8080:80"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // And the spec passes through the same filter as the other arguments.
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/net/publish")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"container_ip":"10.200.0.5","spec":"8080:80; rm -rf /"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // The unpublish port is a number and nothing else — it is what identifies
        // the rule to remove.
        for bad in ["abc", "80a", ".."] {
            let resp = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("DELETE")
                        .uri(format!("/v1/net/publish/{bad}"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST, "accepted {bad:?}");
        }
    }
    /// The network mutations that wrote the dataplane without the container's
    /// record (P0-6) answer 501 with `DX-6302` — ALL of them, with a valid body
    /// or not. A valid body is the case that matters: it was the one that got
    /// through and wrote a rule the next reapply undid.
    #[tokio::test]
    async fn network_mutations_without_record_are_refused() {
        let (st, _d) = test_state();
        let app = router(st);

        let req = |method: &str, uri: &str, body: &'static str| {
            Request::builder()
                .method(method)
                .uri(uri.to_string())
                .header("content-type", "application/json")
                .body(Body::from(body))
                .unwrap()
        };
        let cases = [
            (
                "PUT",
                "/v1/net/firewall/10.200.0.5",
                r#"{"id":"abc","fw":{"enabled":true}}"#,
            ),
            (
                "PUT",
                "/v1/net/firewall/not-an-ip",
                r#"{"id":"abc","fw":{"enabled":true}}"#,
            ),
            ("DELETE", "/v1/net/firewall/10.200.0.5", ""),
            ("PUT", "/v1/net/egress", r#"{"deny":true}"#),
            ("PUT", "/v1/net/egress/dlxn0001", r#"{"deny":false}"#),
            (
                "POST",
                "/v1/net/attach-extra",
                r#"{"id":"abc","idx":1,"net":"app","namespace":""}"#,
            ),
            ("DELETE", "/v1/net/attach-extra/abc/1/10.201.0.9", ""),
            ("DELETE", "/v1/net/attach/abc/10.200.0.5", ""),
        ];
        for (method, uri, body) in cases {
            let resp = app.clone().oneshot(req(method, uri, body)).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_IMPLEMENTED, "{method} {uri}");
            let b = body_json(resp).await;
            assert_eq!(b["number"], "DX-6302", "{method} {uri}: {b}");
            assert_eq!(b["code"], "DX_UNAVAILABLE", "{method} {uri}: {b}");
        }

        let put = |uri: &str, body: &'static str| req("PUT", uri, body);
        // The bandwidth limit validates the id.
        let resp = app
            .oneshot(put(
                "/v1/containers/../rate",
                r#"{"rate_bit":1000,"burst_bytes":100}"#,
            ))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
    /// Addresses: the filter, and the `valid_mac` that exists because of the colons.
    /// (Network attach is no longer served — see
    /// `network_mutations_without_record_are_refused`.)
    #[tokio::test]
    async fn dhcp_and_attach_validate_their_input() {
        let (st, d) = test_state();
        let _root = ROOT_ENV.lock().await;
        std::env::set_var("DELONIX_ROOT", d.path());
        let app = router(st);

        let g = |uri: String| {
            app.clone()
                .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        };

        // MAC with characters that are neither hex nor `:`.
        let resp = g("/v1/net/dhcp/app/zz:zz:zz:zz:zz:zz".into())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // A legitimate MAC must NOT be refused — that is what `valid_mac` exists
        // to guarantee, because the module's `valid_arg` rejects the colons.
        // The network does not exist, so 404 (not 400): the distinction is the point.
        let resp = g("/v1/net/dhcp/net-that-does-not-exist/aa:bb:cc:dd:ee:ff".into())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        // The IP derived from an id responds without touching state.
        let resp = g("/v1/net/container-ip/abc123".into()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_json(resp).await["ip"].is_string());

        // An invalid id is refused.
        let resp = g("/v1/net/container-ip/..".into()).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
