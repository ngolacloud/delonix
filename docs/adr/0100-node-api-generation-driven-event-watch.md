# ADR-0100: a `generation`-driven event watch — the node-API event stream carries the desired/observed version so a control plane tells a spec change from a state change

- **Status:** Accepted (2026-11-20, by the owner) — sub-step A5 of the fleet-operability Phase A
  (A1→A6), one ADR per decision, one sub-PR per sub-step. The owner chose the hybrid shape (carry the
  version on `WatchEvents`, no new RPC/filter) and the full read-side coverage (resolve the version
  from the per-Kind store so even CLI-stamped records surface on the stream), accepting the replay
  trade-off in §Consequences.
- **Date:** 2026-11-20
- **Deciders:** Walter Angolar
- **Relates to:** ADR-0099 (A4 — the desired/observed two-phase stamp this builds the watch on;
  its §"Explicitly deferred" names *"a `generation`-driven watch (vs today's timestamp watch)"* as
  **A5**, i.e. this ADR). ADR-0090 (state versioning — the `delonix.io/*` annotations and counter
  rule). ADR-0087 (node contract — `WatchEvents`). ADR-0042 step E (the event stream as the
  daemonless bus). ADR-0096/0097/0098 (A1–A3, same `feat/node-api-mtls-transport` branch).
- **Supersedes nothing.** `WatchEvents`' existing timestamp/`since` semantics are unchanged; this is
  additive.

## Context

A4 (ADR-0099) made convergence *readable*: a `Get`/`List` now surfaces `meta.generation` vs
`meta.observed_generation`, so a control plane can ask "has my desired state been realized?" — but
only by polling one resource at a time. ADR-0099 §Consequences wrote "reads convergence from one
`Get` (or `Watch`)", yet that **`Watch` does not exist for versioning**: the one time-driven stream
on the contract is `WatchEvents`, and it is blind to generation.

What `WatchEvents` is today ([`events.rs`](../../crates/interfaces/delonix-node-api/src/events.rs),
[node.proto:38](../../proto/delonix/node/v1/node.proto)):

- It tails `<root>/events.jsonl` — the daemonless bus (each command appends a line and exits). It is
  **timestamp/`since`-driven**: replay the retained log from a wall-clock instant, then follow.
- Each `Event` carries `kind/action/target/attributes` — **state actions** (`start`, `die`, `oom`,
  `create`, `remove`…). It carries **no** `generation` and cannot say whether a line means "the
  desired spec changed" or "the machine's state moved".

Two facts from the code shape the decision:

1. **The node-API `Container` mutation path never touches the event log.** `events::emit` appears in
   `image_ops`/`pod_ops`/`stacks` but **not** in `container_launch.rs` / `container_update.rs`. So a
   `CreateContainer`/`UpdateContainer` by the control plane — the very writes whose generation A4
   bumps — emit **no** event at all. Today's watch literally cannot see that a container's generation
   rose.
2. **The event log is ephemeral and generation-free by design.** `delonix_node::events::Event` has
   `ts/kind/action/id/name/detail` — no generation — and the module states "*history is not the
   point of this*", rotating to a single `.1` generation. It is a bus, not a versioned resource
   store.

The goal of A5: give the control plane a **stream** on which it can tell a **desired change**
(generation went up — "I asked for something new") from a **state-only change** (`start`/`die`/`oom`
— "the machine moved") — the thing a timestamp watch can never express, because time does not
distinguish the two.

Two constraints, from ADR-0099 and the layering guard, carry over:

- **Daemonless.** No apiserver, no resident controller. Whatever call mutates a record is the only
  writer; the emit must happen inside that call, not from a background loop.
- **Layering.** `delonix-node-api` is an interface crate; the pure versioning rule lives in
  `delonix-model::versioning` (ADR-0099 §1). The watch reuses it; it adds no interface→context edge.

## Decision

Make `WatchEvents` **generation-aware** — the **hybrid** shape the owner chose: keep the one existing
RPC and its timestamp replay, surface the version *on every event*, and let the control plane
discriminate client-side. **No new RPC, no new server-side filter.** Four parts.

### 1. `Event` carries the version (contract, additive)

Add to the contract's `Event` (node.proto), additively — a consumer that ignores them keeps working:

```proto
  // ADR-0099/0100 record metadata, read off the referenced resource when this
  // event is streamed. Absent (0) means "unversioned" — a defined state, not an
  // error. generation > observed_generation ⇒ a desired change not yet realized;
  // equal and non-zero ⇒ converged; both 0 ⇒ a resource with no version (or a
  // kind this watch does not resolve).
  int64 generation = 8;
  int64 observed_generation = 9;
```

### 2. Two version events, emitted by the two-phase stamp (make the desired change *visible*)

Today a `CreateContainer`/`UpdateContainer` through the node API emits **nothing** to the log
(fact 1 above), so a control plane watching the stream would never even see that it asked for a new
spec. So the ADR-0099 two-phase stamp gains a sibling emit, via one shared node-api helper, for any
Kind that stamps — this is what puts the desired change (and its realization) **on the stream** at
all:

- **`applied`** — emitted by the **desired stamp** (at accept, right after `stamp_desired` bumps
  `generation`): "a new spec was accepted". At this instant the record reads
  `generation > observed_generation` — "asked for, not yet realized".
- **`converged`** — emitted by the **observed advance** (after a successful reconcile, right after
  `stamp_observed`): "the node caught up". The record now reads `generation == observed_generation`.
- A call whose reconcile **fails** never reaches `converged`, so the last version line for that
  resource is `applied` and the record stays `generation > observed_generation` — a stuck change is
  representable on the stream, exactly as A4 made it representable on `Get`.

The `action` (`applied` vs a runtime `start`/`die`) is the control plane's explicit discriminator;
the typed version pair (part 3) is read alongside it.

### 3. Read side: resolve the version from the per-Kind store, for every event (the owner's full coverage)

`events::message()` becomes resource-aware: for **every** event it resolves the referenced resource
(`kind`, `name`) through the per-Kind record store and fills `generation`/`observed_generation` with
`delonix_model::versioning` (`read_version`, the same A4 reader). This covers **all** Kinds —
including records stamped **only** by the CLI `stack apply`, which never emit a version event yet now
surface their generation on any of their lifecycle events. The dispatch is a small `kind → store
load` map; a kind the watch does not resolve, or a record with no annotations, reads `0/0`
(absent-safe). The store read is off a 200 ms poll over small batches, so the tail cost is bounded.

### 4. Discrimination is client-side (no server-side filter)

The control plane branches on the fields it now has, with no new request knob:

- `action == "applied"` ⇒ **desired changed** (a new spec was accepted). Live, it reads
  `generation > observed_generation` — convergence pending.
- `action == "converged"` ⇒ **realized** (the node caught up); reads `generation ==
  observed_generation`.
- any runtime action (`start`/`die`/`oom`/…) ⇒ **state-only**; the version pair on it is the
  resource's current `(generation, observed_generation)` for context (e.g. a `die` while
  `generation > observed_generation` is a resource that failed before converging).

A client that wants only spec changes filters `action == "applied"` itself. The existing `kinds` and
`since` filters are untouched and compose with this.

### Materialization scope of this slice

- **Read side:** every event, every resolvable Kind, from the store (part 3) — done once in
  `message()`. Records stamped by the CLI or by the node API both surface.
- **Write side (emitting `applied`/`converged`):** `Container` (create and field-masked update),
  the resource A4's acceptance named, materialized and proven live. `Volume` and the rest emit the
  version events as their write paths adopt the shared helper; until then their lifecycle events
  still stream with the version read from the store (part 3) — no reader change.

### Explicitly deferred

- **A server-side `spec_changes_only` filter** on `WatchEventsRequest` — client-side discrimination
  covers A5; a server filter is a later optimization, not this cut.
- **A dedicated `WatchResources`/convergence RPC** (streaming `ResourceMeta` deltas by polling the
  store) — considered and set aside: it is a second stream and a second mechanism for what the
  existing bus now answers.
- **Capturing the version *at the instant of each event* (vs reading it now).** The owner chose
  store-read for full CLI coverage; the consequence (replay reports the current generation, §below)
  is accepted. Carrying the pair on the event record is the later refinement if replay fidelity ever
  matters more than CLI-only coverage.

## Consequences

- The control plane gets convergence **as a stream**, not just by polling `Get`: it follows
  `WatchEvents`, sees `applied` when a desired spec is accepted and `converged` when the node
  catches up, and tells either from a `die`/`oom` by the action + the version pair. One RPC, no new
  verb, no daemon.
- **Daemonless preserved.** Both emits happen inside the single mutating call, beside the ADR-0099
  stamps.
- **`WatchEvents` timestamp semantics unchanged.** `since` replay, `kinds`, the refused `namespace`
  filter — all as before; the two fields are additive and resolve to `0/0` for any unversioned or
  unresolvable resource.
- **Full coverage, including CLI-stamped records.** Because the version is read from the store at
  stream time, a resource stamped only by `stack apply` (no node-API mutation, no version event)
  still shows its `generation`/`observed_generation` on any of its lifecycle events.
- **Store I/O on the tail — bounded.** `message()` resolves the referenced record per event. The
  read is off a 200 ms poll over small batches; a kind the watch does not map reads `0/0` without a
  load. Reusing `read_version` keeps the rule identical to A4's `Get`.
- **Replay reports the *current* version, not the historical one** (the accepted trade-off). A
  reconnecting client replaying the retained log reads each resource's generation **as it is now**,
  not as it was when the line was written; the `applied`/`converged` **action** still marks where a
  transition happened. Live following (the control plane's real use) is unaffected — the store read
  right after an emit reflects that emit's state.
- **Layering respected.** The version rule comes from `delonix-model::versioning` (ADR-0099);
  resolving a record uses the node-api store ports it already holds — no interface→context edge.
- **Reversible.** The proto fields are new; the version events are new actions a prior consumer
  ignored; a revert drops them and nothing else depends on them yet.
