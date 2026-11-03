# ADR-0090: State versioning — `generation`, `observedGeneration`, `storedVersion` as record metadata

- **Status:** Accepted (2026-11-03, by the owner) — the first decision of the ADR-0090 slot,
  built in campaign phase 2c-ii, slice by slice, each behaviour-preserving with a migration test
  before anything reads the new fields.
- **Date:** 2026-11-03
- **Deciders:** Walter Angolar
- **Relates to:** ADR-0088 (declarative foundation; its A2 addendum earmarked these three fields
  as "the only genuinely PERSISTED parts" of a status, as opposed to the DERIVED `conditions`/
  `Ready`/`outputs` built in 2c-i), ADR-0086 (catalog v1 — `storedVersion` is what a future
  `manifest convert` reads to know which version a record was written in), ADR-0019 (revision
  history — the same "record what was asked for" discipline the `last-applied` stamp already
  follows).
- **Reserved siblings:** `ADR-0089` is reserved for the convergent `KubernetesCluster` decision
  (ADR-0088 D-list item, not yet written). The ADR-0090 slot is the earmarked home for a set of
  later-phase capabilities the catalog brief named together — microVM identity, `IPAddress`/LB,
  fencing, `Node`, SLIs. **This first cut decides ONLY state versioning** (the part phase 2c-ii
  needs); each companion capability is decided in the phase that builds it, so nothing here
  pre-commits a design that has no code yet.

## Context

ADR-0088 A2 split a resource's status into two kinds of thing, and the addendum (2026-11-03)
made the split explicit after measuring the engine's "this engine persists no status" stance:

- **Derived on read** — `conditions` (incl. `Ready`), `outputs`. The engine already derives these
  (`reconcile_status`/`presence`/`pod_ip`), and a stored copy would go stale. Phase **2c-i** built
  this (`is_ready` = inverse of `is_pending`; a top-level `status:` in an input manifest is
  accepted and ignored so `get -o yaml` round-trips). It holds the "no stored status" decision.
- **Genuinely persisted** — `generation`, `observedGeneration`, `storedVersion`. These are not
  derivable from the machine: a spec-version counter, the generation the record was last
  reconciled at, and the apiVersion the record was written under. They are **record metadata,
  akin to `delonix.io/last-applied`**, not user-facing status. This ADR decides them, and phase
  **2c-ii** builds the persistence per-resource with a tested migration.

The engine has no apiserver to bump `generation` on a spec write and no resident controller to
bump `observedGeneration` after a reconcile — `stack apply` is the only writer, and it writes the
`last-applied` stamp only **after** a resource's layer succeeds (`stamp_all`, `cmd/stack.rs`).
That single writer is the fact the decision has to fit, not fight.

## Decision

Three fields are persisted on each ownable resource's record, carried the same way the
`last-applied` fingerprint already is — a `delonix.io/*` annotation on the per-Kind store record,
written by the Kind's existing `stamp()` at the end of a successful apply. No new store, no new
struct field on every record; the same mechanism, three more keys. **Absent-safe**: a record
written before this slice carries none of them, and that is a defined state ("unversioned"), not
an error — the next apply stamps them.

### `delonix.io/generation` — the spec-version counter

A monotonic integer, starting at `1` on first apply. On a successful apply of a resource whose
**applied spec changed** — the `last-applied` fingerprint this apply writes differs from the one
already on the record — `generation` increments; when the applied spec is unchanged, it is kept.
`generation` is thus an audit counter: "how many distinct specs this resource has been applied
with", bumped by the same comparison the stamp already makes, never on an ownership-only or
status-only write.

A resource with no stored generation (unversioned, or created by hand and then adopted) is
treated as generation `0`; its first stamped apply writes `1`.

### `delonix.io/observed-generation` — the generation the machine is at

The generation the last **successful** reconcile brought the machine to. Because the only writer
(`stamp_all`) runs after the resource's layer succeeded, at stamp time the machine IS at the
generation just written, so `observedGeneration == generation` on the happy path. It is persisted
rather than assumed equal so a reader (`get -o yaml`, the node contract's `status`) states it
without recomputation, and so the one case where they differ is representable: a resource whose
apply **failed** in its layer is not stamped, so its `generation`/`observedGeneration` stay at the
last success — a future refinement that writes a *desired* generation before the reconcile (so a
pending change shows `generation > observedGeneration`) is a separate slice and is **out of scope
here**; this cut keeps them equal and says so.

### `delonix.io/stored-version` — the apiVersion the record was written under

The `apiVersion` string the resource was applied with (e.g. `compute.delonix.io/v1alpha1`),
written at stamp. It is the field a future `manifest convert` (ADR-0086 catalog v1) reads to know
which version a stored record speaks, so a v1beta1 Kind can convert a v1alpha1 record on read
without guessing. Until catalog v1 lands there is exactly one version per Kind, so `storedVersion`
is informational now and load-bearing later — written now so the records exist to convert when the
converter does.

### Migration — additive, no data moved

Records written before this slice have none of the three annotations. The migration is the
absent-safe default, not a data move: on read, a missing `generation` reads as `0`/unversioned, a
missing `storedVersion` as the Kind's current apiVersion; the **next apply** stamps all three. No
file is relocated and no record is rewritten out of band. The phase-2c-ii commit carries a test
that an unversioned record loads, reports unversioned, and is upgraded (generation `1`,
`storedVersion` set) by the next apply — the "tested migration" ADR-0088's slice list names.

### Explicitly deferred

The **`vms/<name>.json` → `vms/<ns>/<name>.json` relayout** (making a VM's name unique per
namespace rather than per node, revisiting ADR-0069 #2) is *enabled* by `storedVersion` — a record
relayout is exactly the kind of change a stored version lets a reader migrate safely — but it is a
**separate decision** about identity, not part of this state-versioning cut. It moves files on
disk (real risk) and changes a uniqueness contract, so it gets its own slice and its own GO
decision; this ADR only makes it *possible*, it does not do it.

## Consequences

- `generation`/`observedGeneration`/`storedVersion` are **record metadata, never a user knob**: a
  top-level `status:` or a `metadata.generation` in an input manifest is accepted and ignored
  (the 2c-i round-trip rule extends to them), so `get -o yaml` output reapplies cleanly.
- The node contract (ADR-0087) carries these three over `ResourceService.Get/Watch` in the
  `status`, additively — a consumer that ignored status keeps working.
- The persistence is **per-Kind**, through each Kind's `stamp()`, because that is where
  `last-applied` already lives; a Kind that is deliberately not stamped (shared content: `Image`,
  `App`, `Secret`) carries no generation either, for the same reason it carries no owner.
- The decision is **reversible per slice**: the annotations are additive and absent-safe, so a
  revert drops three keys nothing else depends on yet.
- It stays **daemonless**: nothing here adds a process or a watch — the counter is written by the
  same apply that already writes the stamp.
