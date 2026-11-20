# ADR-0097 — A node earns its identity by enrolling: one-time token → CSR → short-lived client cert, signed by a node CA, revocable by CRL

**Status:** Accepted (2026-11-19) — Phase A/A2 of the fleet-operability brief
(a node operable by an external control plane). **Builds on:** ADR-0096 (A1 — the
node contract served over TCP with mutual TLS; a client certificate chaining to a
configured `--tls-client-ca` is required at the handshake). **Relates to:**
ADR-0040 (the node contract and its socket activation), ADR-0075 (D1 — a server
is resolved beside the running exe). **Followed by:** A3 (authorization by the
caller's certificate identity + a per-call audit record). This ADR gives A1's
`--tls-client-ca` a *source* — a node CA and a way for a new member to obtain a
certificate from it — and a *sink* — a CRL the server consults. It does **not**
decide who may do what (A3): until A3, an enrolled, non-revoked client is still
authorized for everything the socket serves, exactly as ADR-0096 §6 states.

> **The decision, in one line:** a new fleet member proves possession of a
> **one-time enrollment token** to a **node CA** (an enrollment authority that
> holds the CA key, external to every node-API server), sending a **CSR**; the
> authority returns a **short-lived client certificate** (`clientAuth`, default
> 1 h) that the member then presents to the A1 mutual-TLS listener. Rotation is
> free — the server trusts the CA, not the leaf, so a fresh certificate from the
> same CA is accepted with no restart and the old one simply expires. Revocation
> is a **CRL** signed by the node CA and loaded by the server
> (`WebPkiClientVerifier::with_crls`); a revoked or expired certificate is
> refused at the handshake.

## Context

A1 made the node contract reachable from another host and authenticated the
transport: a TCP connection is served only after a mutual-TLS handshake in which
the client presents a certificate chaining to a configured client-CA bundle
(`WebPkiClientVerifier`, require-and-verify — `build_server_config`,
`crates/interfaces/delonix-node-api/src/lib.rs`). A1 deliberately stopped there:
*"The engine does not issue or own this material in A1 — the CA is the
operator's. A2 adds the node-identity flow (token → CSR → short-lived cert,
rotation, revocation) that produces the server cert and a trust path; A1 only
consumes what it is given"* (ADR-0096 §5).

That leaves two gaps a fleet cannot live with:

1. **How does a new member get a certificate at all?** Handing every node a
   long-lived key copied by hand does not scale and never rotates — a leaked key
   is valid until someone remembers it exists. A fleet needs a *bootstrap*: a new
   member arrives with a single, scoped, one-time secret and leaves with a
   certificate it generated itself (the private key never travels).

2. **How is a certificate taken away?** A1 rejects anything that does not chain
   to the CA, but a certificate that *does* chain is valid until it expires. A
   compromised member, or one being decommissioned, must be refusable *before*
   its certificate's natural expiry. That is revocation, and it is absent in A1
   (ADR-0096 Consequences: *"CRL/rotation handling is A2"*).

### Why a short-lived cert obtained by CSR, not a bearer token or a shared key

A bearer token presented on every call is the credential itself travelling on
every call; mutual TLS already proves possession of a key without sending it, and
A1 is built on it. A pre-shared client key copied to each node has no issuer, no
expiry a verifier can see, and no revocation short of rebuilding the CA bundle and
restarting every server. A certificate signed from a **CSR** keeps the private key
on the member that generated it (the authority sees only the public half), carries
its own validity window the verifier enforces for free, and is revocable by serial
through a CRL — the three properties the two gaps above demand. It is also exactly
what A1 already verifies, so the server side changes by *addition* (a CRL), not by
a new trust mechanism.

### Why the node CA key is external to the node-API server

A1's server holds the client-CA **certificate** as a trust anchor. It must never
hold the CA **key**: a node-API server is the broadest attack surface in the fleet
(it creates containers and VMs), and a server that could *mint* client certs could
mint its own caller. So the signing key lives only in the **enrollment authority**
— a separate surface, started deliberately by an operator, that a node-API server
never runs. "External CA" in the brief is precisely this separation: the key that
authorizes callers is external to everything that answers them. The authority is a
*bootstrap* surface, not one of the versioned contract servers (CRI, mgmt, node,
docker-api), so it does not follow their socket-activated / sibling-binary
convention (ADR-0040 D2.4): it is run on demand by `delonix serve node-enroll`,
not auto-started by an arriving connection, because a connection must never be
able to wake the holder of the CA key.

### Why the enrollment endpoint is server-TLS, token-authenticated — not mutual TLS

The member enrolling has no client certificate yet — that is the whole point — so
the enrollment endpoint cannot require one (it would be the chicken asking for the
egg). It is instead **server-authenticated TLS**: the authority presents a
certificate chained to the node CA, and the member **pins the node CA** when it
connects, so a man-in-the-middle cannot impersonate the authority and harvest the
token. The member authenticates itself to the authority with the **one-time
token** in the request body, over that server-authenticated channel. This mirrors
kubeadm's bootstrap exactly: discovery pins the cluster CA, a bootstrap token
authorizes a CSR, the CSR is signed, and the token is then useless. Plaintext is
never served here either (the engine's fail-closed posture, ADR-0096 §3): no TLS
material for the authority ⇒ it refuses to start.

## Decision

1. **Node identity is a short-lived X.509 client certificate** whose `clientAuth`
   EKU and subject CN name the member, chaining to a **node CA**. The node-API
   server (A1) already requires such a chain; A2 names that CA the *node CA* and
   supplies both its issuance path and its revocation path.

2. **The node CA key lives only in the enrollment authority.** `delonix node-ca
   init` generates the CA (key + self-signed cert, ring provider via `rcgen`,
   `CA:TRUE`) and writes `node-ca.crt` (the trust anchor an operator passes to
   every server as A1's `--tls-client-ca`) and `node-ca.key` (kept only where the
   authority runs). A node-API server is never given the key.

3. **Enrollment = one-time token → CSR → short-lived cert.**
   - `delonix node-enroll` (member side): generates a fresh key pair, builds a CSR
     (subject CN = the member's identity, optional SANs), and `POST`s
     `{ token, csr_pem }` to the authority over TLS **pinned to `node-ca.crt`**.
     On success it writes `client.key` (never transmitted) and `client.crt`.
   - `delonix serve node-enroll` (authority side): a server-TLS-only HTTPS endpoint
     (`POST /v1/enroll`) presenting a cert chained to the node CA. It holds the set
     of valid tokens as **salted hashes** (a token is accepted at most once, then
     consumed), reloads the node CA key, parses the CSR, and signs a certificate
     with `clientAuth` EKU and a **short validity** (`--cert-ttl`, default 1 h,
     `not_before` skewed 1 min into the past for clock drift), returning
     `{ cert_pem, ca_pem }`. The server reuses A1's PEM loading
     (`CertificateDer::pem_slice_iter` / `PrivateKeyDer::from_pem_slice`) for its
     own TLS material; the certificate it mints is the one A1's
     `WebPkiClientVerifier` then accepts.

4. **Rotation is intrinsic and needs no server restart.** The node-API server
   trusts the CA, not any individual leaf. A member re-runs `node-enroll` (a new
   token, or — a documented A3 follow-up — a renewal authenticated by the current
   cert) and the new certificate is accepted immediately; the previous one is not
   withdrawn, it **expires** on its short clock. No server state changes, nothing
   restarts.

5. **Revocation is a CRL signed by the node CA, consulted by the server.**
   `delonix node-ca revoke --serial <hex>` appends the serial to a CRL that the
   CA signs (`rcgen` `CertificateRevocationListParams::signed_by`) and writes as
   `node-ca.crl` (PEM). The node-API server loads it with a new
   `--tls-client-crl` / `DELONIX_NODE_API_TLS_CLIENT_CRL` and builds its verifier
   with `WebPkiClientVerifier::builder(roots).with_crls(...)`. A revoked — or
   naturally expired — client certificate is then refused **at the handshake**,
   before the contract is reached, exactly where A1 refuses an untrusted one.
   Because the server is daemonless and idle-exits (ADR-0040/0041), a freshly
   written CRL takes effect on the next cold start, bounded by `--idle-timeout`;
   no running process is restarted by hand. (Issuing a certificate whose serial a
   verifier can match requires an **explicit serial** on the leaf; the authority
   sets one per issuance and returns it, so an operator can revoke by it.)

6. **The `delonix.node.v1` contract does not change.** Enrollment is a distinct
   bootstrap surface (`POST /v1/enroll` on the authority), not an RPC of the node
   contract; `proto/delonix/node/v1` and the generated OpenAPI are untouched, and
   `contract_gate` is not involved. Authorization from the certificate's identity,
   and the audit record, remain A3.

7. **The CA key is reloaded, not the CA cert; parsing is confined to the CSR.**
   The authority reloads the CA **key** (`KeyPair::from_pem`) and reconstructs the
   issuer from the canonical parameters `node-ca init` used (subject CN, `CA:TRUE`,
   default SKI = SHA-256 of the public key), so a leaf it signs chains to the
   persisted `node-ca.crt` (same subject DN, same key) **without parsing the CA
   certificate**. The one place an X.509 parser is unavoidable is ingesting the
   member's **CSR** (`CertificateSigningRequestParams::from_pem`), which requires
   `rcgen`'s `x509-parser` feature — enabling it pulls `x509-parser` (with
   `asn1-rs`/`oid-registry`) into the `delonix` binary. That is the one new
   dependency subtree A2 adds, and it is intrinsic to CSR-based enrollment: a CA
   must parse what a member asks it to sign. Importing a wholly foreign CA
   *certificate* (reconstructing the issuer by parsing it rather than from a known
   CN) is now also possible via `from_ca_cert_pem`, but A2 keeps the key-only
   reload so the CA subject is explicit (`--cn`).

## Consequences

- A new member joins with a single one-time token and ends up holding a
  short-lived certificate it generated itself; the private key never leaves it,
  and the token is spent on first use. An external control plane can then drive the
  full node contract over A1's TCP+mTLS listener with that certificate.
- A compromised or decommissioned member is refused before its certificate expires
  by revoking its serial into the CRL; expiry is the backstop that bounds the blast
  radius even without revocation.
- The node-API server's trust surface grows by exactly one optional input (a CRL)
  and no new trust mechanism; the CA signing key is never present on it.
- The `delonix` binary already links `rcgen`, `hyper` (server + client),
  `tokio-rustls`, `reqwest` (blocking, rustls) and `sha2`; `time` is promoted from
  a transitive (via `rcgen`) to a direct dep for the validity window, and
  `rcgen`'s `x509-parser` feature is enabled to ingest CSRs (§7) — the one new
  dependency subtree. The enrollment authority, the member client and CA/CRL
  management are added in-process; the binary does **not** gain a dependency on
  `delonix-node-api` (its contract servers stay sibling binaries, ADR-0040 D2.4).
  The node contract and its servers are unchanged.
- The authority is a deliberately-run, not socket-activated, surface; an operator
  who is not running `serve node-enroll` cannot be made to sign anything by an
  inbound connection.

## Implementation

The enrollment authority, the member client and CA/CRL management all live in the
`delonix` binary (which already links `rcgen`, `hyper`, `tokio-rustls`, `reqwest`
and `sha2`); it does not link `delonix-node-api`. The only change on the node-API
server side is CRL *consumption*.

- `bins/delonix/src/cmd/node_enroll.rs` (new): the node-CA / enrollment core and
  the three CLI surfaces.
  - `NodeCa` — `generate()` (new CA: `CA:TRUE`, `keyCertSign+crlSign`, canonical
    subject CN) for `node-ca init`, and `reload(key_pem)` which reloads the CA
    **key** (`KeyPair::from_pem`) and reconstructs the issuer from the *same*
    canonical params + `.self_signed(key)`, so a leaf it signs chains to the
    persisted `node-ca.crt` (same subject DN, same key) with no CA-cert parsing.
  - `sign_csr(&NodeCa, csr_pem, ttl, now) -> (cert_pem, serial_hex)` —
    `CertificateSigningRequestParams::from_pem` → set `clientAuth` EKU, an
    explicit random serial, `not_before` (−1 min skew)/`not_after` →
    `.signed_by(&ca.cert, &ca.key)`.
  - `build_crl(&NodeCa, &[serial], now, next_update) -> crl_pem`
    (`CertificateRevocationListParams::signed_by`).
  - `TokenStore` — salted (SHA-256) token hashes, one-time consume.
  - `node-ca init|revoke` (CA + CRL management); `serve node-enroll` — a
    server-TLS-only `POST /v1/enroll` endpoint on raw `hyper` over `tokio-rustls`
    (`with_no_client_auth`); `node-enroll` — the member client, which builds a
    key pair + CSR, `POST`s over `reqwest` blocking pinned to `node-ca.crt`
    (`tls_built_in_root_certs(false)` + `add_root_certificate`), and writes
    `client.key`/`client.crt`.
- `crates/interfaces/delonix-node-api/src/lib.rs`: `Tls` gains an optional
  `client_crl_pem`; `build_server_config` threads it into
  `WebPkiClientVerifier::builder(...).with_crls(...)` when present (absent/empty ⇒
  unchanged A1 behaviour).
- `bins/delonix-node-api-bin/src/main.rs`: `--tls-client-crl` flag + env, loaded
  into `Tls`.
- `bins/delonix/src/main.rs`: `NodeCa`/`NodeEnroll` command variants + a
  `serve node-enroll` `ServeCmd` arm; `cli-stability` classification for the new
  groups.
- Proof: unit tests (token one-time; a signed leaf chains to `node-ca.crt`; a
  CRL'd serial fails `WebPkiClientVerifier`); then a live cross-machine run (host
  = authority + node-API server, delonix VM = member) that enrolls, calls
  `node/v1`, rotates, and is refused after revocation/expiry — the A2 acceptance
  report.
