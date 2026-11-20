# ADR-0102 — SPIFFE node identity (X.509-SVID) and a generic workload-attestation RPC

**Status:** Accepted (2026-11-20, by the owner) — Phase H0 of the CNCF compatibility
campaign (mission H), built this session as `delonix-spiffe` + the node contract's
`AttestWorkload` RPC, proven by a real `spire-server` issuing the node certificate and a
real mutual-TLS handshake (`tests/interop/spiffe/`, `reports/interop/h0-spiffe/`).
**Relates to:** ADR-0096 (the node API over TCP with mTLS — the transport this identity
rides on, when it lands), which names A2 as "node identity: … external CA"; this ADR is
the SPIFFE-standard realization of that identity, sequenced before Phase A at the owner's
request. ADR-0040/0042 (one node contract, two encodings). **Doctrine:** the engine names
the STANDARD (SPIFFE, X.509-SVID — RFC) and never the product; "SPIRE" appears only under
`tests/interop/` and `docs/`.

> **The decision, in one line:** the node's mTLS identity MAY be an **X.509-SVID** — an
> X.509 leaf whose `subjectAltName` carries a `spiffe://<trust-domain>/<path>` URI and is
> chained to a **trust bundle the node is configured with**, so an external SPIFFE
> authority (a CA such as SPIRE) can be the issuer with **no product-specific code** in the
> engine. The engine's **own CA stays a first-class fallback** (an rcgen-minted self-signed
> SVID). Separately, the node contract gains a generic **workload-attestation** RPC: given a
> host PID, it returns the workload identity, the image digest and the labels — the facts a
> node-local attestor needs, exposed as a plain contract operation.

## Context

The node contract (`delonix.node.v1`) is served today only on a local unix socket, with no
TLS (ADR-0040/0042). ADR-0096 (A1) adds a TCP listener that requires mutual TLS against a
configurable CA, and explicitly defers "node identity … external CA" to A2. The CNCF
campaign's H0 asks a narrower, standards-shaped question ahead of Phase A: **can the Delonix
node carry a SPIFFE identity so that a SPIFFE authority — not the engine — is the root of
trust?** SPIFFE answers "who is this workload/node" with an X.509-SVID: a short-lived leaf
whose single source of identity is a `spiffe://` URI SAN, validated by chaining to a trust
bundle (the SPIFFE authority's roots).

Two forces shape the decision:

- **The engine knows no consumer, and names no product.** A SPIFFE identity must be
  expressible with open material only — PEM cert, PEM key, PEM bundle, and a URI — never a
  dependency on a specific issuer's API. Whatever mints the SVID (a SPIRE server, another
  CA, or the engine itself) is interchangeable.
- **Daemonless by design.** H0 adds no resident process, no Workload API client, no agent,
  no rotation loop. It is a pure library (`delonix-spiffe`) that turns material into rustls
  configs, plus one read-only contract RPC. SVID rotation and the Workload API are later
  work (they belong with A2's lifecycle, not here).

### Why a separate `delonix-spiffe` foundation crate

The SVID identity is a cross-cutting primitive — the TCP transport (A1), a future client,
and tests all need "build an mTLS config from SVID material and verify a peer's SPIFFE ID".
It depends only on `rustls`, `rustls-pki-types`, `rcgen` and `delonix-model` (all foundation
or non-HEAVY), so it sits in `crates/foundation/` and every layer above may use it. The
async handshake belongs to the caller; the crate itself stays runtime-free (`tokio` /
`tokio-rustls` are **dev-dependencies**, used only by its integration test).

### Why not pull in an X.509 parser for the URI SAN

Reading the `spiffe://` URI out of a leaf's `subjectAltName` is a bounded, well-specified
DER walk (the `GeneralName` `uniformResourceIdentifier` `[6]` of OID `2.5.29.17`). Adding
`x509-parser` pulls a cluster of new ASN.1 crates and trips the dependency gates for one
field; the crate instead carries a small, tested SAN extractor — the same reasoning that
kept `rustls-pemfile` out of `ingress_proxy.rs` (RUSTSEC-2025-0134) in favour of
`rustls-pki-types`' own PEM reader.

## Decision

1. **`delonix-spiffe` (new foundation crate).**
   - `SpiffeId` — parse/validate `spiffe://<trust-domain>/<path>`: scheme fixed, a non-empty
     trust domain, no query/fragment, lower-cased host (RFC-3986 host rules for the trust
     domain). Fail-closed: a malformed id is an error, never a default.
   - `SvidMaterial { chain_pem, key_pem }` + a `TrustBundle` of root certs (PEM), loaded from
     strings or files — exactly the three artefacts a SPIFFE authority delivers.
   - `server_config(svid, peer: PeerPolicy)` → `rustls::ServerConfig` presenting the SVID,
     and `client_config(svid, server: PeerPolicy)` → `rustls::ClientConfig` presenting it.
     `PeerPolicy` is `TrustDomain(name)` or `Id(spiffe_id)` or `AnyInBundle`: the peer's
     certificate is first chained to the trust bundle (webpki), THEN its `spiffe://` URI SAN
     is extracted and matched against the policy. A valid chain with no SPIFFE ID, or an ID
     outside the policy, is rejected — a chain check alone is not a SPIFFE check.
   - Own-CA fallback: `self_signed(spiffe_id)` mints, via `rcgen`, a leaf+key whose only SAN
     is the `spiffe://` URI and a one-cert bundle to verify it — "the own CA stays possible".
2. **Node contract — `NodeService.AttestWorkload`.** `AttestWorkloadRequest { int64 pid }`
   → `WorkloadAttestation { int64 pid; string container_id; string name; string namespace;
   string pod; string image; string image_digest; map<string,string> labels;
   map<string,string> annotations; bool found }`, mapped to `GET
   /v1/node/workloads/{pid}:attest`. The handler reads the node's own container records
   (`StorePort::container_list`), matches the PID to the live init process (confirmed against
   `pid_starttime` to defeat PID reuse), resolves the image digest best-effort from the image
   store (`ImageRepo::list` matched on the reference), and returns the container's labels and
   annotations. An unmatched PID returns `found: false` with the pid echoed — never an error,
   never a guess. The RPC is read-only and carries no SVID itself; it is the attestation
   *source* a selector would consume.
3. **No transport change.** H0 wires the SVID onto no live listener and does not touch the
   unix-socket default. The proof that "a SPIRE server issues the node certificate and the
   node TLS layer accepts it" is a real mutual handshake in `delonix-spiffe`'s test, driven
   by SVIDs a real `spire-server` mints in `tests/interop/spiffe/`.

## Consequences

- A1's TCP listener (ADR-0096) can, when it lands, build its `ServerConfig` from
  `delonix-spiffe` instead of a bare CA file, giving the node a SPIFFE identity for free; A2
  then owns rotation and the Workload API. This ADR deliberately stops at the material→config
  boundary so those remain open.
- The attestation RPC exposes only what the node already records. Image **labels** and
  os/arch are not persisted anywhere today (the OCI config's `Labels` are not parsed), so the
  label source is the container record's labels/annotations; the digest is resolved from the
  image store and may be empty when the reference is not a stored tag. Both limits are stated
  in the record, not hidden.
- No new privilege boundary and no daemon, so no GO/NO-GO spike is required (guardrails #1,
  #5). The crate is dependency-clean (no private repo, nothing HEAVY in non-dev deps).
