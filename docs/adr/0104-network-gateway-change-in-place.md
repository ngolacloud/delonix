# ADR-0104: A `NetworkGateway` changes in place — a rule or alias dropped or edited is retracted or updated on its own, under the same owner mark

- **Status:** Proposed (2026-11-27) — design only, nothing implemented; awaiting the owner's
  decision.
- **Date:** 2026-11-27
- **Deciders:** Walter Angolar (owner)
- **Relates to:** ADR-0051 (the OPNsense `GatewayProvider`; Phase 3 chose remove-then-reapply
  for a spec change), ADR-0059 D4 (validate → plan → apply → observe → verify, steps with
  compensations), ADR-0072 (the owner token is per declaring record and never rewritten),
  ADR-0088 A5 (plan digests), ADR-0105 (rollback and dataplane verification, which this one
  makes safe to rely on).

## Context

An external client that drives a perimeter firewall through `delonix.node.v1` (`ApplyStack` of
a `kind: NetworkGateway`, reaching an OPNsense appliance) reported, on 2026-11-27, that it can
only ever **add** rules. Measured in the code:

- **`apply` only ensures.** `bins/delonix/src/cmd/network_gateway.rs:19-27` says it in so
  many words: `apply()` ensures every alias and rule currently declared and «does not retract
  one dropped from the list while others stay». The record grows by union
  (`network_gateway.rs:950-953`, `union_by`), so a rule dropped from the manifest stays in the
  record and stays on the appliance.
- **An edit is refused, not applied.** `GatewayProvider::ensure_alias`/`ensure_rule`
  (`crates/contexts/delonix-networking/src/domain/gateway.rs:279-300`) refuse an owned object
  whose content differs (`RemoteObjectDrifted`). The OPNsense client never calls `setRule`/
  `set_item` (`crates/providers/delonix-provider-opnsense/src/lib.rs:50-57`), and the catalog
  row says so: `NetGatewayUpdateInPlace => NotImplemented`
  (`crates/providers/delonix-provider-opnsense/src/capabilities.rs:40`).
- **The only retraction is the whole document.** `--replace NetworkGateway/<name>` or
  dropping the document under `stack apply --prune` runs `remove_for_replace`
  (`network_gateway.rs:1124`): every rule, then every alias, then **a commit**
  (`network_gateway.rs:1236`), then `release_owner` (`:1246`) and the record is deleted
  (`:1249`). The re-apply then generates a **new** owner token (`owner_mark`, `:831-837`),
  stages everything again and **commits a second time** (`:1029`).

On a production perimeter that is two activations with nothing of the document between them:
the pass rules are gone (an outage for whatever they admitted) and the block rules are gone
(exposure for whatever they refused). The window lasts as long as the re-apply takes, and a
failure in the re-apply leaves the appliance with none of the document's rules. A client that
respects the perimeter therefore refuses every plan whose gateway change is a `REPLACE` or a
`DELETE`, and can never retract a rule it once declared.

The building blocks for something better already exist:

- the port can remove one object by identity, only if owned: `remove_alias`/`remove_rule`
  (`gateway.rs:292`, `:304-308`) — today called only by the teardown;
- OPNsense stages every write and activates them in one `firewall/filter/apply`
  (ADR-0051 Phase 0; `GatewayProvider::commit`, `gateway.rs:342-354`), so a removal and an
  ensure staged together become active in the same pf reload;
- the appliance's API has `setRule` and `moveRuleBefore` on the measured 26.1.2 image
  (`docs/discovery/64_NET_PROVIDER_CONTRATO_SPIKE.md:160-162`); ADR-0051 refused to *guess*
  `set_rule`'s request shape, not to use it once measured.

Guardrails touched: **no silent failure** (#6) — an object the engine cannot retract must be
named, never left behind quietly; **ownership** (ADR-0072) — the token must survive the
change. No daemon, no tenancy, no new dependency, no new privilege boundary (all of this is
calls to an appliance's API that the engine already makes).

## Decision

**D1. `apply` retracts what the document no longer declares, in the same transaction.** The
record keeps the *last applied* sets (not a union). On apply, the difference between the record
and the new spec is computed per object identity (alias name, rule description, policy name,
NAT description). Each dropped object becomes a `remove_rule`/`remove_alias` step, staged in
the **same** transaction as the ensures, with the measured order: ensure aliases → ensure or
update rules → remove dropped rules → remove dropped aliases (a rule must never outlive the
alias it names, ADR-0051). One `commit` activates all of it. The step ledger already written
ahead (`network_gateway.rs:900-911`) records each removal before it runs, so an interrupted
run is resumed, not repeated.

**D2. An edited object is updated where it is, behind a measured spike.** A rule or alias
whose content changed is updated with the provider's own update call (`setRule`/`setItem` on
OPNsense), keeping its uuid and its owner category, and reported as `UPDATE` in the plan.
Before the code, a spike against the 26.1.2 image measures the exact request shape, that the
owner category survives a `setRule`, and that a `setItem` on an alias in use by a rule is
accepted. Until that spike is GO, an edited object is planned as **retract + create in the same
commit** (D1's transaction) — no window, at the cost of a new uuid — and the catalog row stays
`not-implemented`. Only a GO flips `net.gateway.update-in-place` to `supported`, with the live
test as evidence.

**D3. The owner mark is never regenerated by a change.** Retraction and update operate under
the record's existing `OwnerMark`. `release_owner` happens only when the document itself is
removed (`--prune`/`destroy`/`delete`), never on a change. An object found without the mark is
left alone and named, as today.

**D4. The plan says what will move.** `stack plan` (and `PlanStack`) shows a gateway change as
`UPDATE` with the objects that will be retracted, created and updated, and the plan digest
(ADR-0059 D4, ADR-0088 A5) covers them. A `REPLACE` of a `NetworkGateway` remains only for what
truly needs it (moving the document to another provider, `network_gateway.rs:926-930`).

**D5. A provider without the capability refuses, it does not degrade.** A gateway provider
whose report lacks `net.gateway.update-in-place` answers an edit with `unsupported_capability`
before any effect, and the retract-in-the-same-commit path is offered only where the provider
has `net.apply.staged`. Never a silent fallback to a two-commit replace.

## Alternatives considered

- **Do nothing; keep `--replace`.** Rejected: it is the outage/exposure window above, on the
  one Kind whose whole purpose is the perimeter. Clients either accept the window or stop
  retracting rules, and rules that nobody can retract accumulate on production firewalls.
- **Let the client delete objects on the appliance directly.** Rejected: it bypasses the owner
  mark and the record, the next plan reports drift, and every client reimplements ownership.
- **Retract with a second commit right after the apply.** Rejected: still two activations, and
  a failure between them leaves a rule active that the operator removed.
- **Implement `setRule` now, without the spike.** Rejected for the same reason ADR-0051
  refused it: guessing an appliance's request shape is the trap. D2's interim path gives the
  same safety (one commit) without guessing.

## Consequences

- A rule or alias can be removed or changed on a live perimeter without a window; a client no
  longer has to refuse every gateway `REPLACE`/`DELETE` to stay safe.
- The record changes meaning from «everything ever declared» to «what was last applied»; a
  record written by an older engine is migrated by treating its union as the last-applied set
  (its extra objects are exactly the ones the next apply will retract — the plan shows them
  first).
- D2's interim path changes the uuid of an edited rule; anything outside the engine that keyed
  on the uuid (appliance logs, dashboards) sees a new rule. Named, not hidden.
- More remote calls per apply (a removal per dropped object); measured cost is one API call
  each, inside the existing staged transaction.
- A safe change still depends on the rules being right: an edit that cuts traffic is activated
  as declared. Recovering from that is ADR-0105, not this one.
