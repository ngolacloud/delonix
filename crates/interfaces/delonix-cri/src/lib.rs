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
/// [`cap_ceiling`] — [`CapCeiling::default`] is the safe ceiling; [`CapCeiling::unlimited`]
/// lifts it and must be asked for.
pub fn serve_blocking(
    base: PathBuf,
    images: Arc<dyn ImageOps>,
    containers: Arc<dyn ContainerOps>,
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
        let streamer =
            streaming::Streamer::new(base.clone(), containers.clone(), advertised.clone());
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
        let (banner, is_warning) = ceiling.startup_banner();
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
        let rtsvc = runtime_svc::DelonixRuntime::new(base, containers, streamer, ceiling);
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
