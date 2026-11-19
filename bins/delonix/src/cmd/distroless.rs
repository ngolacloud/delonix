//! Distroless-by-default: the engine's third pillar beside rootless-first and
//! daemonless. Delonix scaffolds containers whose RUNTIME stage is a distroless
//! image — no shell, no package manager, no OS userland — so the attack surface
//! is the application and nothing else.
//!
//! # Two moving parts
//!
//! 1. **Configurable registry.** The distroless registry defaults to
//!    `gcr.io/distroless` (Google's distroless, the industry default) and is
//!    overridable with `DELONIX_DISTROLESS_REGISTRY` — point it at a mirror, an
//!    air-gapped registry, or Chainguard. The scaffold bakes the resolved value
//!    into every generated `Delonixfile` (the `__DISTROLESS_REGISTRY__` token).
//!
//! 2. **Fallback on pull.** A distroless image can be unreachable: an air-gapped
//!    host, a blocked registry, an arch with no published variant. When the pull
//!    of a *recognised* distroless reference comes back **not found** (NOT a
//!    network/auth error — those must still surface), the engine retries with a
//!    conventional base that carries the same runtime, and says so on stderr.
//!    Distroless-by-default never becomes distroless-or-nothing.

/// Environment override for the distroless registry (host + namespace, no
/// trailing slash). Empty/unset → [`DEFAULT_DISTROLESS_REGISTRY`].
pub(crate) const DISTROLESS_REGISTRY_ENV: &str = "DELONIX_DISTROLESS_REGISTRY";

/// The default distroless registry. Google's distroless is the de-facto
/// standard: static/base/cc/python3/nodejs variants, cosign-signed, multi-arch.
pub(crate) const DEFAULT_DISTROLESS_REGISTRY: &str = "gcr.io/distroless";

/// The scaffold token the generated `Delonixfile`s carry; substituted with
/// [`registry`] at generation time so a project is pinned to the registry that
/// was configured when it was created.
pub(crate) const REGISTRY_TOKEN: &str = "__DISTROLESS_REGISTRY__";

/// One runtime flavour: the distroless image (relative to the registry) and the
/// conventional base that carries the SAME runtime when the distroless one
/// cannot be pulled. The fallback is version-consistent with the matching
/// template's build stage (e.g. a venv built on Python 3.11 falls back to
/// `python:3.11-slim`, never 3.13).
pub(crate) struct Flavour {
    /// Path under the registry, including the tag — e.g. `static-debian12:nonroot`.
    pub suffix: &'static str,
    /// Conventional, always-reachable base carrying the same runtime.
    pub fallback: &'static str,
}

/// The flavours Delonix scaffolds. Keyed by the bare image name (the suffix with
/// its tag stripped), which is what [`fallback_for`] matches against.
pub(crate) const FLAVOURS: &[Flavour] = &[
    // Static binaries (Go built with CGO_ENABLED=0).
    Flavour { suffix: "static-debian12:nonroot", fallback: "alpine:3.24" },
    // glibc + libgcc/libstdc++ (Rust, C/C++).
    Flavour { suffix: "cc-debian12:nonroot", fallback: "debian:trixie-slim" },
    // Generic apps that only need glibc + CA roots.
    Flavour { suffix: "base-debian12:nonroot", fallback: "debian:trixie-slim" },
    // NOTE — Node AND Python have NO fallback on purpose: their interpreter path
    // diverges between the distroless image and the conventional base, and a
    // base swap cannot rewrite the command.
    //   · distroless nodejs → `/nodejs/bin/node`; `node:*-slim` → `/usr/local/bin/node`.
    //   · distroless python3 → `/usr/bin/python3.11`; `python:*-slim` → `/usr/local/bin/python3.11`.
    // A `*-slim` fallback would yield a container whose command is `ENOENT` at
    // start — strictly worse than a clear "image not found". Both therefore
    // surface the not-found error; the conventional path stays reachable by
    // scaffolding with `--image node:22-slim` / `--image python:3.11-slim`.
    // Go (static) and Rust (cc→glibc) have no such divergence: their binary
    // lives at the SAME path on the distroless image and the conventional base,
    // so those fallbacks boot unchanged.
];

/// The effective distroless registry: `DELONIX_DISTROLESS_REGISTRY` when set to
/// a non-empty value, otherwise [`DEFAULT_DISTROLESS_REGISTRY`]. A trailing `/`
/// is trimmed so the token substitution never produces a double slash.
pub(crate) fn registry() -> String {
    std::env::var(DISTROLESS_REGISTRY_ENV)
        .ok()
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_DISTROLESS_REGISTRY.to_string())
}

/// Strip an optional `:tag` and/or `@digest` from a reference's final path
/// component, returning `(bare_reference, last_segment_without_tag)`.
fn bare_and_last(reference: &str) -> (String, String) {
    // Drop a digest first (`@sha256:…`), then a tag on the LAST path segment
    // only (a registry port like `host:5000/img` must not be mistaken for a tag).
    let no_digest = reference.split('@').next().unwrap_or(reference);
    let (prefix, last) = match no_digest.rsplit_once('/') {
        Some((p, l)) => (Some(p), l),
        None => (None, no_digest),
    };
    let last_bare = last.split(':').next().unwrap_or(last);
    let bare = match prefix {
        Some(p) => format!("{p}/{last_bare}"),
        None => last_bare.to_string(),
    };
    (bare, last_bare.to_string())
}

/// Does this reference look like a distroless image (so a not-found pull is a
/// candidate for the conventional fallback)? True when it sits under the
/// configured registry, under the built-in `gcr.io/distroless` default (so
/// projects scaffolded before an env change still fall back), or under
/// Chainguard's `cgr.dev`.
fn looks_distroless(bare: &str) -> bool {
    let reg = registry();
    bare.starts_with(&format!("{reg}/"))
        || bare.contains("/distroless/")
        || bare.starts_with("cgr.dev/")
}

/// The conventional fallback for a distroless reference, or `None` if this is
/// not a recognised distroless image (an arbitrary user image must never be
/// silently swapped). Matching is by the bare image name (tag stripped) so an
/// overridden registry still resolves — `<any-registry>/static-debian12:nonroot`
/// and `gcr.io/distroless/static-debian12` both map to the static fallback.
pub(crate) fn fallback_for(reference: &str) -> Option<&'static str> {
    let (bare, last) = bare_and_last(reference);
    if !looks_distroless(&bare) {
        return None;
    }
    FLAVOURS
        .iter()
        .find(|f| f.suffix.split(':').next() == Some(last.as_str()))
        .map(|f| f.fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_registry_when_unset() {
        // The test must not depend on the ambient env; assert the pure default.
        assert_eq!(DEFAULT_DISTROLESS_REGISTRY, "gcr.io/distroless");
    }

    #[test]
    fn bare_and_last_strips_tag_and_digest() {
        let (b, l) = bare_and_last("gcr.io/distroless/static-debian12:nonroot");
        assert_eq!(b, "gcr.io/distroless/static-debian12");
        assert_eq!(l, "static-debian12");
        let (b, l) = bare_and_last("host:5000/distroless/python3-debian12@sha256:abc");
        assert_eq!(b, "host:5000/distroless/python3-debian12");
        assert_eq!(l, "python3-debian12");
    }

    #[test]
    fn fallback_matches_distroless_only() {
        assert_eq!(
            fallback_for("gcr.io/distroless/static-debian12:nonroot"),
            Some("alpine:3.24")
        );
        // Python is excluded like Node (interpreter path diverges) — not-found surfaces.
        assert_eq!(fallback_for("gcr.io/distroless/python3-debian12:nonroot"), None);
        // An arbitrary image is never swapped.
        assert_eq!(fallback_for("alpine:3.20"), None);
        assert_eq!(fallback_for("docker.io/library/nginx:1.27"), None);
        // Unknown distroless variant: recognised as distroless but no mapping.
        assert_eq!(fallback_for("gcr.io/distroless/java21-debian12"), None);
        // Node is deliberately excluded (the ENTRYPOINT path diverges between
        // distroless and node:*-slim) — a not-found must surface, not fall back.
        assert_eq!(fallback_for("gcr.io/distroless/nodejs22-debian12:nonroot"), None);
    }

    #[test]
    fn fallback_resolves_a_mirror_that_keeps_the_distroless_namespace() {
        // Matching is by bare image name, so a mirror under `.../distroless/…`
        // still resolves without depending on the ambient env var.
        assert_eq!(
            fallback_for("my-mirror.example.com/distroless/base-debian12:nonroot"),
            Some("debian:trixie-slim")
        );
    }
}
