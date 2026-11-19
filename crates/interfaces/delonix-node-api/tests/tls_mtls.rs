//! The node contract over TCP with mutual TLS (ADR-0096, Phase A/A1). The
//! server is the one `delonix serve node-api` runs — `serve` with a TLS
//! material and a TCP listener — not a test double. A control plane on another
//! host reaches `GET /v1` only by presenting a client certificate that chains
//! to the node's configured client CA; a client with no certificate, and a
//! client whose certificate chains to a DIFFERENT CA, are refused at the TLS
//! handshake and never reach the contract. This is the fail-closed claim of
//! the decision: the transport authenticates the caller, cleartext is never
//! served, and an untrusted client gets nothing.

mod common;

use std::io::{Read, Write};
use std::os::fd::IntoRawFd;
use std::sync::Arc;

use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, KeyPair};

/// A CA kept live so it can sign more than one leaf in this run.
struct Ca {
    cert_pem: String,
    cert: rcgen::Certificate,
    key: KeyPair,
}

fn generate_ca(cn: &str) -> Ca {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params.distinguished_name.push(DnType::CommonName, cn);
    let key = KeyPair::generate().unwrap();
    let cert = params.self_signed(&key).unwrap();
    Ca {
        cert_pem: cert.pem(),
        cert,
        key,
    }
}

/// A leaf (cert_pem, key_pem) signed by `ca`, with the given SANs.
fn issue_leaf(ca: &Ca, cn: &str, sans: &[&str]) -> (String, String) {
    let mut params =
        CertificateParams::new(sans.iter().map(|s| s.to_string()).collect::<Vec<_>>()).unwrap();
    params.is_ca = IsCa::NoCa;
    params.distinguished_name.push(DnType::CommonName, cn);
    let key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&key, &ca.cert, &ca.key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// A pre-bound loopback TCP listener handed to the server as a socket the
/// service manager passed (`Listen::Inherited`): the test owns the port, and
/// this is exactly the shape socket activation delivers — an already-bound,
/// listening TCP socket.
fn bound_tcp() -> (u16, std::os::fd::RawFd) {
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    (port, l.into_raw_fd())
}

/// A client's attempt at `GET /v1` over TLS. `identity` is the client's own
/// (cert_pem, key_pem) when it presents one. `trust_ca_pem` is the CA the
/// client trusts for the SERVER's certificate. Returns the raw response text,
/// or an error when the connection never produced one (a refused handshake).
fn https_get_v1(
    port: u16,
    identity: Option<(&str, &str)>,
    trust_ca_pem: &str,
) -> Result<String, String> {
    use rustls::pki_types::pem::PemObject;
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};

    let mut roots = rustls::RootCertStore::empty();
    for ca in CertificateDer::pem_slice_iter(trust_ca_pem.as_bytes()) {
        roots.add(ca.unwrap()).unwrap();
    }
    let builder = rustls::ClientConfig::builder().with_root_certificates(roots);
    let mut config = match identity {
        Some((cert_pem, key_pem)) => {
            let certs: Vec<_> = CertificateDer::pem_slice_iter(cert_pem.as_bytes())
                .collect::<Result<_, _>>()
                .unwrap();
            let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes()).unwrap();
            builder
                .with_client_auth_cert(certs, key)
                .map_err(|e| format!("client auth cert: {e}"))?
        }
        None => builder.with_no_client_auth(),
    };
    config.alpn_protocols = vec![b"http/1.1".to_vec()];

    let server_name = ServerName::try_from("localhost").unwrap();
    let mut conn =
        rustls::ClientConnection::new(Arc::new(config), server_name).map_err(|e| e.to_string())?;
    let mut sock = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("tcp connect: {e}"))?;
    sock.set_read_timeout(Some(std::time::Duration::from_secs(10)))
        .unwrap();
    let mut tls = rustls::Stream::new(&mut conn, &mut sock);
    tls.write_all(b"GET /v1 HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .map_err(|e| format!("write: {e}"))?;
    tls.flush().map_err(|e| format!("flush: {e}"))?;
    let mut buf = Vec::new();
    tls.read_to_end(&mut buf)
        .map_err(|e| format!("read: {e}"))?;
    if buf.is_empty() {
        return Err("empty response".into());
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

#[test]
fn the_node_contract_over_tcp_requires_a_trusted_client_certificate() {
    // Installing the provider is idempotent; the server thread installs its
    // own, this is for the client side of the test.
    let _ = rustls::crypto::ring::default_provider().install_default();

    // The node's own CA (it signs the server cert and is the trust anchor for
    // client certs), and a DIFFERENT CA a hostile client would use.
    let node_ca = generate_ca("delonix-node-ca");
    let other_ca = generate_ca("someone-elses-ca");

    let (server_cert, server_key) = issue_leaf(&node_ca, "node", &["127.0.0.1", "localhost"]);
    let (ok_client_cert, ok_client_key) = issue_leaf(&node_ca, "control-plane", &[]);
    let (bad_client_cert, bad_client_key) = issue_leaf(&other_ca, "intruder", &[]);

    let (port, fd) = bound_tcp();
    let tls = delonix_node_api::Tls {
        cert_pem: server_cert,
        key_pem: server_key,
        client_ca_pem: node_ca.cert_pem.clone(),
    };
    std::thread::spawn(move || {
        let _ = delonix_node_api::serve(
            delonix_node_api::Listen::Inherited(fd),
            None,
            Some(tls),
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

    // Give the server its moment to take the socket and build the acceptor.
    std::thread::sleep(std::time::Duration::from_millis(300));

    // A client with a certificate the node's CA signed reaches the contract.
    let ok = https_get_v1(
        port,
        Some((&ok_client_cert, &ok_client_key)),
        &node_ca.cert_pem,
    )
    .expect("a trusted client reaches the node contract over mTLS");
    assert!(
        ok.starts_with("HTTP/1.1 200"),
        "the trusted client did not get 200: {ok:?}"
    );
    assert!(
        ok.contains("delonix.node.v1"),
        "the body is not the API root: {ok:?}"
    );

    // A client that presents NO certificate is refused — the server requires
    // one (fail-closed), so the handshake never completes and no response is
    // produced.
    let no_cert = https_get_v1(port, None, &node_ca.cert_pem);
    assert!(
        no_cert.is_err(),
        "a client with no certificate was served: {no_cert:?}"
    );

    // A client whose certificate chains to a DIFFERENT CA is refused the same
    // way: the node trusts only its configured client CA.
    let wrong_ca = https_get_v1(
        port,
        Some((&bad_client_cert, &bad_client_key)),
        &node_ca.cert_pem,
    );
    assert!(
        wrong_ca.is_err(),
        "a client from an untrusted CA was served: {wrong_ca:?}"
    );
}
