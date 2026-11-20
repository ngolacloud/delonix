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

use rcgen::{
    date_time_ymd, BasicConstraints, CertificateParams, CertificateRevocationListParams, DnType,
    IsCa, KeyIdMethod, KeyPair, RevokedCertParams, SerialNumber,
};

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

/// A leaf with an EXPLICIT serial, so a test can later revoke it by that serial
/// (ADR-0097 A2 — the node CA sets a serial per issuance precisely so a CRL can
/// name it). Returns `(cert_pem, key_pem)`.
fn issue_leaf_with_serial(ca: &Ca, cn: &str, serial: u64) -> (String, String) {
    let mut params = CertificateParams::new(Vec::<String>::new()).unwrap();
    params.is_ca = IsCa::NoCa;
    params.distinguished_name.push(DnType::CommonName, cn);
    params.serial_number = Some(SerialNumber::from(serial));
    let key = KeyPair::generate().unwrap();
    let cert = params.signed_by(&key, &ca.cert, &ca.key).unwrap();
    (cert.pem(), key.serialize_pem())
}

/// A CRL (PEM) signed by `ca`, listing `revoked` serials. The validity window is
/// a wide fixed bracket (it only has to contain «now»); revocation does not
/// depend on the wall clock here.
fn crl_pem(ca: &Ca, revoked: &[u64]) -> String {
    let params = CertificateRevocationListParams {
        this_update: date_time_ymd(2020, 1, 1),
        next_update: date_time_ymd(2100, 1, 1),
        crl_number: SerialNumber::from(1u64),
        issuing_distribution_point: None,
        revoked_certs: revoked
            .iter()
            .map(|s| RevokedCertParams {
                serial_number: SerialNumber::from(*s),
                revocation_time: date_time_ymd(2020, 1, 1),
                reason_code: None,
                invalidity_date: None,
            })
            .collect(),
        key_identifier_method: KeyIdMethod::Sha256,
    };
    params.signed_by(&ca.cert, &ca.key).unwrap().pem().unwrap()
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
        client_crl_pem: None,
    };
    std::thread::spawn(move || {
        let _ = delonix_node_api::serve(
            delonix_node_api::Listen::Inherited(fd),
            None,
            Some(tls),
            None,
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

/// ADR-0097 A2: a CRL supplied to the server makes it refuse a revoked client's
/// certificate at the handshake, while another certificate from the SAME CA —
/// whose serial is not listed — is still served. This is the server side of
/// revocation (`build_server_config` → `WebPkiClientVerifier::with_crls`), proven
/// at the real handshake, not by inspection. The daemonless cold-start and the
/// node-CA/enrollment flow that produces the CRL are the live A2 acceptance.
#[test]
fn a_revoked_client_certificate_is_refused_while_a_live_one_is_served() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let node_ca = generate_ca("delonix-node-ca");
    let (server_cert, server_key) = issue_leaf(&node_ca, "node", &["127.0.0.1", "localhost"]);
    // Two client certs from the node CA; one will be revoked by its serial.
    let (revoked_cert, revoked_key) = issue_leaf_with_serial(&node_ca, "doomed-member", 0xDEAD);
    let (live_cert, live_key) = issue_leaf_with_serial(&node_ca, "live-member", 0xBEEF);
    let crl = crl_pem(&node_ca, &[0xDEAD]);

    let (port, fd) = bound_tcp();
    let tls = delonix_node_api::Tls {
        cert_pem: server_cert,
        key_pem: server_key,
        client_ca_pem: node_ca.cert_pem.clone(),
        client_crl_pem: Some(crl),
    };
    std::thread::spawn(move || {
        let _ = delonix_node_api::serve(
            delonix_node_api::Listen::Inherited(fd),
            None,
            Some(tls),
            None,
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
    std::thread::sleep(std::time::Duration::from_millis(300));

    // The revoked certificate — a valid chain to the CA, but listed in the CRL —
    // is refused at the handshake, never reaching the contract.
    let revoked = https_get_v1(port, Some((&revoked_cert, &revoked_key)), &node_ca.cert_pem);
    assert!(
        revoked.is_err(),
        "a REVOKED client certificate was served: {revoked:?}"
    );

    // A different certificate from the same CA, not listed in the CRL, is served
    // — revocation is by serial, not a CA-wide withdrawal.
    let live = https_get_v1(port, Some((&live_cert, &live_key)), &node_ca.cert_pem)
        .expect("a non-revoked client from the same CA reaches the contract");
    assert!(
        live.starts_with("HTTP/1.1 200") && live.contains("delonix.node.v1"),
        "the live client did not get the API root: {live:?}"
    );
}

/// ADR-0098 A3: with an authorization policy in effect, the caller's identity
/// (its client-certificate subject CN) is checked against the policy before the
/// handler runs. An authorized identity reaches the contract (200); an
/// unauthorized one — a perfectly valid certificate from the same CA, so the A1
/// handshake accepts it — is refused with `403 PERMISSION_DENIED` before
/// dispatch, and the denial is written to the audit log named by its cert
/// identity, while the authorized read is not. This is the in-process proof of
/// the enforcement + audit; the live cross-machine run is the A3 acceptance.
#[test]
fn a_policy_authorizes_by_cert_identity_and_audits_the_denial() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let node_ca = generate_ca("delonix-node-ca");
    let (server_cert, server_key) = issue_leaf(&node_ca, "node", &["127.0.0.1", "localhost"]);
    // Two client certs from the SAME node CA: both pass the handshake (A1); the
    // policy tells them apart (A3).
    let (ok_cert, ok_key) = issue_leaf(&node_ca, "control-plane", &[]);
    let (bad_cert, bad_key) = issue_leaf(&node_ca, "intruder", &[]);

    let audit_path = std::env::temp_dir().join(format!(
        "delonix-node-authz-audit-{}.jsonl",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&audit_path);

    // Only `control-plane` is granted; `intruder` matches no rule (default-deny).
    let policy = delonix_node_api::authz::Policy::from_json(
        r#"{"default":"deny","rules":[{"identity":"control-plane","operations":["*"],"namespaces":["*"]}]}"#,
    )
    .unwrap();
    let authz = delonix_node_api::authz::Authz {
        policy,
        audit: delonix_node_api::authz::AuditSink::to_file(audit_path.to_str().unwrap()).unwrap(),
    };

    let (port, fd) = bound_tcp();
    let tls = delonix_node_api::Tls {
        cert_pem: server_cert,
        key_pem: server_key,
        client_ca_pem: node_ca.cert_pem.clone(),
        client_crl_pem: None,
    };
    std::thread::spawn(move || {
        let _ = delonix_node_api::serve(
            delonix_node_api::Listen::Inherited(fd),
            None,
            Some(tls),
            Some(authz),
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
    std::thread::sleep(std::time::Duration::from_millis(300));

    // The authorized identity reaches the contract.
    let ok = https_get_v1(port, Some((&ok_cert, &ok_key)), &node_ca.cert_pem)
        .expect("the authorized identity reaches the node contract");
    assert!(
        ok.starts_with("HTTP/1.1 200") && ok.contains("delonix.node.v1"),
        "the authorized identity did not get the API root: {ok:?}"
    );

    // The unauthorized identity — a valid cert from the same CA — is refused 403
    // before the handler, with the permission-denied problem document.
    let denied = https_get_v1(port, Some((&bad_cert, &bad_key)), &node_ca.cert_pem)
        .expect("a denied call still produces an HTTP response (not a dropped connection)");
    assert!(
        denied.starts_with("HTTP/1.1 403"),
        "the unauthorized identity was not refused 403: {denied:?}"
    );
    assert!(
        denied.contains("not authorized"),
        "the 403 is not the permission-denied problem document: {denied:?}"
    );

    // The audit log carries the denial, named by the certificate identity; the
    // allowed read is not audited (ADR-0098 §5: denials always, allowed reads not).
    std::thread::sleep(std::time::Duration::from_millis(100));
    let audit = std::fs::read_to_string(&audit_path).unwrap_or_default();
    assert!(
        audit.contains(r#""identity":"intruder""#) && audit.contains(r#""decision":"deny""#),
        "the audit log is missing the denial by identity: {audit:?}"
    );
    assert!(
        !audit.contains(r#""identity":"control-plane""#),
        "an allowed read must not be audited: {audit:?}"
    );
    let _ = std::fs::remove_file(&audit_path);
}
