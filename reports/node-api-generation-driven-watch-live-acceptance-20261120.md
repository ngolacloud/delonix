# A5 — a `generation`-driven event watch on the node API: live acceptance (2026-11-20)

ADR-0100. Branch `feat/node-api-mtls-transport` (A1 committed 64f01d72; A2+A3+A4+A5 on top, not
committed). Sub-step A5 of the fleet-operability Phase A (A1→A6).

## What was built

`WatchEvents` is now generation-aware, the hybrid shape the owner chose: the one existing RPC and its
timestamp replay keep working; the version rides on every event; the control plane discriminates
client-side. No new RPC, no new server-side filter.

- **Contract (additive):** `Event` (node.proto) carries `generation` (field 8) and
  `observed_generation` (field 9).
- **Write side — the version events:** the ADR-0099 two-phase stamp gained a sibling emit, so a
  desired change the control plane asks for becomes VISIBLE on the stream (the container mutation
  path emitted nothing to the log before). `container_launch` emits `applied` after the desired
  stamp and `converged` after the observed advance; `container_update` emits them when a resize
  actually changed the spec (`gen_after > gen_before`). Helpers live in `node-api::versioning`
  (`emit_applied`/`emit_converged`, `ACTION_APPLIED`/`ACTION_CONVERGED`).
- **Read side — full coverage from the store:** `events::message` resolves the referenced resource
  (today `container`) through the per-Kind record store and fills the version pair with
  `delonix_model::versioning::read_version` (the same A4 reader). Because the container store is the
  one BOTH the node API and the CLI `stack apply` write, a container stamped by either surfaces its
  generation on any of its lifecycle events. A kind the watch does not resolve, or a record with no
  annotations, reads `0/0` — absent-safe. The store read runs on the blocking pool, off the runtime.
- **Discrimination (client-side):** `applied` + `generation > observed_generation` ⇒ desired changed;
  `converged` + equal ⇒ realized; a runtime action (`crashed`/`die`/…) carries the current pair for
  context and never bumps the generation.

## Proof 1 — full cycle over the real gRPC NodeService (in-process, host, rootless)

`DELONIX_ROOT=<root+busybox> DELONIX_BIN=<delonix> cargo test -p delonix-node-api --test
live_watch_generation -- --ignored watch_tells_a_desired --nocapture` — a watcher opens the stream,
then a real busybox is created, resized and (a representative) runtime death is recorded:

```
stream: action=applied   target=Container/a5-watch-probe generation=1 observed=0   (CREATE: desired; g>o pending)
stream: action=applied                                    generation=2 observed=2   (RESIZE: desired; generation climbs)
stream: action=crashed                                    generation=2 observed=2   (STATE: generation holds)
```

- A create is a desired change: `applied`, generation 1. The stream even caught the pending window
  (`observed 0`) between the desired stamp and the observed advance.
- A changed spec is a new desired generation: `applied`, generation 2 — the watcher sees it climb.
- A runtime `crashed` is NOT a desired change: a different action, and the generation holds at 2.

## Proof 2 — cross-process over the production transport (TCP + mutual TLS)

The WATCH is **gRPC-only** — its REST transcode answers `DX-6001` ("the REST encoding does not carry
streams yet — call it over gRPC"), so the A4 "curl from the VM" shape does not apply to a stream. A
gRPC client with an async rustls connector under tonic drove it over the real transport instead.

Node API served on the host at `tcp://192.168.122.1:29443` with mutual TLS (ADR-0096/A1 transport: a
CA, a server leaf with `subjectAltName=IP:192.168.122.1`, a `control-plane` client leaf). Client =
`cargo test … live_watch_generation -- --ignored watch_over_tcp_mtls`, handshaking that client cert
to the bridge address and following `WatchEvents` cross-process:

```
[mTLS] action=applied target=Container/a5-mtls-probe generation=1 observed=0   (CREATE over mTLS)
[mTLS] action=applied                                 generation=2 observed=2   (RESIZE: FieldMask carried over gRPC)
[mTLS] action=die                                     generation=2 observed=2   (KILL → supervisor, state-only)
```

- The same desired-vs-state distinction holds over the wire the control plane will actually use: the
  version pair serializes identically on TCP+mTLS and on the unix socket.
- The resize's `FieldMask` is carried because the leg is gRPC (the REST PATCH could not — A4's
  finding); the `die` from a real `KillContainer` holds the generation at the last desired value.

## Gates (delta ZERO vs the A3 reference)

- `scripts/arch_fitness.py`: `cli_context_lines 126957 / env_writes 25 / library_prints 119 /
  raw_error 1` — byte-identical to the A3 reference. A5 adds zero. (The test `eprintln!`s were removed
  to hold `library_prints`, as A4 did; the asserts carry the proof.)
- `scripts/lang_ratchet.py`: `docs 382` = the A3 reference; ADR-0100 and this report are English and
  appear nowhere in `--list --only docs`. (The 382>380 and the arch FAILs are pre-existing A2/A3/A4
  debt vs the committed A1 baseline.)
- `cargo build -p delonix-node-api-bin -p delonix-bin`: green.
- `cargo test -p delonix-node-api --lib events::`: 7 pass (incl. 2 new — the store-resolved version
  on an `applied`, and `0/0` on an unversioned state event). `tests/grpc_watch_events`: pass.

## Scope / deferred (per ADR-0100)

- Read side resolves `container` today; other Kinds read `0/0` until the `StorePort` grows their load
  (the reader is Kind-agnostic — no reader change needed when they do).
- A server-side `spec_changes_only` filter and a dedicated `WatchResources` RPC were considered and
  set aside. Capturing the version at the instant of each event (vs reading it now) is the later
  refinement if replay fidelity ever outweighs CLI-stamped coverage — the owner chose store-read.
