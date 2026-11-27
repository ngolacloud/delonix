# ADR-0106: `ApplyStack` is bound to the plan it executes — `PlanStack` returns the plan's digests, `ApplyStack` takes them and refuses a different plan

- **Status:** Proposed (2026-11-27) — design only, nothing implemented; awaiting the owner's
  decision.
- **Date:** 2026-11-27
- **Deciders:** Walter Angolar (owner)
- **Relates to:** ADR-0088 A5 (`stack plan -o json` + `stack apply --plan`/`--plan-digest`),
  ADR-0059 D4 (apply(plan, digest): «a different digest is `stale_plan` and nothing is
  written — the ADR-0042 `If-Match` rule, applied to a plan»), ADR-0042 D2 (ETag/`If-Match`,
  412 on the REST encoding), ADR-0041 D4 and ADR-0107 (the contract's compatibility promise —
  this change is additive), ADR-0096 (the node contract reachable over TCP + mTLS).

## Context

An external client that shows an operator the plan of a stack, waits for approval, then applies
it reported on 2026-11-27 that the node contract gives no guarantee that the apply executes the
plan that was approved. Measured:

- **The contract carries no plan identity.** In `proto/delonix/node/v1/infra.proto`,
  `PlannedChange` has `target`, `action`, `fields`, `reason` (`:350-355`) and `StackPlan` has
  `changes` and `has_changes` (`:345-348`) — no digest. `ApplyStackRequest` has `stack`,
  `manifest`, `prune`, `replace`, `request_id` (`:368-374`) — no reference to a plan.
- **The engine already has the mechanism, on the CLI only.** `stack plan -o json` puts a
  `planDigest` on every change (`bins/delonix/src/cmd/stack.rs:1005`,
  `plan_with_digests` at `:2055-2068`): for `NetworkGateway`/`NetworkZone` the provider-derived
  digest over intent + observed state + provider + capabilities (ADR-0059 D4), for every other
  Kind a digest of the change itself. `stack apply --plan-digest <d>` / `--plan <file>`
  (`stack.rs:144-152`) refuses a stale plan before anything is written (`refuse_stale_plan`,
  `:2145-2204`) with `StalePlan`
  (`crates/contexts/delonix-networking/src/domain/error.rs:98`).
- **The node API drops it on both sides.** `PlanStack` re-executes `delonix stack plan -o json`
  and converts each row with `to_change`
  (`crates/interfaces/delonix-node-api/src/stacks.rs:66-86`), which never reads `planDigest`
  (and sets `reason` to an empty string). `ApplyStack` re-executes `delonix stack apply`
  (`apply_work`, `stacks.rs:180-198`) without `--plan-digest`. Any failure of the re-exec becomes
  `Error::Invalid` with the CLI's stderr (`finish_status`, `stacks.rs:243-258`), so even a
  stale-plan refusal would arrive as a `FAILED` operation, not as a precondition failure.
- **The CLI check is membership, not equality.** `refuse_stale_plan` verifies that every change
  computed *now* has its digest among those given (`stack.rs:2166`, `:2186`); a digest given for
  a change that is no longer in the plan is ignored. A client that re-sends a manifest with one
  document removed (without `--prune`) applies a strict subset of the approved plan and passes
  the check.
- **The precondition shape exists on the socket.** A stale `etag` is refused **before** an
  operation exists, as `FAILED_PRECONDITION` marked with `stale-etag` metadata that the REST
  encoding answers with 412 (`crates/interfaces/delonix-node-api/src/network_ops.rs:5-8`,
  `:31-45`).

Today a careful client re-plans immediately before applying and stops if the plan changed;
there is still a race between its second `PlanStack` and its `ApplyStack`, and nothing on the
node enforces the comparison. Guardrails touched: **no silent failure** (#6) — applying
something other than what was approved, and reporting success, is the worst form of it. No
daemon, no tenancy, no dependency, no privilege boundary.

## Decision

**D1. The plan carries its identity.** `PlannedChange` gains `string plan_digest = 5` (the
CLI's `planDigest`, verbatim) and `StackPlan` gains `string digest = 3`: SHA-256 over the
canonical, sorted list of the changes' `plan_digest` values plus the `prune` flag and the
`replace` list the plan was computed for. `PlanStackRequest` gains `repeated string replace = 4`
so the plan is computed for the same authorisations the apply will carry. `to_change` maps
`planDigest` and `reason`.

**D2. The apply names the plan, and the node compares sets, not members.**
`ApplyStackRequest` gains `string plan_digest = 6`. When it is set, the node recomputes the
plan for the request's manifest, `prune` and `replace` — the same path as `PlanStack` — and
refuses unless the recomputed `StackPlan.digest` equals it: a change added, removed or altered
since the approval is a different plan. The CLI gains the same strictness for a saved plan
(`stack apply --plan <file>` compares the whole set); `--plan-digest` keeps its documented
membership semantics for a hand-picked digest.

**D3. A different plan is a precondition failure, before any operation.** The refusal is
`FAILED_PRECONDITION` with `stale-plan` metadata (the same mechanism as `stale-etag`), mapped to
**412** on REST with an RFC 9457 problem document carrying `stale_plan`, both digests and the
targets whose change differs. No `Operation` is created, nothing is written, and the idempotency
fingerprint (`mutation_fp`, `stacks.rs:131`) includes `plan_digest` so a retry with a different
plan is not replayed as the first answer.

**D4. Optional, and additive.** An `ApplyStack` without `plan_digest` behaves exactly as today
(plan-and-apply in one call). All fields are new field numbers on existing messages: `buf
breaking` (FILE rules, `buf.yaml`) passes and existing clients are unaffected (ADR-0107).

**D5. The node computes, the CLI stays the source.** The digest is produced by the same code
that produces `planDigest` today; the node contract does not get a second implementation. While
`StackService` re-executes the CLI (ADR-0040's transitional shape), the node passes the plan
digest to the CLI and reads a distinct exit status for `stale_plan` instead of parsing stderr.

## Alternatives considered

- **Do nothing; clients re-plan and compare.** Rejected: the comparison lives in every client,
  and the race between the second plan and the apply remains.
- **Expose only the per-change digests (mirror `--plan-digest`).** Rejected as the contract's
  only mechanism: membership lets a subset of the approved plan through (measured above).
  Per-change digests are still returned (D1) so a client can show which change moved.
- **A server-side stored plan (`PlanStack` returns a plan id, `ApplyStack` executes the stored
  plan).** Rejected for now: it needs plan storage and expiry on the node, and an executed plan
  would still have to be recomputed against the machine as it is — which is what the digest
  already does, statelessly.
- **ETag/`If-Match` on a stack resource.** Rejected: a stack is not a stored resource with a
  version on the node; the plan is a function of the manifest and the observed machine, so its
  digest *is* the precondition.

## Consequences

- A client can guarantee that what an operator approved is what runs, enforced on the node, and
  tell a refused plan (412) from a failed apply (an `Operation` in `FAILED`).
- `ApplyStack` with `plan_digest` plans twice on the node (once to compare, once inside the
  apply); the cost is a plan, which is read-only.
- A residual window remains between the node's recomputation and its first write; the gateway
  path narrows it with its own pre-checks (`check_no_foreign_pending`) inside the transaction,
  other Kinds do not. Named, not hidden.
- The CLI's `--plan` becomes stricter (set equality); a saved plan that was relied on as a
  partial filter now refuses. This is a behaviour change documented under `BREAKING` in the
  CHANGELOG under the 0.x policy (`VERSIONING.md`).
