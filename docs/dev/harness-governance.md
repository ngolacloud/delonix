# Harness governance — changing a control needs a reason and a reviewer

The gates are the repository's memory of its own invariants. A baseline that
quietly loosens, a tolerance that quietly widens, a denominator that quietly
shrinks, a workflow that quietly drops a check — each one lowers the floor
without anyone deciding to, and the next regression walks straight through the
gap the gate was there to close. This page is the single source of truth for
**changing a harness control**: when it needs a record, what the record must
contain, and who reviews it. The `delonix-gatekeeper` agent owns the diagnosis;
`scripts/harness_control_gate.py` makes the rule checkable.

## What a "harness control" is

- **Baselines / ratchets** — `scripts/*_baseline.json` (arch, folhas, cli_exec,
  capability, lang, bench, tmp_roots) and the `*_ratchet.py` logic that reads them.
- **Tolerances / denominators** — a gate's threshold or divisor, e.g.
  `bench_gate.py` `--tolerance` / `DEFAULT_TOLERANCE`, a coverage denominator
  (`capability.applicable`, `cli_exec.leaves`).
- **Workflows and gate rules** — `.github/workflows/*.yml` and `scripts/*_gate.py`
  themselves: the code that decides whether the gate fails.

## Tighten is free; loosen needs a record

A change to a control is one of three, and only the direction — not the file —
decides what it costs:

- **TIGHTEN** (floor rises, debt count falls, coverage rises, tolerance narrows):
  land it; the ratchet is meant to move this way.
- **LOOSEN** (floor falls, debt count rises, coverage falls, tolerance widens,
  a check is removed or weakened): it needs a **governance record** (below).
- **OPAQUE** (a change to workflow/gate code whose direction cannot be inferred
  mechanically): treated as LOOSEN — fail closed — until a record says otherwise.

Direction is **not uniform across metrics**: for a debt count
(`arch_baseline.library_prints`, `lang_baseline.*`) *lower* is tighter; for a
coverage count (`capability.proved`, `cli_exec.executed`) *higher* is tighter;
for a tolerance *lower* is tighter. The direction per control lives in
`scripts/harness_control_gate.py` (`CONTROL_DIRECTION`); an unlisted key is OPAQUE.

## The governance record

A LOOSEN/OPAQUE change carries, in the commit body (or a linked
`reports/harness-control-changes/` note), four fields:

1. **Diff** — the exact control and the before→after value.
2. **Reason** — why the floor must move.
3. **Impacto / impact** — the measurable effect (what is now permitted that was
   not; how many sites; which suite's denominator changed and by how much).
4. **Revisão / review** — evidence of review by someone **other than the
   implementer**, *where applicable* (see the single-maintainer limit below).

`scripts/harness_control_gate.py` fails a LOOSEN/OPAQUE change whose record is
absent or missing a field.

## What CI verifies locally vs what depends on GitHub

Keep the two apart, because only one side is in this repository:

- **Verified locally by CI** (`.github/workflows/ci.yml`, reproducible by `make ci`):
  the gate suite runs and fails on a broken invariant; `harness_control_gate.py`
  runs in `script-tests` and fails a loosening without a record. This is in-repo
  and deterministic.
- **Depends on GitHub configuration** (not in this repository, cannot be proven by
  `make ci`): that the loosening check is a **required status check**, that the PR
  cannot merge without it, that a **CODEOWNERS review** is enforced, and branch
  protection forbids a direct push. The harness can *refuse earlier*; it cannot
  *grant* what only GitHub enforces. State which side holds a given rule.

## The single-maintainer limit (verifiable)

`CODEOWNERS` is `* @angolardevops` — **one maintainer today**. «Review by someone
other than the implementer» therefore cannot be *technically imposed*: there is no
second reviewer to require. The verifiable limit, stated plainly:

- The governance **record is still mandatory** and lands in history — it is the
  audit trail a later reviewer (or a second maintainer) reads.
- The **independent-review requirement activates** the moment a second maintainer
  appears in `CODEOWNERS` and branch protection requires their review — no code
  change here is needed, only the GitHub configuration.
- Until then, the `review` field records self-review or a deferred-review note,
  and that limitation is recorded, not hidden as if independent review happened.
