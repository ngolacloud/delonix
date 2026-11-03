# ADR-0087: One generic resource service on the node socket, not a service per Kind

- **Status:** Accepted (2026-11-03, by the owner's catalog brief) — decided here; built in
  campaign phase 6. Supersedes the per-Kind service design and names the `buf breaking`
  exception it needs. Implements ADR-0003 (capability model) at the socket.
- **Date:** 2026-11-03
- **Deciders:** Walter Angolar
- **Relates to / supersedes:** ADR-0040 (node contract P0–P5), ADR-0041/0042 (error model),
  ADR-0003 (tenancy-free capability model — implemented here), ADR-0010 (the API stays local).
  Supersedes the one-request-per-RPC / per-Kind-service shape that the contract gate imposes
  today (AGENTS.md "The node contract is a gate").

## Context

Measured at `64a5953a` (re-measurement doc 70): the node contract serves **ten typed services,
all implemented** (`delonix-node-api/src/service.rs:948-993`) — Container, Pod, VirtualMachine,
Image, Stack, Network, Volume, Storage, Node, Operation. (`VM.Console` and `Image.Push/Build/
Scan` return UNIMPLEMENTED; everything else is served.) The earlier brief's premise — "these
services are in the `.proto` but not served" — is obsolete; Sprint 13 served them. The contract
gate (`scripts/contract_gate.py`, AGENTS.md:148,159-166) **imposes** one request message per
RPC and explicit `namespace`/`name` identity, and **forbids** a generic `ResourceMeta`.

The engine is the data plane for a local agent that drives it with the same declarative
manifests a person writes. A service per Kind means: adding a catalog Kind (ADR-0086 adds six)
requires a new service, new RPCs, new generated code, and a new client integration — for a
resource whose entire lifecycle is already `plan`/`apply`/`get`/`list`/`watch`/`delete`. The
`StackService.ApplyStack` already does this over a manifest; it just is not reachable per-Kind.

Authorization today is **only** `SO_PEERCRED` uid-equality plus a `0600` socket (`lib.rs:295`);
ADR-0003 is accepted but its capability gate is **not built** (the `enum Capability` that exists
is the provider capability catalog of ADR-0050, not an authorizer). On a multi-client node that
is not enough: a credential must see only its namespaces.

## Decisions

**D1. One `ResourceService`, generic over every catalog Kind.** `Apply` (returns an
`Operation`), `Plan`, `Get`, `List`, `Watch` (resumable by offset, closing the current
timestamp-only gap), and `Delete`, taking a typed identity (`group`, `version`, `kind`,
`namespace`, `name`) and the manifest body. Requests are validated by the **same schema** and
run through the **same reconciler** as `stack apply` — the agent sends the manifests a person
writes. Adding a Kind to the catalog serves it automatically, with no new RPC.

**D2. The per-Kind services are retired, and this ADR names the `buf breaking` exception.** The
contract gate's `buf breaking` run gets a named, dated exception for removing
`VirtualMachineService`/`ContainerService`/`PodService`/`ImageService`/`StackService` and the
mutating RPCs of `Network/Volume/Storage` services (read-only `NodeService` stays). Per AGENTS.md,
a `buf breaking` exception exists **only** with an ADR that names it — this is that ADR. The
services are served today, so there are in-principle clients; in practice the only client on the
node socket is the engine's own CLI and the local agent (ADR-0010 keeps the API local), so the
migration window is one release during which both the generic service and the retired ones are
served, with the old ones emitting a deprecation detail. The contract-gate convention of
"one request per RPC, explicit identity" is replaced for `ResourceService` by a typed
`ResourceRef` (not an untyped `ResourceMeta`): identity stays explicit and per-request, it is
simply uniform across Kinds.

**D3. Streaming and node RPCs stay their own services.** What is not declarative keeps a typed
RPC: `Exec`, `Logs`, `Attach`, the VM console, and port-forward (all streaming; `Attach` and
port-forward are new to the contract). Plus `NodeService`, `OperationService` (`Get`, `Watch`,
`Cancel`) and `WatchEvents`. These are not resources and do not belong in `ResourceService`.

**D4. Calls stay in-process, and the launch/exec re-exec stays documented.** The server calls
the `delonix_*` crates in-process (ADR-0040 D5); the one exception is launch/exec/pod-create,
which re-exec the `delonix` binary because `setns`/`clone` are single-thread-incompatible with
the multi-thread server — that exception is kept and documented, not a target to remove.

**D5. `ErrorDetail` is typed on every path.** The REST path already emits RFC 9457
`application/problem+json`; the gRPC path today attaches only the DX number as a trailer
(`network_ops.rs:73-74`), while `common.proto:113` promises the full typed `ErrorDetail` as a
binary detail. gRPC is brought up to the contract: the typed `ErrorDetail` is attached on every
error path, so the two encodings carry the same error.

**D6. Namespace authorization at the socket (implements ADR-0003).** A local credential carries
a scope of namespaces, verbs and Kinds. The node-scoped Kinds (`Node`, `StoragePool`, `IPPool`,
`Router`, `RuntimePolicy`) accept only an operator-scoped credential. `List`, `Watch` and
operations are filtered by scope; a namespace outside the scope is refused with
`PERMISSION_DENIED` — **never** an empty list (an empty list would read as "nothing there" and
hide the resource). `SO_PEERCRED` uid-equality stays as the outer gate; the capability scope is
the inner one. This is ADR-0003's gate, finally built, on the surface it was accepted for.

## Consequences

- A generic service means one validation and one reconciler path for every Kind; the per-Kind
  services stop being the integration cost of adding a Kind.
- The `buf breaking` exception is a **one-time, named** event; after the migration release the
  retired services are gone and `buf breaking` has no standing exception again.
- Authorization changes the socket's observable behaviour (a scoped credential sees fewer
  namespaces); this is a contract change, documented, with a test that two credentials on the
  same node do not see each other's resources, operations or events.
- The in-process boundary is unchanged; this ADR does not add a daemon (the server is still
  socket-activated and idle-exits, ADR-0040 P5).
- Rollback: the retired services still serve during the migration release, so a client that has
  not moved keeps working until the next major.
