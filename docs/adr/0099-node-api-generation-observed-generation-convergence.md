# ADR-0099: `generation`/`observedGeneration` on the node-API mutation path — a desired/observed split so a remote control plane reads convergence

- **Status:** Accepted (2026-11-20, by the owner) — sub-step A4 of the fleet-operability
  Phase A (A1→A6), one ADR per decision, one sub-PR per sub-step. The owner chose
  `generation`/`observedGeneration` as A4's scope from the Phase A gap map.
- **Date:** 2026-11-20
- **Deciders:** Walter Angolar
- **Relates to:** ADR-0090 (state versioning — decided the three `delonix.io/*` record-metadata
  annotations and their counter rule, materialized today ONLY in the CLI `stack apply` writer via
  `stamp_all`→`stamp()`; this ADR extends that mechanism to the node-API mutation path and takes up
  the "desired generation before the reconcile" refinement ADR-0090 §observed-generation **explicitly
  deferred**). ADR-0087 (node contract — the `status` these fields ride on). ADR-0096/0097/0098 (A1–A3:
  the same `feat/node-api-mtls-transport` branch — remote TCP+mTLS, enrollment, authz+audit). ADR-0088
  (declarative foundation — status split into derived vs persisted).
- **Supersedes nothing.** ADR-0090 stays authoritative for the CLI path; its happy-path invariant
  (`observedGeneration == generation`, stamped only post-success) is **unchanged** here.

## Context

Phase A makes the engine operable as a fleet by an external control plane over the node contract. A1–A3
gave that plane a remote, authenticated, authorized, audited transport. What it still cannot do is **read
convergence**: when it applies a change, it has no machine-readable "did the node catch up to what I asked?"
The only concurrency primitive on the contract today is the opaque `etag`/`If-Match` (optimistic locking),
which answers "did someone else write since I read?" — not "has my desired state been realized?".

ADR-0090 already decided the vocabulary for this — `generation` (a spec-version counter), `observedGeneration`
(the generation the machine was last reconciled to), `storedVersion` — as `delonix.io/*` record-metadata
annotations written by each Kind's `stamp()`. Two facts make it not-yet-usable over the node API:

1. **The node-API mutation path never stamps.** `CreateContainer`/`UpdateContainer`/`CreateVolume` write the
   record directly through `StorePort` (`container_launch.rs:480`, `container_update.rs`, `volume_ops`), never
   through `stamp_all`/`stamp()`. A record created or updated by the control plane carries **no** `generation`
   and **no** `delonix.io/last-applied` — only `stack apply` (CLI) stamps. So the control plane's own writes
   produce unversioned records.
2. **The contract's `status`/`meta` does not surface the fields.** `ResourceMeta` (common.proto) stops at
   field 8 (`etag`); no `generation`/`observedGeneration`/`storedVersion`. Even where the annotations exist on
   the record, a consumer would have to scrape the raw `annotations` map and parse strings.

Worse for the stated goal, ADR-0090's cut keeps `observedGeneration == generation` on the happy path (it
stamps only **after** a reconcile succeeds) and **explicitly deferred** the refinement that writes a *desired*
generation **before** the reconcile — the one that makes a pending change show `generation > observedGeneration`.
That deferred refinement is exactly what "the control plane reads convergence" requires, and it is A4's job.

Two constraints shape the decision, not fight it:

- **Daemonless.** The engine has no apiserver to bump a generation and no resident controller to advance an
  observed one. The only writer of a given record is whatever call is mutating it right now. ADR-0099 must bump
  and advance within that one call, not from a background loop.
- **Layering.** `delonix-node-api` is an *interface* crate; it depends on `delonix-model`,
  `delonix-compute`, … but deliberately **not** on `delonix-stack` (the Cargo.toml guards against
  interface→context edges). The counter rule and annotation keys live in `delonix-stack`
  (`src/app/reconcile.rs`). The node API cannot reach them without either a forbidden edge or a shared home.

## Decision

Extend the **exact ADR-0090 mechanism** to the node-API mutation path, with a two-phase (desired/observed)
stamp, and surface the three fields on the contract as typed metadata. Four parts.

### 1. Lift the pure versioning primitives down to `delonix-model`

Move the counter rule and annotation vocabulary — the constants `delonix.io/generation`,
`delonix.io/observed-generation`, `delonix.io/stored-version`, `delonix.io/last-applied`, the pure functions
`next_generation`, `version_annotations`, and `encode_last_applied`/`decode_last_applied` — from
`delonix-stack::app::reconcile` into a new module `delonix-model::versioning`. `delonix-model` is the lowest
crate **both** the CLI reconciler and the node API already depend on, so this gives one source of truth for the
rule without any interface→context edge.

What stays out of the moved code: the `apiVersion` lookup (`kinds::facts`, the catalog) stays in
`delonix-stack`. The moved functions already take `api_version: &str` as a parameter, so each caller supplies
it — the CLI from `kinds::facts(kind)`, the node API from the Kind it is mutating. `delonix-stack::reconcile`
**re-exports** the moved items (`pub use delonix_model::versioning::*`) so every existing CLI call site and
test compiles unchanged — a behaviour-preserving move, not a rewrite. `stamp_annotations`/`stamp_annotation_delta`
(which call `kinds::facts`) stay in `delonix-stack` as thin wrappers over the moved `version_annotations`.

### 2. A desired/observed split for the node-API mutation path (the deferred refinement)

The CLI stamps once, post-success, with `observedGeneration == generation`. The node-API mutation model is
asynchronous for Create/Delete (they return an `Operation`; the work runs in `create_work` between
`operations::begin` and `finish`) and field-masked for Update. ADR-0099 splits the stamp into two writes on
this path, both still inside the single mutating call — no daemon:

- **Desired stamp, at accept** — before the reconcile work runs (right after the name is claimed / spec
  validated, where the record is first written). Compute the applied-spec fingerprint of the *desired* spec,
  write `delonix.io/last-applied` and bump `delonix.io/generation` by the ADR-0090 rule (`next_generation`:
  bump iff the applied spec changed vs the one already stored, else keep; `0`→`1` on first apply). **Leave
  `observedGeneration` at its previous value** (absent ⇒ `0`). After this write, a record whose desired spec
  just changed shows `generation > observedGeneration` — "asked for, not yet realized".
- **Observed advance, on success** — when the reconcile work for that call completes successfully (the
  `operations::finish` ok path for async verbs; the end of a successful `UpdateContainer` for the field-masked
  verb), set `observedGeneration = generation`. The record now shows `generation == observedGeneration` —
  "converged". A call whose work **fails** does not reach this write, so the record keeps
  `generation > observedGeneration` — a failed reconcile is now representable, which ADR-0090's post-success-only
  stamp could not express.

New pure helpers in `delonix-model::versioning` carry this, so the two-phase rule also lives in exactly one
place: `desired_annotations(prev, new_last_applied, api_version)` (bumps `generation`, **preserves** the prior
`observedGeneration`, writes `storedVersion` + `last-applied`) and `observed_generation_entry(prev)` (reads the
current `generation` off the record, returns the `observed-generation` entry to set it equal). The existing
single-phase `version_annotations` is untouched, so the CLI path and ADR-0090's happy-path invariant do not
change.

### 3. Surface the three fields as typed contract metadata (additive)

Add to `ResourceMeta` (common.proto), additively — ADR-0090 §Consequences already earmarked this carry:

```proto
  // ADR-0090/0099 record metadata. Absent (0 / "") means "unversioned" — a defined
  // state, not an error. generation counts distinct applied specs; observed_generation
  // is the generation the node last reconciled to (observed < generation ⇒ converging).
  int64  generation = 9;
  int64  observed_generation = 10;
  string stored_version = 11;
```

A shared node-api helper reads the three annotations off any record and fills the typed fields for **every**
Kind's `ResourceMeta` builder, so reads are uniform and **absent-safe** everywhere: a Kind whose write path does
not yet stamp (or a record predating this slice) reports `generation 0` — exactly ADR-0090's "unversioned"
default, never an error. The raw `annotations` map (field 5) keeps carrying the same `delonix.io/*` keys; the
typed fields are the clean surface the control plane branches on without string-parsing.

These stay **record metadata, never a user knob** (ADR-0090): a `generation` in an input spec is ignored, so a
`get`→`apply` round-trip is clean.

### 4. Materialization scope of this slice

- **Write side (stamping):** `Container` (create **and** field-masked update) is materialized and proven live —
  it is the resource A4's acceptance names. `Volume` follows on the same `delonix-model::versioning` helper
  (its store already takes an annotation delta). Remaining Kinds are **absent-safe** until their write path
  adopts the helper — they report `generation 0`, no error.
- **Read side (surfacing):** done uniformly for all Kinds via the shared meta helper, so any record that *has*
  the annotations (including ones the CLI `stack apply` stamped) shows them over the node contract immediately.

### Explicitly deferred

- **Cross-writer fingerprint unification.** The CLI canonicalizes a YAML manifest; the node API canonicalizes a
  proto spec. Their `last-applied` encodings need not be byte-identical, so a record written by one path and then
  applied by the other may bump `generation` as a "change". A4's use case is the control plane writing through the
  node API; a shared canonical form across both writers is a later refinement, not this cut.
- **A `generation`-driven watch** (vs today's timestamp watch) is **A5**, out of scope here.
- `storedVersion` stays informational until catalog v1 (ADR-0086) ships a `manifest convert`, as ADR-0090 said.

## Consequences

- The control plane reads convergence from one `Get` (or `Watch`): `meta.generation` vs
  `meta.observed_generation`. Equal ⇒ realized; observed behind ⇒ converging or (if the op failed) stuck at the
  last success. No new RPC, no polling verb — it rides the existing metadata.
- **Daemonless preserved.** Both writes happen inside the single mutating call; nothing adds a process or a watch.
- **ADR-0090's CLI path is unchanged** — the move is behaviour-preserving (re-exported), and the new two-phase
  helpers are additional, used only by the node-API path. The CLI keeps stamping `observed == generation`
  post-success.
- **Layering respected:** the rule lives in `delonix-model`, reached by both the context (`delonix-stack`) and the
  interface (`delonix-node-api`) without an interface→context edge.
- **Additive, reversible per part:** the proto fields are new (a consumer that ignored them keeps working); the
  stamping is absent-safe; a revert drops fields and keys nothing else yet depends on.
- **Audit (ADR-0098) unaffected:** stamping writes record annotations, not an authz decision; no new audited surface.
