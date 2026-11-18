# Prompt template — the shape an active prompt must have

A campaign prompt tells a fresh session what to do and what «done» means. Without a
fixed shape, prompts drift: no base SHA (so «it worked» is unverifiable), a dangling
ADR reference, or a conclusion copied from a months-old report that no longer holds.
This template is the shape; `scripts/prompt_lint_gate.py` enforces it statically.

**Historical prompts are evidence — do not rewrite them.** Past `reports/*/PROMPTS.md`
stay as they were, the record of what was asked and when. Only **active** prompts
(the ones about to run) use the fields below.

## The fields (all required except Issue/ADR)

Write them as `- **Field:** value` lines, in a prompt's header:

- **ID:** stable identifier, e.g. `DX-E2E-012` (unique within its backlog).
- **Base/SHA:** the branch and commit the prompt is written against, e.g.
  `develop @ e923d617`. A bare `develop`/`main` is accepted; a SHA is better.
- **Objective:** one sentence — the single outcome.
- **Issue/ADR:** the issue and/or `ADR-NNNN` it implements, or `none`. A named
  `ADR-NNNN` must resolve to `docs/adr/NNNN-*.md`.
- **Authorized artefacts:** the exact files/dirs the task may edit (repo-relative).
  Each must exist (a glob or directory is fine).
- **Dependencies:** other fronts/contracts it needs integrated first, or `none`.
  An external dependency (another repo, e.g. the control-plane) is named as external.
- **Lab profile:** the environment and tier — `rootless (Tier B)` or
  `rootful-VM (Tier D)` (`.ai/agents/permission-tiers.md`).
- **Tests:** the regression test that fails without the change and passes with it.
- **Evidence validity:** `fresh` for evidence produced against this Base/SHA, or
  `historical (<date>)` when it leans on an older report — in which case it MUST
  add `reconfirm against code and a fresh run` before any change is generated.
- **Expected output:** the deliverable and its format (diff, report, PASS/FAIL table).

## The staleness guard

A prompt whose **Evidence validity** is `historical` without an explicit
reconfirmation instruction fails the lint: a conclusion from an old `report.md` is a
hypothesis until re-checked against current code and a recent run. The lint also
fails a **dangling reference** — an `ADR-NNNN` with no matching file, or an
authorized artefact that does not exist.

## Example (passes the lint)

```
- **ID:** DX-HARNESS-001
- **Base/SHA:** develop @ e923d617
- **Objective:** add a benign fixture proving tier-A refuses a remote effect.
- **Issue/ADR:** ADR-0040
- **Authorized artefacts:** scripts/execution_tier_fixtures/, scripts/test_execution_tier_gate.py
- **Dependencies:** none
- **Lab profile:** rootless (Tier B)
- **Tests:** scripts/test_execution_tier_gate.py (new case red without the fixture)
- **Evidence validity:** fresh
- **Expected output:** diff + rc=0 of the test battery
```
