# A2 live acceptance — node identity by enrollment (ADR-0097)

**Date:** 2026-11-20 · **Branch:** `feat/node-api-mtls-transport` · **Builds on:** A1 (ADR-0096, commit `64f01d72`)
**Verdict:** PASS (all three acceptance criteria), with three test failures confirmed pre-existing (not A2).

ADR-0097 gives A1's `--tls-client-ca` a *source* (a node CA + a token→CSR→cert
enrollment flow) and a *sink* (a CRL the server consults). This report proves the
flow end-to-end with the binaries built from this worktree — first on host
loopback with the real CLI verbs, then across a real machine boundary (host serves,
a delonix VM enrolls and calls), à l'identique of A1's topology.

## Topology

| role | machine | address |
|------|---------|---------|
| **enrollment authority** (holds the node CA key) | the host | `delonix serve node-enroll` on `tcp://…:7443`, server-TLS only |
| **node** (node/v1 server) | the host | `delonix-node-api` on `tcp://…:19443`, mutual TLS, trusts the node CA |
| **member** (client) | delonix VM `a2-member` (Ubuntu 24.04, libvirt) | `192.168.122.229`, reaches the host at its gateway `192.168.122.1` |

Binaries: `target/debug/{delonix,delonix-node-api}` from this worktree. The node CA
is created by `delonix node-ca init`; the node-API server certificate is operator-issued
from that CA with `openssl` (exactly as A1 issued its PKI). The CA **key** lives only
with the authority — the node-API server is given the CA **cert** only.

## Criterion 1 — a one-time token buys a short client cert that drives node/v1

### Host loopback (real CLI verbs)
- `delonix node-ca init` → `node-ca.crt` (`CA:TRUE`, `keyCertSign+crlSign`) + `node-ca.key`.
- `delonix serve node-enroll --token TOK-AAA … --cert-ttl 1h` → *“enrollment authority
  listening on https://127.0.0.1:7443/v1/enroll (mutual-TLS-free, token-authenticated)”*.
- `delonix node-enroll --token TOK-AAA --cn member-1` → *“enrolled as CN=member-1
  (serial 2cd8…2029)”*. The issued cert: `openssl verify` **OK** against `node-ca.crt`;
  EKU = **TLS Web Client Authentication**; carries the explicit serial.
- **One-time:** a second `node-enroll` with `TOK-AAA` → `enrollment refused (HTTP 401):
  unknown or already-used enrollment token`.
- With the enrolled cert against `delonix-node-api` on `:19443`:
  - `GET /v1` → **HTTP 200**
  - `POST /v1/namespaces/default/containers` → **HTTP 200**, `state=OPERATION_STATE_SUCCEEDED verb=create`
  - `GET …/containers/a2-ctr` → **HTTP 200**
  - `DELETE …/containers/a2-ctr` → **HTTP 200**, `state=SUCCEEDED verb=delete`
  - `GET …/containers/a2-ctr` → **HTTP 404** (gone)

### Cross-host (the member is the delonix VM — host serves, VM calls)
`node-ca.crt` and the `delonix` binary were copied to `a2-member`; the member ran the
real `delonix node-enroll` against the host authority at `192.168.122.1:7443`:
- `delonix node-enroll --server 192.168.122.1:7443 --ca node-ca.crt --token XH-TOKEN-1 --cn vm-member`
  → *“enrolled as CN=vm-member (serial 37f3…3876)”*.
- **One-time:** re-spending `XH-TOKEN-1` from the VM → `HTTP 401` refused.
- The VM then drove the host's node/v1 over TCP+mTLS with the enrolled cert:
  `GET /v1` **200**; `POST …/containers` **200** (`SUCCEEDED/create`); `GET` **200**;
  `DELETE` **200** (`SUCCEEDED/delete`); `GET` **404**.

The private key never left the member — only the CSR and, back, the certificate.

## Criterion 2 — rotation in, expiry/revocation out, refused at the handshake, no hand restart

- **Rotation (no restart):** with the node-API server still running, the member enrolled a
  **new** certificate (`TOK-BBB` on loopback; `XH-TOKEN-2` on the VM) signed by the **same**
  node CA and called `GET /v1` → **HTTP 200**. The server trusts the CA, not a leaf, so the
  new cert is accepted at once; the node-API process was never restarted.
- **Expiry:** an authority with `--cert-ttl 4s` issued a cert that reached `GET /v1` → **200**
  while fresh; 6 s later, the same cert against the same (untouched) node-API →
  `curl: (56) … sslv3 alert certificate expired`. No body, refused at the handshake.
- **Revocation via the daemonless cold-start (no hand restart):** the node-API was installed
  as a **socket-activated** unit (`serve node-api --install-units … --tls-client-crl … --idle-timeout 3s`;
  the unit carries `DELONIX_NODE_API_TLS_CLIENT_CRL`). A member cert reached `GET /v1` → **200**.
  `delonix node-ca revoke --serial <that serial>` rewrote the CRL in place; the socket-activated
  service idle-exited (`inactive`, the socket still listening); the next connection cold-started,
  read the new CRL, and the revoked cert → `curl: (56) … sslv3 alert certificate revoked`
  (**HTTP 000**, no body), **while a different, non-revoked cert from the same CA → HTTP 200**.
  No process was restarted by hand — the socket activation recycled it.

(The same “revoked refused / live served in ONE running process” is also proven at the handshake,
without any restart, by the in-process integration test below.)

## Criterion 3 — ADR, tests, gates, contract

- **ADR-0097** written in English (`docs/adr/0097-node-identity-enrollment-csr-short-cert.md`), Accepted.
- **Unit/integration tests (touched crates) green:**
  - `delonix-node-api` `tests/tls_mtls.rs` — adds
    `a_revoked_client_certificate_is_refused_while_a_live_one_is_served`: one running server
    loaded with a CRL refuses the revoked client at the handshake **and** serves a non-revoked
    client from the same CA (no restart). The A1 mTLS test still passes.
  - `delonix-bin` `cmd::node_enroll::tests` — token is spendable exactly once; a reloaded CA
    signs a leaf that chains to the persisted cert; the CRL lists each revoked serial; a non-CSR
    is rejected; hex round-trips.
  - `cargo test -p delonix-node-api -p delonix-node-api-bin` fully green; `-p delonix-bin`:
    1733 pass, only the pre-existing `cmd::provider::*` (2) and `cmd::schema::the_published_schema_is_up_to_date`
    (1) fail — **verified failing identically on the base commit `64f01d72`** (manifest-schema /
    provider-matrix drift, unrelated to A2).
- **Gates, no new debt vs base:** `cargo fmt --check` clean on the touched crates; `clippy` clean on
  my files (only the pre-existing `container.rs:6579` remains); `lang_ratchet.py` **green**
  (docs 380, user_text 0 — the ADR is English, the pt-AO help strings are in the `pt.po` catalog);
  `arch_fitness.py` output is **byte-for-byte identical** between the base tree and this one
  (cli_context_lines 126957, env_writes 25, library_prints 119, raw_error_variant 1 — all pre-existing;
  `cmd/help`/`cmd/distroless` missing Part D are pre-existing too). New CLI surface (`node-ca`,
  `node-enroll`, `serve node-enroll`) is classified in `cli-stability`, carries manual entries, and
  every new help/arg/example string has a pt-AO translation.
- **Contract untouched:** `proto/delonix/node/v1` has **no** change (A2 is identity/transport). The
  one new dependency subtree is `x509-parser`, enabled on `rcgen` to ingest the member's CSR
  (`CertificateSigningRequestParams::from_pem`) — intrinsic to CSR-based enrollment, documented in
  ADR-0097 §7; `time` was promoted from transitive to direct for the short validity window.

## Teardown

Both host servers and every proof process stopped; the socket-activated user units removed
(`systemctl --user disable --now` + file delete + daemon-reload); the `a2-member` VM destroyed; all
scratch PKI under the scratchpad deleted. The owner's `obs-vm-1` and `pve92` (both `shut off`) were
never touched. No source code changed during the proof; `proto/delonix/node/v1` intact.
