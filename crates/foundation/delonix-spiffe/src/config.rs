//! Turning SVID material into rustls configs, and the own-CA fallback.
//!
//! The crypto provider is `ring`, passed EXPLICITLY (`builder_with_provider`)
//! rather than taken from the process default — a foundation crate must not
//! depend on some other part of the process having installed a default, and must
//! not race to install one itself.

use std::sync::Arc;

use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, Ia5String, IsCa, KeyPair,
    SanType,
};
use rustls::{ClientConfig, ServerConfig};

use crate::id::SpiffeId;
use crate::material::{SvidMaterial, TrustBundle};
use crate::verify::{PeerPolicy, SpiffeClientVerifier, SpiffeServerVerifier};
use crate::{Error, Result};

fn provider() -> Arc<rustls::crypto::CryptoProvider> {
    Arc::new(rustls::crypto::ring::default_provider())
}

/// A rustls `ServerConfig` that PRESENTS the node's SVID and requires the
/// connecting client to present an SVID the `client` policy accepts — mutual TLS,
/// SPIFFE on both ends. This is the config A1's TCP listener (ADR-0096) would
/// build the node's identity from.
pub fn server_config(
    svid: &SvidMaterial,
    bundle: &TrustBundle,
    client: PeerPolicy,
) -> Result<ServerConfig> {
    let provider = provider();
    let algs = provider.signature_verification_algorithms;
    let verifier = SpiffeClientVerifier::new(bundle.roots.clone(), algs, client);
    ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Tls(format!("server protocol versions: {e}")))?
        .with_client_cert_verifier(verifier)
        .with_single_cert(svid.chain.clone(), svid.key.clone_key())
        .map_err(|e| Error::Tls(format!("server SVID cert/key: {e}")))
}

/// A rustls `ClientConfig` that PRESENTS the node's SVID and verifies the server
/// presents an SVID the `server` policy accepts. The stock server-name check is
/// replaced by the SPIFFE id check — a SPIFFE SVID has no DNS SAN to match.
pub fn client_config(
    svid: &SvidMaterial,
    bundle: &TrustBundle,
    server: PeerPolicy,
) -> Result<ClientConfig> {
    let provider = provider();
    let algs = provider.signature_verification_algorithms;
    let verifier = SpiffeServerVerifier::new(bundle.roots.clone(), algs, server);
    ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Tls(format!("client protocol versions: {e}")))?
        .dangerous()
        .with_custom_certificate_verifier(verifier)
        .with_client_auth_cert(svid.chain.clone(), svid.key.clone_key())
        .map_err(|e| Error::Tls(format!("client SVID cert/key: {e}")))
}

/// The own-CA fallback: mint, with the engine's own CA (rcgen), an SVID for
/// `id` plus the one-cert bundle that verifies it. "The own CA stays possible" —
/// the same code path (material → config) serves a SPIRE-issued SVID and a
/// self-issued one; only the issuer differs. The leaf carries both server and
/// client EKU, as a SPIFFE SVID does, so it works on either end of an mTLS pair.
pub fn self_signed(id: &SpiffeId) -> Result<(SvidMaterial, TrustBundle)> {
    let rcgen_err =
        |ctx: &'static str| move |e: rcgen::Error| Error::Material(format!("{ctx}: {e}"));

    // The own CA.
    let ca_key = KeyPair::generate().map_err(rcgen_err("own CA key"))?;
    let mut ca_params =
        CertificateParams::new(Vec::<String>::new()).map_err(rcgen_err("own CA"))?;
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "delonix-spiffe local trust domain");
    let ca = ca_params
        .self_signed(&ca_key)
        .map_err(rcgen_err("own CA"))?;

    // The leaf SVID: its only identity is the spiffe:// URI SAN.
    let uri = Ia5String::try_from(id.uri())
        .map_err(|e| Error::Spiffe(format!("SPIFFE id is not a valid URI SAN: {e}")))?;
    let leaf_key = KeyPair::generate().map_err(rcgen_err("SVID key"))?;
    let mut leaf_params =
        CertificateParams::new(Vec::<String>::new()).map_err(rcgen_err("SVID"))?;
    leaf_params.is_ca = IsCa::NoCa;
    leaf_params.subject_alt_names = vec![SanType::URI(uri)];
    leaf_params.extended_key_usages = vec![
        ExtendedKeyUsagePurpose::ServerAuth,
        ExtendedKeyUsagePurpose::ClientAuth,
    ];
    let leaf = leaf_params
        .signed_by(&leaf_key, &ca, &ca_key)
        .map_err(rcgen_err("SVID"))?;

    let svid = SvidMaterial::from_pem(&leaf.pem(), &leaf_key.serialize_pem())?;
    let bundle = TrustBundle::from_pem(&ca.pem())?;
    Ok((svid, bundle))
}
