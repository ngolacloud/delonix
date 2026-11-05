//! Round-trip gRPC REAL contra o servidor CRI — o gap que o `AGENTS.md` declarava.
//!
//! O que estava escrito lá: «Não validado com um kubelet/`crictl` real (nenhum
//! dos dois existe neste host, e `build_client(false)` no `build.rs` não gera
//! stubs de cliente gRPC): o caminho gRPC está coberto pelo teste do
//! `create_container` real + `cap_flags`/`cap_args`, e a camada tonic são três
//! linhas de `blocking(...)`».
//!
//! «São três linhas» é uma razão para achar que funciona, não uma medição. Este
//! teste mede: sobe o servidor num socket unix, fala com ele pelo cliente
//! gerado, e confirma que o `Status` chega com as condições preenchidas.
//!
//! O custo de gerar o cliente foi medido antes de o ligar: **3,5 s** de build no
//! crate. É o preço de deixar de deduzir a camada de transporte.

use delonix_cri::cri::runtime_service_client::RuntimeServiceClient;
use delonix_cri::cri::{
    ListMetricDescriptorsRequest, ListPodSandboxMetricsRequest, ListPodSandboxRequest,
    PodSandboxConfig, PodSandboxMetadata, PortMapping, Protocol, Protocol as Proto,
    RunPodSandboxRequest, StatusRequest, VersionRequest,
};

/// The image port the CRI server needs (R3h). These tests exercise only the
/// RuntimeService (version/status/sandbox), so an empty image backing is enough —
/// the real image path is served by the composed engine in `delonix serve cri`.
struct NoImages;
impl delonix_cri::ImageOps for NoImages {
    fn list_images(&self) -> Result<Vec<serde_json::Value>, delonix_model::Error> {
        Ok(Vec::new())
    }
    fn image_status(
        &self,
        _reference: &str,
    ) -> Result<Option<serde_json::Value>, delonix_model::Error> {
        Ok(None)
    }
    fn pull_image(
        &self,
        _reference: &str,
        _creds: Option<(String, String)>,
    ) -> Result<String, delonix_model::Error> {
        Ok(String::new())
    }
    fn remove_image(&self, _reference: &str) -> Result<(), delonix_model::Error> {
        Ok(())
    }
    fn image_fs_used_bytes(&self) -> u64 {
        0
    }
}

/// The container port the CRI server needs (R3h). These tests seed no containers
/// and exercise only version/status/sandbox-list/metrics, so an empty backing is
/// enough — `status` lists live refs through it and gets `Ok(vec![])`; the real
/// store path is served by the composed engine in `delonix serve cri`.
struct NoContainers;
impl delonix_cri::ContainerOps for NoContainers {
    fn list(&self) -> Result<Vec<delonix_compute::Container>, delonix_model::Error> {
        Ok(Vec::new())
    }
    fn load(
        &self,
        id_or_name: &str,
    ) -> Result<delonix_compute::Container, delonix_model::Error> {
        Err(delonix_model::Error::NotFound(id_or_name.to_string()))
    }
    fn update(
        &self,
        id_or_name: &str,
        _f: &mut dyn FnMut(&mut delonix_compute::Container) -> bool,
    ) -> Result<delonix_compute::Container, delonix_model::Error> {
        Err(delonix_model::Error::NotFound(id_or_name.to_string()))
    }
}

/// The runtime (Linux-mechanism) port the CRI server needs (R3h-C): since the
/// interface no longer names `delonix-linux`, these transport tests hand in the
/// same real-adapter-backed port the crate's own unit tests use, so the server's
/// `is_rootless`/cgroup/capability paths are the genuine ones.
struct TestRuntime;
impl delonix_cri::RuntimeOps for TestRuntime {
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
        update: &delonix_cri::ResourceUpdateSpec<'_>,
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

/// The SDN/CNI port for the round-trip test (R3h-D). `status`/`version` only reach
/// the leaf infra methods; the CNI-orchestration methods are stubs here because
/// this test never drives the sandbox path (that is exercised live, not in a unit
/// test), and keeping them inert avoids dragging a serializer into the test crate.
struct TestNetwork;
impl delonix_cri::NetworkOps for TestNetwork {
    fn infra_status(&self) -> delonix_cri::InfraStatus {
        let s = delonix_sdn::infra::status();
        delonix_cri::InfraStatus {
            up: s.up,
            refcount: s.refcount,
            holder_pid: s.holder_pid,
            slirp_pid: s.slirp_pid,
        }
    }
    fn root_network_ready(&self) -> Result<(), (String, String)> {
        let dirs = delonix_sdn::cni::plugin_dirs();
        match delonix_sdn::cni::readiness(
            std::path::Path::new(delonix_sdn::cni::DEFAULT_CONF_DIR),
            &dirs,
        )
        .not_ready(&dirs)
        {
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
    fn cni_resolve_conf(&self, _rootless: bool) -> delonix_cri::CniConfResult {
        delonix_cri::CniConfResult::Disabled
    }
    fn cni_with_port_mappings(&self, conf_json: &str, _ports: &[delonix_cri::CniPort]) -> String {
        conf_json.to_string()
    }
    fn cni_attach(
        &self,
        _pod: &str,
        _rootless: bool,
        _conf_json: &str,
        _cni_args: Option<&str>,
    ) -> Result<delonix_cri::CniAttached, String> {
        Err("CNI attach is not exercised by the gRPC round-trip test".to_string())
    }
    fn cni_set_netns_sysctls(&self, _netns: &str, _sysctls: &[(String, String)]) -> Result<(), String> {
        Ok(())
    }
    fn cni_detach(&self, _pod: &str, _rootless: bool, _conf_json: &str) {}
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

/// A SHORT socket path: `sun_path` is 108 bytes and an agent session's
/// `$TMPDIR` is already past 90, so the directory lives in `/tmp` itself
/// (`/tmp/.tmpXXXXXX/x.sock`, 22 bytes). The `TempDir` removes it on every
/// exit, a failed assert included, which a `remove_file` on the last line
/// did not.
fn short_sock() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir_in("/tmp").expect("a directory under /tmp");
    let sock = dir.path().join("x.sock").to_str().unwrap().to_owned();
    (dir, sock)
}

#[tokio::test]
async fn o_status_chega_pelo_transporte_grpc_a_serio() {
    let (_sock_dir, sock) = short_sock();
    let base_dir = tempfile::tempdir().unwrap();
    let base = base_dir.path().to_path_buf();

    // O servidor corre numa thread própria (o `serve_blocking` tem o seu runtime).
    let s = sock.clone();
    let b = base.clone();
    let servidor = std::thread::spawn(move || {
        let _ = delonix_cri::serve_blocking(
            b,
            std::sync::Arc::new(NoImages),
            std::sync::Arc::new(NoContainers),
            std::sync::Arc::new(TestRuntime),
            std::sync::Arc::new(TestNetwork),
            &format!("unix://{s}"),
            delonix_cri::CapCeiling::unlimited(),
        );
    });

    // Esperar por CONDIÇÃO, nunca por tempo.
    for _ in 0..100 {
        if std::path::Path::new(&sock).exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        std::path::Path::new(&sock).exists(),
        "o servidor não chegou a criar o socket"
    );

    // Um socket que ACEITA não é um servidor que RESPONDE — daí a chamada real.
    let mut cli = connect(&sock).await;

    let v = cli
        .version(VersionRequest::default())
        .await
        .expect("Version pelo transporte gRPC")
        .into_inner();
    assert!(
        !v.runtime_name.is_empty(),
        "o Version tem de nomear o runtime"
    );

    let st = cli
        .status(StatusRequest { verbose: false })
        .await
        .expect("Status pelo transporte gRPC")
        .into_inner()
        .status
        .expect("StatusResponse.status preenchido");

    // As duas condições que o kubelet lê para decidir se o nó serve.
    let tipos: Vec<&str> = st.conditions.iter().map(|c| c.r#type.as_str()).collect();
    assert!(
        tipos.contains(&"RuntimeReady"),
        "faltou RuntimeReady: {tipos:?}"
    );
    assert!(
        tipos.contains(&"NetworkReady"),
        "faltou NetworkReady: {tipos:?}"
    );
    let rr = st
        .conditions
        .iter()
        .find(|c| c.r#type == "RuntimeReady")
        .unwrap();
    assert!(
        rr.status,
        "chegámos aqui pelo gRPC, logo o runtime respondeu — RuntimeReady tem de ser true"
    );

    // `ListMetricDescriptors`/`ListPodSandboxMetrics` — real gRPC round-trip,
    // not just the internal function called directly. Empty base directory
    // here, so no sandboxes exist; what matters is that the descriptor names
    // arrive over the wire and match this runtime's own metric names (the
    // spec-mandated pairing `every_emitted_metric_name_has_a_descriptor`
    // checks internally, now also proven through the transport).
    let descriptors = cli
        .list_metric_descriptors(ListMetricDescriptorsRequest {})
        .await
        .expect("ListMetricDescriptors over the real gRPC transport")
        .into_inner()
        .descriptors;
    assert!(
        !descriptors.is_empty(),
        "o runtime tem de declarar pelo menos um descritor de métrica"
    );
    let names: Vec<&str> = descriptors.iter().map(|d| d.name.as_str()).collect();
    assert!(
        names.contains(&"container_cpu_usage_core_nanoseconds"),
        "faltou o descritor de CPU por container: {names:?}"
    );

    let pod_metrics = cli
        .list_pod_sandbox_metrics(ListPodSandboxMetricsRequest {})
        .await
        .expect("ListPodSandboxMetrics over the real gRPC transport")
        .into_inner()
        .pod_metrics;
    assert!(
        pod_metrics.is_empty(),
        "no sandboxes exist, the list has to come back empty — never fabricated"
    );

    drop(cli);
    // O servidor não tem paragem limpa (é um `serve_blocking`); o processo de
    // teste termina e leva-o. Não se faz `join`, que penduraria.
    drop(servidor);
}

/// The connector the two tests share: a socket that ACCEPTS is not a server
/// that ANSWERS, so every assertion here goes over a real channel.
async fn connect(sock: &str) -> RuntimeServiceClient<tonic::transport::Channel> {
    let s = sock.to_owned();
    let canal = tonic::transport::Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(tower::service_fn(move |_| {
            let p = s.clone();
            async move {
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(
                    tokio::net::UnixStream::connect(p).await?,
                ))
            }
        }))
        .await
        .expect("connecting to the server's unix socket");
    RuntimeServiceClient::new(canal)
}

/// ADR-0074 D1: a `hostPort` this node cannot publish is refused from
/// `RunPodSandbox`, over the kubelet's own transport — and **nothing is left
/// behind**, which is the half a unit test cannot prove.
///
/// Before this, the spec `8080:80/sctp` was stored, the sandbox was created, and
/// the pod only died at `StartContainer` with `invalid protocol in
/// '8080:80/sctp'`: late, after the sandbox existed, naming the spec instead of
/// the cause, and retried by the kubelet forever. The empty-state assertion at
/// the end is what says the refusal happens BEFORE any creation.
#[tokio::test]
async fn an_sctp_host_port_is_refused_over_grpc_and_leaves_nothing_behind() {
    let (_sock_dir, sock) = short_sock();
    let base_dir = tempfile::tempdir().unwrap();
    let base = base_dir.path().to_path_buf();

    let s = sock.clone();
    let b = base.clone();
    let server = std::thread::spawn(move || {
        let _ = delonix_cri::serve_blocking(
            b,
            std::sync::Arc::new(NoImages),
            std::sync::Arc::new(NoContainers),
            std::sync::Arc::new(TestRuntime),
            std::sync::Arc::new(TestNetwork),
            &format!("unix://{s}"),
            delonix_cri::CapCeiling::unlimited(),
        );
    });
    for _ in 0..100 {
        if std::path::Path::new(&sock).exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(
        std::path::Path::new(&sock).exists(),
        "the server never created the socket"
    );

    let mut cli = connect(&sock).await;

    let cfg = |proto: Protocol, host_port: i32| PodSandboxConfig {
        metadata: Some(PodSandboxMetadata {
            name: "sip".into(),
            uid: "u1".into(),
            namespace: "default".into(),
            attempt: 0,
        }),
        port_mappings: vec![PortMapping {
            protocol: proto as i32,
            container_port: 5070,
            host_port,
            host_ip: String::new(),
        }],
        ..Default::default()
    };

    let err = cli
        .run_pod_sandbox(RunPodSandboxRequest {
            config: Some(cfg(Proto::Sctp, 5070)),
            runtime_handler: String::new(),
        })
        .await
        .expect_err("an SCTP hostPort cannot be published by this node");
    assert_eq!(
        err.code(),
        tonic::Code::FailedPrecondition,
        "the class is what the kubelet turns into a pod event: {err:?}"
    );
    assert!(err.message().contains("5070"), "{}", err.message());
    assert!(err.message().contains("sctp"), "{}", err.message());

    // The half a unit test cannot reach: the refusal came BEFORE any creation,
    // so the node has no sandbox to clean up.
    let sandboxes = cli
        .list_pod_sandbox(ListPodSandboxRequest { filter: None })
        .await
        .expect("ListPodSandbox over the real transport")
        .into_inner()
        .items;
    assert!(
        sandboxes.is_empty(),
        "a refused sandbox must not exist: {sandboxes:?}"
    );

    drop(cli);
    drop(server);
}
