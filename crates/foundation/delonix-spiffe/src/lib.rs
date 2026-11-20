//! `delonix-spiffe` — the Delonix node's SPIFFE identity, as rustls material.
//!
//! # What this crate is
//!
//! A SPIFFE identity is an **X.509-SVID**: an X.509 leaf whose single source of
//! identity is a `spiffe://<trust-domain>/<path>` URI in its `subjectAltName`,
//! validated by chaining to a **trust bundle** (the SPIFFE authority's roots).
//! This crate turns the three artefacts a SPIFFE authority delivers — the SVID
//! chain, its private key, and the trust bundle, all PEM — into rustls
//! [`ServerConfig`](rustls::ServerConfig) / [`ClientConfig`](rustls::ClientConfig)
//! that PRESENT the node's SVID and VERIFY a peer's SVID by its SPIFFE id.
//!
//! An external SPIFFE authority (a CA such as SPIRE) can therefore be the issuer
//! with no product-specific code here; the engine names the STANDARD, never the
//! product. The engine's own CA stays a first-class fallback — [`self_signed`]
//! mints an SVID with the same shape, so the material → config code path is the
//! same whether the issuer is external or the node itself (ADR-0102).
//!
//! # What this crate is NOT
//!
//! It is not a SPIRE agent and not the Workload API: there is no resident
//! process, no SVID rotation loop, no node-attestor plugin (daemonless by
//! design — those belong with the node-identity lifecycle, A2/ADR-0096). It also
//! owns no transport: the async TLS handshake belongs to the caller, which is why
//! `tokio`/`tokio-rustls` are only DEV dependencies (exercised by `tests/`).
//!
//! ```
//! use delonix_spiffe::{self_signed, server_config, client_config, PeerPolicy, SpiffeId};
//!
//! let id = SpiffeId::parse("spiffe://example.org/node").unwrap();
//! let (svid, bundle) = self_signed(&id).unwrap();
//! // The node presents this SVID on a server AND a client, mTLS on both ends.
//! let _server = server_config(&svid, &bundle, PeerPolicy::TrustDomain("example.org".into())).unwrap();
//! let _client = client_config(&svid, &bundle, PeerPolicy::Id(id)).unwrap();
//! ```

mod config;
mod id;
mod material;
mod san;
mod verify;

pub use config::{client_config, self_signed, server_config};
pub use id::SpiffeId;
pub use material::{SvidMaterial, TrustBundle};
pub use san::uri_sans;
pub use verify::PeerPolicy;

/// What can go wrong building a SPIFFE identity. Fail-closed: a malformed id,
/// unreadable material, or a config rustls refuses is an error, never a default.
#[derive(Debug)]
pub enum Error {
    /// A malformed or out-of-spec SPIFFE id.
    Spiffe(String),
    /// Bad PEM, a missing key, or an empty chain/bundle.
    Material(String),
    /// rustls refused to build the config from the material.
    Tls(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Spiffe(m) => write!(f, "SPIFFE id: {m}"),
            Error::Material(m) => write!(f, "SVID material: {m}"),
            Error::Tls(m) => write!(f, "TLS config: {m}"),
        }
    }
}

impl std::error::Error for Error {}

/// The crate's result type.
pub type Result<T> = std::result::Result<T, Error>;
