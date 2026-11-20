//! `SpiffeId` — a parsed, validated SPIFFE ID (`spiffe://<trust-domain>/<path>`).
//!
//! The identity of an X.509-SVID is ENTIRELY in this URI (it is the single
//! `uniformResourceIdentifier` SAN of the leaf). Parsing is fail-closed: a
//! malformed id is an error, never a silently-accepted default — a verifier that
//! guessed an identity would be worse than one that refused.

use crate::{Error, Result};

/// The fixed scheme of every SPIFFE ID.
const SCHEME: &str = "spiffe://";
/// SPIFFE IDs are bounded (SPIFFE-ID spec §2): 2048 bytes total, trust domain
/// ≤ 255. A bound is what keeps a crafted SAN from being a denial-of-service.
const MAX_ID: usize = 2048;
const MAX_TRUST_DOMAIN: usize = 255;

/// A validated SPIFFE ID. The trust domain and the path are kept apart because a
/// peer policy matches on the trust domain alone far more often than on the whole
/// id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpiffeId {
    trust_domain: String,
    /// Path WITH its leading `/` (e.g. `/node/web-1`), or empty for a bare
    /// trust-domain id (`spiffe://example.org`).
    path: String,
}

impl SpiffeId {
    /// Parses and validates a SPIFFE ID. The rules are the SPIFFE-ID spec's: the
    /// `spiffe` scheme, a non-empty trust domain of `[a-z0-9.-_]` (no userinfo,
    /// no port, no uppercase), and a path with no empty, `.` or `..` segments and
    /// no query or fragment.
    pub fn parse(s: &str) -> Result<Self> {
        if s.len() > MAX_ID {
            return Err(Error::Spiffe(format!(
                "SPIFFE ID longer than {MAX_ID} bytes"
            )));
        }
        let rest = s.strip_prefix(SCHEME).ok_or_else(|| {
            Error::Spiffe(format!("not a SPIFFE ID (no `{SCHEME}` scheme): {s:?}"))
        })?;
        // A query or fragment has no place in a SPIFFE ID; reject before the split
        // so neither can hide inside the path.
        if let Some(bad) = rest.find(['?', '#']) {
            return Err(Error::Spiffe(format!(
                "SPIFFE ID carries a query/fragment at byte {bad}: {s:?}"
            )));
        }
        let (authority, path) = match rest.find('/') {
            Some(i) => (&rest[..i], &rest[i..]),
            None => (rest, ""),
        };
        Self::check_trust_domain(authority)?;
        Self::check_path(path)?;
        Ok(SpiffeId {
            trust_domain: authority.to_string(),
            path: path.to_string(),
        })
    }

    fn check_trust_domain(td: &str) -> Result<()> {
        if td.is_empty() {
            return Err(Error::Spiffe("SPIFFE ID has an empty trust domain".into()));
        }
        if td.len() > MAX_TRUST_DOMAIN {
            return Err(Error::Spiffe(format!(
                "trust domain longer than {MAX_TRUST_DOMAIN} bytes"
            )));
        }
        // No userinfo (`@`) and no port (`:`) — a SPIFFE trust domain is a bare
        // name, not a URL authority. The allowed set is the spec's.
        for c in td.chars() {
            let ok = c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_');
            if !ok {
                return Err(Error::Spiffe(format!(
                    "trust domain has a character outside [a-z0-9.-_]: {c:?}"
                )));
            }
        }
        Ok(())
    }

    fn check_path(path: &str) -> Result<()> {
        if path.is_empty() {
            return Ok(());
        }
        // A path, when present, is absolute and has no empty, `.` or `..`
        // segments (SPIFFE-ID spec §2.2) — `split` on the leading `/` yields a
        // first empty element that is expected, every other empty one is a `//`.
        let mut segs = path.split('/');
        let first = segs.next();
        debug_assert_eq!(first, Some("")); // the leading '/'
        for seg in segs {
            if seg.is_empty() {
                return Err(Error::Spiffe("SPIFFE ID path has an empty segment (`//`)".into()));
            }
            if seg == "." || seg == ".." {
                return Err(Error::Spiffe(format!(
                    "SPIFFE ID path has a relative segment ({seg:?})"
                )));
            }
        }
        Ok(())
    }

    /// The trust domain (`example.org`), the authority half of the id.
    pub fn trust_domain(&self) -> &str {
        &self.trust_domain
    }

    /// The path including its leading `/`, or `""` for a bare trust-domain id.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The full id, exactly as it appears in the SAN (`spiffe://example.org/node`).
    pub fn uri(&self) -> String {
        format!("{SCHEME}{}{}", self.trust_domain, self.path)
    }
}

impl std::fmt::Display for SpiffeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{SCHEME}{}{}", self.trust_domain, self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_trust_domain_and_path() {
        let id = SpiffeId::parse("spiffe://example.org/node/web-1").unwrap();
        assert_eq!(id.trust_domain(), "example.org");
        assert_eq!(id.path(), "/node/web-1");
        assert_eq!(id.uri(), "spiffe://example.org/node/web-1");
        assert_eq!(id.to_string(), "spiffe://example.org/node/web-1");
    }

    #[test]
    fn parses_bare_trust_domain() {
        let id = SpiffeId::parse("spiffe://example.org").unwrap();
        assert_eq!(id.trust_domain(), "example.org");
        assert_eq!(id.path(), "");
        assert_eq!(id.uri(), "spiffe://example.org");
    }

    #[test]
    fn rejects_wrong_scheme() {
        assert!(SpiffeId::parse("https://example.org/node").is_err());
        assert!(SpiffeId::parse("spiffe:/example.org").is_err());
    }

    #[test]
    fn rejects_empty_trust_domain() {
        assert!(SpiffeId::parse("spiffe:///node").is_err());
    }

    #[test]
    fn rejects_uppercase_userinfo_and_port() {
        assert!(SpiffeId::parse("spiffe://Example.org/node").is_err());
        assert!(SpiffeId::parse("spiffe://user@example.org/node").is_err());
        assert!(SpiffeId::parse("spiffe://example.org:8443/node").is_err());
    }

    #[test]
    fn rejects_bad_path() {
        assert!(SpiffeId::parse("spiffe://example.org/a//b").is_err());
        assert!(SpiffeId::parse("spiffe://example.org/./b").is_err());
        assert!(SpiffeId::parse("spiffe://example.org/../b").is_err());
        assert!(SpiffeId::parse("spiffe://example.org/a?x=1").is_err());
        assert!(SpiffeId::parse("spiffe://example.org/a#f").is_err());
    }
}
