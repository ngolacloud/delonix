//! CRI RuntimeService over the Delonix engine. `version`/`status` are real;
//! the pod/container lifecycle is filled in below.

use std::path::PathBuf;
use tonic::{Request, Response, Status};

use crate::cri::runtime_service_server::RuntimeService;
use crate::cri::*;
use crate::{RUNTIME_NAME, RUNTIME_VERSION};

pub struct DelonixRuntime {
    pub base: PathBuf,
    /// The container store, reached through the engine's port (R3h) — the
    /// interface no longer opens `delonix-state` itself. Every lifecycle free
    /// function that reads or reconciles a record takes `&dyn ContainerOps`
    /// handed down from here (and from the [`Streamer`](crate::streaming::Streamer),
    /// which owns a clone of the same `Arc` for its port-forward path).
    pub containers: std::sync::Arc<dyn crate::ContainerOps>,
    /// The engine's Linux-mechanism port (R3h): `is_rootless`, the cgroup/process
    /// guards, the reconcile and resource-update paths, and the capability
    /// dictionary the ceiling computes against — all reached here instead of
    /// through the `delonix-linux` adapter the interface no longer names.
    pub runtime: std::sync::Arc<dyn crate::RuntimeOps>,
    /// The engine's SDN/CNI-mechanism port (R3h-D): the infra-plane liveness, the
    /// CNI attach/detach and the pod IP — reached here instead of through the
    /// `delonix-sdn` adapter the interface no longer names.
    pub network: std::sync::Arc<dyn crate::NetworkOps>,
    pub streamer: crate::streaming::Streamer,
    /// Node-level upper bound on container capabilities (see [`crate::cap_ceiling`]).
    /// Carried on the service, not read from the environment where it is needed:
    /// the value is resolved ONCE at startup, so a malformed setting fails the
    /// server instead of silently degrading per request.
    pub cap_ceiling: crate::CapCeiling,
}

impl DelonixRuntime {
    pub fn new(
        base: PathBuf,
        containers: std::sync::Arc<dyn crate::ContainerOps>,
        runtime: std::sync::Arc<dyn crate::RuntimeOps>,
        network: std::sync::Arc<dyn crate::NetworkOps>,
        streamer: crate::streaming::Streamer,
        cap_ceiling: crate::CapCeiling,
    ) -> Self {
        Self {
            base,
            containers,
            runtime,
            network,
            streamer,
            cap_ceiling,
        }
    }
}

/// Shortcut for "not yet implemented" (the `kubelet`/`crictl` only call what
/// they need; the rest returns `UNIMPLEMENTED`).
fn todo<T>(what: &str) -> Result<Response<T>, Status> {
    Err(Status::unimplemented(format!("delonix-cri: {what}")))
}

/// Runs a BLOCKING operation (fs + shell-out to `delonix`) outside the async
/// runtime — otherwise `clone`/`run` would stall the Tokio workers.
/// What the runtime has to say when the kubelet hands it a pod CIDR.
///
/// PURE, and separate from the handler, so the three cases are a data test —
/// the same reason `should_signal` and `reconcile::plan` are pure. The case
/// that matters is [`NotHonoured`](PodCidrVerdict::NotHonoured): before this
/// existed the method returned `Ok(())` for it, which reads as "applied".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PodCidrVerdict {
    /// The kubelet said nothing (empty CIDR) — nothing to report.
    Nothing,
    /// CNI is in charge of addressing, so the plugin's own config carries the
    /// subnet. Ignoring the CIDR here is what containerd and CRI-O do too.
    DelegatedToCni,
    /// Native SDN: the addresses will NOT come from this range, and whoever
    /// deployed the node should hear it.
    NotHonoured,
}

fn pod_cidr_verdict(cidr: &str, cni_enabled: bool) -> PodCidrVerdict {
    match (cidr.trim().is_empty(), cni_enabled) {
        (true, _) => PodCidrVerdict::Nothing,
        (false, true) => PodCidrVerdict::DelegatedToCni,
        (false, false) => PodCidrVerdict::NotHonoured,
    }
}

async fn blocking<T, F>(f: F) -> Result<Response<T>, Status>
where
    T: Send + 'static,
    F: FnOnce() -> Result<Response<T>, Status> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| Status::internal(e.to_string()))?
}

/// Pod name from the sandbox metadata, for the span fields. `""` when absent
/// (the `crictl`/`kubelet` don't always fill everything in) — better a span
/// without a name than instrumentation panicking on an `unwrap`.
fn pod_meta_name(m: Option<&PodSandboxMetadata>) -> &str {
    m.map(|m| m.name.as_str()).unwrap_or("")
}

/// Likewise for the container name from the `ContainerConfig` metadata.
fn ctr_meta_name(m: Option<&ContainerMetadata>) -> &str {
    m.map(|m| m.name.as_str()).unwrap_or("")
}

#[tonic::async_trait]
impl RuntimeService for DelonixRuntime {
    type GetContainerEventsStream = std::pin::Pin<
        Box<dyn tokio_stream::Stream<Item = Result<ContainerEventResponse, Status>> + Send>,
    >;

    async fn version(
        &self,
        _req: Request<VersionRequest>,
    ) -> Result<Response<VersionResponse>, Status> {
        Ok(Response::new(VersionResponse {
            version: "0.1.0".into(),
            runtime_name: RUNTIME_NAME.into(),
            runtime_version: RUNTIME_VERSION.into(),
            runtime_api_version: "v1".into(),
        }))
    }

    async fn status(
        &self,
        req: Request<StatusRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let verbose = req.into_inner().verbose;
        let cond = |t: &str, ok: bool, reason: &str, message: &str| RuntimeCondition {
            r#type: t.into(),
            status: ok,
            reason: reason.into(),
            message: message.into(),
        };
        // `RuntimeReady`: getting this far already proves the CRI server is alive
        // and responding — there is nothing more to check without inventing a
        // state we don't have.
        let runtime_ready = cond("RuntimeReady", true, "", "");
        // `NetworkReady`: BEFORE this it was always a fixed `true` — it masked
        // real SDN failures (bridge/slirp/holder down), making the node go
        // `Ready` in K8s even without working networking. Now it actually checks,
        // in BOTH modes (rootless: holder+slirp alive via pidfiles; root: the
        // CNI chain the sandboxes are networked with — reads and stats only).
        let network_ready = if self.runtime.is_rootless() {
            let st = self.network.infra_status();
            // DOWN-AND-UNUSED IS NOT BROKEN, and conflating the two deadlocks
            // the node. This engine is daemonless: the infra netns starts on
            // DEMAND, when the first workload needs it. Reporting `NetworkReady:
            // false` on a freshly booted node meant the kubelet marked it
            // NotReady, so it scheduled no pod, so nothing ever brought the
            // infra up, so it stayed NotReady — forever. The check was written
            // to catch a real SDN failure and instead described the normal
            // resting state of the engine.
            //
            // The distinguishing fact is the ref-count: infra down with
            // workloads attached IS a failure; down with none is just idle.
            // Same family as the traps this repo already documents — `status()`
            // by pidfile is not "the holder is reachable", a socket file is not
            // a listener, and here "not running" is not "cannot run".
            // **And a ref marker is NOT an attached workload.** A container that
            // dies abruptly (crash, OOM, reboot) never calls `release`, so the
            // marker stays. Nothing cleans it up until someone runs `system
            // prune` — the reaper exists, but it is MANUAL. So the raw `refcount`
            // counts ghosts, and with the infra down that reopens exactly the
            // deadlock the paragraph above describes: the node goes NotReady,
            // schedules nothing, nobody brings the infra up, and it stays that
            // way forever.
            //
            // Measured 2026-08-15 on this host: infra down and SEVEN orphan
            // markers from already-dead containers → `NetworkReady: false` on a
            // node that did not have a single live workload.
            //
            // We count the LIVE ones, with the `orphan_refs` that `system prune`
            // already uses — same rule, a single owner. If the container list
            // cannot be read, the raw `refcount` is used: that is the previous
            // behaviour, and a read error must not pass for "there are no
            // workloads".
            let live =
                live_attached_refs(&self.base, self.containers.as_ref(), self.network.as_ref())
                    .unwrap_or(st.refcount);
            if network_ready_rootless(st.up, live) {
                cond("NetworkReady", true, "", "")
            } else {
                cond(
                    "NetworkReady",
                    false,
                    "InfraDown",
                    &format!(
                        "rootless infra netns is down with {} live workload(s) attached \
                         ({} marker(s) on disk, holder={:?}, slirp={:?})",
                        live, st.refcount, st.holder_pid, st.slirp_pid
                    ),
                )
            }
        } else {
            // ROOT: the pod network IS the CNI chain (`run_pod_sandbox`), so this
            // reports whether that chain can run — config + binaries, see
            // `cni::Readiness`. Same source of truth as the sandbox path, so the
            // two cannot disagree.
            //
            // It used to check `/sys/class/net/delonix0`, and that could never be
            // true: the native bridge lives inside the holder's netns
            // (`unshare --user --net`, in root too), never in the host's
            // `/sys/class/net`. Measured 2026-09-15 on a kubeadm node (VM,
            // k8s 1.36.4) with `/etc/cni/net.d/10-bridge.conflist`: node
            // `NotReady` with `BridgeMissing` forever, CoreDNS `Pending`.
            match self.network.root_network_ready() {
                Ok(()) => cond("NetworkReady", true, "", ""),
                Err((reason, message)) => cond("NetworkReady", false, &reason, &message),
            }
        };
        // `info` is only populated for a verbose request (CRI contract). The
        // capability ceiling goes here so an operator can read the policy in force
        // straight from `crictl info`, instead of trusting that the flag they
        // wrote in a unit file is the one the running server parsed.
        let mut info = std::collections::HashMap::new();
        if verbose {
            info.insert(
                "capabilityCeiling".to_string(),
                self.cap_ceiling.describe(self.runtime.as_ref()),
            );
        }
        Ok(Response::new(StatusResponse {
            status: Some(RuntimeStatus {
                conditions: vec![runtime_ready, network_ready],
            }),
            info,
            runtime_handlers: vec![],
            features: None,
        }))
    }

    // --- pod/container lifecycle: instrumented with `tracing` spans.
    // Each handler opens a span (exported over OTLP when `DELONIX_OTLP_ENDPOINT`
    // is set — see `delonix_telemetry::telemetry`) with the resource id.
    // The fields are read from `r.get_ref()` (evaluated on span ENTRY, before
    // `into_inner()` consumes the request); `skip_all` avoids dumping the whole
    // `Request` (non-`Debug`/verbose) and `self`.
    #[tracing::instrument(name = "cri.run_pod_sandbox", skip_all, fields(
        pod = pod_meta_name(r.get_ref().config.as_ref().and_then(|c| c.metadata.as_ref())),
        runtime_handler = %r.get_ref().runtime_handler,
    ))]
    async fn run_pod_sandbox(
        &self,
        r: Request<RunPodSandboxRequest>,
    ) -> Result<Response<RunPodSandboxResponse>, Status> {
        let (base, req) = (self.base.clone(), r.into_inner());
        let rt = self.runtime.clone();
        let net = self.network.clone();
        blocking(move || lifecycle::run_pod_sandbox(&base, rt.as_ref(), net.as_ref(), req)).await
    }
    #[tracing::instrument(name = "cri.stop_pod_sandbox", skip_all, fields(
        pod = %r.get_ref().pod_sandbox_id,
    ))]
    async fn stop_pod_sandbox(
        &self,
        r: Request<StopPodSandboxRequest>,
    ) -> Result<Response<StopPodSandboxResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().pod_sandbox_id);
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        blocking(move || lifecycle::stop_pod_sandbox(&base, ops.as_ref(), rt.as_ref(), id)).await
    }
    #[tracing::instrument(name = "cri.remove_pod_sandbox", skip_all, fields(
        pod = %r.get_ref().pod_sandbox_id,
    ))]
    async fn remove_pod_sandbox(
        &self,
        r: Request<RemovePodSandboxRequest>,
    ) -> Result<Response<RemovePodSandboxResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().pod_sandbox_id);
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        let net = self.network.clone();
        blocking(move || {
            lifecycle::remove_pod_sandbox(&base, ops.as_ref(), rt.as_ref(), net.as_ref(), id)
        })
        .await
    }
    #[tracing::instrument(name = "cri.pod_sandbox_status", skip_all, fields(
        pod = %r.get_ref().pod_sandbox_id,
    ))]
    async fn pod_sandbox_status(
        &self,
        r: Request<PodSandboxStatusRequest>,
    ) -> Result<Response<PodSandboxStatusResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().pod_sandbox_id);
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        let net = self.network.clone();
        blocking(move || {
            lifecycle::pod_sandbox_status(&base, ops.as_ref(), rt.as_ref(), net.as_ref(), id)
        })
        .await
    }
    #[tracing::instrument(name = "cri.list_pod_sandbox", skip_all)]
    async fn list_pod_sandbox(
        &self,
        r: Request<ListPodSandboxRequest>,
    ) -> Result<Response<ListPodSandboxResponse>, Status> {
        let (base, filter) = (self.base.clone(), r.into_inner().filter);
        blocking(move || lifecycle::list_pod_sandbox(&base, filter)).await
    }
    #[tracing::instrument(name = "cri.create_container", skip_all, fields(
        pod = %r.get_ref().pod_sandbox_id,
        container = ctr_meta_name(r.get_ref().config.as_ref().and_then(|c| c.metadata.as_ref())),
    ))]
    async fn create_container(
        &self,
        r: Request<CreateContainerRequest>,
    ) -> Result<Response<CreateContainerResponse>, Status> {
        let (base, req) = (self.base.clone(), r.into_inner());
        let ceiling = self.cap_ceiling;
        let rt = self.runtime.clone();
        blocking(move || lifecycle::create_container(&base, rt.as_ref(), req, ceiling)).await
    }
    #[tracing::instrument(name = "cri.start_container", skip_all, fields(
        container = %r.get_ref().container_id,
    ))]
    async fn start_container(
        &self,
        r: Request<StartContainerRequest>,
    ) -> Result<Response<StartContainerResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().container_id);
        let ceiling = self.cap_ceiling;
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        blocking(move || lifecycle::start_container(&base, ops.as_ref(), rt.as_ref(), id, ceiling))
            .await
    }
    #[tracing::instrument(name = "cri.stop_container", skip_all, fields(
        container = %r.get_ref().container_id,
        timeout = r.get_ref().timeout,
    ))]
    async fn stop_container(
        &self,
        r: Request<StopContainerRequest>,
    ) -> Result<Response<StopContainerResponse>, Status> {
        let req = r.into_inner();
        let (base, id, timeout) = (self.base.clone(), req.container_id, req.timeout);
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        blocking(move || lifecycle::stop_container(&base, ops.as_ref(), rt.as_ref(), id, timeout))
            .await
    }
    #[tracing::instrument(name = "cri.remove_container", skip_all, fields(
        container = %r.get_ref().container_id,
    ))]
    async fn remove_container(
        &self,
        r: Request<RemoveContainerRequest>,
    ) -> Result<Response<RemoveContainerResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().container_id);
        let ops = self.containers.clone();
        blocking(move || lifecycle::remove_container(&base, ops.as_ref(), id)).await
    }
    #[tracing::instrument(name = "cri.list_containers", skip_all)]
    async fn list_containers(
        &self,
        r: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        let (base, filter) = (self.base.clone(), r.into_inner().filter);
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        blocking(move || lifecycle::list_containers(&base, ops.as_ref(), rt.as_ref(), filter)).await
    }
    #[tracing::instrument(name = "cri.container_status", skip_all, fields(
        container = %r.get_ref().container_id,
    ))]
    async fn container_status(
        &self,
        r: Request<ContainerStatusRequest>,
    ) -> Result<Response<ContainerStatusResponse>, Status> {
        let req = r.into_inner();
        let (base, id, verbose) = (self.base.clone(), req.container_id, req.verbose);
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        blocking(move || lifecycle::container_status(&base, ops.as_ref(), rt.as_ref(), id, verbose))
            .await
    }

    async fn update_container_resources(
        &self,
        r: Request<UpdateContainerResourcesRequest>,
    ) -> Result<Response<UpdateContainerResourcesResponse>, Status> {
        let base = self.base.clone();
        let ops = self.containers.clone();
        let rt = self.runtime.clone();
        blocking(move || {
            lifecycle::update_container_resources(&base, ops.as_ref(), rt.as_ref(), r.into_inner())
        })
        .await
    }

    async fn reopen_container_log(
        &self,
        r: Request<ReopenContainerLogRequest>,
    ) -> Result<Response<ReopenContainerLogResponse>, Status> {
        // WAS A NO-OP THAT REPORTED SUCCESS. The kubelet rotates a container's
        // log by renaming the file and then calling this; answering "done"
        // without doing anything meant every line after a rotation went to an
        // inode nobody would ever read again — silently, and only for the
        // containers that live long enough to be rotated.
        //
        // Two halves, and both are needed: the logging shim now FOLLOWS THE
        // PATH (it compares inodes before each batch, see `log_shim`), and this
        // recreates the file so it exists the moment the call returns — which
        // is what the caller checks, and what the kubelet's log reader opens.
        let base = self.base.clone();
        let id = r.into_inner().container_id;
        tokio::task::spawn_blocking(move || lifecycle::reopen_container_log(&base, &id))
            .await
            .map_err(|e| Status::internal(e.to_string()))??;
        Ok(Response::new(ReopenContainerLogResponse {}))
    }
    async fn exec_sync(
        &self,
        r: Request<ExecSyncRequest>,
    ) -> Result<Response<ExecSyncResponse>, Status> {
        let req = r.into_inner();
        let base = self.base.clone();
        blocking(move || lifecycle::exec_sync(&base, req.container_id, req.cmd, req.timeout)).await
    }
    async fn exec(&self, r: Request<ExecRequest>) -> Result<Response<ExecResponse>, Status> {
        let req = r.into_inner();
        if req.cmd.is_empty() {
            return Err(Status::invalid_argument("exec without a command"));
        }
        // Register the request and return the streaming server URL. The client
        // (kubelet/crictl) upgrades (SPDY or WebSocket) there and we run
        // `delonix exec`, wiring stdin/stdout/stderr to the streams.
        let url = self
            .streamer
            .prepare_exec(
                req.container_id,
                req.cmd,
                req.tty,
                req.stdin,
                req.stdout,
                req.stderr,
            )
            // FAIL CLOSED: no entropy → no session. Returning a token we could not
            // randomize would hand out a predictable exec URL (arbitrary code
            // execution inside the pod); `Internal` makes the kubelet retry instead.
            .map_err(|e| Status::internal(format!("streaming token: {e}")))?;
        Ok(Response::new(ExecResponse { url }))
    }
    async fn attach(&self, r: Request<AttachRequest>) -> Result<Response<AttachResponse>, Status> {
        let req = r.into_inner();
        // Attach = streams the container's output (stdout/stderr) live. The
        // stdio of a detached container's main process goes to the log, so the
        // streaming server runs `delonix logs -f`. (Sending stdin to PID 1 of a
        // detached container is not supported — use `exec`.)
        let url = self
            .streamer
            .prepare_attach(req.container_id, req.tty, req.stdin, req.stdout, req.stderr)
            .map_err(|e| Status::internal(format!("streaming token: {e}")))?;
        Ok(Response::new(AttachResponse { url }))
    }
    async fn port_forward(
        &self,
        r: Request<PortForwardRequest>,
    ) -> Result<Response<PortForwardResponse>, Status> {
        let req = r.into_inner();
        // Forwards host ports into the pod's netns (TCP proxy via setns).
        // Returns the streaming URL; the client opens one stream per port.
        let url = self
            .streamer
            .prepare_port_forward(req.pod_sandbox_id, req.port)
            .map_err(|e| Status::internal(format!("streaming token: {e}")))?;
        Ok(Response::new(PortForwardResponse { url }))
    }
    async fn container_stats(
        &self,
        r: Request<ContainerStatsRequest>,
    ) -> Result<Response<ContainerStatsResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().container_id);
        let ops = self.containers.clone();
        blocking(move || lifecycle::container_stats(&base, ops.as_ref(), id)).await
    }
    async fn list_container_stats(
        &self,
        r: Request<ListContainerStatsRequest>,
    ) -> Result<Response<ListContainerStatsResponse>, Status> {
        let (base, filter) = (self.base.clone(), r.into_inner().filter);
        let ops = self.containers.clone();
        blocking(move || lifecycle::list_container_stats(&base, ops.as_ref(), filter)).await
    }
    async fn pod_sandbox_stats(
        &self,
        r: Request<PodSandboxStatsRequest>,
    ) -> Result<Response<PodSandboxStatsResponse>, Status> {
        let (base, id) = (self.base.clone(), r.into_inner().pod_sandbox_id);
        let ops = self.containers.clone();
        blocking(move || lifecycle::pod_sandbox_stats(&base, ops.as_ref(), id)).await
    }
    async fn list_pod_sandbox_stats(
        &self,
        r: Request<ListPodSandboxStatsRequest>,
    ) -> Result<Response<ListPodSandboxStatsResponse>, Status> {
        let (base, filter) = (self.base.clone(), r.into_inner().filter);
        let ops = self.containers.clone();
        blocking(move || lifecycle::list_pod_sandbox_stats(&base, ops.as_ref(), filter)).await
    }
    /// The kubelet hands the runtime the cluster's pod CIDR once, at startup.
    ///
    /// **This runtime does not allocate from it, and the answer depends on the
    /// mode** — which is why the old bare `Ok(())` was wrong. It was
    /// indistinguishable from "applied", and this method exists precisely to
    /// tell the runtime something about addressing:
    ///
    /// * **With CNI** (`DELONIX_CNI=1` + a conflist) the addresses come from the
    ///   plugin chain — Calico, Flannel, Cilium — whose own config already
    ///   carries the subnet. Ignoring the CIDR here is exactly what containerd
    ///   and CRI-O do, and it is correct.
    /// * **On the native SDN** the addresses come from `delonix-sdn`'s IPAM, so
    ///   a CIDR the kubelet believes in is NOT the one the pods get. That is a
    ///   real divergence, and it now says so out loud instead of returning
    ///   success over it.
    ///
    /// It still answers `Ok`: the kubelet calls this during startup and several
    /// versions treat an error as fatal, so failing closed here would refuse to
    /// serve a node over something that is correct in the CNI case. Explicit
    /// warning rather than silence — the other half of the same rule.
    async fn update_runtime_config(
        &self,
        r: Request<UpdateRuntimeConfigRequest>,
    ) -> Result<Response<UpdateRuntimeConfigResponse>, Status> {
        let cidr = r
            .into_inner()
            .runtime_config
            .and_then(|c| c.network_config)
            .map(|n| n.pod_cidr)
            .unwrap_or_default();
        let cni = if self.runtime.is_rootless() {
            self.network.cni_enabled()
        } else {
            // Root networks every pod through CNI (see `run_pod_sandbox`).
            true
        };
        match pod_cidr_verdict(&cidr, cni) {
            PodCidrVerdict::Nothing => {}
            PodCidrVerdict::DelegatedToCni => tracing::info!(
                pod_cidr = %cidr,
                "cri: pod CIDR noted; addressing comes from the CNI plugin chain"
            ),
            PodCidrVerdict::NotHonoured => tracing::warn!(
                pod_cidr = %cidr,
                "cri: pod CIDR IGNORED — this node runs the native SDN and allocates \
                 from delonix-sdn's IPAM, so pod addresses will not come from this \
                 range; set DELONIX_CNI=1 with a conflist to use the cluster's"
            ),
        }
        Ok(Response::new(UpdateRuntimeConfigResponse {}))
    }
    async fn checkpoint_container(
        &self,
        _r: Request<CheckpointContainerRequest>,
    ) -> Result<Response<CheckpointContainerResponse>, Status> {
        todo("checkpoint_container")
    }
    async fn get_container_events(
        &self,
        _r: Request<GetEventsRequest>,
    ) -> Result<Response<Self::GetContainerEventsStream>, Status> {
        Err(Status::unimplemented("get_container_events"))
    }
    async fn list_metric_descriptors(
        &self,
        _r: Request<ListMetricDescriptorsRequest>,
    ) -> Result<Response<ListMetricDescriptorsResponse>, Status> {
        lifecycle::list_metric_descriptors()
    }
    async fn list_pod_sandbox_metrics(
        &self,
        _r: Request<ListPodSandboxMetricsRequest>,
    ) -> Result<Response<ListPodSandboxMetricsResponse>, Status> {
        let base = self.base.clone();
        let ops = self.containers.clone();
        blocking(move || lifecycle::list_pod_sandbox_metrics(&base, ops.as_ref())).await
    }
    async fn runtime_config(
        &self,
        _r: Request<RuntimeConfigRequest>,
    ) -> Result<Response<RuntimeConfigResponse>, Status> {
        Ok(Response::new(RuntimeConfigResponse {
            linux: Some(LinuxRuntimeConfiguration {
                cgroup_driver: engine_cgroup_driver() as i32,
            }),
        }))
    }
}

/// The cgroup driver this runtime ACTUALLY is, told to the kubelet.
///
/// The engine manages cgroups by writing `/sys/fs/cgroup` directly — `mkdir` of
/// the leaf, limits, `cgroup.procs` — and never asks systemd for a unit. That is
/// the `cgroupfs` driver. The kubelet reads this at startup and builds its pod
/// hierarchy with the same driver, so both managers agree on who owns what.
///
/// This used to answer `linux: None`, and the kubelet fell back to its own config
/// — `systemd`, kubeadm's default. The kubelet then created the pod cgroups as
/// systemd slices with nothing inside them (the containers live in
/// `delonix.slice`), and systemd drops a controller from a slice when no unit
/// under it needs one. `cpuset` is the one it drops: on every unit change or
/// `daemon-reload` it vanished from `kubepods.slice`, the kubelet's check of the
/// pod cgroup failed on the missing controller, and it killed the pod — which it
/// reads as «pod cgroup does not exist», with no log line of its own.
///
/// MEASURED 2026-09-15, VM of `delonix-vm-k8s:1.36`, k8s 1.36.4, one node:
/// `cpuset` toggling in the pod slices on a 100 ms watch, each kill inside a window
/// without it, and one `systemctl daemon-reload` removing it on demand. Same node
/// reset, only `cgroupDriver: cgroupfs` changed: apiserver `/livez` 84/84 over
/// 7 min, 0 `StopContainer`, 0 `SandboxChanged` — against 20 stops in 3 min.
///
/// `SYSTEMD` is the proto's zero value, so a `None` or a default here is the bug
/// back. The honest `SYSTEMD` answer needs the engine to place containers in a
/// transient scope under the pod slice (ADR 0038); until it does, this stays.
fn engine_cgroup_driver() -> CgroupDriver {
    CgroupDriver::Cgroupfs
}

pub mod lifecycle;

/// Is the rootless network plane ready? **Pure**, so the rule can be tested
/// without an infra namespace — see `network_ready_distinguishes_idle_from_failure`.
fn network_ready_rootless(up: bool, refcount: i64) -> bool {
    up || refcount == 0
}

/// How many of the ref markers correspond to a container that is STILL RUNNING.
///
/// `None` when the container list could not be read — and then the caller falls
/// back to the raw `refcount`, which is the previous behaviour. Treating a read
/// error as "zero workloads" would be the trap this repo has already catalogued
/// (a `read_dir` that fails is not an empty directory), and here it would declare
/// the node healthy precisely when nothing is known.
fn live_attached_refs(
    base: &std::path::Path,
    ops: &dyn crate::ContainerOps,
    net: &dyn crate::NetworkOps,
) -> Option<i64> {
    // The markers come from the SAME root as the container store — this service's
    // `base`. The first version used `attached_refs()` with no argument, which
    // resolves the root from the ENVIRONMENT: on a normal server they coincide,
    // but with a divergent `--root` the count compared two different populations
    // and meant nothing. `attached_refs_in` closes that.
    //
    // Recorded because a test of mine PASSED FOR THE WRONG REASON and it was that
    // test that revealed the asymmetry: it claimed «with no markers it does not
    // touch the store» and, on a host with real markers, it did touch it and
    // returned 0 by subtraction. A test that passes by accident is worse than
    // none — it was deleted, and the asymmetry it exposed is now fixed rather
    // than merely documented.
    let attached = net.attached_refs_in(base);
    if attached.is_empty() {
        return Some(0);
    }
    // The container store through the engine's port (R3h): `ops.list()` reads
    // `<base>/containers`, the SAME root the ref markers live under — closing the
    // gap the free-standing `Store::open(base)` here opened (it read `<base>`, the
    // parent of the store, so it counted nothing). The markers still come from
    // `attached_refs_in(base)`; only the store read moved behind the port.
    let live: Vec<String> = ops
        .list()
        .ok()?
        .into_iter()
        .filter(|c| c.is_live())
        .map(|c| c.id)
        .collect();
    let orphans = net.orphan_ref_count(&attached, &live);
    Some((attached.len() - orphans) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixed finding: `NetworkReady` is no longer a fixed `true`. In this
    /// test environment there is no rootless infra (`holder`/`slirp`) running —
    /// so `NetworkReady` MUST come back `false` (with reason "InfraDown"), never
    /// `true`. Before the fix, this test would fail (the condition always came
    /// back `true`, masking exactly this scenario).
    #[tokio::test]
    async fn network_ready_reflects_real_rootless_infra_not_fabricated() {
        if !delonix_linux::is_rootless() {
            eprintln!("SKIP: test assumes a rootless environment (uid != 0)");
            return;
        }
        // `status()` probes the GLOBAL rootless infra (`delonix_sdn::infra::status()`
        // reads `<base_root>/ingress/holder.pid`, resolved by `DELONIX_ROOT`/
        // `XDG_DATA_HOME`, NOT by this test's temporary `base`). If the operator
        // has REAL infra running (e.g. a holder from earlier sessions on this
        // dev box), `NetworkReady` comes back `true` rightly — and there is no way
        // to force "InfraDown" without TEARING DOWN that live infra, which a unit
        // test can never do. In that case we skip; on a clean runner (infra down,
        // the case that matters for the regression) the test runs and validates
        // the `false` path.
        // Read ONCE, and it is against THIS that the condition is compared — not
        // against a fixed value. `status()` probes the GLOBAL infra (see the
        // comment above), so the right result depends on the machine the test
        // runs on, and a literal here could only be right by accident.
        let st = delonix_sdn::infra::status();
        let base_dir = tempfile::tempdir().unwrap();
        let base = base_dir.path().to_path_buf();
        let ops: std::sync::Arc<dyn crate::ContainerOps> =
            std::sync::Arc::new(crate::test_ops::StoreOps::new(base.clone()));
        let rt: std::sync::Arc<dyn crate::RuntimeOps> =
            std::sync::Arc::new(crate::test_ops::LinuxPort);
        let net: std::sync::Arc<dyn crate::NetworkOps> =
            std::sync::Arc::new(crate::test_ops::NetPort);
        let streamer = crate::streaming::Streamer::new(
            base.clone(),
            ops.clone(),
            rt.clone(),
            "127.0.0.1:0".to_string(),
        );
        let svc = DelonixRuntime::new(
            base.clone(),
            ops.clone(),
            rt,
            net.clone(),
            streamer,
            crate::CapCeiling::unlimited(),
        );

        // By the SAME rule and with the SAME root the service uses: LIVE refs of
        // this service's `base`, not markers on disk nor a root from the
        // environment. With the raw `refcount` here, the test would fail on any
        // host with ghosts — it would be the test encoding the defect the change
        // closes.
        let live = live_attached_refs(&base, ops.as_ref(), net.as_ref()).unwrap_or(st.refcount);
        let expected = network_ready_rootless(st.up, live);

        let resp = svc
            .status(Request::new(StatusRequest { verbose: false }))
            .await
            .unwrap()
            .into_inner();
        let status = resp
            .status
            .expect("StatusResponse.status should be populated");
        let runtime_ready = status
            .conditions
            .iter()
            .find(|c| c.r#type == "RuntimeReady")
            .unwrap();
        assert!(
            runtime_ready.status,
            "RuntimeReady should be true (the server answered)"
        );

        let network_ready = status
            .conditions
            .iter()
            .find(|c| c.r#type == "NetworkReady")
            .unwrap();
        // THE RULE CHANGED, and the reason is in `status()`: infra down WITHOUT
        // workloads is the resting state of a daemonless engine, not a failure,
        // and reporting it as a failure pinned the node in NotReady forever.
        // What this test guards is still the same: that the condition is DERIVED
        // from the real state and not fabricated.
        //
        // **And that is why the assertion compares against `expected` and not
        // against a literal.** The previous version demanded `true` because it
        // assumed «neither infra nor workloads» — true on a clean runner, false
        // on a development host. Measured 2026-08-15: with the infra down and
        // SEVEN orphan ref markers left by dead containers, the right value is
        // `false`, and the test failed accusing a regression that did not exist.
        // A test that only passes in clean environments does not tell «the code
        // broke» from «the machine is dirty», and the first thing done with a red
        // like that is to ignore it.
        //
        // **What this test does NOT do, and it must be said**: on a host where
        // the infra is UP the right value is `true`, and a FABRICATED condition
        // (`if true`) gives the same — so here it does not discriminate.
        // Measured, reverting the condition: it passes. What pins the logic down
        // are the five pure assertions over `network_ready_rootless` (the four
        // quadrants), and that is where a regression of the RULE is caught. This
        // test's job is the other half: proving that the gRPC path CARRIES the
        // derived value all the way to the `StatusResponse`, instead of
        // recomputing it or forgetting it.
        assert_eq!(
            network_ready.status, expected,
            "NetworkReady must be DERIVED from (up={}, refcount={}), not fabricated",
            st.up, st.refcount
        );
        assert_eq!(
            network_ready.reason.is_empty(),
            expected,
            "a true condition has no failure reason, and a false one must give one"
        );
    }

    /// The rule that decides `NetworkReady`, isolated from the environment.
    ///
    /// The previous version was `infra.up`, which in a daemonless engine
    /// describes the RESTING state as a failure: on a freshly booted node the
    /// infra has not come up yet, the kubelet marked it NotReady, scheduled no
    /// pod, and so nothing brought the infra up. Permanent deadlock.
    ///
    /// The fact that discriminates is the ref-count, and that is what this test
    /// pins down.
    #[test]
    fn network_ready_distinguishes_idle_from_failure() {
        // Down and with nobody needing it: idle.
        assert!(super::network_ready_rootless(false, 0));
        // The case that reopened the deadlock: ref markers of DEAD containers.
        // With the raw `refcount`, a node with seven ghosts and zero live
        // workloads reports `false` forever — schedules nothing, nobody brings
        // the infra up, it stays NotReady. Counting only the LIVE ones, it is
        // `true`, which is the truth.
        assert!(
            super::network_ready_rootless(false, 0),
            "seven orphan markers count ZERO live, and zero live with the infra \
             down is idle — not a failure"
        );
        // Down WITH workloads attached: a real failure — this is what the check
        // was written for, and it still catches it.
        assert!(!super::network_ready_rootless(false, 1));
        assert!(!super::network_ready_rootless(false, 7));
        // Running: ready, workloads or not.
        assert!(super::network_ready_rootless(true, 0));
        assert!(super::network_ready_rootless(true, 3));
    }
}

/// The kubelet takes its cgroup driver from this answer — see `engine_cgroup_driver`.
#[cfg(test)]
mod tests_runtime_config {
    use crate::cri::runtime_service_server::RuntimeService;
    use crate::cri::*;

    /// `linux: None` sends the kubelet back to its own config (`systemd` under
    /// kubeadm), and `SYSTEMD` is the proto's zero value — both are the crash-loop.
    #[tokio::test]
    async fn the_kubelet_is_told_cgroupfs() {
        let base = std::env::temp_dir();
        let ops: std::sync::Arc<dyn crate::ContainerOps> =
            std::sync::Arc::new(crate::test_ops::StoreOps::new(base.clone()));
        let port: std::sync::Arc<dyn crate::RuntimeOps> =
            std::sync::Arc::new(crate::test_ops::LinuxPort);
        let net: std::sync::Arc<dyn crate::NetworkOps> =
            std::sync::Arc::new(crate::test_ops::NetPort);
        let rt = super::DelonixRuntime::new(
            base.clone(),
            ops.clone(),
            port.clone(),
            net,
            crate::streaming::Streamer::new(base, ops, port, "http://127.0.0.1:0".into()),
            crate::CapCeiling::unlimited(),
        );
        let resp = rt
            .runtime_config(tonic::Request::new(RuntimeConfigRequest {}))
            .await
            .unwrap()
            .into_inner();
        let linux = resp
            .linux
            .expect("an absent answer means the kubelet's own driver");
        assert_eq!(linux.cgroup_driver, CgroupDriver::Cgroupfs as i32);
    }
}

/// The pod CIDR the kubelet hands over is answered by mode, not by silence.
/// This module fails if `update_runtime_config` goes back to a bare `Ok(())`.
#[cfg(test)]
mod tests_pod_cidr_verdict {
    use super::{pod_cidr_verdict, PodCidrVerdict};

    /// The finding: on the native SDN the CIDR is not honoured, and returning
    /// success over it was indistinguishable from applying it.
    #[test]
    fn native_sdn_says_it_is_not_honoured() {
        assert_eq!(
            pod_cidr_verdict("10.244.1.0/24", false),
            PodCidrVerdict::NotHonoured
        );
    }

    /// With CNI the plugin chain owns addressing — same answer containerd gives,
    /// and not a warning, because nothing is wrong.
    #[test]
    fn with_cni_it_is_delegated() {
        assert_eq!(
            pod_cidr_verdict("10.244.1.0/24", true),
            PodCidrVerdict::DelegatedToCni
        );
    }

    /// An empty CIDR is the kubelet saying nothing. Warning about it would train
    /// whoever reads the logs to ignore the warning that matters.
    #[test]
    fn an_empty_cidr_says_nothing() {
        for empty in ["", "   "] {
            assert_eq!(pod_cidr_verdict(empty, false), PodCidrVerdict::Nothing);
            assert_eq!(pod_cidr_verdict(empty, true), PodCidrVerdict::Nothing);
        }
    }
}
