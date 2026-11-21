//! `WatchEvents` is generation-aware (ADR-0100 / A5): a watcher on the node
//! contract tells a DESIRED change (a new spec accepted — `applied`, the
//! generation climbs) from a STATE-only event (`crashed`, the generation holds),
//! over the real gRPC service. A control plane follows the stream and branches on
//! the action + the version pair, no new RPC and no new filter.
//!
//! `#[ignore]` by default, same harness as `live_container_update` (the create
//! launches a real busybox):
//!
//! ```sh
//! cargo build -p delonix-bin
//! ROOT=$(mktemp -d)
//! DELONIX_ROOT=$ROOT target/debug/delonix pull busybox:latest
//! DELONIX_ROOT=$ROOT \
//!   DELONIX_BIN=$PWD/target/debug/delonix \
//!   cargo test -p delonix-node-api --test live_watch_generation -- --ignored --nocapture
//! ```

mod common;

use std::time::Duration;

use delonix_node_api::proto::v1::container_service_client::ContainerServiceClient;
use delonix_node_api::proto::v1::node_service_client::NodeServiceClient;
use delonix_node_api::proto::v1::{Container, ResourceMeta};
use delonix_node_api::proto::v1::{
    ContainerSpec, CreateContainerRequest, DeleteContainerRequest, Event, KillContainerRequest,
    Resources, UpdateContainerRequest, WatchEventsRequest,
};

fn short_sock() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir_in("/tmp").expect("a directory under /tmp");
    let sock = dir.path().join("x.sock").to_str().unwrap().to_owned();
    (dir, sock)
}

async fn wait_for(path: &str) {
    for _ in 0..200 {
        if std::path::Path::new(path).exists() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("the server never created {path}");
}

/// Pull the next `container` event off the stream whose action is in `wanted`,
/// skipping the `converged`/other lines the create also emits. Bounded so a
/// missing event fails the test instead of hanging.
async fn next_action(s: &mut tonic::Streaming<Event>, wanted: &[&str]) -> Event {
    let deadline = Duration::from_secs(15);
    loop {
        let ev = tokio::time::timeout(deadline, s.message())
            .await
            .expect("an event within 15s")
            .expect("the stream errored")
            .expect("the stream ended");
        if wanted.contains(&ev.action.as_str()) {
            return ev;
        }
    }
}

/// Pull the next event that is NOT a version transition (`applied`/`converged`)
/// — a runtime/state event, whatever the supervisor named it (`crashed`, `die`,
/// `killed`, …).
async fn next_state(s: &mut tonic::Streaming<Event>) -> Event {
    let deadline = Duration::from_secs(15);
    loop {
        let ev = tokio::time::timeout(deadline, s.message())
            .await
            .expect("an event within 15s")
            .expect("the stream errored")
            .expect("the stream ended");
        if ev.action != "applied" && ev.action != "converged" {
            return ev;
        }
    }
}

#[tokio::test]
#[ignore = "launches a real container; run with --ignored and DELONIX_BIN + a DELONIX_ROOT holding busybox"]
async fn watch_tells_a_desired_change_from_a_state_event() {
    assert!(
        std::env::var_os("DELONIX_BIN").is_some() && std::env::var_os("DELONIX_ROOT").is_some(),
        "needs DELONIX_BIN=<path to delonix> and DELONIX_ROOT=<root with busybox pulled>: \
         run with --ignored"
    );

    let (_sock_dir, sock) = short_sock();
    let s = sock.clone();
    std::thread::spawn(move || {
        let _ = delonix_node_api::serve_blocking(
            &format!("unix://{s}"),
            common::backing(),
            common::backing_store(),
            common::backing_resolver(),
            common::backing_net(),
            common::backing_images(),
            common::backing_providers(),
            common::backing_linux(),
            common::backing_volumes(),
            common::backing_ceph(),
        );
    });
    wait_for(&sock).await;

    let connect = || {
        let p = sock.clone();
        async move {
            tonic::transport::Endpoint::try_from("http://[::]:50051")
                .unwrap()
                .connect_with_connector(tower::service_fn(move |_| {
                    let p = p.clone();
                    async move {
                        Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(
                            tokio::net::UnixStream::connect(p).await?,
                        ))
                    }
                }))
                .await
                .expect("connect to the server's unix socket")
        }
    };
    let mut nodes = NodeServiceClient::new(connect().await);
    let mut c = ContainerServiceClient::new(connect().await);

    let name = "a5-watch-probe";
    let _ = c
        .delete_container(DeleteContainerRequest {
            name: name.into(),
            namespace: "default".into(),
            force: true,
            ..Default::default()
        })
        .await;

    // Open the watch BEFORE the change: no `since`, so it streams only what the
    // control plane does from here on. Filter to containers.
    let mut stream = nodes
        .watch_events(WatchEventsRequest {
            kinds: vec!["container".into()],
            ..Default::default()
        })
        .await
        .expect("WatchEvents opens")
        .into_inner();
    // Give the polling loop a beat to record its starting offset.
    tokio::time::sleep(Duration::from_millis(300)).await;

    // ---- DESIRED CHANGE 1: create a container. The mutation path stamps
    // generation 1 and emits `applied`.
    const START: i64 = 128 * 1024 * 1024;
    const LOWERED: i64 = 64 * 1024 * 1024;
    c.create_container(CreateContainerRequest {
        namespace: "default".into(),
        name: name.into(),
        spec: Some(ContainerSpec {
            image: "busybox:latest".into(),
            command: vec!["sh".into(), "-c".into(), "sleep 120".into()],
            resources: Some(Resources {
                memory_limit_bytes: START,
                ..Default::default()
            }),
            ..Default::default()
        }),
        start: true,
        request_id: "a5-create-1".into(),
        ..Default::default()
    })
    .await
    .expect("CreateContainer over gRPC");

    // A `applied` for the created container — a DESIRED change on the stream.
    let applied1 = next_action(&mut stream, &["applied"]).await;
    assert_eq!(applied1.target, "Container/a5-watch-probe");
    assert_eq!(
        applied1.generation, 1,
        "a create is a desired change: generation 1 (saw {applied1:?})"
    );

    // ---- DESIRED CHANGE 2: resize (a changed spec). generation climbs to 2,
    // and `applied` is emitted again.
    c.update_container(UpdateContainerRequest {
        namespace: "default".into(),
        name: name.into(),
        container: Some(Container {
            meta: Some(ResourceMeta {
                name: name.into(),
                namespace: "default".into(),
                ..Default::default()
            }),
            spec: Some(ContainerSpec {
                resources: Some(Resources {
                    memory_limit_bytes: LOWERED,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        update_mask: Some(pbjson_types::FieldMask {
            paths: vec!["spec.resources.memory_limit_bytes".into()],
        }),
    })
    .await
    .expect("UpdateContainer over gRPC");

    let applied2 = next_action(&mut stream, &["applied"]).await;
    assert_eq!(
        applied2.generation, 2,
        "a changed spec is a new desired generation: the stream shows it climb to 2 (saw {applied2:?})"
    );

    // ---- STATE-ONLY EVENT: the supervisor records a runtime transition
    // (`crashed`, exactly `cmd/container.rs` emits on a container death). It is
    // NOT a desired change: the action is not `applied` and the generation holds.
    let root = delonix_node_api::node::state_root();
    delonix_node::events::emit(&root, "container", "crashed", name, name, Some("exit=137"));

    let state = next_action(&mut stream, &["crashed"]).await;
    assert_eq!(
        state.action, "crashed",
        "a runtime event, not a desired one"
    );
    assert_eq!(
        state.generation, 2,
        "a state event does NOT bump the generation: it holds at the last desired one (saw {state:?})"
    );

    // The whole point: the watcher told the two apart — `applied` climbed the
    // generation (1 → 2); `crashed` left it where it was.
    assert!(applied1.generation < applied2.generation);
    assert_eq!(applied2.generation, state.generation);

    // ---- Cleanup.
    c.delete_container(DeleteContainerRequest {
        name: name.into(),
        namespace: "default".into(),
        force: true,
        ..Default::default()
    })
    .await
    .expect("DeleteContainer");
}

/// The SAME proof over the production transport: a gRPC client that handshakes
/// mutual TLS to the node API on a `tcp://` address (ADR-0096, A1) and follows
/// `WatchEvents` cross-process — not the in-process unix socket. The version
/// pair serializes identically, so the control plane tells a desired change from
/// a state change over the wire it will actually use. Driven against a running
/// `delonix serve node-api --addr tcp://<addr>` with its CA + a client cert:
///
/// ```sh
/// A5_ADDR=192.168.122.1:29443 A5_CA=.../ca.crt A5_CERT=.../client.crt A5_KEY=.../client.key \
///   DELONIX_BIN=... cargo test -p delonix-node-api --test live_watch_generation \
///   -- --ignored watch_over_tcp_mtls --nocapture
/// ```
#[tokio::test]
#[ignore = "cross-process mTLS leg; needs a running `serve node-api --addr tcp://...` and A5_ADDR/A5_CA/A5_CERT/A5_KEY"]
async fn watch_over_tcp_mtls_tells_a_desired_change_from_a_state_event() {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};

    // This test only runs when it is ASKED for (`--ignored`), so an unset
    // address is not "not wired this run" — it is the caller asking for the
    // cross-process leg without giving it a target. Returning here would pass
    // without proving anything; it fails naming what is missing instead.
    let addr = std::env::var("A5_ADDR").unwrap_or_else(|_| {
        panic!(
            "A5_ADDR is not set — this is the cross-process mTLS leg and it has \
             nothing to connect to. Give it A5_ADDR/A5_CA/A5_CERT/A5_KEY against a \
             running `delonix serve node-api --addr tcp://<addr>` (see the doc comment)."
        )
    });
    let read = |var: &str| std::fs::read(std::env::var(var).expect(var)).expect(var);
    let (ca, cert, key) = (read("A5_CA"), read("A5_CERT"), read("A5_KEY"));

    let mut roots = rustls::RootCertStore::empty();
    for c in CertificateDer::pem_slice_iter(&ca) {
        roots.add(c.unwrap()).unwrap();
    }
    let certs: Vec<_> = CertificateDer::pem_slice_iter(&cert)
        .collect::<Result<_, _>>()
        .unwrap();
    let pkey = PrivateKeyDer::from_pem_slice(&key).unwrap();
    let mut config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_client_auth_cert(certs, pkey)
        .expect("client auth cert");
    config.alpn_protocols = vec![b"h2".to_vec()]; // gRPC is HTTP/2
    let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));

    let (host, port) = addr.rsplit_once(':').expect("A5_ADDR is host:port");
    let (host, port): (String, u16) = (host.to_string(), port.parse().expect("port"));
    let ip: std::net::IpAddr = host
        .parse()
        .expect("A5_ADDR host is an IP matching the cert SAN");
    let server_name = ServerName::IpAddress(ip.into());

    let channel = tonic::transport::Endpoint::try_from(format!("https://{addr}"))
        .unwrap()
        .connect_with_connector(tower::service_fn(move |_| {
            let connector = connector.clone();
            let host = host.clone();
            let server_name = server_name.clone();
            async move {
                let tcp = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
                let tls = connector
                    .connect(server_name, tcp)
                    .await
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
                Ok::<_, std::io::Error>(hyper_util::rt::TokioIo::new(tls))
            }
        }))
        .await
        .expect("gRPC channel over TCP + mutual TLS");

    let mut nodes = NodeServiceClient::new(channel.clone());
    let mut c = ContainerServiceClient::new(channel.clone());

    let name = "a5-mtls-probe";
    let _ = c
        .delete_container(DeleteContainerRequest {
            name: name.into(),
            namespace: "default".into(),
            force: true,
            ..Default::default()
        })
        .await;

    let mut stream = nodes
        .watch_events(WatchEventsRequest {
            kinds: vec!["container".into()],
            ..Default::default()
        })
        .await
        .expect("WatchEvents opens over mTLS")
        .into_inner();
    tokio::time::sleep(Duration::from_millis(300)).await;

    const START: i64 = 128 * 1024 * 1024;
    const LOWERED: i64 = 64 * 1024 * 1024;
    // DESIRED CHANGE 1 — create.
    c.create_container(CreateContainerRequest {
        namespace: "default".into(),
        name: name.into(),
        spec: Some(ContainerSpec {
            image: "busybox:latest".into(),
            command: vec!["sh".into(), "-c".into(), "sleep 120".into()],
            resources: Some(Resources {
                memory_limit_bytes: START,
                ..Default::default()
            }),
            ..Default::default()
        }),
        start: true,
        request_id: "a5-mtls-1".into(),
        ..Default::default()
    })
    .await
    .expect("CreateContainer over mTLS");
    let applied1 = next_action(&mut stream, &["applied"]).await;
    assert_eq!(applied1.target, "Container/a5-mtls-probe");
    assert_eq!(applied1.generation, 1, "saw {applied1:?}");

    // DESIRED CHANGE 2 — resize (gRPC carries the FieldMask the REST leg cannot).
    c.update_container(UpdateContainerRequest {
        namespace: "default".into(),
        name: name.into(),
        container: Some(Container {
            meta: Some(ResourceMeta {
                name: name.into(),
                namespace: "default".into(),
                ..Default::default()
            }),
            spec: Some(ContainerSpec {
                resources: Some(Resources {
                    memory_limit_bytes: LOWERED,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        update_mask: Some(pbjson_types::FieldMask {
            paths: vec!["spec.resources.memory_limit_bytes".into()],
        }),
    })
    .await
    .expect("UpdateContainer over mTLS");
    let applied2 = next_action(&mut stream, &["applied"]).await;
    assert_eq!(
        applied2.generation, 2,
        "the generation climbs over the wire (saw {applied2:?})"
    );

    // STATE-ONLY EVENT — kill the container; the supervisor records a runtime
    // transition (not a desired one). The generation holds at the last desired.
    c.kill_container(KillContainerRequest {
        name: name.into(),
        namespace: "default".into(),
        signal: "KILL".into(),
    })
    .await
    .expect("KillContainer over mTLS");
    let state = next_state(&mut stream).await;
    assert_ne!(state.action, "applied");
    assert_eq!(
        state.generation, 2,
        "a state event does not bump the generation: it holds at the last desired one (saw {state:?})"
    );
    assert!(applied1.generation < applied2.generation);
    assert_eq!(applied2.generation, state.generation);

    c.delete_container(DeleteContainerRequest {
        name: name.into(),
        namespace: "default".into(),
        force: true,
        ..Default::default()
    })
    .await
    .expect("DeleteContainer");
}
