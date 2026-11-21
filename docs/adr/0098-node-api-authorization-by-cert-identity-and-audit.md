# ADR-0098 — The node API authorizes each call by the client certificate's identity against a declarative policy, and audits every mutating call

**Status:** Accepted (2026-11-20) — Phase A/A3 of the fleet-operability brief (a
node operable by an external control plane). **Builds on:** ADR-0096 (A1 — the
node contract served over TCP with mutual TLS; a client certificate chaining to
`--tls-client-ca` is required at the handshake), ADR-0097 (A2 — a member earns a
short-lived client certificate whose subject CN names it, by enrolling against a
node CA). **Relates to:** ADR-0040 (the node contract, its two encodings and its
socket activation), ADR-0042 (one versioned contract, two encodings — gRPC and
HTTP/JSON), ADR-0050 (the served slice of the contract). **Closes** the posture
A1/A2 left open: *"a caller authenticated by a valid client certificate is
authorized for every operation the socket serves"* (ADR-0096 §6, ADR-0097 §0).

> **The decision, in one line:** every call that arrives over the **TCP+mTLS**
> transport is authorized before it is dispatched — the caller's **identity is
> the subject CN of its client certificate** (a URI SAN is an accepted
> alternative), checked against a **declarative policy** that grants named
> identities a set of **operations**, each optionally scoped to **namespaces**;
> a call the policy does not grant is refused with `PermissionDenied` (HTTP 403,
> gRPC `PERMISSION_DENIED`) **before the handler runs**, and **every mutating
> call** — allowed or denied — leaves a **one-line JSON audit record** carrying
> the identity, the operation, the namespace and the outcome. The **local unix
> transport is unchanged** (`SO_PEERCRED`, the owning uid); authorization applies
> only where there is a client certificate to read an identity from. With **no
> policy configured** the transport keeps A1/A2's behaviour (an authenticated
> client is authorized for everything), so A3 is opt-in and breaks no existing
> deployment.

## Context

A1 made the node contract reachable from another host and authenticated the
*transport*: a TCP connection is served only after a mutual-TLS handshake in
which the client presents a certificate chaining to the configured client CA
(`build_server_config`, `crates/interfaces/delonix-node-api/src/lib.rs`). A2 gave
that certificate a *source and a lifecycle*: a member enrolls against a node CA
and receives a short-lived certificate whose **subject CN names the member**
(ADR-0097 §3). Both ADRs deliberately stopped at authentication and said so in
the same words: until A3, *"an enrolled, non-revoked client is still authorized
for everything the socket serves"* (ADR-0097 §0, ADR-0096 §6).

That is the gap A3 closes. A fleet's control plane is not one principal: a
scheduler that creates containers in a tenant's namespace, a backup agent that
only reads volumes, an operator that may delete VMs — each presents a
certificate, and the node must act on *who* presented it, not merely that
*someone valid* did. Without A3 the mTLS listener is all-or-nothing: the day a
second identity exists, every identity has every power.

Three things must be decided, and this ADR decides them in order: **where the
identity comes from**, **where the rules live and what they say**, and **what the
audit record is and where it goes**.

### Why the subject CN is the identity (a URI SAN accepted alongside it)

A2 already made the CN authoritative: the enrollment authority signs a leaf whose
*subject CN is the member's identity* (ADR-0097 §3, `sign_csr`). A3 reads back
exactly what A2 wrote — the CN is the name a policy rule matches. No new notion
of identity is introduced, and the enrollment flow needs no change: the string an
operator puts in a rule is the same string they put in `node-enroll --cn`.

A **URI SAN** is accepted as an *alternative* identity because the fleet may move
to SPIFFE-style identities (`spiffe://trust-domain/node/<id>`), which live in a
URI SAN, not the CN (a SPIFFE leaf's CN is conventionally empty). Recognizing a
URI SAN now means a policy can name either form and the identity model does not
have to change when SPIFFE arrives. The CN is the primary identity; if the leaf
carries URI SANs, each is also offered to the matcher. DNS/IP SANs are *not*
identities — they name a host, not a principal, and a node-to-node call is still
*some principal* calling, so they are ignored for authorization.

Parsing the leaf's subject and SANs from the `CertificateDer` the handshake
yields needs an X.509 parser. `rustls`/`webpki` verify a chain but do not expose
the subject CN, so A3 adds **`x509-parser`** as a direct dependency of
`delonix-node-api` to read CN + URI-SAN from the already-verified leaf. The
subtree (`x509-parser`/`asn1-rs`/`oid-registry`) is already in the workspace lock
— A2 pulled it into the `delonix` binary to ingest CSRs (ADR-0097 §7) — so this
adds no new third-party code to the tree, only a new edge from this crate. The
parse runs **after** the handshake has already verified the chain to the CA: a
malformed or hostile leaf never reaches the parser, because an untrusted one is
dropped at the handshake (A1).

### Why a declarative policy file, default-deny when present, external to the engine

The rules are **configuration, not code** — the same stance A1 took on TLS
material (ADR-0096 §5): the engine *consumes* a policy an operator writes, it does
not compile the fleet's authorization graph into the binary. The policy is a
**JSON file** (the format the node already reads its admission `policy.json` in,
ADR-0026 — and `serde_json` is already a dependency, so this adds no new crate),
its path given by `--authz-policy` / `DELONIX_NODE_API_AUTHZ_POLICY`, loaded once
at start
and held for the process's short life (the server idle-exits, ADR-0041, so an
edited policy takes effect on the next cold start — the same bound a changed CRL
has in A2, ADR-0097 §5).

- **No policy ⇒ A1/A2 posture, unchanged.** An operator who has not written a
  policy gets exactly today's behaviour: an authenticated client is authorized
  for everything. A3 does not break the A1/A2 deployments that have no second
  identity yet. This is the one place A3 is *not* fail-closed, and it is a
  deliberate, documented back-compat choice, not an oversight: the transport
  still refuses every unauthenticated caller at the handshake (A1), so "no
  policy" is "every authenticated caller is trusted", which is precisely what
  ADR-0096 §6 promised held until A3.
- **A policy present ⇒ default-deny.** Once a policy exists, a call is dispatched
  only if some rule grants it; an identity no rule mentions can do nothing. This
  is the only safe default for an authorization list — a typo removes power, it
  never grants it.

A rule grants one identity (an exact CN/URI, or a trailing-`*` glob such as
`scheduler-*`) a set of **operations** and a set of **namespaces**:

```json
{
  "default": "deny",
  "rules": [
    { "identity": "control-plane",
      "operations": ["*"],
      "namespaces": ["*"] },

    { "identity": "backup-agent",
      "operations": ["VolumeService.ListVolumes", "VolumeService.GetVolume"],
      "namespaces": ["*"] },

    { "identity": "tenant-a-scheduler",
      "operations": ["ContainerService.*", "PodService.*"],
      "namespaces": ["team-a"] }
  ]
}
```

`identity` is an exact subject CN, a `spiffe://…` URI SAN, or a `prefix-*` glob;
`operations` is `"*"`, a group `"Service.*"`, or an exact `"Service.Method"`;
`namespaces` is `"*"`, an exact namespace, or a `prefix-*` glob. `default` must be
`"deny"` when a policy is present (the only safe value; a file that sets anything
else is refused at load, so "no policy" and "allow-all policy" cannot be confused).

An **operation** is named canonically as `Service.Method` — the pair the node
contract already uses everywhere (`transcode::Route { service, rpc }`, the gRPC
path `/delonix.node.v1.<Service>/<Method>`). A group is `Service.*`. The wildcard
`*` is every operation. This names operations in the contract's own vocabulary,
so a rule is readable against `proto/` and against the OpenAPI without a
translation table.

### Why authorization is enforced at the router's demux boundary, and how namespace granularity holds for both encodings

The node contract is served in two encodings over the one TLS listener: gRPC and
HTTP/JSON, both merged into one `axum::Router` (`router_with`), both converging on
the same `NodeApi` service methods (REST through `transcode::unary`, which builds
a `tonic::Request` and calls the identical method). Authorization must bind at a
point **every** request of **both** encodings passes through, or it is
bypassable.

That point is a **tower layer wrapping the whole router**, applied on the TLS
transport only. It reads the per-connection identity, resolves the **operation**
from the request, decides against the policy, writes the audit record, and either
forwards the request or returns the `PERMISSION_DENIED` problem document — the
handler never runs for a denied call.

- **Operation** is resolved from the request path alone, no body decode: a gRPC
  path is `/delonix.node.v1.<Service>/<Method>`; a REST path is resolved through
  the same `transcode::resolve` the router already uses, giving `route.service` /
  `route.rpc`. Either way the operation is known before the body is read. **This
  gate alone fully satisfies the primary criterion** — an identity with no grant
  for an operation is refused, in either encoding, before dispatch.

- **Namespace** is resolved **without decoding the request body**: in the node
  contract a namespaced resource carries its namespace as a **URL path segment**
  (`/v1/namespaces/{namespace}/containers/{name}…`, ADR-0042), which
  `transcode::resolve` already returns as a bound variable. So for the **REST
  encoding the layer reads the target namespace straight from the path** and
  enforces a namespace-scoped rule in full. A node-wide operation (no
  `{namespace}` in its template — images, VMs, operations) is matched against a
  rule's namespaces as the sentinel `-` (node scope), which `"*"` grants.

- **gRPC carries the namespace in the protobuf body**, which this layer does not
  decode. Rather than ship a bypass, the rule is **fail-closed**: a
  **namespace-scoped** rule (its `namespaces` is not `["*"]`) is **not honoured
  over gRPC** — an identity whose only grant for an operation is namespace-scoped
  is **denied** that operation over gRPC. gRPC thus cannot be used to escape a
  namespace restriction; an identity that must call over gRPC is granted
  `namespaces = ["*"]` (or uses the REST encoding, which the control plane's
  HTTP/JSON client speaks and which the live proof uses). This is recorded in the
  audit as a deny with reason `namespace-scope-requires-rest`. It is a real
  constraint, stated plainly, not a silent hole: the alternative — decoding every
  gRPC body generically to peek a `namespace` field — is a parser on the hot path
  that A3 judges not worth its surface while the control plane drives REST.

### Why a per-call JSON audit record, to a file and to the tracing stream

"Each mutating call leaves an audit record" is a brief requirement, and an audit
record is only worth writing if it is **machine-readable and complete**. The
record is **one JSON object per line** (JSON Lines — greppable, `jq`-able,
shippable to the logging pipeline A2's siblings already feed):

```json
{"ts":"2026-11-20T09:14:22.431Z","identity":"tenant-a-scheduler","identity_source":"cn",
 "operation":"ContainerService.CreateContainer","namespace":"team-a","transport":"rest",
 "decision":"allow","reason":"rule:tenant-a-scheduler","remote":"192.168.122.41:54122"}
```

- **What is audited.** Every **mutating** call (the `MUTATIONS` set the REST
  encoding already enumerates, plus their gRPC equivalents — create/delete/start/
  stop/… across containers, pods, VMs, networks, volumes, images, stacks, and
  snapshots) is audited whether it is allowed or denied — this is the brief's
  floor. **Every denial** of any call (read or mutate) is audited too, because a
  refused read is a security-relevant event. An allowed **read** is not audited
  by default (it is the high-volume, low-signal case); a future `audit = "all"`
  knob can widen it, out of scope here.
- **Where it goes.** The primary sink is a **file**, appended one line per call,
  path from `--audit-log` / `DELONIX_NODE_API_AUDIT_LOG`. Independently, the same
  record is always emitted as a structured `tracing` event at `INFO` on a stable
  target (`delonix_node_api::audit`), so even with no file configured the record
  is observable in the service's journal/stream (the "file/stream" the brief
  asks for). The file is opened in append mode so concurrent connections (each a
  task) do not truncate each other; a single `write_all` of one
  newline-terminated line keeps records whole under concurrency.
- **No secrets in the record.** The identity is a certificate subject, not a
  credential; the record carries the identity, operation, namespace, decision,
  reason, transport and peer address — never key material, never the request
  body.

## Decision

1. **The caller's identity is its client certificate's subject CN**, with any
   **URI SAN** accepted as an alternative identity; DNS/IP SANs are not
   identities. The identity is extracted once per TLS connection, after the
   handshake has verified the chain (A1), from the leaf `CertificateDer` via
   `x509-parser`, and carried into every request on that connection as a
   `CallerIdentity` request extension. The unix transport injects no identity and
   is not subject to authorization (unchanged, `SO_PEERCRED`).

2. **Authorization is a declarative JSON policy, external to the engine**, loaded
   from `--authz-policy` / `DELONIX_NODE_API_AUTHZ_POLICY`. **Absent ⇒ A1/A2
   posture** (an authenticated client is authorized for everything). **Present ⇒
   default-deny**: a call is allowed only if a rule grants its identity that
   operation in that namespace. A rule is `{ identity, operations, namespaces }`
   with exact/`prefix-*` identity match, `*` / `Service.*` / `Service.Method`
   operations, and `*` / exact / `prefix-*` namespaces.

3. **Enforcement is a tower layer wrapping the router, on the TLS transport
   only.** The operation is resolved from the request path (gRPC
   `/delonix.node.v1.<Svc>/<Method>`; REST via `transcode::resolve`). The target
   namespace is resolved from the REST path variable, or the node-scope sentinel
   for node-wide operations. A denied call is answered with the engine's
   `PermissionDenied` problem document (HTTP 403 / gRPC `PERMISSION_DENIED`, the
   mapping `engine_error` already has) **before** the handler runs.

4. **gRPC + a namespace-scoped rule is fail-closed.** Because the layer does not
   decode the gRPC body, a rule whose `namespaces` is not `["*"]` is not honoured
   over gRPC: the operation is denied over gRPC for an identity whose only grant
   is namespace-scoped (reason `namespace-scope-requires-rest`). No gRPC path can
   escape a namespace restriction. REST enforces namespace scope in full.

5. **Every mutating call, and every denial, is audited** as one JSON line to the
   audit file (`--audit-log` / `DELONIX_NODE_API_AUDIT_LOG`, append) and, always,
   as a structured `tracing` INFO event on target `delonix_node_api::audit`. The
   record carries `ts, identity, identity_source, operation, namespace,
   transport, decision, reason, remote`; never key material or request bodies.

6. **The `delonix.node.v1` contract does not change.** A3 is authorization and
   audit over the existing two encodings; `proto/delonix/node/v1`, the generated
   OpenAPI and `contract_gate` are untouched. A3 adds **no new top-level `delonix`
   command group**: like A1's `--tls-*`, it is consumed through
   `delonix-node-api-bin` flags and written into the units by `serve node-api
   --install-units`.

## Consequences

- A fleet with more than one principal can finally express least privilege: a
  backup agent reads volumes and nothing else; a tenant scheduler creates in its
  namespace and nowhere else; the control plane keeps `*`. The day a second
  identity is enrolled, it starts with no power until a rule grants it.
- A refused caller gets a first-class engine error (403 / `PERMISSION_DENIED`),
  the same document shape every other node error uses — not a connection drop and
  not a silent 200.
- Every state-changing call on the remote transport is attributable to a
  certificate identity in a greppable log, present whether or not the call was
  allowed — the floor an operator needs to answer "who deleted that VM".
- The node-API server's trust surface grows by exactly two optional inputs (a
  policy file, an audit-log path) and one direct dependency edge
  (`delonix-node-api → x509-parser`, a subtree already in the lock via A2). No
  new third-party code enters the tree; the CA signing key is still never present
  on the server (A2).
- gRPC callers that need namespace granularity must hold `namespaces = ["*"]` or
  use the REST encoding — a stated, audited constraint, the price of not putting
  a protobuf body parser on the authorization path in A3.
- With no policy and no audit-log configured, the transport is byte-for-byte A1/
  A2: A3 is strictly additive and opt-in.

## Implementation

- `crates/interfaces/delonix-node-api/src/authz.rs` (new): the policy model and
  the decision.
  - `CallerIdentity { cn: Option<String>, uris: Vec<String>, source }` — parsed
    from the leaf by `x509-parser` (`X509Certificate::from_der` → subject CN,
    `SubjectAlternativeName` URI general-names).
  - `Policy` — parsed from JSON (`serde_json::from_str`, already a dep); `Rule {
    identity, operations, namespaces }`; `Policy::decide(&CallerIdentity, operation, Namespace,
    transport) -> Decision { allow: bool, reason: String }`. Absent policy is a
    distinct `Policy::AllowAll` so the layer can keep the A1/A2 posture without a
    sentinel rule. Operation and namespace matchers (`*`, `Service.*`,
    `prefix-*`). The gRPC + namespace-scope fail-closed rule lives here, keyed on
    `transport`.
  - `AuditRecord` + `AuditSink` — a sink holding an optional `Arc<Mutex<File>>`
    (append) and always emitting the `tracing` event; `record(&AuditRecord)`
    serializes one line and `write_all`s it. The `MUTATIONS` classification is
    shared with `service.rs` (lifted to a small `pub(crate)` set keyed by
    `Service.Method`).
- `crates/interfaces/delonix-node-api/src/lib.rs`:
  - a `Authz { policy: Policy, audit: AuditSink }` bundle threaded through
    `serve`/`serve_blocking`/`serve_with` as `Option<Authz>` beside `Option<Tls>`
    (authz is meaningful only with TLS; given for a unix listener it is refused,
    as TLS is).
  - `serve_tls_listener` extracts the `CallerIdentity` from
    `tls.get_ref().1.peer_certificates()` after `acceptor.accept`, and wraps the
    per-connection hyper service so each request gets the identity + the peer
    `SocketAddr` as extensions; the router carries the authz layer
    (`axum::middleware::from_fn_with_state`) that reads them and enforces §3–§5.
  - the layer answers a denial by reusing `problem`/`engine_error`
    (`Status::permission_denied` → 403 / `PERMISSION_DENIED`), so a denied call's
    body is the same problem document shape as every other node error.
- `bins/delonix-node-api-bin/src/main.rs`: `--authz-policy` and `--audit-log`
  flags + their env vars, loaded into `Authz` and passed beside `Tls`; a policy
  given with a unix listener is refused (symmetry with TLS, ADR-0096 §3).
- `bins/delonix/src/cmd/serve.rs`: `serve node-api --install-units` writes
  `DELONIX_NODE_API_AUTHZ_POLICY` / `DELONIX_NODE_API_AUDIT_LOG` into the
  `.service` environment when given, beside the TLS env A1 writes; no new unit.
- Proof: unit tests (CN/URI extraction from a leaf; `Policy::decide` over
  allow/deny/namespace-scope/gRPC-fail-closed; one audit line is valid JSON with
  the expected fields); then a live cross-machine run (host = node-API server over
  `tcp://…:19443` with a policy; delonix VM = two enrolled identities, one
  authorized for an operation/namespace and one not) showing the unauthorized
  caller gets 403 and the authorized one 200, with the audit file carrying both —
  the A3 acceptance report.
