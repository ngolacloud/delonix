# ADR-0105: A gateway apply can be confirmed or reverted by the appliance itself, and is verified against traffic, not only against the loaded ruleset

- **Status:** Proposed (2026-11-27) — design only, nothing implemented; awaiting the owner's
  decision. Phase 1 is gated on spike S1 (below).
- **Date:** 2026-11-27
- **Deciders:** Walter Angolar (owner)
- **Relates to:** ADR-0059 D4 (apply: «staged and not activated → the provider's rollback»;
  verify: «a traffic probe only when `net.verify.dataplane` is supported», otherwise
  `verified: control-plane-only`), ADR-0051 (OPNsense provider, Phase 0 measured the staged
  apply), ADR-0019 (stack revision history — a record, never a source of truth), ADR-0104
  (change in place, which a revert relies on), ADR-0050 (the capability catalog).

## Context

The same external client as ADR-0104 drives a production perimeter through `ApplyStack` of a
`kind: NetworkGateway`. Its first version must be able to undo a rule set that, once active,
cuts traffic — including the client's own management path to the node or to the appliance — and
to know that an apply actually filters as declared. Measured in the code:

- **Neither capability is implemented on OPNsense.** `NetApplyRollback => NotImplemented` and
  `NetVerifyDataplane => NotImplemented`
  (`crates/providers/delonix-provider-opnsense/src/capabilities.rs:60`, `:64`).
- **The catalog's `net.apply.rollback` is a pre-activation discard.** Its definition is
  «Staged changes are discarded before activation when a step fails»
  (`crates/foundation/delonix-model/src/provider/capability.rs:304-305`). It does not cover the
  case that matters here: a document that applies **successfully** and is wrong.
- **What the provider does today on failure.** `Client::commit`
  (`crates/providers/delonix-provider-opnsense/src/lib.rs:923-932`) deletes again what it
  created when a foreign pending change appears before activation; «what it DELETED cannot be
  restored and is named in the error». Activation is a plain `POST firewall/filter/apply` with
  no revision (`lib.rs:1023-1025`).
- **The appliance has the mechanism.** OPNsense 26.1.2's `FilterBaseController` exposes
  `savepoint`, `apply(rollback_revision)` (which arms the appliance's own `rollback_timer`),
  `cancelRollback` and `revert` (`docs/discovery/64_NET_PROVIDER_CONTRATO_SPIKE.md:157-160`),
  recorded as `net.apply.rollback` «partial — never exercised» (`:177`).
- **What verification exists is ruleset-level.** After activation the commit proves that
  nothing of the engine's is still pending by comparing the configured rules with the labels pf
  has loaded (`diagnostics/firewall/pf_statistics/rules`) and the configured aliases with pf's
  tables (`lib.rs:762-785`, `:923-926`). That proves the rules are **loaded**; it does not prove
  they **act** on the traffic they were written for (a rule shadowed by an earlier one, an
  alias resolving to nothing, the wrong interface).
- **ADR-0059 D4's failure results are not produced.** The reasons `partial_apply`,
  `verification_failed` and `rollback_failed` exist in the dictionary
  (`crates/foundation/delonix-model/src/codes.rs:280-284`, `:327-329`), but nothing outside
  `codes.rs` constructs them (the Proxmox SDN path has its own `SdnRollbackFailed`,
  `crates/providers/delonix-provider-proxmox/src/error.rs:73`; the gateway path has none), and
  no apply result says `verified: control-plane-only`.

Guardrails touched: **daemonless** (#1) — an engine-side timer that reverts after N seconds
would be a resident process; **no silent failure** (#6) — an apply that reports success
without having verified must say so; **no new privilege boundary** (#5) — a traffic probe must
not need raw sockets or capabilities the node does not already hold.

## Decision

**D1. Two catalog rows, two meanings.** `net.apply.rollback` keeps its definition (discard
staged changes before activation) and is implemented on OPNsense as what the commit already
almost does: every change this value staged — creations *and* deletions — is undone when a step
fails before activation, using a `savepoint` taken before the first staged write. A new row,
**`net.apply.confirmed`** — «an activation reverts by itself unless it is confirmed within a
window» — names the post-activation case. Adding a row bumps the catalog version (ADR-0050).

**D2. A confirmed apply, with the timer on the appliance.** `stack apply --confirm-within
<duration>` (and the matching field on the node contract's apply) does, for a gateway
document: `savepoint` → stage → `apply(rollback_revision)`. The appliance's own
`rollback_timer` reverts the activation if no confirmation arrives. Confirmation is an explicit
second call (`delonix networkgateway confirm <name>`, and an RPC on the node contract) that
runs D4's verification and then `cancelRollback`. If the change cut the client's path, the
client simply cannot confirm and the appliance restores the previous ruleset. The engine keeps
**no** timer and no process between the two calls: the pending confirmation is a field in the
document's record (revision, deadline), read by `describe`/`GetOperation`, so guardrail #1
holds. A provider without `net.apply.confirmed` refuses `--confirm-within` before any effect.

**D3. Revert to the previous document is a normal apply of the previous revision.** Outside a
confirmation window, «undo» re-applies the document's previous revision (ADR-0019) through the
ordinary plan → apply path — which needs ADR-0104 to avoid a window — with its own plan and
digest. The appliance's `revert(revision)` is **not** used outside the confirmation window: it
restores a whole configuration section and would undo changes others made in between.

**D4. Verification has three named levels, and the result says which one was reached.**
- `ruleset-loaded` — today's proof (every owned rule's uuid is a loaded pf label, every owned
  alias a loaded table), now reported instead of implied.
- `probe` — the document may declare probes (`verify: [{from, to, port, protocol, expect:
  allow|deny}]`), each executed as an unprivileged TCP connect or UDP exchange from where the
  engine can run it (the node), with `expect` compared to the outcome. ICMP and raw sockets
  are out of scope (guardrail #5). Spike S2 decides whether the appliance's own diagnostics API
  can run a probe from the appliance side.
- `control-plane-only` — said out loud when neither is available (ADR-0059 D4.5).
`net.verify.dataplane` becomes `supported` only with `probe` and its live test.

**D5. The failure results exist.** `partial_apply` (an activation happened and a later step
failed, ledger attached), `verification_failed` (a probe's outcome differs from `expect`; with
`--confirm-within`, the confirmation is withheld and the appliance reverts) and
`rollback_failed` are produced by the gateway apply and surface on the node contract as the
`Operation`'s error with their `DX-` number.

**Spike S1 (before D1/D2 code), against the 26.1.2 image:** which configuration sections a
`savepoint`/`apply(rev)` covers — filter rules only, or aliases (`alias/reconfigure`) and NAT
(`source_nat`, `d_nat`) too; the timer's default and whether it is settable per call; what
`cancelRollback` answers after the timer fired; and whether a `revert` is atomic. If the
revision does not cover aliases and NAT, `net.apply.confirmed` is declared `partial` with that
reason, and a document whose change touches them is refused under `--confirm-within`, never
half-protected.

## Alternatives considered

- **Do nothing; the client keeps a fixed anti-lockout rule and observes.** Rejected as the
  engine's answer: it protects one path, not the traffic a wrong rule cuts, and every client
  re-invents it.
- **An engine-side timer (a process that waits N seconds and reverts).** Rejected: a resident
  process per apply is a daemon (guardrail #1), and it dies with the node — exactly when the
  appliance-side timer still works.
- **Use the appliance's `revert` as the general undo.** Rejected: it restores a whole section
  and would silently discard other people's changes (guardrail #6).
- **Only a readback.** Rejected as «verification»: it is what exists, and it cannot see a rule
  that is loaded but shadowed. It stays, named `ruleset-loaded`.

## Consequences

- A client can apply a perimeter change with a safety net that survives the client losing
  connectivity, and can tell «loaded» from «filters as intended».
- The record gains a pending-confirmation state; `stack plan` must refuse a new apply of the
  same document while a confirmation is pending (or it would race the appliance's timer).
- Probes are declared by the document's author; the engine verifies what it was asked to
  verify and says so, it does not invent probes.
- The OPNsense provider gets more appliance-specific code (savepoint/revision handling); a
  second gateway provider answers the rows by its own means or declares them unsupported.
- Depends on ADR-0104 for D3 to be windowless; D1, D2, D4 and D5 do not.
