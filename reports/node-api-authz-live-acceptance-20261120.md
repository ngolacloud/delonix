# A3 live acceptance — authorization by certificate identity + audit (ADR-0098)

**Date:** 2026-11-20. **Branch:** `feat/node-api-mtls-transport` (A1 committed at
`64f01d72`; A2 + A3 uncommitted in the worktree). **Subject:** the node API served
over TCP+mutual-TLS (A1/A2) now authorizes each call by the client certificate's
identity against a declarative policy, and audits every mutating call and every
denial (ADR-0098).

This is the cross-machine proof the brief's first done-criterion asks for: a real
`delonix-node-api` server (the one `serve` runs, not a double) on the host,
reached over the network from a **delonix VM**, where one certificate identity is
authorized and another is not.

## Setup

- **Server (host):** `delonix-node-api --addr tcp://192.168.122.1:19443
  --tls-cert node.crt --tls-key node.key --tls-client-ca ca.crt
  --authz-policy authz.json --audit-log audit.jsonl --idle-timeout 0`.
  Startup log: `listening (tcp, mutual TLS) addr=192.168.122.1:19443 authz=true`.
- **PKI:** one CA (`CN=delonix-node-ca`); a server leaf with
  `subjectAltName=IP:192.168.122.1`; two client leaves from the same CA —
  `CN=control-plane` and `CN=intruder`. Both pass the A1 handshake (same CA); the
  policy tells them apart (A3).
- **Policy (`authz.json`):** `default: "deny"`, one rule granting `control-plane`
  `operations:["*"]` in `namespaces:["*"]`. `intruder` matches no rule.
- **Client (other machine):** delonix VM `a3node`
  (`delonix-vm-k8s:1.36`, libvirt), IP `192.168.122.150` via DHCP on the libvirt
  default network; `curl 8.5.0` with the client cert/key and the CA. The client
  materials were streamed into the VM over `vm ssh`.

## Result — enforcement (criterion 1: unauthorized ≠ 200, live)

From inside the VM (`192.168.122.150`) against `https://192.168.122.1:19443`:

| caller (cert CN) | request | HTTP | meaning |
|---|---|---|---|
| `control-plane` (authorized) | `GET /v1` | **200** | reaches the contract (API root) |
| `intruder` (unauthorized) | `GET /v1` | **403** | refused before the handler |
| `control-plane` (authorized) | `POST …/namespaces/default/volumes` | **400** | passed authz (audited allow); handler rejected the minimal body — **not** a 403 |
| `intruder` (unauthorized) | `POST …/namespaces/default/volumes` | **403** | refused before the handler |

The `403` body is the engine's standard problem document, not a dropped
connection or a silent 200:

```json
{"code":"DX_PERMISSION_DENIED","dx":"DX-7000","grpc_status":7,"status":403,
 "title":"permission denied","instance":"/v1",
 "detail":"permission denied: the client certificate is not authorized for NodeService.GetApiRoot"}
```

## Result — audit (criterion 2: every mutating call, with identity/op/outcome)

The host `audit.jsonl` after the run (one JSON object per line), VM records shown:

```json
{"ts":"…","identity":"intruder","identity_source":"cn","operation":"NodeService.GetApiRoot","namespace":"-","transport":"rest","decision":"deny","reason":"no-matching-rule","remote":"192.168.122.150:49290"}
{"ts":"…","identity":"control-plane","identity_source":"cn","operation":"VolumeService.CreateVolume","namespace":"default","transport":"rest","decision":"allow","reason":"rule","remote":"192.168.122.150:49306"}
{"ts":"…","identity":"intruder","identity_source":"cn","operation":"VolumeService.CreateVolume","namespace":"default","transport":"rest","decision":"deny","reason":"no-matching-rule","remote":"192.168.122.150:49322"}
```

Observed, matching ADR-0098 §5 exactly:

- the **mutating** call (`CreateVolume`) is audited whether **allowed**
  (`control-plane`) or **denied** (`intruder`);
- every **denial** is audited (the `intruder` `GetApiRoot` reads);
- the **allowed read** (`control-plane` `GetApiRoot`) is **not** audited — it never
  appears in the log;
- each record carries the **certificate identity** (`identity`+`identity_source:cn`
  — the CN read from the leaf), the **operation** (`Service.Method`), the
  **namespace** (`default` from the REST path, `-` for node scope), the decision,
  the reason, and the caller's **remote** address.

The same records are emitted to the `tracing` stream (target
`delonix_node_api::audit`, INFO) — the "file/stream" the brief asks for —
confirmed in the server log, so a deployment with no file still observes them.

## Other criteria

- **ADR-0098** written in English; `proto/delonix/node/v1`, the generated OpenAPI
  and `contract_gate` untouched (A3 is authorization + audit over the existing two
  encodings, not a contract change).
- **Gates, zero new debt vs base:** `arch_fitness.py` output is byte-identical
  before (A1+A2) and after (A1+A2+A3) — the pre-existing reds (`cli_context_lines`,
  `env_writes`, `library_prints`, `raw_error_variant_matches`, `cmd/distroless`,
  `cmd/help`) are unchanged in value; `lang_ratchet` docs stays `382` (the ADR is
  English); the known-red tests are exactly the base's three (provider×2,
  schema×1), with 1733 other `delonix` tests passing.
- **Tests:** `authz` unit suite (15, incl. the DER CN/URI-SAN parse, the decision
  matrix, the gRPC namespace-scope fail-closed, the audit-line JSON); the
  `tls_mtls` integration suite (3, incl. the new in-process authorized→200 /
  unauthorized→403 / audited-denial); `delonix-node-api-bin` (6); the serve
  unit-install test carrying `DELONIX_NODE_API_AUTHZ_POLICY`/`_AUDIT_LOG`.

## Teardown

Server stopped; VM `a3node` destroyed (`vm rm --force`). `obs-vm-1`/`pve92`
untouched. Port 19443 used (9443 was occupied on the host).
