//! The three PEM artefacts a SPIFFE authority delivers, as rustls types: the
//! SVID (leaf + any intermediates) with its private key, and the trust bundle
//! (the authority's root certs). Loading is fail-closed — an empty chain, a
//! missing key, or a bundle with no certificate is an error here, not a config
//! that fails to handshake later with no explanation.

use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};

use crate::{Error, Result};

/// An X.509-SVID: the leaf first, any intermediates after, and the matching
/// private key. This is what the node PRESENTS — on a TLS server, a TLS client,
/// or both (a SPIFFE SVID carries server+client EKU).
pub struct SvidMaterial {
    pub(crate) chain: Vec<CertificateDer<'static>>,
    pub(crate) key: PrivateKeyDer<'static>,
}

impl SvidMaterial {
    /// Loads an SVID from a certificate-chain PEM (leaf first) and a private-key
    /// PEM — the two files a SPIRE agent writes, or `spire-server x509 mint`
    /// prints.
    pub fn from_pem(chain_pem: &str, key_pem: &str) -> Result<Self> {
        let chain: Vec<CertificateDer<'static>> =
            CertificateDer::pem_slice_iter(chain_pem.as_bytes())
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| Error::Material(format!("SVID certificate PEM: {e}")))?;
        if chain.is_empty() {
            return Err(Error::Material(
                "SVID certificate PEM has no CERTIFICATE".into(),
            ));
        }
        let key = PrivateKeyDer::from_pem_slice(key_pem.as_bytes())
            .map_err(|e| Error::Material(format!("SVID private-key PEM: {e}")))?;
        Ok(SvidMaterial { chain, key })
    }

    /// Same, reading the two PEMs from files.
    pub fn from_files(
        chain_path: impl AsRef<std::path::Path>,
        key_path: impl AsRef<std::path::Path>,
    ) -> Result<Self> {
        let chain = read(chain_path.as_ref())?;
        let key = read(key_path.as_ref())?;
        Self::from_pem(&chain, &key)
    }

    /// The leaf — the certificate whose `spiffe://` SAN is this SVID's identity.
    fn leaf(&self) -> &CertificateDer<'static> {
        &self.chain[0] // non-empty by construction
    }

    /// This SVID's own SPIFFE id — the `spiffe://` URI SAN of its leaf — so the
    /// node can learn the identity it presents. `None` if the leaf carries no
    /// (valid) SPIFFE URI SAN, which for a real SVID would be a defect in the
    /// issuer, not here.
    pub fn spiffe_id(&self) -> Option<crate::SpiffeId> {
        crate::san::uri_sans(self.leaf())
            .iter()
            .find_map(|u| crate::SpiffeId::parse(u).ok())
    }
}

/// The SPIFFE trust bundle: the authority's root certificate(s). A peer SVID is
/// accepted only if it chains to one of these.
#[derive(Clone)]
pub struct TrustBundle {
    pub(crate) roots: Vec<CertificateDer<'static>>,
}

impl TrustBundle {
    /// Loads a bundle from a PEM that holds one or more root certificates (the
    /// `bundle.pem` SPIRE writes).
    pub fn from_pem(bundle_pem: &str) -> Result<Self> {
        let roots: Vec<CertificateDer<'static>> =
            CertificateDer::pem_slice_iter(bundle_pem.as_bytes())
                .collect::<std::result::Result<_, _>>()
                .map_err(|e| Error::Material(format!("trust-bundle PEM: {e}")))?;
        if roots.is_empty() {
            return Err(Error::Material(
                "trust-bundle PEM has no CERTIFICATE".into(),
            ));
        }
        Ok(TrustBundle { roots })
    }

    /// Same, reading the PEM from a file.
    pub fn from_file(path: impl AsRef<std::path::Path>) -> Result<Self> {
        Self::from_pem(&read(path.as_ref())?)
    }
}

fn read(path: &std::path::Path) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| Error::Material(format!("read {}: {e}", path.display())))
}
