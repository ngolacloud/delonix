//! End-to-end proof that the SPIFFE configs this crate builds complete a REAL
//! mutual-TLS handshake — the async half the library deliberately leaves to the
//! caller, here driven by `tokio-rustls` (a dev dependency only).
//!
//! Two layers of proof:
//!   * the own-CA (rcgen) path, self-contained, always run: a mutual handshake
//!     succeeds, and the wrong-bundle / wrong-trust-domain cases are refused;
//!   * the SPIRE path (`spire_issued_svid_is_accepted`), run by the lab
//!     (`tests/interop/spiffe/`) with `DELONIX_SPIFFE_SVID_CERT/KEY/BUNDLE` set
//!     to a certificate a real `spire-server` minted — this is the H0 evidence.

use std::sync::Arc;

use delonix_spiffe::{
    client_config, self_signed, server_config, PeerPolicy, SpiffeId, SvidMaterial, TrustBundle,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, ServerConfig};
use tokio_rustls::{TlsAcceptor, TlsConnector};

/// Runs one mutual-TLS handshake over loopback and a one-byte echo. `Ok(())`
/// means both the client verified the server's SVID AND the server verified the
/// client's; an `Err` carries whichever side refused.
async fn mutual_handshake(server: ServerConfig, client: ClientConfig) -> Result<(), String> {
    let acceptor = TlsAcceptor::from(Arc::new(server));
    let connector = TlsConnector::from(Arc::new(client));
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.map_err(|e| format!("accept: {e}"))?;
        let mut tls = acceptor
            .accept(tcp)
            .await
            .map_err(|e| format!("server handshake: {e}"))?;
        let mut byte = [0u8; 1];
        tls.read_exact(&mut byte)
            .await
            .map_err(|e| format!("server read: {e}"))?;
        tls.write_all(&byte)
            .await
            .map_err(|e| format!("server write: {e}"))?;
        tls.shutdown().await.ok();
        Ok::<(), String>(())
    });

    // The ServerName is required by the API but IGNORED by the SPIFFE verifier —
    // identity is the URI SAN, not a DNS name. Any valid name works.
    let name = ServerName::try_from("delonix-node").unwrap();
    let tcp = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("connect: {e}"))?;
    let client_side = async {
        let mut tls = connector
            .connect(name, tcp)
            .await
            .map_err(|e| format!("client handshake: {e}"))?;
        tls.write_all(&[42])
            .await
            .map_err(|e| format!("client write: {e}"))?;
        let mut byte = [0u8; 1];
        tls.read_exact(&mut byte)
            .await
            .map_err(|e| format!("client read: {e}"))?;
        tls.shutdown().await.ok();
        if byte[0] != 42 {
            return Err("echo mismatch".to_string());
        }
        Ok::<(), String>(())
    };

    // Surface whichever side failed; a handshake refusal shows up on BOTH, so the
    // client error is usually the more specific one.
    let (client_res, server_res) = tokio::join!(client_side, server);
    client_res?;
    server_res.map_err(|e| format!("server task: {e}"))??;
    Ok(())
}

#[tokio::test]
async fn own_ca_mutual_handshake_succeeds() {
    let id = SpiffeId::parse("spiffe://example.org/node/web-1").unwrap();
    let (svid, bundle) = self_signed(&id).unwrap();
    assert_eq!(svid.spiffe_id().unwrap(), id);

    let server = server_config(&svid, &bundle, PeerPolicy::TrustDomain("example.org".into())).unwrap();
    let client = client_config(&svid, &bundle, PeerPolicy::Id(id)).unwrap();

    mutual_handshake(server, client)
        .await
        .expect("a SPIFFE SVID issued by the own CA must be accepted on both ends");
}

#[tokio::test]
async fn peer_outside_the_bundle_is_rejected() {
    // Two independent trust domains: each has its OWN CA, so a peer from one does
    // not chain to the other's bundle.
    let ours = SpiffeId::parse("spiffe://example.org/node").unwrap();
    let theirs = SpiffeId::parse("spiffe://evil.example/node").unwrap();
    let (our_svid, our_bundle) = self_signed(&ours).unwrap();
    let (their_svid, _their_bundle) = self_signed(&theirs).unwrap();

    // Server trusts OUR bundle; the client presents a cert from a FOREIGN CA.
    let server = server_config(&our_svid, &our_bundle, PeerPolicy::AnyInBundle).unwrap();
    let client = client_config(&their_svid, &our_bundle, PeerPolicy::AnyInBundle).unwrap();

    let err = mutual_handshake(server, client)
        .await
        .expect_err("a client whose cert does not chain to the server's bundle must be refused");
    assert!(!err.is_empty());
}

#[tokio::test]
async fn wrong_trust_domain_policy_is_rejected() {
    // The chain is valid (same own CA), but the server's policy requires a
    // DIFFERENT trust domain than the id in the client's SVID.
    let id = SpiffeId::parse("spiffe://example.org/node").unwrap();
    let (svid, bundle) = self_signed(&id).unwrap();

    let server = server_config(&svid, &bundle, PeerPolicy::TrustDomain("other.org".into())).unwrap();
    let client = client_config(&svid, &bundle, PeerPolicy::AnyInBundle).unwrap();

    mutual_handshake(server, client)
        .await
        .expect_err("a valid chain with an id outside the policy's trust domain must be refused");
}

/// The H0 evidence: a certificate a REAL `spire-server` minted is accepted by the
/// node's TLS layer on both ends. `#[ignore]` because it needs a running SPIRE —
/// the lab (`tests/interop/spiffe/run.sh`) mints the SVID, sets the three env
/// paths and runs it with `--ignored`. It FAILS (not silently skips) if asked to
/// run without that material, naming what is missing.
#[tokio::test]
#[ignore = "needs a real spire-server — run via tests/interop/spiffe/run.sh (sets DELONIX_SPIFFE_SVID_CERT/KEY/BUNDLE)"]
async fn spire_issued_svid_is_accepted() {
    let path = |var: &str| {
        std::env::var(var)
            .unwrap_or_else(|_| panic!("{var} must be set by tests/interop/spiffe/run.sh"))
    };
    let svid = SvidMaterial::from_files(path("DELONIX_SPIFFE_SVID_CERT"), path("DELONIX_SPIFFE_SVID_KEY"))
        .expect("load SPIRE-issued SVID");
    let trust = TrustBundle::from_file(path("DELONIX_SPIFFE_SVID_BUNDLE")).expect("load SPIRE trust bundle");
    let id = svid
        .spiffe_id()
        .expect("a SPIRE X.509-SVID must carry a spiffe:// URI SAN");

    // The node presents the SPIRE-issued SVID on BOTH ends and requires its own
    // trust domain — exactly "the node accepts a cert the SPIRE CA signed".
    let policy = || PeerPolicy::TrustDomain(id.trust_domain().to_string());
    let server = server_config(&svid, &trust, policy()).expect("server config from SPIRE SVID");
    let client = client_config(&svid, &trust, policy()).expect("client config from SPIRE SVID");

    mutual_handshake(server, client)
        .await
        .expect("the node TLS layer must accept the SPIRE-issued X.509-SVID");
}
