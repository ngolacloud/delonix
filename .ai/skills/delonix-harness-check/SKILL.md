# SKILL.md — Delonix Harness Check

## Skill Name

`delonix-harness-check`

## Purpose

Verify the **harness itself** — the gates, baselines, agent/skill specs and their
contracts — not the engine. It reuses the repository's existing `Makefile`,
`scripts/` gates and `.github/workflows/` rather than inventing a parallel check,
and it states which checks CI verifies locally versus which depend on GitHub
configuration (`docs/dev/harness-governance.md`).

Route here when a task is about the harness: a gate, a baseline, an agent or skill
spec, the permission/authorization contracts, or the context budget. For engine
validation route to `delonix-runtime-e2e-quality` (see its `ROUTER.md`).

**Maximum tier: A→B (rootless).** Everything below runs without root.

## What to run (reuse, do not reinvent)

1. **The CI gate suite**, exactly as CI runs it:
   - `make ci` — `fmt-check` + `gates` + `lint` + `test` (needs the toolchain).
   - `make gates` alone — the Python fitness gates with no build (fast).
   - `python3 scripts/test_*.py` — the `script-tests` job (every gate's unit test).
2. **The harness contracts added by the governance campaign** (pure, no build):
   - `python3 scripts/authorization_gate.py check-fixtures scripts/authorization_fixtures`
   - `python3 scripts/execution_tier_gate.py check-fixtures scripts/execution_tier_fixtures`
   - `python3 scripts/agents_budget_gate.py`  (context budget + spec discovery)
   - `python3 scripts/harness_control_gate.py check-fixtures scripts/harness_control_fixtures`
   - `python3 scripts/skill_router_gate.py`  (skills present, router covers domains)
   - `python3 scripts/prompt_lint_gate.py check-fixtures scripts/prompt_lint_fixtures`
   - `python3 scripts/evidence_gate.py check-fixtures scripts/evidence_fixtures`  (+ `render`/`promote` for a run's evidence)
3. **Index/doc integrity**:
   - `python3 scripts/agents_toc.py --check` — the AGENTS.md index is not stale.
   - `python3 scripts/lang_ratchet.py` — the language ratchet holds.

## How to report

Report each check as a structured line: SHA, the exact command, its exit code,
and `PASS`/`FAIL`/`SKIP`/`BLOCKED` — never "green on intent". A check that could
not run (missing toolchain, needs a client the executor lacks) is `SKIP`/`BLOCKED`
with the reason, not a silent pass.

## CI-local vs GitHub (do not conflate)

- **Verified locally** (reproducible by `make ci`): the gate suite and every
  `scripts/test_*.py` fail on a broken invariant. This is deterministic and in-repo.
- **Depends on GitHub configuration** (cannot be proven by `make ci`): that a given
  gate is a **required status check**, that branch protection blocks a direct push,
  that a CODEOWNERS review is enforced. The harness refuses earlier; it cannot grant
  what only GitHub enforces. With a single maintainer, independent review is a
  recorded limit, not an imposed control (`docs/dev/harness-governance.md`).

## Golden rule

A harness check that did not run is not a pass. Cite the command and its real exit
code; declare `BLOCKED` for anything the executor or client cannot impose, and
route a dependency on another repo (e.g. the control-plane) as external.
