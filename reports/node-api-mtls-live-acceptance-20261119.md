# A1 live acceptance — the node API over TCP with mutual TLS (ADR-0096)

**Date:** 2026-11-19 · **Branch:** `feat/node-api-mtls-transport` · **Commit under test:** `64f01d72`
**Verdict:** PASS (all three acceptance criteria), with one finding unrelated to the TLS transport.

ADR-0096 states: *"Live cross-host proof on `pve92` is the A1 acceptance run."* This
report is that run. The loopback integration test (`tests/tls_mtls.rs`) already
proved the handshake in-process; this proves it across a real machine boundary and
exercises the full node contract (Container **and** VirtualMachine lifecycle) from a
remote client.

## Topology (why not literally "on pve92")

The proof requires creating a **VirtualMachine** through the node API. A VM can only
be created where the delonix engine and `/dev/kvm` live — i.e. on a delonix **node**.
`pve92` is a Proxmox VE *appliance* (a libvirt domain, shut off, owned by the lab
owner, never to be modified); it is not a delonix node and cannot serve a node API
that creates delonix VMs. So "pve92" in the brief means "the live lab", and the
faithful cross-machine proof is:

| role | machine | address |
|------|---------|---------|
| **node** (server) | the host | `delonix-node-api` on `tcp://0.0.0.0:{19443,29443}`, mutual TLS |
| **other machine** (client) | delonix VM `a1-client` (Ubuntu 24.04, libvirt) | `192.168.122.133`, reaches the node at its gateway `192.168.122.1` |

Binaries built from this worktree (`target/debug/delonix`, `target/debug/delonix-node-api`).
The installed `delonix 0.1.1` predates A1 and has no `--tls-*` flags, so it could not
have served this.

## PKI (operator-owned, external to the engine — ADR-0096 §5)

`openssl` (ed25519), one trusted CA signing the server cert and a good client cert,
plus a **second** (rogue) CA signing a client cert that must be refused:

- `ca.crt` — the trusted CA; passed as `--tls-client-ca` and as the client's `--cacert`.
- `server.crt` — SAN `IP:192.168.122.1, IP:192.168.30.1, IP:127.0.0.1, DNS:delonix`.
- `client.crt` — `CN=a1-control-plane`, chains to `ca.crt` → `openssl verify` OK.
- `rogue-client.crt` — chains to `rogue-ca.crt`, **not** to `ca.crt` → `openssl verify` fails (error 20).

## Criterion 1 — remote Create+Get+Delete of a Container and a VM, over TCP+mTLS

Server started directly: `delonix-node-api --addr tcp://0.0.0.0:19443 --tls-cert … --tls-key … --tls-client-ca …`
→ log: `listening (tcp, mutual TLS) addr=0.0.0.0:19443`.

**From `a1-client` (192.168.122.133 → 192.168.122.1:19443), good client cert:**

Container (`POST/GET/DELETE /v1/namespaces/a1-accept/containers[/a1-ctr-vm]`):
- `POST` → `Operation state=OPERATION_STATE_SUCCEEDED target=Container/a1-ctr-vm id=r-ctr-vm-1` `[HTTP 200]`
- `GET` → `[HTTP 200]`, full Container (`status.reason=Created`).
- `DELETE` → `Operation state=SUCCEEDED verb=delete` `[HTTP 200]`
- `GET` after delete → `[HTTP 404]`

VirtualMachine (`POST/GET/DELETE /v1/namespaces/default/virtualmachines[/a1-nodevm]`):
- `POST` → `Operation SUCCEEDED target=VirtualMachine/a1-nodevm id=r-nodevm-1` `[HTTP 200]`; a real
  cloud-hypervisor VM booted (IP `10.200.254.26`, backend reports guest running).
- `GET` → `[HTTP 200]`, full VirtualMachine (`namespace=default`, `phase=RUNNING`).
- `DELETE` → `Operation SUCCEEDED verb=delete`; the CH VM was actually destroyed.
- `GET` after delete → `[HTTP 404]`

**Socket activation** (`serve node-api --install-units --addr tcp://0.0.0.0:29443 --tls-* `):
writes `~/.config/systemd/user/delonix-node-api.{socket,service}`, enables the socket.
- socket unit: `ListenStream=0.0.0.0:29443`, `Accept=no`, `SocketMode=0600` (benign on AF_INET).
- service unit: `ExecStart=…/target/debug/delonix-node-api --idle-timeout=120s` + the three
  `Environment=DELONIX_NODE_API_TLS_*` paths (resolved **beside** the exe, ADR-0075 D1).
- `systemctl --user` showed the socket `active (listening)`, service `inactive`.
- First mTLS connection from `a1-client` → `GET /v1` `[HTTP 200]`; service then `active (running)`,
  `TriggeredBy: delonix-node-api.socket`, log `listening (tcp, mutual TLS) addr=0.0.0.0:29443`.
- A Container Create+Get+Delete over the socket-activated `:29443` from `a1-client` also
  succeeded (`SUCCEEDED` / `[HTTP 200]` / `[HTTP 404]`) — it serves the identical contract.

No daemon: the service is socket-activated and idle-exits after 120s.

## Criterion 2 — a cert outside the CA (and no cert) is refused at the handshake

From `a1-client` against both `:19443` and the socket-activated `:29443`:
- **good cert** → `[HTTP 200]` (the `/v1` discovery document).
- **no client cert** → `curl: (56) … tlsv13 alert certificate required` — connection drops, no body.
- **rogue-CA client cert** → `curl: (56) … tlsv1 alert unknown ca` — connection drops, no body.

The contract is never reached by an unauthenticated or wrong-CA client; the rejection
is at the TLS layer, exactly as ADR-0096 §2 requires. (Same three outcomes also
reproduced on host loopback `127.0.0.1:19443`.)

## Criterion 3 — idempotency on the remote API (`Idempotency-Key`/`request_id`)

Re-POSTing a create with the **same** `request_id`, from `a1-client`:
- Container `request_id=ctr-vm-1`: the second `POST` returned the **byte-identical**
  Operation (`id=r-ctr-vm-1`, same `create_time`), and `LIST` showed **exactly 1** `a1-ctr-vm`.
- VirtualMachine `request_id=nodevm-1`: the second `POST` returned the same `id=r-nodevm-1`;
  `LIST` showed exactly 1 `a1-nodevm`.

The retry replays the first Operation instead of creating a second resource — the
`operations.rs` ledger behaves identically over the remote TCP+mTLS path.

## Finding (not an A1/transport defect) — VM read/delete ignore non-default namespaces

A VirtualMachine created in a **non-default** namespace (`a1-accept`) is **listable** but
**not gettable or deletable by name** through the node API, and the CLI cannot delete it either:

- `POST …/namespaces/a1-accept/virtualmachines` → `SUCCEEDED`, a real CH VM boots; it appears
  in `GET …/namespaces/a1-accept/virtualmachines` (LIST) with `namespace=a1-accept`.
- `GET …/namespaces/a1-accept/virtualmachines/a1-vm` → `[HTTP 404]`.
- `DELETE` under either `a1-accept` or `default` → `[HTTP 404]`.
- `delonix vm destroy a1-vm` → `DX-4501 no such VM` (CLI resolves the default namespace only),
  even though `delonix vm ls` lists it with `NAMESPACE=a1-accept`. The orphaned CH VM had to
  be reaped manually (process kill + state-dir removal under `vms/a1-accept/`).

Root cause is in the engine VM read path, not in A1: `vms::list_in` enumerates every
namespace dir (`records()` → `VmPort::list()`), whereas `vms::get_in`/`vm_ops::delete_with_fn`
resolve by name through `VmPort::status`/`exists`, which only see the **default** namespace
dir — so the `vm.namespace != wanted` guard (`vms.rs:378`, `vm_ops.rs:537`) never matches.
Containers do **not** have this (Container GET/DELETE in `a1-accept` worked). Candidate for a
follow-up fix (or to be subsumed by A3's identity/namespace work); it does **not** affect the
TCP+mTLS transport that A1 delivers. The default-namespace VM lifecycle above is fully green.

## Teardown

Units disabled and removed (`systemctl --user disable --now` + file delete + daemon-reload);
both servers stopped; `a1-client` destroyed; every `a1-*` container/VM/CH-process gone; PKI
(`target/a1-pki/`, gitignored) deleted. No change to the owner's `obs-vm-1`, `pve92`, or any
worktree. No source code changed (proof only); `proto/delonix/node/v1` untouched.
