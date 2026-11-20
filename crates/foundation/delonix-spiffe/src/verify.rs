//! The SPIFFE peer verifiers. A SPIFFE X.509-SVID identifies by its `spiffe://`
//! URI SAN, NOT by a DNS name — so rustls's stock `WebPki*Verifier`, which
//! enforces a server-name match, cannot be used as-is. These verifiers do the
//! two checks SPIFFE actually requires, in order:
//!
//! 1. the peer certificate chains to the configured trust bundle (webpki path
//!    validation, with the right EKU and validity — no DNS-name check);
//! 2. the peer's leaf carries a `spiffe://` URI SAN whose id the [`PeerPolicy`]
//!    accepts.
//!
//! A valid chain with no SPIFFE id, or an id outside the policy, is rejected: a
//! chain check alone is not a SPIFFE check.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, WebPkiSupportedAlgorithms};
use rustls::pki_types::{CertificateDer, ServerName, TrustAnchor, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, Error as TlsError, SignatureScheme};
use webpki::{EndEntityCert, KeyUsage};

use crate::id::SpiffeId;

/// Which peer identities a verifier accepts, after the chain is proven.
#[derive(Debug, Clone)]
pub enum PeerPolicy {
    /// Any valid SPIFFE id that chains to the bundle — used when the trust
    /// bundle is the whole authorization.
    AnyInBundle,
    /// Any id in this trust domain (`spiffe://<this>/...`).
    TrustDomain(String),
    /// Exactly this id.
    Id(SpiffeId),
}

impl PeerPolicy {
    fn accepts(&self, id: &SpiffeId) -> bool {
        match self {
            PeerPolicy::AnyInBundle => true,
            PeerPolicy::TrustDomain(td) => id.trust_domain() == td,
            PeerPolicy::Id(expected) => id == expected,
        }
    }
}

/// Chain the peer leaf to the bundle (no name check), then require a
/// policy-accepted `spiffe://` URI SAN. Shared by both verifiers.
fn verify_svid(
    bundle: &[CertificateDer<'static>],
    algs: &WebPkiSupportedAlgorithms,
    policy: &PeerPolicy,
    end_entity: &CertificateDer<'_>,
    intermediates: &[CertificateDer<'_>],
    now: UnixTime,
    usage: KeyUsage,
) -> Result<(), TlsError> {
    let anchors: Vec<TrustAnchor<'_>> = bundle
        .iter()
        .map(webpki::anchor_from_trusted_cert)
        .collect::<std::result::Result<_, _>>()
        .map_err(|e| TlsError::General(format!("SPIFFE trust bundle: {e}")))?;
    let ee = EndEntityCert::try_from(end_entity)
        .map_err(|e| TlsError::General(format!("SPIFFE peer leaf: {e}")))?;
    ee.verify_for_usage(algs.all, &anchors, intermediates, now, usage, None, None)
        .map_err(|e| TlsError::General(format!("SPIFFE chain does not validate: {e}")))?;
    // Identity is the URI SAN. Take the first id the policy accepts; if none is
    // present or accepted, the peer is not who the policy requires.
    let accepted = crate::san::uri_sans(end_entity)
        .iter()
        .filter_map(|u| SpiffeId::parse(u).ok())
        .any(|id| policy.accepts(&id));
    if !accepted {
        return Err(TlsError::General(
            "peer certificate carries no SPIFFE id the policy accepts".into(),
        ));
    }
    Ok(())
}

/// Verifies the SERVER's SVID (installed on a CLIENT config).
#[derive(Debug)]
pub(crate) struct SpiffeServerVerifier {
    bundle: Vec<CertificateDer<'static>>,
    algs: WebPkiSupportedAlgorithms,
    policy: PeerPolicy,
}

impl SpiffeServerVerifier {
    pub(crate) fn new(
        bundle: Vec<CertificateDer<'static>>,
        algs: WebPkiSupportedAlgorithms,
        policy: PeerPolicy,
    ) -> Arc<Self> {
        Arc::new(Self {
            bundle,
            algs,
            policy,
        })
    }
}

impl ServerCertVerifier for SpiffeServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        verify_svid(
            &self.bundle,
            &self.algs,
            &self.policy,
            end_entity,
            intermediates,
            now,
            KeyUsage::server_auth(),
        )?;
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}

/// Verifies the CLIENT's SVID (installed on a SERVER config). mTLS is mandatory —
/// the defaults of `ClientCertVerifier` already require a client certificate.
#[derive(Debug)]
pub(crate) struct SpiffeClientVerifier {
    bundle: Vec<CertificateDer<'static>>,
    algs: WebPkiSupportedAlgorithms,
    policy: PeerPolicy,
    root_hints: Vec<DistinguishedName>,
}

impl SpiffeClientVerifier {
    pub(crate) fn new(
        bundle: Vec<CertificateDer<'static>>,
        algs: WebPkiSupportedAlgorithms,
        policy: PeerPolicy,
    ) -> Arc<Self> {
        Arc::new(Self {
            bundle,
            algs,
            policy,
            root_hints: Vec::new(),
        })
    }
}

impl ClientCertVerifier for SpiffeClientVerifier {
    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &self.root_hints
    }

    fn verify_client_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        now: UnixTime,
    ) -> Result<ClientCertVerified, TlsError> {
        verify_svid(
            &self.bundle,
            &self.algs,
            &self.policy,
            end_entity,
            intermediates,
            now,
            KeyUsage::client_auth(),
        )?;
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls12_signature(message, cert, dss, &self.algs)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        verify_tls13_signature(message, cert, dss, &self.algs)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algs.supported_schemes()
    }
}
