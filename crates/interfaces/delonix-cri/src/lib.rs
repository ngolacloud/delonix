//! `delonix-cri` — Kubernetes **CRI** (Container Runtime Interface) server.
//!
//! Implements the `runtime.v1` gRPC (RuntimeService + ImageService) over the
//! Delonix engine, so that a **kubelet** (or `crictl`) can use Delonix as the
//! node runtime. The stubs are generated from `proto/api.proto` (CRI v1, no
//! gogoproto). C2.
//!
//! The tonic gRPC pattern returns `Result<Response<T>, Status>`; `Status` is
//! large by nature, so we silence `result_large_err` across the whole crate.
#![allow(clippy::result_large_err)]

use delonix_host::peer_cred::peer_uid;
use std::path::PathBuf;
use std::sync::Arc;
use tokio_stream::StreamExt as _;
use tonic::{Request, Response, Status};

pub mod cri {
    #![allow(clippy::all)]
    tonic::include_proto!("runtime.v1");
}

use cri::image_service_server::{ImageService, ImageServiceServer};
use cri::runtime_service_server::RuntimeServiceServer;
use cri::*;

pub mod cap_ceiling;
pub mod child_handle;
mod runtime_svc;
pub mod spdy;
pub mod streaming;

pub use cap_ceiling::{CapCeiling, CeilingMode};

const RUNTIME_NAME: &str = "delonix";
const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Generic error → gRPC `Status`.
fn st<E: std::fmt::Display>(e: E) -> Status {
    Status::internal(e.to_string())
}

/// The composed engine's image use cases this CRI `ImageService` serves (R3h).
/// The interface no longer names the `delonix-oci` adapter: the multicall mounts
/// a `delonix-engine::Engine` and hands in an implementor, exactly as the mgmt/mcp
/// servers receive their ports (ADR-0040 A5). Image records cross as a serialized
/// summary (`{id, user, repo_tags, size}`) — the adapter's `Image` type is not
/// nameable here, and the CRI proto mapping ([`to_cri_image`]) is this boundary's
/// job. Errors arrive as the shared [`delonix_model::Error`], turned into a gRPC
/// `Status` via [`st`].
pub trait ImageOps: Send + Sync {
    /// Every local image, each a `{id, user, repo_tags, size}` summary.
    fn list_images(&self) -> Result<Vec<serde_json::Value>, delonix_model::Error>;
    /// One image's summary by reference, or `None` if it is not stored.
    fn image_status(
        &self,
        reference: &str,
    ) -> Result<Option<serde_json::Value>, delonix_model::Error>;
    /// Pull an image (optionally with resolved `(username, password)`); the id.
    fn pull_image(
        &self,
        reference: &str,
        creds: Option<(String, String)>,
    ) -> Result<String, delonix_model::Error>;
    /// Remove an image (the CRI contract is idempotent — the caller ignores a miss).
    fn remove_image(&self, reference: &str) -> Result<(), delonix_model::Error>;
    /// Total on-disk bytes of all image layers (CRI `ImageFsInfo`).
    fn image_fs_used_bytes(&self) -> u64;
}

/// The composed engine's container-store use cases this CRI RuntimeService reads
/// and reconciles through (R3h). The interface no longer names the `delonix-state`
/// adapter: the multicall mounts a `delonix-engine::Engine` and hands in an
/// implementor, exactly as it does for images (ADR-0040 A5). Records cross as the
/// shared [`delonix_compute::Container`]; errors as the shared
/// [`delonix_model::Error`] every surface already maps to a gRPC `Status`.
///
/// `update` is the engine's **safe read-modify-write** under an exclusive `flock`
/// (re-read under the lock): the ONE write path the concurrent CRI issues to the
/// container store. It is object-safe — the closure crosses as `&mut dyn FnMut`
/// (a `dyn ContainerOps` cannot carry a generic method), and the implementor
/// forwards it to the engine's generic `update`.
pub trait ContainerOps: Send + Sync {
    /// Every container this node has.
    fn list(&self) -> Result<Vec<delonix_compute::Container>, delonix_model::Error>;
    /// One container's full record, by name or id (prefix); the prefixed CRI
    /// names (`cri-<id>`, `pod-cri-<id>`) resolve here exactly as on the store.
    fn load(&self, id_or_name: &str)
        -> Result<delonix_compute::Container, delonix_model::Error>;
    /// Locked read-modify-write: `f` mutates the record re-read under the lock and
    /// returns `false` to abort the write. The final (or read) record comes back.
    fn update(
        &self,
        id_or_name: &str,
        f: &mut dyn FnMut(&mut delonix_compute::Container) -> bool,
    ) -> Result<delonix_compute::Container, delonix_model::Error>;
}

/// A CRI-local mirror of the engine's Linux resource-update input (R3h): the same
/// fields as the adapter's native update struct, in std types only, so the
/// interface names no adapter type. `update_container_resources` fills it from the
/// CRI request and the multicall maps it back across [`RuntimeOps::update_kube_resources`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourceUpdateSpec<'a> {
    pub memory: Option<&'a str>,
    pub cpus: Option<&'a str>,
    pub cpu_weight: Option<&'a str>,
    pub cpuset: Option<&'a str>,
    pub cpuset_mems: Option<&'a str>,
    pub hugepage_limits: &'a [(String, u64)],
    pub unified: &'a [(String, String)],
    pub oom_score_adj: Option<i32>,
}

/// The composed engine's **Linux mechanism** use cases this CRI server routes
/// through (R3h). The interface no longer names the `delonix-linux` adapter: the
/// multicall mounts the engine and hands in an implementor, exactly as it does for
/// images and the container store (ADR-0040 A5). Everything that crosses is a std
/// type or the shared [`delonix_compute::Container`]; no adapter type is nameable
/// here. It splits into two groups — the host/cgroup process primitives the
/// lifecycle needs, and the capability dictionary the node ceiling
/// ([`cap_ceiling`]) computes against, so the ceiling never keeps a second copy of
/// the engine's own capability rules.
pub trait RuntimeOps: Send + Sync {
    /// Whether this node runs rootless (no root; inside a user namespace).
    fn is_rootless(&self) -> bool;
    /// Whether `pid` — recorded together with `starttime` — is still the exact
    /// process we recorded (the PID-reuse guard before a signal or a `setns`).
    fn safe_to_signal(&self, pid: i32, starttime: Option<u64>) -> bool;
    /// The cgroup v2 leaf controllers the engine's slice delegates (default set),
    /// ensuring the slice exists as a side effect.
    fn leaf_controllers(&self) -> Vec<String>;
    /// Reconcile a container record's status against kernel reality, in place;
    /// `true` when the record changed — the signal the locked `update` closure
    /// uses to decide whether to persist.
    fn reconcile_status(&self, c: &mut delonix_compute::Container) -> bool;
    /// The `crash_reason` marker the engine writes when the OOM killer took a
    /// container down (compared against a record's `crash_reason`).
    fn oom_killed_reason(&self) -> &'static str;
    /// Validate an OCI seccomp profile JSON, fail-closed; the error text on reject.
    fn seccomp_validate(&self, json: &str) -> Result<(), String>;
    /// Apply a kube resource update to a container's live cgroup. `Ok(true)` when it
    /// reached a live cgroup (applied or deferred), `Ok(false)` when it did not
    /// (the container looked alive but its cgroup is gone — `NotEnforced`).
    fn update_kube_resources(
        &self,
        c: &delonix_compute::Container,
        update: &ResourceUpdateSpec<'_>,
    ) -> Result<bool, delonix_model::Error>;

    // -- capability dictionary (the node ceiling, `cap_ceiling`, computes against
    //    the engine's own rules; never a second copy) --

    /// Mask of every capability the kernel has (`/proc/sys/kernel/cap_last_cap`).
    fn all_caps_mask(&self) -> u64;
    /// Bit number of a capability name (`CAP_` prefix optional, case-insensitive),
    /// or `None` for an unknown name.
    fn cap_bit(&self, name: &str) -> Option<u8>;
    /// The engine's default kept-capability set, as a mask.
    fn default_kept_mask(&self) -> u64;
    /// The capability names of a mask, in the engine's canonical order.
    fn names_from_mask(&self, mask: u64) -> Vec<&'static str>;
    /// The mask the engine would grant for the given `--cap-drop`/`--cap-add`
    /// (its own resolution rule, so a clamp cannot drift from it).
    fn resolve_cap_keep(&self, cap_drop: &[String], cap_add: &[String]) -> u64;
}

/// The rootless network plane's liveness, as this server reads it (R3h-D): the two
/// fields the `NetworkReady` rule needs. A std mirror of the adapter's own status
/// struct, so the interface names no SDN type.
#[derive(Debug, Clone, Copy, Default)]
pub struct InfraStatus {
    /// The infra namespace (holder/slirp) is up and its control socket answers.
    pub up: bool,
    /// How many ref markers are on disk (may count ghosts; the caller filters).
    pub refcount: i64,
    /// The holder process pid, if running (for the `NetworkReady` diagnostic).
    pub holder_pid: Option<i32>,
    /// The slirp process pid, if running (for the `NetworkReady` diagnostic).
    pub slirp_pid: Option<i32>,
}

/// One CRI `hostPort` mapping the pod asked to publish, in std types (R3h-D). The
/// multicall folds these into the node's CNI chain across [`NetworkOps::cni_prepare_conf`].
#[derive(Debug, Clone)]
pub struct CniPort {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: String,
    pub host_ip: Option<String>,
}

/// A node CNI chain resolved and port-mapped for a sandbox (R3h-D), as the opaque
/// JSON the CRI persists and hands back for the DEL. The interface never sees the
/// adapter's `NetConfList`: `json` is the serialized chain, `name` and
/// `publishes_host_ports` are the only facts the CRI's own refusal policy reads.
#[derive(Debug, Clone)]
pub struct CniConf {
    pub name: String,
    pub publishes_host_ports: bool,
    pub json: String,
}

/// Whether the node has a usable CNI chain for a sandbox (R3h-D). `Disabled` is the
/// rootless, no-`DELONIX_CNI` case (the caller takes the native path); `NotReady`
/// is the root case with no runnable chain (the CRI turns it into the pod's event).
#[derive(Debug, Clone)]
pub enum CniConfResult {
    Disabled,
    Ready(CniConf),
    NotReady { reason: String, message: String },
}

/// The result of attaching a sandbox to its CNI network (R3h-D).
#[derive(Debug, Clone, Default)]
pub struct CniAttached {
    pub ip: String,
    pub netns_path: String,
}

/// The composed engine's **SDN / CNI mechanism** use cases this CRI server routes
/// through (R3h-D). The interface no longer names the `delonix-sdn` adapter: the
/// multicall hands in an implementor, exactly as for the Linux mechanism
/// ([`RuntimeOps`], ADR-0040 A5). The CRI keeps its own networking POLICY (the
/// rootless/root branching, the refusal wording, the `CNI_ARGS` it builds); only
/// the leaf mechanism crosses, and the CNI chain crosses as the opaque JSON of
/// [`CniConf`], never as an adapter type.
pub trait NetworkOps: Send + Sync {
    /// The rootless infra plane's liveness (`NetworkReady` rootless rule).
    fn infra_status(&self) -> InfraStatus;
    /// Whether a ROOT node has a runnable CNI chain; `Err((reason, message))` is the
    /// `NetworkReady=false` condition the kubelet reads.
    fn root_network_ready(&self) -> Result<(), (String, String)>;
    /// The ingress ref markers under this server's root (for the live-ref count).
    fn attached_refs_in(&self, base: &std::path::Path) -> Vec<String>;
    /// How many of `attached` point at a container no longer in `live` (orphans).
    fn orphan_ref_count(&self, attached: &[String], live: &[String]) -> usize;
    /// Whether opt-in CNI (`DELONIX_CNI` + a conflist) is configured on this node.
    fn cni_enabled(&self) -> bool;
    /// Resolve the node's CNI chain for a sandbox. `rootless` picks the holder's
    /// opt-in conflist; root picks the node's `/etc/cni/net.d` chain (and reports
    /// `NotReady` when it cannot run).
    fn cni_resolve_conf(&self, rootless: bool) -> CniConfResult;
    /// Fold the requested host-port mappings into a resolved chain's JSON, for the
    /// plugins that declare the `portMappings` capability; the new chain JSON.
    fn cni_with_port_mappings(&self, conf_json: &str, ports: &[CniPort]) -> String;
    /// Attach the sandbox `pod` to `conf_json` (the [`CniConf::json`]); the IP and,
    /// for the root path, the named netns. `cni_args` is the CRI's `CNI_ARGS`.
    fn cni_attach(
        &self,
        pod: &str,
        rootless: bool,
        conf_json: &str,
        cni_args: Option<&str>,
    ) -> Result<CniAttached, String>;
    /// Apply the pod netns sysctls (root path), after attach.
    fn cni_set_netns_sysctls(
        &self,
        netns_path: &str,
        sysctls: &[(String, String)],
    ) -> Result<(), String>;
    /// Detach the sandbox `pod` from its CNI network (best-effort). Root uses the
    /// stored `conf_json`; rootless re-reads the node's enabled conflist itself.
    fn cni_detach(&self, pod: &str, rootless: bool, conf_json: &str);
    /// The deterministic rootless pod IP for the shared ingress netns.
    fn container_ip(&self, name: &str) -> String;
    /// The normalized transport (`"tcp"`/`"udp"`) a hostPort can publish on the
    /// rootless datapath, or `None` when it cannot carry it (e.g. SCTP on slirp).
    fn publishable_proto(&self, name: &str) -> Option<&'static str>;
}

// ---------------------------------------------------------------------------
// ImageService — fully functional over the Delonix engine (R3h).
// ---------------------------------------------------------------------------

pub struct DelonixImage {
    /// The state root, for the `ImageFsInfo` mountpoint label only — the image
    /// reads/writes themselves go through `engine`.
    pub base: PathBuf,
    pub engine: Arc<dyn ImageOps>,
}

#[tonic::async_trait]
impl ImageService for DelonixImage {
    async fn list_images(
        &self,
        _req: Request<ListImagesRequest>,
    ) -> Result<Response<ListImagesResponse>, Status> {
        let engine = self.engine.clone();
        let list = tokio::task::spawn_blocking(move || engine.list_images())
            .await
            .map_err(st)?
            .map_err(st)?;
        let images = list.iter().map(to_cri_image).collect();
        Ok(Response::new(ListImagesResponse { images }))
    }

    async fn image_status(
        &self,
        req: Request<ImageStatusRequest>,
    ) -> Result<Response<ImageStatusResponse>, Status> {
        let name = req.into_inner().image.map(|s| s.image).unwrap_or_default();
        let engine = self.engine.clone();
        let found = tokio::task::spawn_blocking(move || engine.image_status(&name).ok().flatten())
            .await
            .map_err(st)?;
        let image = found.as_ref().map(to_cri_image);
        Ok(Response::new(ImageStatusResponse {
            image,
            info: Default::default(),
        }))
    }

    async fn pull_image(
        &self,
        req: Request<PullImageRequest>,
    ) -> Result<Response<PullImageResponse>, Status> {
        let r = req.into_inner();
        let name = r.image.map(|s| s.image).unwrap_or_default();
        if name.is_empty() {
            return Err(Status::invalid_argument("empty image"));
        }
        // `AuthConfig` comes from the Pod's `imagePullSecrets` (the kubelet
        // resolves them and sends them here) — WITHOUT this, any private registry
        // fails because `pull_from_registry` only uses local credentials
        // (`delonix login` on the node itself, which usually does not have the
        // tenant's credentials). Only `username`+`password` are supported for now
        // (the base CRI schema); `identity_token`/`registry_token`/`auth`
        // (already-combined base64) are left for when a real case that needs them
        // shows up.
        let creds = r
            .auth
            .filter(|a| !a.username.is_empty())
            .map(|a| (a.username, a.password));
        let engine = self.engine.clone();
        let image_ref = tokio::task::spawn_blocking(move || engine.pull_image(&name, creds))
            .await
            .map_err(st)?
            .map_err(st)?;
        delonix_telemetry::metrics::inc_image_pulled();
        Ok(Response::new(PullImageResponse { image_ref }))
    }

    async fn remove_image(
        &self,
        req: Request<RemoveImageRequest>,
    ) -> Result<Response<RemoveImageResponse>, Status> {
        let name = req.into_inner().image.map(|s| s.image).unwrap_or_default();
        let engine = self.engine.clone();
        // CRI: RemoveImage is IDEMPOTENT — removing a nonexistent image is success
        // (the kubelet calls it in GC cycles without guaranteeing it still exists).
        tokio::task::spawn_blocking(move || {
            let _ = engine.remove_image(&name);
        })
        .await
        .map_err(st)?;
        Ok(Response::new(RemoveImageResponse {}))
    }

    async fn image_fs_info(
        &self,
        _req: Request<ImageFsInfoRequest>,
    ) -> Result<Response<ImageFsInfoResponse>, Status> {
        let engine = self.engine.clone();
        let used = tokio::task::spawn_blocking(move || engine.image_fs_used_bytes())
            .await
            .map_err(st)?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as i64)
            .unwrap_or(0);
        let fs = FilesystemUsage {
            timestamp: now,
            fs_id: Some(FilesystemIdentifier {
                mountpoint: self.base.to_string_lossy().into_owned(),
            }),
            used_bytes: Some(UInt64Value { value: used }),
            inodes_used: Some(UInt64Value { value: 0 }),
        };
        Ok(Response::new(ImageFsInfoResponse {
            image_filesystems: vec![fs.clone()],
            container_filesystems: vec![fs],
        }))
    }
}

/// Maps the image OCI config's `user` to the `uid`/`username` fields of the CRI
/// `Image` (`crictl`/kubelet use them to validate RunAsNonRoot). Rule:
/// `""` (no USER) → root, `uid = 0`; numeric `"uid[:gid]"` → `uid`; a name →
/// `username` (resolved at runtime against the image's `/etc/passwd`). The
/// conformance spec `image status … should not have Uid|Username empty` requires
/// that ONE of the two be filled — before, both came back empty.
///
/// The `USER` directive is `user[:group]` in BOTH forms. The group is NOT part
/// of the CRI `username`/`uid`: only the user half is taken, from the same
/// `split(':').next()` used for the numeric case. Returning the whole string
/// (`"www-data:1004"`) made the spec `image status … should not have Uid|Username
/// empty` fail on the `name:group` image, which compares `username` to the user
/// name alone (`"www-data"`).
fn image_user(user: &str) -> (Option<Int64Value>, String) {
    let u = user.trim();
    if u.is_empty() {
        return (Some(Int64Value { value: 0 }), String::new());
    }
    let user_part = u.split(':').next().unwrap_or(u);
    match user_part.parse::<i64>() {
        Ok(uid) => (Some(Int64Value { value: uid }), String::new()),
        Err(_) => (None, user_part.to_string()),
    }
}

/// Splits a store's reference list into the CRI `Image`'s two distinct fields:
/// `(repo_tags, repo_digests)`. A reference is a DIGEST reference when it carries
/// an `@` (`name@sha256:…`, the form a `crictl pull <name>@<digest>` records) and
/// a tag reference otherwise (`name:tag`). The engine's `ImageStore` keeps every
/// reference in one `repo_tags` list; the split is the CRI boundary's job because
/// the proto has separate fields and the kubelet/`crictl` read them apart — a pull
/// by digest must come back with `repo_tags` EMPTY and the digest in `repo_digests`
/// (conformance `public image with digest should be pulled and removed`), and a
/// pull by tag the other way round.
///
/// Split on `@` and not on `sha256:` so a non-`sha256` algorithm still classifies
/// as a digest; the `:` of the tag never collides because the `@` precedes it.
fn partition_refs(refs: &[String]) -> (Vec<String>, Vec<String>) {
    refs.iter().cloned().partition(|r| !r.contains('@'))
}

/// The engine's image summary (`{id, user, repo_tags, size}`) → the CRI `Image`
/// message (R3h). Shared by `list_images` and `image_status` so the two cannot
/// drift. The layer size and the store reads live in the engine now; this maps
/// the composed fields onto the proto (splitting `repo_tags` into tags/digests
/// and the OCI `user` into `uid`/`username`).
fn to_cri_image(v: &serde_json::Value) -> Image {
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let user = v.get("user").and_then(|x| x.as_str()).unwrap_or_default();
    let (uid, username) = image_user(user);
    let refs: Vec<String> = v
        .get("repo_tags")
        .and_then(|x| x.as_array())
        .map(|a| a.iter().filter_map(|r| r.as_str().map(String::from)).collect())
        .unwrap_or_default();
    let (repo_tags, repo_digests) = partition_refs(&refs);
    let size = v.get("size").and_then(|x| x.as_u64()).unwrap_or(0);
    Image {
        id: id.clone(),
        repo_tags,
        repo_digests,
        size,
        uid,
        username,
        spec: Some(ImageSpec {
            image: id,
            ..Default::default()
        }),
        ..Default::default()
    }
}

// ---------------------------------------------------------------------------
// Server startup (unix socket), with both services.
// ---------------------------------------------------------------------------

/// Starts the CRI server on a **unix socket** (`addr` = path, or
/// `unix:///path`). Blocks the thread (creates the Tokio runtime).
/// `GET /metrics` — body in OpenMetrics format (what `prometheus-client`
/// produces), from the shared registry in `delonix-telemetry`.
async fn metrics_handler() -> impl axum::response::IntoResponse {
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )],
        delonix_telemetry::metrics::encode(),
    )
}

/// Serves the CRI socket with the node's capability ceiling in force. See
/// [`cap_ceiling`] — [`CapCeiling::node_default`] is the safe ceiling; [`CapCeiling::unlimited`]
/// lifts it and must be asked for. `runtime` is the engine's Linux-mechanism port
/// (R3h): the server reaches `is_rootless`, the cgroup/process primitives and the
/// capability dictionary through it, never through the adapter directly.
pub fn serve_blocking(
    base: PathBuf,
    images: Arc<dyn ImageOps>,
    containers: Arc<dyn ContainerOps>,
    runtime: Arc<dyn RuntimeOps>,
    network: Arc<dyn NetworkOps>,
    addr: &str,
    ceiling: CapCeiling,
) -> Result<(), delonix_model::Error> {
    let path = addr.strip_prefix("unix://").unwrap_or(addr).to_string();
    delonix_node::alloc_tuning::limit_malloc_arenas();
    let rt = tokio::runtime::Builder::new_multi_thread()
        // Two, not one per core. This serves the kubelet over a unix socket:
        // every request is I/O against the store and the network holder, and
        // the default (one worker per core, 32 on the host this was measured
        // on) buys no throughput while costing an 8 MiB stack and a malloc
        // arena each. `current_thread` is deliberately NOT used — a slow
        // request would then block the next one, and image pulls are slow.
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| delonix_model::Error::Runtime {
            context: "tokio",
            message: e.to_string(),
        })?;
    rt.block_on(async move {
        let _ = std::fs::remove_file(&path); // clean up an old socket
        let uds = tokio::net::UnixListener::bind(&path).map_err(|e| {
            delonix_model::Error::Runtime {
                context: "bind",
                message: e.to_string(),
            }
        })?;
        // SECURITY: same gap as `delonix-mgmt` (audit finding, 2026-07-21) — no
        // `SO_PEERCRED` check and no restrictive socket mode, so under a permissive
        // umask ANY local process could reach the kubelet-facing RuntimeService
        // (create/exec/rm containers). Mirror the holder's control socket: 0600 +
        // reject any peer that isn't this process's own euid, before it reaches a
        // single gRPC handler.
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        // SAFETY: geteuid() has no preconditions.
        let own_uid = unsafe { libc::geteuid() };
        // BUG FIXED: a transient `accept()` error (EMFILE/ENFILE/ECONNABORTED —
        // all self-clearing, `accept(2)`'s own man page says to just retry) used
        // to be KEPT in the stream (`Err(_) => true`) and handed straight to
        // tonic as an item; tonic treats an `Err` from its incoming stream as
        // FATAL and tears down the whole gRPC server — every in-flight kubelet
        // request killed by a hiccup unrelated to any of them. Now the error is
        // logged and the item is dropped (`filter_map` → `None`), so the accept
        // loop keeps serving instead of the process dying.
        let incoming =
            tokio_stream::wrappers::UnixListenerStream::new(uds).filter_map(move |conn| {
                match conn {
                    Ok(stream) if peer_uid(&stream) == Some(own_uid) => {
                        Some(Ok::<_, std::io::Error>(stream))
                    }
                    Ok(_) => None, // wrong-uid peer: silently dropped, as before
                    Err(e) => {
                        tracing::warn!(error = %e, "delonix-cri: accept() error (transient, retrying)");
                        None
                    }
                }
            });
        eprintln!("delonix-cri (CRI v1) listening on unix://{path}");

        // Streaming server (exec/attach/port-forward): HTTP/WebSocket on a
        // loopback port. The RPCs return URLs pointing here.
        let stream_listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|e| delonix_model::Error::Runtime {
                context: "bind-stream",
                message: e.to_string(),
            })?;
        let stream_port = stream_listener
            .local_addr()
            .map(|a| a.port())
            .map_err(|e| delonix_model::Error::Runtime {
                context: "stream-addr",
                message: e.to_string(),
            })?;
        let advertised = format!("http://127.0.0.1:{stream_port}");
        let streamer = streaming::Streamer::new(
            base.clone(),
            containers.clone(),
            runtime.clone(),
            advertised.clone(),
        );
        eprintln!("delonix-cri: streaming (exec/attach) at {advertised}");
        let app = streamer.clone().router();
        tokio::spawn(async move {
            let _ = axum::serve(stream_listener, app).await;
        });

        // Prometheus metrics (OPTIONAL): a dedicated HTTP listener, like
        // containerd/CRI-O. Off by default; enabled by `DELONIX_METRICS_ADDR`
        // (e.g. `0.0.0.0:9100`). Does not live on the gRPC socket — Prometheus
        // speaks HTTP.
        if let Some(maddr) = std::env::var_os("DELONIX_METRICS_ADDR") {
            let maddr = maddr.to_string_lossy().into_owned();
            tokio::spawn(async move {
                match tokio::net::TcpListener::bind(&maddr).await {
                    Ok(l) => {
                        tracing::info!(addr = %maddr, "delonix-cri: /metrics listening");
                        let app = axum::Router::new()
                            .route("/metrics", axum::routing::get(metrics_handler));
                        let _ = axum::serve(l, app).await;
                    }
                    Err(e) => {
                        tracing::error!(addr = %maddr, error = %e, "delonix-cri: /metrics bind failed")
                    }
                }
            });
        }

        // Announced on both channels the operator may be watching (the terminal,
        // for `delonix serve cri`; the journal, for `delonix-cri.service`): a
        // ceiling that is in force but invisible would be diagnosed as "the
        // runtime dropped my capabilities for no reason".
        let (banner, is_warning) = ceiling.startup_banner(runtime.as_ref());
        let line = if is_warning {
            format!("delonix-cri: WARNING: {banner}")
        } else {
            format!("delonix-cri: {banner}")
        };
        eprintln!("{line}");
        if is_warning {
            tracing::warn!("{line}");
        } else {
            tracing::info!("{line}");
        }

        let img = DelonixImage {
            base: base.clone(),
            engine: images,
        };
        let rtsvc =
            runtime_svc::DelonixRuntime::new(base, containers, runtime, network, streamer, ceiling);
        tonic::transport::Server::builder()
            .add_service(RuntimeServiceServer::new(rtsvc))
            .add_service(ImageServiceServer::new(img))
            .serve_with_incoming(incoming)
            .await
            .map_err(|e| delonix_model::Error::Runtime {
                context: "serve",
                message: e.to_string(),
            })
    })
}

/// A [`ContainerOps`] backed by a real on-disk `delonix-state` store, for the
/// crate's own tests only (R3h). The server never builds this — in production the
/// multicall hands in the engine's port — but the unit tests seed records with
/// `delonix-state` (a dev-dependency) and need a port that reads the SAME place
/// the engine's `ContainerService` does: `<base>/containers`. Keeping the backing
/// here, not in each test module, means one definition of "the store the port
/// speaks to".
#[cfg(test)]
pub(crate) mod test_ops {
    use super::ContainerOps;
    use std::path::PathBuf;

    pub(crate) struct StoreOps {
        base: PathBuf,
    }

    impl StoreOps {
        pub(crate) fn new(base: impl Into<PathBuf>) -> Self {
            Self { base: base.into() }
        }

        fn store(&self) -> Result<delonix_state::Store, delonix_model::Error> {
            delonix_state::Store::open(self.base.join("containers")).map_err(Into::into)
        }
    }

    impl ContainerOps for StoreOps {
        fn list(&self) -> Result<Vec<delonix_compute::Container>, delonix_model::Error> {
            self.store()?.list().map_err(Into::into)
        }
        fn load(
            &self,
            id_or_name: &str,
        ) -> Result<delonix_compute::Container, delonix_model::Error> {
            self.store()?.load(id_or_name).map_err(Into::into)
        }
        fn update(
            &self,
            id_or_name: &str,
            f: &mut dyn FnMut(&mut delonix_compute::Container) -> bool,
        ) -> Result<delonix_compute::Container, delonix_model::Error> {
            self.store()?
                .update(id_or_name, |c| f(c))
                .map_err(delonix_model::Error::from)
        }
    }

    /// A [`RuntimeOps`](super::RuntimeOps) backed by the real `delonix-linux` adapter
    /// (a dev-dependency), for the crate's own tests only (R3h). The server never
    /// builds this — in production the multicall hands in the engine's port — but
    /// the unit tests exercise the SAME Linux mechanism the engine delegates to, so
    /// the capability dictionary and the process/cgroup guards under test are the
    /// genuine ones, not a stand-in that could drift from them.
    pub(crate) struct LinuxPort;

    impl super::RuntimeOps for LinuxPort {
        fn is_rootless(&self) -> bool {
            delonix_linux::is_rootless()
        }
        fn safe_to_signal(&self, pid: i32, starttime: Option<u64>) -> bool {
            delonix_linux::safe_to_signal(pid, starttime)
        }
        fn leaf_controllers(&self) -> Vec<String> {
            delonix_linux::leaf_controllers()
        }
        fn reconcile_status(&self, c: &mut delonix_compute::Container) -> bool {
            delonix_linux::reconcile_status(c)
        }
        fn oom_killed_reason(&self) -> &'static str {
            delonix_linux::OOM_KILLED
        }
        fn seccomp_validate(&self, json: &str) -> Result<(), String> {
            delonix_linux::seccomp_profile::parse(json).map(|_| ())
        }
        fn update_kube_resources(
            &self,
            c: &delonix_compute::Container,
            update: &super::ResourceUpdateSpec<'_>,
        ) -> Result<bool, delonix_model::Error> {
            let u = delonix_linux::ResourceUpdate {
                memory: update.memory,
                cpus: update.cpus,
                cpu_weight: update.cpu_weight,
                cpuset: update.cpuset,
                cpuset_mems: update.cpuset_mems,
                hugepage_limits: update.hugepage_limits,
                unified: update.unified,
                oom_score_adj: update.oom_score_adj,
            };
            delonix_linux::update_kube_resources(c, &u)
                .map(|lu| lu != delonix_linux::LimitUpdate::NotEnforced)
                .map_err(delonix_model::Error::from)
        }
        fn all_caps_mask(&self) -> u64 {
            delonix_linux::capabilities::all_caps_mask()
        }
        fn cap_bit(&self, name: &str) -> Option<u8> {
            delonix_linux::capabilities::cap_num(name)
        }
        fn default_kept_mask(&self) -> u64 {
            delonix_linux::capabilities::default_kept_mask()
        }
        fn names_from_mask(&self, mask: u64) -> Vec<&'static str> {
            delonix_linux::capabilities::names_from_mask(mask)
        }
        fn resolve_cap_keep(&self, cap_drop: &[String], cap_add: &[String]) -> u64 {
            delonix_linux::capabilities::resolve_cap_keep(cap_drop, cap_add)
        }
    }

    /// A [`NetworkOps`](super::NetworkOps) backed by the real `delonix-sdn` adapter
    /// (a dev-dependency), for the crate's own tests only (R3h-D). Mirrors what the
    /// multicall's port does — the leaf SDN/CNI mechanism, with the CNI chain as the
    /// opaque JSON the interface persists — so the tests exercise the genuine infra
    /// and CNI behaviour, not a stand-in that could drift from the adapter.
    pub(crate) struct NetPort;

    fn to_sdn_ports(ports: &[super::CniPort]) -> Vec<delonix_sdn::cni::PortMapping> {
        ports
            .iter()
            .map(|p| delonix_sdn::cni::PortMapping {
                host_port: p.host_port,
                container_port: p.container_port,
                protocol: p.protocol.clone(),
                host_ip: p.host_ip.clone(),
            })
            .collect()
    }

    fn conf_to_cni(conf: &delonix_sdn::cni::NetConfList) -> super::CniConf {
        super::CniConf {
            name: conf.name.clone(),
            publishes_host_ports: delonix_sdn::cni::publishes_host_ports(conf),
            json: serde_json::to_string(conf).unwrap_or_default(),
        }
    }

    impl super::NetworkOps for NetPort {
        fn infra_status(&self) -> super::InfraStatus {
            let s = delonix_sdn::infra::status();
            super::InfraStatus {
                up: s.up,
                refcount: s.refcount,
                holder_pid: s.holder_pid,
                slirp_pid: s.slirp_pid,
            }
        }
        fn root_network_ready(&self) -> Result<(), (String, String)> {
            let dirs = delonix_sdn::cni::plugin_dirs();
            let r = delonix_sdn::cni::readiness(
                std::path::Path::new(delonix_sdn::cni::DEFAULT_CONF_DIR),
                &dirs,
            );
            match r.not_ready(&dirs) {
                None => Ok(()),
                Some((reason, message)) => Err((reason.to_string(), message)),
            }
        }
        fn attached_refs_in(&self, base: &std::path::Path) -> Vec<String> {
            delonix_sdn::infra::attached_refs_in(base)
        }
        fn orphan_ref_count(&self, attached: &[String], live: &[String]) -> usize {
            let live: std::collections::HashSet<String> = live.iter().cloned().collect();
            delonix_sdn::infra::orphan_refs(attached, &live).len()
        }
        fn cni_enabled(&self) -> bool {
            delonix_sdn::cni::enabled_conf().is_some()
        }
        fn cni_resolve_conf(&self, rootless: bool) -> super::CniConfResult {
            if rootless {
                match delonix_sdn::cni::enabled_conf() {
                    Some(conf) => super::CniConfResult::Ready(conf_to_cni(&conf)),
                    None => super::CniConfResult::Disabled,
                }
            } else {
                let dirs = delonix_sdn::cni::plugin_dirs();
                match delonix_sdn::cni::readiness(
                    std::path::Path::new(delonix_sdn::cni::DEFAULT_CONF_DIR),
                    &dirs,
                ) {
                    delonix_sdn::cni::Readiness::Ready(conf) => {
                        super::CniConfResult::Ready(conf_to_cni(&conf))
                    }
                    other => {
                        let (reason, message) = other.not_ready(&dirs).unwrap_or_default();
                        super::CniConfResult::NotReady {
                            reason: reason.to_string(),
                            message,
                        }
                    }
                }
            }
        }
        fn cni_with_port_mappings(&self, conf_json: &str, ports: &[super::CniPort]) -> String {
            match delonix_sdn::cni::parse_config(conf_json) {
                Ok(conf) => {
                    let mapped = delonix_sdn::cni::with_port_mappings(&conf, &to_sdn_ports(ports));
                    serde_json::to_string(&mapped).unwrap_or_else(|_| conf_json.to_string())
                }
                Err(_) => conf_json.to_string(),
            }
        }
        fn cni_attach(
            &self,
            pod: &str,
            rootless: bool,
            conf_json: &str,
            cni_args: Option<&str>,
        ) -> Result<super::CniAttached, String> {
            if rootless {
                let (netns, cidr) =
                    delonix_sdn::infra::cni_attach_container(pod, conf_json, cni_args)
                        .map_err(|e| e.to_string())?;
                Ok(super::CniAttached {
                    ip: cidr,
                    netns_path: netns,
                })
            } else {
                let conf =
                    delonix_sdn::cni::parse_config(conf_json).map_err(|e| e.to_string())?;
                let cidr = delonix_sdn::cni::attach_named_netns(
                    &conf,
                    pod,
                    pod,
                    delonix_sdn::cni::DEFAULT_IFNAME,
                    cni_args,
                )
                .map_err(|e| e.to_string())?;
                Ok(super::CniAttached {
                    ip: cidr,
                    netns_path: delonix_sdn::cni::named_netns_path(pod),
                })
            }
        }
        fn cni_set_netns_sysctls(
            &self,
            netns_path: &str,
            sysctls: &[(String, String)],
        ) -> Result<(), String> {
            delonix_sdn::cni::set_netns_sysctls(netns_path, sysctls).map_err(|e| e.to_string())
        }
        fn cni_detach(&self, pod: &str, rootless: bool, conf_json: &str) {
            if rootless {
                if let Some(conf) = delonix_sdn::cni::enabled_conf() {
                    let cj = serde_json::to_string(&conf).unwrap_or_default();
                    let _ = delonix_sdn::infra::cni_detach_container(pod, &cj);
                }
            } else {
                let conf = delonix_sdn::cni::parse_config(conf_json).ok();
                delonix_sdn::cni::detach_named_netns(
                    conf.as_ref(),
                    pod,
                    pod,
                    delonix_sdn::cni::DEFAULT_IFNAME,
                );
            }
        }
        fn container_ip(&self, name: &str) -> String {
            delonix_sdn::infra::container_ip(name)
        }
        fn publishable_proto(&self, name: &str) -> Option<&'static str> {
            match delonix_sdn::Proto::parse(name).ok()? {
                delonix_sdn::Proto::Tcp => Some("tcp"),
                delonix_sdn::Proto::Udp => Some("udp"),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `image_user`: maps the OCI `USER` to the CRI `(uid, username)` pair. Each
    // case is a `test-image-user-*` image the `critest` "image status … should
    // not have Uid|Username empty" spec pulls, with the value it asserts.
    #[test]
    fn image_user_maps_user_to_uid_or_username() {
        // No USER → root, uid 0, no name (the spec wants ONE of the two set).
        assert_eq!(
            image_user(""),
            (Some(Int64Value { value: 0 }), String::new())
        );
        assert_eq!(
            image_user("  "),
            (Some(Int64Value { value: 0 }), String::new())
        );
        // Numeric uid → uid set, username empty.
        assert_eq!(
            image_user("1002"),
            (Some(Int64Value { value: 1002 }), String::new())
        );
        // Numeric uid:gid → the gid is dropped; uid only.
        assert_eq!(
            image_user("1003:1003"),
            (Some(Int64Value { value: 1003 }), String::new())
        );
        // Name → username set, uid nil.
        assert_eq!(image_user("www-data"), (None, "www-data".to_string()));
    }

    // The `name:group` form is the regression: it returned the whole string
    // (`"www-data:1004"`), and the spec compares `username` to the user name
    // ALONE. The group must be dropped, exactly as for the numeric case.
    #[test]
    fn image_user_drops_group_from_name() {
        assert_eq!(image_user("www-data:1004"), (None, "www-data".to_string()));
        assert_eq!(
            image_user("www-data:www-data"),
            (None, "www-data".to_string())
        );
    }

    // `partition_refs`: a digest reference (`name@sha256:…`) belongs in
    // `repo_digests`, a tag reference (`name:tag`) in `repo_tags`. A pull by
    // digest must come back with `repo_tags` EMPTY (conformance "public image
    // with digest should be pulled and removed").
    #[test]
    fn partition_refs_splits_digests_from_tags() {
        let (tags, digests) =
            partition_refs(&["gcr.io/x/test-image-digest@sha256:9700f9a2f5bf".to_string()]);
        assert!(tags.is_empty(), "a digest pull must not populate repo_tags");
        assert_eq!(
            digests,
            vec!["gcr.io/x/test-image-digest@sha256:9700f9a2f5bf"]
        );

        let (tags, digests) = partition_refs(&["docker.io/library/nginx:latest".to_string()]);
        assert_eq!(tags, vec!["docker.io/library/nginx:latest"]);
        assert!(digests.is_empty());

        // Mixed list keeps each reference on its correct side, order preserved.
        let (tags, digests) =
            partition_refs(&["repo:tag".to_string(), "repo@sha256:abc".to_string()]);
        assert_eq!(tags, vec!["repo:tag"]);
        assert_eq!(digests, vec!["repo@sha256:abc"]);
    }
}
