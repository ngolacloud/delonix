# ADR-0088: The declarative foundation — schema-driven diff, a common status, a dependency graph, and lifecycle controls

- **Status:** Accepted (2026-11-03, by the owner's catalog brief) — decided here; built in
  campaign phase 2, slice by slice, each slice behaviour-preserving with a parity test before
  the old mechanism is removed. Supersedes the "no drift detection" decision.
- **Date:** 2026-11-03
- **Deciders:** Walter Angolar
- **Relates to / supersedes:** ADR-0007 (generated schema — extended with per-field update
  classes), ADR-0012 (reboot convergence class), ADR-0019 (revision history), ADR-0069 D1
  (strict manifests). **Supersedes** the recorded decision that this engine does not do drift
  detection (`AGENTS.md` "IaC nativo", lines ~1246/1691: «Não faz drift-detection — isso é
  trabalho de um orchestrator com controllers, deliberadamente»). Built over ADR-0086 (catalog)
  and ADR-0087 (node contract).

## Context

Measured at `64a5953a` (re-measurement `docs/discovery/70_ENGINE_CATALOG_V1_REMEASUREMENT.md`):
the convergence engine is strong — a pure three-way diff against `delonix.io/last-applied`
(`reconcile.rs:605`), ownership by label, `Create/Adopt/Update/Reboot/Replace/Delete/Conflict`,
`--detailed-exitcode`, revisions (ADR-0019), and **no separate state file**. It is open exactly
in the "k8s-status / Crossplane" pieces this phase adds:

- **A1.** Field update-class is a **hand-written table per Kind** (`hot_fields`
  `reconcile.rs:277-437`, `grow_only_fields:451`); the schema (`docs/schema`) carries **no
  machine-readable class** (the words "live/restart/replace" are prose in `description`). A
  field in no table silently classifies Cold (`_ => &[]`, `:436`).
- **A2.** There is **no `status` block** — no `generation`, `observedGeneration`, `conditions`
  (Ready/Progressing/Degraded), `outputs` or `storedVersion`. `Condition` (`condition.rs:5-11`)
  is a pre-requisite check, not a persisted status, and `stack wait` reads a presence marker,
  not a Ready condition.
- **A3.** The two scopes already exist (`enum Namespaced`, `kinds.rs:236`); the catalog remap is
  ADR-0086's and the VM-name scoping is gated by ADR-0090 state versioning. Nothing for phase 2.
- **A4.** Apply order is the **fixed order of the FACTS table rows** (`kinds.rs:275-304`,
  `run_layers` `stack.rs:2417`), one Kind at a time; there is no `dependsOn`, no toposort, no
  cycle detection (`validate_graph` checks existence only).
- **A5.** Three-way diff and `plan -o json` are done; `protect`, `ignore-fields`, `drift` and a
  per-stack lock are absent, and `apply` consumes a manifest + `--plan-digest`, not a plan file.

**The binding constraint of this phase (from the brief):** every decision below ships with
**zero behaviour change to the current Kinds** — all tests and `e2e.sh` pass unchanged. The
mechanism is *how*, not *what*; the observable plan/apply/wait for today's manifests is
identical.

## Decisions

**D1. The field update-class lives in the schema, and the diff is schema-driven (A1).** Every
published-schema field carries `x-delonix-update` ∈ {`live`, `restart`, `replace`,
`immutable`}, emitted by the generator (ADR-0007, `schemars`), plus `x-delonix-grow-only: true`
where a `live` field may only grow (the current `grow_only_fields`). The classes map to the
existing convergence classes: `live`→hot (converge in place), `restart`→reboot (ADR-0012 stop/
start), `replace`→cold (destroy+recreate), `immutable`→the change is refused by name on `plan`.
A single generic diff walks the schema and reads the class; each Kind keeps only two jobs —
read actual state, apply one change. **Two gates** (extend `arch_fitness`/schema check): a field
with no class fails, and a `live`/`restart` field with no registered applier fails. **Parity
before removal:** a test asserts the schema-derived class equals today's `hot_fields`/
`grow_only_fields` for every current Kind; the hand tables are deleted only after it is green.

**D2. One common `metadata`/`status`, and Ready means the same everywhere (A2).**
`metadata.generation` is written by the engine and bumped when the spec changes (not on a
status-only write). `status` carries `observedGeneration`, `conditions` (`Ready`, `Progressing`,
`Degraded`, each with a stable `reason`, `message`, `lastTransitionTime`), `outputs` and
`storedVersion`. **`Ready` is computed to mean exactly what `is_pending` means today** for each
Kind's observability, so `stack wait` — which now reads only `Ready` — behaves identically for
current manifests; the four-mode presence becomes the internal computation of `Ready`, not a
user-visible knob. A top-level `status:` in an input manifest is accepted and ignored (so
`get -o yaml | apply` round-trips). References are `<x>Ref` (`name`, plus `namespace` when in
another namespace); sets are `selector.matchLabels`. These last two are conventions the catalog
(ADR-0086) applies as it rewrites specs; phase 2 adds the `status` plumbing and the Ready
semantics without renaming any current field.

**D3. Apply order is a dependency graph, not a fixed table (A4).** References (`*Ref`,
selectors, `outputRef`, `dependsOn`) form a DAG; `apply` runs it by levels, in parallel within a
level; `destroy` runs it reversed; a cycle is refused on `plan` with the path that forms it. The
fixed FACTS order is removed. **Parity before removal:** for a manifest with no explicit
dependencies (every shipped example today), the derived topological order must be compatible
with the current FACTS order — a test asserts the graph order equals the FACTS order for the
shipped examples, and parallelism within a level is proven not to change any outcome, before the
hardcoded `run_layers` sequence is deleted.

**D4. Lifecycle controls, including drift (A5).** `stack plan -o plano.json` + `stack apply
plano.json` (apply gains a plan-file input beside the manifest + `--plan-digest`). The
annotation `delonix.io/protect: "true"` makes `destroy` and `--prune` refuse that resource by
name (DX-coded, fail-closed). `delonix.io/ignore-fields` drops named fields from the diff (for a
field another actor owns, e.g. `spec.replicas` under an external scaler). A per-stack lock
serialises concurrent applies of the same stack. **`stack drift`** reports or corrects per a
per-stack `driftPolicy` (`report`|`correct`), run by the `GitOpsSource` timer — **this
supersedes** the recorded "no drift detection" decision: the brief makes drift a first-class,
opt-in, still-daemonless capability (the timer is systemd's, ADR-0021, not a resident process).
All of these are **absent-safe**: a manifest without the annotation, without a plan file, and a
stack without a `driftPolicy` behave exactly as today — which is how the zero-behaviour-change
constraint holds while the capability is added.

## Consequences

- The per-Kind `hot_fields`/`grow_only_fields` tables and the hardcoded `run_layers` order are
  removed **only after** their parity tests are green; until then both coexist and the test is
  the gate. A slice that cannot prove parity does not land.
- `drift` reverses a written decision; the AGENTS.md section and ADR-0069's note are updated to
  point here, and the reason the earlier decision gave (an orchestrator's job) is answered: the
  engine does it daemonlessly, opt-in per stack, over the existing GitOps timer, which is the
  design the earlier note said it lacked.
- `status` is additive; a consumer that ignored status keeps working, and the node contract
  (ADR-0087) carries the same `status` over `ResourceService.Get/Watch`.
- Rollback is per slice: each is a separate commit with its parity test, revertible without the
  others.

## Implementation slices (phase 2, each a tested commit)

1. **2a — A5 absent-safe controls:** thread manifest/record annotations into the decision;
   `delonix.io/protect` (guard `destroy`/`prune`), `delonix.io/ignore-fields` (filter
   `diff_fields`), per-stack lock. Insertion: `Desired` gains `ignore_fields`/`protect`;
   `reconcile::diff_fields` skips ignored keys; `stack.rs` `destroy_one`/`prune` refuse a
   protected resource; a `FileLock` wraps the apply entry.
2. **2b — A1 schema-driven diff:** `x-delonix-update`/`x-delonix-grow-only` in the generator;
   `reconcile` reads the class; parity test vs `hot_fields`/`grow_only_fields`; the two gates.
3. **2c — A2 status + Ready:** `status` on `ManifestDoc` (accepted+ignored on input),
   `metadata.generation`, computed `Ready`, `stack wait` reads Ready; wait-parity test.
4. **2d — A4 dependency graph:** build the DAG from references; level apply/parallel/reverse
   destroy; cycle refusal with path; order-parity test vs FACTS for the shipped examples.
5. **2e — A5 drift + plan-file:** `stack apply plano.json`; `stack drift` with per-stack
   `driftPolicy`, driven by the `GitOpsSource` timer.

## Addendum (2026-11-03): how A1 is realised, measured against the code

D1 said "every published-schema field carries `x-delonix-update`" and "a generic diff walks
the schema". Measuring the reconciler (`docs/discovery/70_…`) showed this does not fit the
engine as built, and the owner chose the fit over the letter:

- The reconciler does **not** diff schema fields. It diffs a **flat, curated set of comparison
  keys per Kind** (`reconcile::hot_fields`/`grow_only_fields`, the keys of `Desired.fields`),
  which are flattened/renamed versions of the nested, camelCase schema fields
  (`memory`/`cpus` live under `resources`, `netBps` under a `net` block). The two namespaces do
  not align 1:1, so "annotate every schema field and walk the schema" would mean realigning the
  diff's whole key space — a change that cannot hold the phase's zero-behaviour-change invariant.
- **Decision (owner, 2026-11-03):** A1's class is a **declarative per-key classifier**,
  `reconcile::update_class(kind, field) -> UpdateClass {Live, GrowOnly, Replace}`, the ONE place
  the diff reads a field's static class (`is_hot`/`is_hot_change` route through it). The RESTART
  class stays dynamic in `Facts` (a backend decides it per resource), so it is not a static
  per-field class. `x-delonix-update` is still surfaced in the published schema for the
  top-level spec fields whose name matches a comparison key (phase 2b-ii), and the two gates
  (every reconciled field classified; a `live`/`restart` field has an applier) check the
  classifier and the per-Kind `RECONCILED_*_FIELDS` invariant — not the raw JSON schema.
- **Slice split:** 2b-i (this addendum) unifies the classifier with a parity test proving
  `update_class` reproduces the hand tables for every Kind, and asserts Live/GrowOnly are
  disjoint. 2b-ii adds the two gates and the schema `x-delonix-update` emission. The hand tables
  stay as `update_class`'s documented backing data (each entry carries its reasoning); they are
  no longer read directly by the diff.
