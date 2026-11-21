# ADR-0096 — The node API is reachable over TCP with mTLS, by socket activation

**Status:** Accepted (2026-11-19) — Phase A/A1 of the fleet-operability brief
(a node operable by an external control plane). **Relates to:** ADR-0040 (the
node contract and its socket activation, P5), ADR-0041 (the idle exit, D5),
ADR-0042 (one versioned contract, two encodings). **Followed by:** A2 (node
identity: one-time token → CSR → short-lived cert, rotation, revocation,
external CA), A3 (authorization by caller identity + audit). This ADR is the
transport; A2/A3 give the TCP path an identity and rules. Until A3 lands, a
caller authenticated by a valid client certificate is authorized for every
operation the socket serves — the same posture the local unix socket has for
the owning uid today.

> **The decision, in one line:** the node contract (`delonix.node.v1`, the same
> gRPC + HTTP/JSON) may be served on a **TCP** listener that **requires mutual
> TLS** — a client certificate chained to a configurable CA — in addition to the
> local unix socket. The listener is still **socket-activated** (a second
> `.socket` unit) or directly bound behind an explicit flag; **no new daemon**.
> Plaintext on TCP is never served: a TCP listener without TLS material is
> **refused, fail-closed**.

## Context

The node API serves `delonix.node.v1` on exactly one listener per process, a
**local unix socket**, guarded by `SO_PEERCRED` (the connecting uid must equal
the server's own — `serve_listener`, `crates/interfaces/delonix-node-api/src/lib.rs`).
Socket activation holds the listening socket in a systemd `.socket` unit and the
server exits when idle (ADR-0040 P5, ADR-0041 D5); `activation::check_listening_unix_stream`
refuses any inherited descriptor that is not a listening `AF_UNIX` stream (DX-1903),
precisely so that a wrong `ListenStream=` cannot be served without the peer-credential
check meaning anything.

That is correct for a local caller and wrong for the brief: **an external control
plane cannot reach the node at all.** A unix socket has no network path, and the
only authentication is "same uid on the same host". Making the node operable as
part of a fleet requires the same contract reachable from another machine, and
reachable **safely** — authentication that survives the network, not a uid match
that is meaningless across hosts.

### Why not a plaintext TCP socket plus application auth

Bearer tokens or an app-layer check over plaintext TCP expose every call and the
credential to anyone on the path, and leave the node trusting a header it cannot
tie to a key. The engine's posture is fail-closed and "the engine validates its
own contract" (AGENTS.md): the transport itself must authenticate both ends.
Mutual TLS does that — the client proves possession of a key whose certificate
chains to a CA the node was configured to trust, and the node proves the same to
the client — and it is the mechanism the engine already uses for etcd peer/client
trust and the ingress proxy (`bins/delonix/src/cmd/pki.rs`, `ingress_proxy.rs`),
so it adds no new cryptographic surface.

### Why not a resident listener (a daemon)

Guardrail #1 is daemonless. The unix path is socket-activated and idle-exits; the
TCP path keeps that property. A second `.socket` unit (`delonix-node-api-tls.socket`,
`ListenStream=<addr>:<port>`) holds the TCP socket; the first connection starts
the service, which receives the socket through `LISTEN_FDS` and idle-exits as
before. A connection arriving during the exit waits in the socket's queue and
starts it again, so nothing is lost. No ADR-justified resident process is
introduced.

## Decision

1. **One listener per process, two transports.** A process serves either the
   local unix socket (`SO_PEERCRED`, unchanged, the default and the rootless
   path) **or** a TCP socket wrapped in TLS. The socket-activation model is
   unchanged: exactly one `ListenStream=`, `Accept=no`, one `LISTEN_FDS=1`.

2. **TCP requires mutual TLS, fail-closed.** The TLS `ServerConfig` is built with
   **require-and-verify** client authentication (`WebPkiClientVerifier`) against a
   configured **client CA** bundle, plus the server's own cert chain and key. A
   client that presents no certificate, or one that does not chain to the client
   CA, is **rejected at the TLS handshake** — the connection never reaches the
   contract. ALPN offers `h2` and `http/1.1` so the same listener serves gRPC and
   HTTP/JSON, exactly as the unix listener does today over cleartext HTTP/2 prior
   knowledge.

3. **No plaintext TCP, ever.** If the listener is TCP (an inherited `AF_INET`/
   `AF_INET6` socket, or an explicit `tcp://host:port` address) and any of the
   three TLS materials (server cert, server key, client CA) is missing, the server
   **refuses to start** with a coded error that says so — it never falls back to
   cleartext. Symmetrically, TLS material supplied for a **unix** listener is
   refused: it would read as protection that is not there.

4. **The descriptor's family selects the transport.** `activation` learns to
   classify an inherited listening stream socket as `Unix` or `Tcp`
   (`classify_listening_stream`); `check_listening_unix_stream` stays as the
   assertion used on the unix path. An `AF_INET`/`AF_INET6` listening stream is no
   longer an error (DX-1903) — it selects the TLS path, which then demands TLS
   material by rule (3).

5. **TLS material is configuration, external to the engine.** The three PEM paths
   are read from flags (`--tls-cert`, `--tls-key`, `--tls-client-ca`) or their
   environment variables (`DELONIX_NODE_API_TLS_CERT`/`_KEY`/`_CLIENT_CA`), flags
   winning. The engine does not issue or own this material in A1 — the CA is the
   operator's. A2 adds the node-identity flow (token → CSR → short-lived cert,
   rotation, revocation) that produces the server cert and a trust path; A1 only
   consumes what it is given.

6. **Caller identity and authorization are A3, not A1.** A1 authenticates the
   transport: a valid client cert is required. Extracting the caller's identity
   from that cert and enforcing declarative per-operation/per-namespace rules,
   with a per-call audit record, is A3. Until then the TCP path grants an
   authenticated client the same access the unix path grants the owning uid.

## Consequences

- An external control plane on another host can drive the full node contract over
  one `tcp://host:port`, authenticated by a client certificate, with no daemon and
  the idle-exit intact.
- A revoked or untrusted client is refused at the handshake (CRL/rotation handling
  is A2; A1 rejects anything that does not chain to the configured client CA).
- The unix path is byte-for-byte unchanged: rootless-first keeps its local,
  no-TLS, `SO_PEERCRED` default.
- The node/v1 **contract does not change** — same RPCs, same `proto/`, same
  generated OpenAPI. This is a transport decision; `contract_gate` is untouched.
- A new dependency edge `delonix-node-api → tokio-rustls` (already pinned in the
  workspace, `ring` provider, no `aws-lc`/OpenSSL C surface).

## Implementation

- `crates/interfaces/delonix-node-api/src/activation.rs`: `classify_listening_stream`
  (+ `ActivatedKind`), `check_listening_unix_stream` becomes an assertion over it;
  `AF_INET`/`AF_INET6` now classify as `Tcp` with unit coverage.
- `crates/interfaces/delonix-node-api/src/lib.rs`: a `Tls` struct (loaded PEM),
  `serve`/`serve_blocking`/`serve_with` take `Option<Tls>`; a `serve_tls_listener`
  mirrors `serve_listener` (shared idle condition + connection driver) and performs
  the per-connection handshake, dropping a refused one.
- `bins/delonix-node-api-bin/src/main.rs`: `--tls-*` flags + env, PEM loaded and
  passed as `Tls`; the fail-closed rules (3); the crypto provider installed once.
- `bins/delonix/src/cmd/serve.rs`: `serve node-api --install-units` writes a TCP
  `.socket` (`ListenStream=<addr>:<port>`) and the TLS environment into the
  `.service`, refusing a TCP unit without the three TLS materials.
- Proof: a loopback mTLS integration test — a valid client cert reaches `GET /v1`;
  a client with no cert and a client with the wrong CA are refused at the
  handshake. The test fails before this change (TCP is refused by construction)
  and is green after. Live cross-host proof on `pve92` is the A1 acceptance run.
