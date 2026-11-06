# SKILL.md — Delonix Bug Triage

## Skill Name

`delonix-bug-triage`

## Purpose

Close the loop the harness was missing: the `delonix-runtime-e2e-quality` skill
PRODUCES backlogs of findings, and `fila` QUEUES ready-to-run work, but nothing
defined grooms the former into the latter. This skill is that grooming step —
it turns raw E2E/security/perf findings into a deduplicated, severity-ordered,
actionable queue, so a finding becomes a tracked session instead of a line in a
report nobody re-reads.

## Domain

Triage of findings for the container/microVM engine: bugs, capability gaps,
security findings, performance regressions — their severity, their dedup against
what is already tracked, and their routing into the engineering workflow.

## Trigger conditions

Invoke after an E2E validation pass has written backlogs, when asked "what
should we work on next / what is open", or when a batch of findings (from a
review agent, an issue sweep, a measured parity gap) needs to become ordered,
non-duplicated work.

## Permission posture

Tier A–B (see `.ai/agents/permission-tiers.md`): it reads findings and the
workflow state, and at most proposes/records queue entries. It does not fix the
bugs, start worktrees, push, or mutate the engine.

## Inputs (the real sources)

- E2E skill output under `reports/e2e/<version>/`: `BUG_BACKLOG.md`,
  `GAP_BACKLOG.md`, `SECURITY_BACKLOG.md`, `PERFORMANCE_BACKLOG.md`, and the
  master `BACKLOG.md`, with deterministic finding IDs and severities P0–P4
  (see `.ai/skills/delonix-runtime-e2e-quality/` and its `templates/backlog.md`
  / `templates/finding.md`). Security findings follow `references/security-checks.md`.
- Open GitHub issues and measured Docker/Podman parity gaps (the same sources
  the user-level `fila` skill draws on).
- Already-tracked work: `.claude/gitflow/registry.json` (per-task base/state/
  worktree) and open session contracts in `.claude/sessions/INDEX.md`.

## Method

1. **Collect** — gather findings from the backlogs and any issue/parity sweep.
2. **Dedup** — drop findings already tracked (match against open `registry.json`
   tasks, open session contracts, and existing issues). A finding already in
   flight is not re-queued.
3. **Classify & order** — keep the e2e classification (BUG/GAP/SEC/PERFORMANCE)
   and severity (P0–P4); order P0→P4. A SEC finding defers to the
   `delonix-security-review` severity model, not a re-scored one.
4. **Route** — express each surviving finding as a ready-to-run session prompt
   with a base branch decided by GitFlow rules (`fix/…` from `main`, everything
   else from `develop`), respecting the WIP cap `fila` enforces. Do NOT open the
   worktree or branch here — that is the next session's `/escopo`.
5. **Record** — hand the ordered queue to `fila` / note it so it is picked up;
   never silently drop a finding (if the queue is capped, say what was deferred).

## Expected output

A deduplicated, severity-ordered queue: per item — finding ID, class, severity,
one-line objective, the GitFlow base branch, and whether it is ready or blocked
(and on what). An explicit note of anything deferred by the WIP cap — no silent
truncation.

## Validation strategy

Dedup is checked against the ACTUAL tracked state (`registry.json`, session
INDEX, issues), not memory — a finding re-queued every pass is the failure mode.
Severity and class are carried from the source finding, not re-invented. The
queue is only "ready" for items whose objective is observable (a criterion the
next session can tick), not vague intentions.
