# ADR-0108: A member renews its client certificate by presenting the current one — same identity, new key, inside a window, within a bounded lineage

- **Status:** Proposed (2026-11-28) — design only, nothing implemented; awaiting the owner's
  decision.
- **Date:** 2026-11-28
- **Deciders:** Walter Angolar (owner)
- **Relates to:** ADR-0097 §4 (closes its «documented A3 follow-up»: *a renewal authenticated by
  the current cert*), ADR-0097 §3/§5 (the enrollment authority, the short `--cert-ttl`, the CRL),
  ADR-0096 (the TCP + mutual-TLS listener the renewed certificate is presented to), ADR-0098 (the
  caller's identity is its subject CN, a URI SAN accepted alongside it — the thing a renewal must
  never change), ADR-0102 (an external SPIFFE authority is the other way to get rotation; this
  ADR keeps the engine's own CA self-sufficient).

> **The decision, in one line:** the enrollment authority gains an opt-in `POST /v1/renew` on
> its existing listener. The caller authenticates by **mutual TLS with its current, unexpired,
> unrevoked client certificate**, sends a CSR for a **new key** with the **same subject and the
> same SANs**, and gets a fresh short certificate. Renewal opens at **half the presented
> certificate's lifetime**. A **lineage** (every certificate descended from one token enrollment)
> is renewable for at most `--renew-max-age` (default 30 days) and dies as soon as **any** of its
> serials is in the CRL. The authority records every issuance in an **append-only issuance log**:
> it is what makes the lineage, the age cap and «each serial renews once» durable without
> putting anything new in the certificate. The engine runs no renewal loop; the member calls
> `delonix node-enroll --renew --if-due` from whatever scheduler it already has.

## Context

ADR-0097 gives a member a certificate by spending a one-time token, and says how a member gets
the *next* one (§4): *«A member re-runs `node-enroll` (a new token, or — a documented A3
follow-up — a renewal authenticated by the current cert)»*. A3 became ADR-0098 (authorization),
and the second branch was never decided. Measured on `develop` (`6bf1dab9`):

- **There is no renewal path.** No ADR and no code mentions it: `grep -ri renew` finds nothing
  in `crates/interfaces/` and nothing related in `bins/delonix/src/cmd/node_enroll.rs`. The
  authority serves one route, `POST /v1/enroll` (`node_enroll.rs:701`), and it requires a token
  (`:740`).
- **The authority cannot see a client certificate.** Its TLS config is built with
  `.with_no_client_auth()` (`node_enroll.rs:637`, `server_only_tls`). That was correct for
  enrollment (the member has no certificate yet, ADR-0097 «Why the enrollment endpoint is
  server-TLS»). For a renewal, though, the client certificate *is* the credential.
- **Certificates are short by design.** `--cert-ttl` defaults to 1 h and must be positive
  (`bins/delonix/src/cmd/serve.rs:242-245`). The manual even shows `--cert-ttl 60s`
  (`manual_entries.rs:2657`).
- **The authority does not know about revocation.** `node-ca revoke` writes the CRL and a
  `<out>.serials` sidecar (`node_enroll.rs:321-338`, `build_crl` `:150-175`). Only the node-API
  server reads it (`--tls-client-crl`, `crates/interfaces/delonix-node-api/src/lib.rs:274-291`,
  `with_crls`). `serve node-enroll` has no CRL input (`AuthorityConfig`, `node_enroll.rs:515-537`).
- **A token buys any identity.** `sign_csr` takes the subject DN and the SANs from the CSR as
  sent (`CertificateSigningRequestParams::from_pem`, `node_enroll.rs:109-124`). The member picks
  them (`--cn`, `--san`, `:408-414`, `:434-436`). Because ADR-0098 offers every URI SAN to the
  policy matcher as an identity, the subject and SANs together *are* the identity. Renewal must
  therefore copy that identity exactly and never let the caller change it.
- **The authority keeps no durable state.** Spent tokens live in memory (`TokenStore`, `:177-207`:
  *«persistence across restarts is out of scope for v1»*). Issued serials are only printed
  (`println!("enroll: issued a client certificate (serial {serial})")`, `:758`).
- **A CSR proves possession of its key.** rcgen 0.13.2 verifies the CSR's signature when it
  parses it (`csr.rs:104`, `verify_signature`). A renewal CSR therefore proves the caller holds
  the *new* key, and the TLS handshake proves it holds the *current* one.
- **rcgen cannot write an extension under a UUID OID.** `CustomExtension::from_oid_content` takes
  `&[u64]` arcs (`certificate.rs:920`), and a `2.25.<uuid>` arc is 128 bits. Carrying lineage
  data inside the certificate would need a registered private-enterprise arc. This rules out the
  cheapest-looking alternative (see «Alternatives considered»).

The result is that a long-running external client (for example, a control plane with one
executor per site, calling the node over ADR-0096's listener) faces two bad options. It loses
access every `--cert-ttl`, and an operator has to mint and deliver a fresh one-time token every
hour. Or the operator raises `--cert-ttl` to days or months. That second option brings back
exactly the long-lived, stolen-is-valid-until-noticed credential that ADR-0097 set out to remove.

Guardrails touched: **#1 (daemonless).** The authority stays a deliberately-run surface, and the
engine schedules nothing. **#2.** No consumer is named, and the decision serves any mTLS client
of the node. **#4.** Every change lives in `bins/delonix`; no engine crate changes, and no new
dependency subtree is added (`x509-parser` is already in the binary through rcgen's feature,
ADR-0097 §7, and is a workspace dependency, `Cargo.toml:163-166`). **#5.** No privilege boundary
moves, but this is credential issuance, so the implementation gets a `delonix-runtime-sec` review
before it merges. **#6 (fail-closed).** Every condition below refuses; none silently corrects.

## Decision

### D1 — Renewal is a route on the existing authority, opt-in, behind optional client auth

`serve node-enroll` gains `--renew` (off by default). With it, the same listener also serves
`POST /v1/renew`. The listener's rustls config moves from `with_no_client_auth()` to
`WebPkiClientVerifier::builder(<node-ca.crt>).with_crls(<crl>).allow_unauthenticated()`:

- A client that presents **no** certificate completes the handshake as today and can call only
  `/v1/enroll`. `/v1/renew` without a peer certificate is `401 client-certificate-required`.
  This check costs nothing to get right: renewal copies its identity from the peer certificate,
  so without one there is nothing to copy.
- A client that presents a certificate is verified **at the handshake**. The certificate must
  chain to the node CA, be within its validity, and have no serial in the CRL. A presented but
  invalid certificate fails the handshake (rustls rejects it even with `allow_unauthenticated`),
  exactly where the node-API server refuses it. **«Only while still valid»** is therefore
  enforced by TLS, not by handler code: an expired certificate cannot renew, and its owner
  re-enrolls with a token.
- `--renew` without `--crl` and `--issued-log` is refused at start (D4, D5). There is no
  renewal surface that cannot see revocation.

The member pins `node-ca.crt` for the authority's server identity, as it does when enrolling.

### D2 — The identity is copied from the presented certificate; the CSR contributes only a new key

The renewed leaf's subject DN and SAN set are taken **from the verified peer certificate**, not
from the CSR. The CSR must still carry the *same* subject DN and the *same* SAN set, compared
exactly; otherwise the request is refused with `400 identity-mismatch`. The authority refuses
rather than quietly overriding, so a client that believes it is changing its name finds out
(guardrail #6). Changing identity is a new enrollment with a token.

The CSR's public key **must differ** from the presented certificate's key (`400 key-reuse`).
Each renewal rotates the key, so a key copied once stops being useful one renewal later unless
the copier also keeps renewing, and D5 detects that. Everything else is set by the authority, as
`sign_csr` does today: `clientAuth` EKU, a fresh random serial, the AKI, and `not_before`
skewed back one minute.

### D3 — The window: from half the presented lifetime to its expiry

A renewal is accepted when `now ≥ not_before + (not_after − not_before) / 2`. Before that it is
refused with `409 not-yet-renewable` and a `renewable_after` timestamp. The upper bound is
`not_after`, enforced by the handshake (D1). With the default 1 h TTL, a member may renew from
the 30th minute, so it has half a lifetime to reach the authority. The lower bound stops a
member (or a thief) from minting certificates back-to-back. The client-side `--if-due` uses the
same rule (D6). The new certificate's lifetime is the authority's current `--cert-ttl`, capped
by D4. It is never copied from the presented certificate, so shortening `--cert-ttl` takes
effect at the next renewal.

### D4 — A lineage is bounded in age and dies with any of its serials

A **lineage** is a token enrollment plus every certificate renewed from it. Its id is the serial
of the token-issued certificate, and its birth is that issuance time. Both come from the
issuance log (D5); the certificate itself carries nothing new.

- **Age cap.** `serve node-enroll --renew-max-age <duration>` (default `30d`, must exceed
  `--cert-ttl`; there is no «unlimited» value). A renewed certificate's `not_after` is
  `min(now + cert_ttl, birth + renew_max_age)`. Once `now ≥ birth + renew_max_age`, renewal is
  refused with `403 identity-max-age`, and the member must enroll again with a token. A stolen
  certificate is therefore worth at most `renew_max_age` even if nobody notices, and an
  operator re-authorizes every long-running client at a known interval.
- **Revocation kills the lineage.** The authority refuses a renewal (`403 lineage-revoked`) if
  the presented serial **or any of its ancestors** is in the CRL. Revoking the one serial
  `node-enroll` printed at enrollment therefore stops every future renewal of that identity.
  Revoking a later serial stops the chain from that point on.
- **Cutting the live certificate too.** A node-API server sees only the leaf it is shown. So
  `node-ca revoke` gains `--lineage <hex> --issued-log <file>`, which revokes **every** serial
  the log lists for that lineage, the live one included. The node-API servers then refuse the
  current certificate at their next cold start (ADR-0097 §5, bounded by `--idle-timeout`), not
  only at its expiry.
- **The authority reads the CRL as it changes.** It is a long-running process. Before each
  accepted connection it checks the CRL file's mtime and rebuilds its `ServerConfig` when the
  file changes. If the file cannot be read or parsed, the authority keeps enrolling but serves
  `/v1/renew` as `503 crl-unavailable` until the file is fixed (fail-closed for renewal).
- **CRL bootstrap.** `node-ca init` also writes an empty signed `node-ca.crl` (and its empty
  sidecar). `node-ca revoke` stops requiring `--serial`, so that, run with no serials, it
  rewrites the CRL from the sidecar for a CA created before this ADR.

### D5 — An append-only issuance log makes lineage, age and «renews once» durable

`serve node-enroll --issued-log <file>` (required with `--renew`, optional otherwise) appends
one JSON line per certificate issued, by enrollment or by renewal:

```json
{"ts":"2026-11-28T10:00:00Z","serial":"…","parent":null,"lineage":"…","born":"2026-11-28T10:00:00Z",
 "subject":"CN=…","sans":[…],"not_after":"…","spki_sha256":"…","cert_pem":"…"}
```

- The line is written and `fsync`ed **before** the certificate is returned. If it cannot be
  written, the reply is `503` and no certificate is issued: a certificate exists only if the log
  records it.
- The authority takes an exclusive lock on the log for its whole run and refuses to start if the
  log is already locked. There is one authority per log, with no write race.
- It loads the log into memory at start (serial → entry) and appends as it goes.
- **Unknown serial.** A presented certificate whose serial the log does not list (issued before
  `--issued-log` existed, or by another authority) is refused with `403 unknown-lineage`, and
  its member enrolls once with a token. A lost log therefore means «everyone re-enrolls», never
  «anyone renews».
- **Each serial renews once.** If the log already has a child of the presented serial:
  - a CSR with the **same** public key (`spki_sha256`) gets the **same** certificate back. A
    retry after a lost reply is idempotent.
  - a CSR with a **different** key is refused with `409 already-renewed`, and the authority
    prints a warning naming the lineage. Two holders of one certificate are a sign that its key
    was copied. The operator decides whether to revoke the lineage; the engine never revokes on
    its own.

The log holds public material only: no private key, no token. It is also the audit trail of who
holds which serial, which today exists only on the authority's stdout.

### D6 — The member side: `node-enroll --renew`, idempotent, scheduled by the caller

`delonix node-enroll --renew --server … --ca node-ca.crt --cert client.crt --key client.key
[--if-due]`. It conflicts with `--token`, `--cn` and `--san`, because the identity comes from
`--cert`.

- It reads the subject and SANs from the current certificate and generates a new key into
  `<key>.next` (`0600`). If a `<key>.next` is already there from an earlier failed attempt, it
  reuses that key, so D5's idempotent retry applies.
- It builds the CSR and `POST`s `{ csr_pem }` to `/v1/renew` over mTLS (`reqwest` `Identity`
  from the current cert and key, `tls_built_in_root_certs(false)` plus the pinned CA, as at
  `node_enroll.rs:452-458`).
- On success it writes the new certificate, renames `<key>.next` over `--key` and the new
  certificate over `--cert` (write, `fsync`, `rename`), and prints the new serial, the lineage
  and `identity_expires_at`. On failure it leaves the current pair untouched.
- `--if-due` exits 0 **without contacting** the authority while the current certificate is
  outside the D3 window. A timer can therefore call it often (for example every `cert_ttl/6`)
  and do nothing most of the time.
- Exit codes tell «not due» (0), «renewed» (0), «refused, re-enroll with a token»
  (`identity-max-age`, `lineage-revoked`, `unknown-lineage`, expired) and «transient, retry»
  (network, `503`, `409 not-yet-renewable`) apart, so a scheduler knows when to page a human.

The engine ships **no** renewal loop or daemon (guardrail #1). The member's own scheduler (a
systemd timer, cron, or its own process loop) calls the command. The manual documents a systemd
timer example.

### D7 — What does not change

- **The node contract.** `proto/delonix/node/v1`, its OpenAPI, and `contract_gate` are not
  involved: `/v1/renew` is on the bootstrap surface, like `/v1/enroll` (ADR-0097 §6).
- **The node-API server.** It still trusts the CA, not the leaf, so a renewed certificate is
  accepted with no restart, as ADR-0097 §4 already promised.
- **ADR-0098.** The identity a policy matches is byte-for-byte the same across renewals (D2),
  so no policy needs to know about renewal.
- **`/v1/enroll`.** It works as today. With `--issued-log` it also logs and returns `lineage`
  and `identity_expires_at`; both are additive response fields.

## Alternatives considered

- **Do nothing: a token per renewal, or a long `--cert-ttl`.** A token per hour means operator
  toil that grows with the number of clients. Spent tokens also come back after an authority
  restart, because `TokenStore` is in memory. A long TTL brings back the stolen-until-noticed
  credential that ADR-0097 removed. Both defeat the reason for short certificates. Rejected.
- **A refresh token returned at enrollment (OAuth style).** This is a bearer secret kept at rest
  on the client and sent on every renewal. mTLS already proves possession of a key without
  sending it, and the node already relies on it. A refresh token adds a second credential with
  its own theft and revocation story. Rejected.
- **Carry the lineage and birth inside the certificate** (a private, non-critical extension), so
  the authority stays stateless. This is the cheapest-looking option, but it needs an OID that
  rcgen can write. rcgen takes `&[u64]` arcs (`certificate.rs:920`), which rules out a
  registration-free `2.25.<uuid>` arc, so it would need an IANA Private Enterprise Number first.
  It also does not give «each serial renews once» or a revoke-the-live-cert-by-lineage, which
  need a record anyway. Rejected for D5. A later ADR can add the extension as an optimization if
  the project registers an arc.
- **Keep the original `not_before` on renewed certificates to carry the birth.** This backdates
  certificates, breaks D3's «half the lifetime», and makes every tool misreport the lifetime.
  Rejected.
- **A separate mTLS-only listener (or port) for renewal.** This means two listeners, two server
  identities, and two things to firewall. With optional client auth, the handshake still does
  all the cryptographic checks, and a path that needs a peer certificate cannot work without one
  (D1). Rejected as more surface for no extra assurance.
- **Renewal as an RPC of `delonix.node.v1`, served by the node-API server.** The node-API server
  must never hold the CA key (ADR-0097 «Why the node CA key is external»), and this would move
  issuance into the broadest attack surface in the fleet. Rejected.
- **Adopt EST (RFC 7030 `simplereenroll`) or ACME (RFC 8555) wire formats.** EST's
  `simplereenroll` has exactly D1–D2's semantics (TLS client auth with the current certificate,
  same subject, same SANs). Its wire format is PKCS#7/CMS over base64, a dependency subtree the
  engine does not have, for a surface that only `delonix` clients call today. The decision keeps
  EST's semantics on ADR-0097's JSON surface, and an EST façade can be added later if a
  third-party client needs one. ACME's challenge model answers a different question (control of
  a name, not continuity of an enrolled identity). Rejected.
- **Allow renewal with the same key.** This is simpler for the client, but a copied key then
  stays useful for the whole `renew_max_age` with no signal. With D2's key rotation plus D5's
  «renews once», a copy shows up as `already-renewed`. Rejected.
- **No age cap (renew forever until revoked).** This makes a certificate whose theft nobody
  notices permanent. The cap is the backstop that does not depend on noticing. Rejected; D4 has
  no «unlimited» value on purpose.
- **Delegate rotation to an external SPIFFE authority (ADR-0102).** This works for an operator
  who runs one, and nothing here prevents it. But ADR-0102 keeps the engine's own CA as a
  first-class fallback, and a fallback that loses every client every hour is not one. This is a
  complement, not a replacement.

## Consequences

- A long-running client keeps a 1 h certificate indefinitely, up to `renew_max_age`, with no
  operator action and no long-lived credential at rest beyond its current key. Short
  certificates become usable for clients that live longer than one TTL.
- An operator re-authorizes each long-running client with a token once per `renew_max_age`
  (30 days by default). This is a deliberate, known chore.
- Killing an identity takes one command (`node-ca revoke --lineage … --issued-log …`). That
  command stops renewals immediately and the live certificate at the node-API servers' next
  cold start.
- The authority becomes **stateful**: its issuance log must be kept and backed up next to
  `node-ca.key`. Losing it is fail-closed, since every client re-enrolls with a token. It is one
  more file whose loss has an operational cost.
- **Availability coupling.** A member must reach the authority within the second half of each
  certificate's lifetime (30 min by default). A longer authority outage means re-enrolling with
  tokens. Operators who need more slack raise `--cert-ttl` moderately, and that trade-off is now
  explicit, not forced.
- **Clock skew.** D3 and D4 are judged on the authority's clock, and `--if-due` on the member's
  clock. A skewed member gets `409 not-yet-renewable` with `renewable_after` and retries; it is
  never silently refused.
- **Known limits, not fixed here:**
  - Spent enrollment tokens still come back after an authority restart (`TokenStore`,
    `node_enroll.rs:177-207`). The issuance log makes persisting them cheap, but that is a
    separate decision.
  - A token still lets its holder choose any subject and SANs. Binding a token to an identity is
    also a separate decision. This ADR only guarantees that renewal never widens what
    enrollment granted.
  - The log assumes one authority. Several authorities sharing one CA need a shared log, and
    that is a later ADR.
- No new dependency subtree. `x509-parser` (already linked through rcgen's feature) reads the
  presented leaf's subject, SANs, SPKI and validity. `reqwest`'s rustls `Identity` presents the
  client certificate.

## Implementation (when Accepted)

All in `bins/delonix`; nothing in `crates/`, `proto/` or the node-API server.

- `bins/delonix/src/cmd/node_enroll.rs`:
  - `server_only_tls` becomes `authority_tls(server, ca, crl: Option)`, using optional client
    auth with CRLs when `--renew` is on.
  - The accept loop rebuilds the config when the CRL's mtime changes.
  - `handle` routes `/v1/renew`, reading the peer certificate from the `tokio_rustls` server
    connection.
  - `sign_csr` splits into `sign_enroll` (the subject from the CSR, as today) and `sign_renew`
    (the subject and SANs from the peer leaf, the key from the CSR, the D2–D4 checks).
  - `IssuedLog` handles the locked append-only JSONL, the in-memory index, `fsync` before the
    reply, the lineage walk and the child lookup.
  - `NodeCaCmd::Init` writes an empty CRL; `Revoke` gets an optional `--serial` and a new
    `--lineage` with `--issued-log`.
  - `NodeEnrollArgs` gets `--renew`, `--cert`, `--key` and `--if-due`, with the `<key>.next`
    atomic swap.
- `bins/delonix/src/cmd/serve.rs`: `--renew`, `--crl`, `--issued-log` and `--renew-max-age`, with
  the start-time refusals of D1 and D4.
- Manual and `cli-stability` entries for the new flags, with a systemd timer example for
  `node-enroll --renew --if-due`.
- **Proof:**
  - Unit tests:
    - a renewal copies the identity and rejects a different CN or SAN, and a reused key;
    - the window boundaries;
    - the age cap on `not_after` and its refusal;
    - a revoked ancestor kills the lineage;
    - a same-key retry returns the same certificate, and a different key is `already-renewed`;
    - an unknown serial is refused;
    - log write failure means no certificate.
  - Then a live run: authority plus node-API server on one host, a member on another, a minute
    TTL. Renew three times across expiries while calling `node/v1`. Revoke the lineage, and the
    next renewal is refused and the node-API server refuses the live certificate after its cold
    start. Hit the age cap with a short `--renew-max-age`.
