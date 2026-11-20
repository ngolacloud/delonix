# ADR-0101: the node scheduling surface — labels, taints, schedulability (cordon) and extended capacity, on `node/v1` and the declarative `kind: Node`

- **Status:** Accepted (2026-11-20, by the owner) — sub-step **A6**, the last of the fleet-operability
  Phase A (A1→A6), one ADR per decision, one sub-PR per sub-step. The owner chose the **broad** scope
  (labels + taints + cordon/uncordon **and** an enriched `Capacity` with GPU/IPs/overcommit), on **both**
  surfaces (the `node/v1` API **and** the declarative `kind: Node` apply), with cordon that **enforces**
  (a cordoned node refuses a new workload create), accepting the partial-measurement trade-off for GPU
  and overcommit in §Consequences.
- **Date:** 2026-11-20
- **Deciders:** Walter Angolar
- **Relates to:** ADR-0099 (A4 — the `generation`/`observed_generation` stamp this reuses for the node
  record; node metadata is single-phase, so `observed_generation == generation`). ADR-0100 (A5). ADR-0098
  (A3 — authorization by cert identity + audit; the new write RPCs are mutating ops that A3 enforces and
  audits automatically). ADR-0096/0097 (A1/A2, same `feat/node-api-mtls-transport` branch). ADR-0090
  (state versioning — the `delonix.io/*` annotations). ADR-0086 D3/D7 (the `kind: Node` Reserved at
  `v1beta1`; D7 node-scoped Kinds). ADR-0087 (node contract — `GetNodeInfo`/`GetCapacity`). ADR-0042 D2
  (the node API, idempotency, RFC 9457 problem docs). ADR-0046 (IPPool/IPAddress ledger — the IP source).
- **Supersedes nothing.** All additions are additive: new `NodeService` RPCs, new fields on `NodeInfo`
  and `Capacity`, a new engine error code, and the first apply for a Kind that was Reserved.

## Context

A1–A5 made the node operable by an external control plane over mutual TLS: a secure transport (A1),
an enrolled identity (A2), authorization + audit per call (A3), readable convergence (A4) and a
generation-driven event stream (A5). What a control plane still cannot do is the thing a fleet
scheduler exists to do: **place work on the right node, and take a node out of rotation**. The Phase A
initial measurement named this gap precisely — *"inventory without GPU/IPs/overcommit/labels/taints/
cordon-drain"* — and the code names it too:
[`kinds.rs:127-133`](../../crates/contexts/delonix-stack/src/domain/kinds.rs#L127-L133) marks
`kind: Node` **Reserved** and says *"what a declarative `kind: Node` writes (labels, taints,
schedulability) is deliberately left for the phase that builds the … read-back surface"*. A6 is that
phase.

What exists today:

- [`NodeInfo`](../../proto/delonix/node/v1/node.proto#L54) carries static facts (engine version, kernel,
  arch, rootless, cgroup driver) — **no** labels, taints or schedulability. There is **no write RPC** on
  `NodeService`; all six RPCs are reads ([`service.rs`](../../crates/interfaces/delonix-node-api/src/service.rs)).
- The node keeps **no persisted node-scoped state**. `node.rs` computes every answer at call time and
  *"never from a second copy"* ([`node.rs:1-8`](../../crates/interfaces/delonix-node-api/src/node.rs#L1));
  nothing records labels/taints/schedulability.
- [`Capacity`](../../proto/delonix/node/v1/node.proto#L80) reports cpu/memory/pids/filesystems and running
  counts, with an [`unmeasured`](../../crates/interfaces/delonix-node-api/src/node.rs#L384) list for any
  axis it cannot establish — *"a zero that comes from a failed measurement is not a zero"*. It reports no
  devices (GPU), no IP inventory, no overcommit.
- `kind: Node` is `Form::Reserved` / `in_stack:false` / `converges:false` / `NotObservable`
  ([`kinds.rs:916-929`](../../crates/contexts/delonix-stack/src/domain/kinds.rs#L916)): it falls into the
  `_` fallback of every per-Kind dispatch in [`cmd/stack.rs`](../../bins/delonix/src/cmd/stack.rs) and is
  never applied, converged, stamped or typed.

Two constraints carry over from the whole campaign:

- **Daemonless, one writer.** No resident controller. Whatever call mutates the record is the only
  writer; there is no reconcile loop to run behind it. Node metadata is therefore **single-phase**:
  the write both sets the desired spec and realizes it, so `observed_generation == generation` always.
- **Layering.** `delonix-node-api` is an interface crate. The record type and the version rule live in
  the foundation crate `delonix-model` (where A4 already put `versioning`); both the interface (node API)
  and the context (stack apply, via the CLI) read and write the one record through it, with no
  interface→context edge.

## Decision

### 1. One shared node record, written by both surfaces (`delonix-model::node_meta`)

A single node-wide record lives at `<state_root>/node/node.json`, the root both the CLI
([`util::state_root`](../../bins/delonix/src/cmd/util.rs#L14)) and the node API
([`node::state_root`](../../crates/interfaces/delonix-node-api/src/node.rs#L24)) already resolve from
`$DELONIX_ROOT`. It is a **standard annotated record**, so A4's machinery applies unchanged:

- `metadata.labels` — the operator-declared **placement labels** (k8s-aligned: node labels are
  metadata, not spec).
- `metadata.annotations` — the `delonix.io/*` version stamps (ADR-0090/0099), read back by A4's
  `read_version`.
- `spec` — `taints` (`{key, value, effect}`), `schedulable` (the cordon flag, default `true`),
  `unschedulable_reason`, and an optional operator-declared `overcommit` `{cpu, memory}` ratio.

The record type `NodeRecord`/`NodeSpec`/`Taint`/`Overcommit` and `load`/`store` (atomic) are defined in
`delonix-model::node_meta`, reusing `delonix-model::versioning` for the generation counter. An absent
file reads as the default (empty labels/taints, **schedulable**, no overcommit) — a fresh node is
schedulable and unlabelled, not an error.

### 2. The `node/v1` write surface — three focused, full-replace RPCs (contract, additive)

`NodeService` gains three mutating RPCs, each replacing one dimension (no `FieldMask`, which A4 proved
does not transcode over REST), each returning the resulting `NodeInfo`, each a `POST` custom verb (the
`:verb` shape `WatchEvents` already uses, so the existing transcoder serves them):

- `SetNodeLabels(map<string,string>)` → `POST /v1/node:setLabels`
- `SetNodeTaints(repeated Taint)` → `POST /v1/node:setTaints`
- `SetSchedulability(bool unschedulable, string reason)` → `POST /v1/node:setSchedulability`

`NodeInfo` grows `labels`, `taints`, `unschedulable`, `unschedulable_reason`, `generation` and
`observed_generation` (all additive). Each write bumps `generation` when the applied dimension changed
and sets `observed_generation = generation` (single-phase). These are **mutating** operations: A3
authorizes them by cert identity and audits every call with no extra wiring beyond listing them in the
`is_mutating` set; an operator grants e.g. `NodeService.SetSchedulability` or `NodeService.*`.

Overcommit is **not** writable over `node/v1` — it is declarative node configuration, set through
`kind: Node` (§4) and *read* on both `NodeInfo`-less `Capacity` and the record. The `node/v1` RPCs cover
the **operational** verbs a control plane changes often (cordon, labels, taints); overcommit changes
with the node's declared config, not with scheduling.

### 3. Cordon **enforces**: a cordoned node refuses a new workload create

When `schedulable == false`, the workload create paths —
[`CreateContainer`](../../crates/interfaces/delonix-node-api/src/container_launch.rs#L491),
`CreatePod`, `CreateVirtualMachine` — refuse **before** claiming the name or beginning the operation,
with `Status::failed_precondition` carrying a new engine code **DX-5003 `api_node_cordoned`** (Conflict
class → HTTP **409**, gRPC `FAILED_PRECONDITION`). This is stronger than Kubernetes, where only the
scheduler honours `unschedulable` and the kubelet still runs what it is told; for a control plane that
talks **directly** to the node it is the correct fail-safe — the node itself is the last line. Reads,
updates to existing workloads, deletes and `uncordon` are unaffected. Taints are **not** enforced by
the node: they are placement metadata the control-plane scheduler honours (tolerations live in the
control plane, which A6 does not build) — the node enforces only schedulability, the one hard gate.

### 4. The declarative `kind: Node` gets its apply (the surface `kinds.rs` deferred)

`kind: Node` is promoted from `Form::Reserved` to `Form::Primary` (`in_stack:false` — a node is a
node-wide singleton, applied on its own with `stack apply -f node.yaml`, not grouped into a user Stack;
`converges:true`; `Presence::Registry`). It gains a typed `NodeSpec` (`schemars::JsonSchema`,
`labels`/`taints`/`schedulable`/`overcommit`), a `NODE_SPEC_FIELDS` allowlist, a `SCHEMA_KINDS` entry,
and arms in the per-Kind dispatches of `cmd/stack.rs` (`desired`, `apply_layer`, `converge_and_stamp`,
`stamp_all`) plus a new `cmd/node.rs` module — the IPAddress pattern
([`cmd/ipaddress.rs`](../../bins/delonix/src/cmd/ipaddress.rs)). The apply writes the **same**
`delonix-model::node_meta` record §1 and stamps generation through `reconcile::stamp_annotations(…, k::NODE)`.
Spec fields are **optional**: an apply updates only the dimensions the manifest declares, so it does not
clobber a cordon or labels set operationally over `node/v1` (declarative apply is the source of truth for
what it declares; it is silent about what it omits).

### 5. `Capacity` enrichment — measured where we can, named `unmeasured` where we cannot

[`node::capacity`](../../crates/interfaces/delonix-node-api/src/node.rs#L378) grows three fields, filled
in the same place, with the same `unmeasured` doctrine:

- **IPs** (`repeated IpPoolCapacity {name, addresses_total, addresses_allocated}`): read from the
  declarative IPPool ledger JSON under `<state_root>/ippool/*.json`
  ([`PoolFile`](../../bins/delonix/src/cmd/ippool.rs#L53)) — `addresses_total` counted with the shared
  `delonix_sdn::Cidr` (so CIDR math does not drift from the binary's `expand`), `addresses_allocated =
  leases.len()`. No pool declared ⇒ nothing reported (absent, not unmeasured — the filesystem doctrine).
- **GPU** (`repeated DeviceCapacity {kind, total, allocatable}`): a cheap, dependency-free probe of
  `/proc/driver/nvidia/gpus/`. Present ⇒ count and report `nvidia.com/gpu`. **Absent ⇒ `"devices.gpu"`
  in `unmeasured`**, because the node API deliberately does not run `nvidia-smi` in a capacity read, and
  a missing driver cannot be told apart from "no GPU" with certainty.
- **Overcommit** (`OvercommitRatios {cpu, memory}`): read from the node record's operator-declared
  `overcommit` (§1/§4). Unset ⇒ `"overcommit.cpu"`/`"overcommit.memory"` in `unmeasured` — there is no
  node-level overcommit ratio in the engine today, so an un-declared one is genuinely unmeasured, not 0.

### Explicitly deferred

- **Tolerations / taint enforcement at the node.** Taints are advertised and honoured by the control
  plane; the node enforces only cordon. A node-side toleration check would need the control plane's
  scheduler model, out of Phase A.
- **Overcommit over `node/v1`.** Writable only declaratively in A6; a `SetOvercommit` RPC can follow.
- **Drain (evict running workloads).** A6 refuses *new* creates when cordoned; evicting what already
  runs is a control-plane action (stop each workload) over the existing RPCs, not a node verb here.
- **Real GPU/overcommit measurement.** No GPU hardware on the proving host; A6 exercises only the
  `unmeasured` path for both. The detailed GPU inventory stays in the resource advisor
  ([`resource_advice.rs`](../../crates/adapters/delonix-linux/src/resource_advice.rs)).
- Inherited polish unchanged: FieldMask-by-query over REST (A4), generation stamp on Volume and other
  Kinds on the watch read-side (A5), the VM get/delete-by-name-in-non-default-namespace finding (A1).

## Consequences

- A control plane can now **label**, **taint** and **cordon/uncordon** a node over mutual TLS, and
  reading `GetNodeInfo` back confirms the write (generation rose). A cordoned node **refuses** new
  workloads with a distinct code (DX-5003/409), so a maintenance window is enforced at the node, not
  merely advertised.
- The same state is reachable **declaratively** (`kind: Node`) — a node's labels/taints/schedulability/
  overcommit become part of GitOps, stamped and converged like every other Kind — and **imperatively**
  (`node/v1`) for the operational verbs. The two share one record; declarative apply is source of truth
  for what it declares and silent about what it omits, so a declarative baseline and an operational
  cordon coexist without clobbering each other.
- `Capacity` gains real IP inventory and an honest GPU/overcommit story: what the node can establish it
  reports; what it cannot it **names** in `unmeasured` rather than reporting a misleading zero. On a host
  with no GPU and no declared overcommit, the live proof shows both in `unmeasured` by design — the
  partial measurement the owner accepted.
- `kind: Node` leaves Reserved: the `core.delonix.io` group now has one more live Kind (it stays
  `v1beta1`; promotion to `v1` is unaffected and still gated on two releases live).
