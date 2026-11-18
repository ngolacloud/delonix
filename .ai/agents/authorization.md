# Authorization — the collaboration-authority contract

Shared reference for every agent in `.ai/agents/` and for every session skill.
It is the **single source of truth** for who may perform the acts that leave the
task worktree and reach the shared repository or the outside world: pushing,
opening or operating a pull request, merging, and releasing.

Two companion contracts sit beside it, and the boundary matters:

- **`permission-tiers.md`** governs the *risk of an operation against the host or
  the engine* (Read → Privileged, tiers A–D). It answers «may this agent run
  this engine/host operation».
- **This file** governs the *authority to publish a change* — the git and
  collaboration acts. It answers «may this agent push / open a PR / merge /
  release, and onto which target».

An act can be low engine-risk and still be forbidden here (a one-line docs
commit is Tier A/B, but *pushing* it needs authorization), and vice-versa. Both
contracts apply; the stricter one wins.

The rules below restate decisions already recorded in
[`docs/dev/engineering-process.md`](../../docs/dev/engineering-process.md) and in
`AGENTS.md`; those files now defer here for the authority model and do not fork
it. When they disagree with this file on *authority*, this file wins.

## The operations, by blast radius

Ascending — each row reaches further than the one above it:

| Operation     | What it is                                                        | Default authority |
|---------------|-------------------------------------------------------------------|-------------------|
| `read`        | Inspect the repo, docs, history, and the remote (read-only).      | Self-authorized   |
| `local_edit`  | Edit files **in the task worktree**.                              | Scope contract    |
| `commit`      | Commit to the **task branch** (never the shared `main`/`develop`).| Scope contract    |
| `push`        | Push a branch to `origin`.                                        | **Explicit**      |
| `pr_create`   | Open a pull request.                                              | **Explicit**      |
| `pr_edit`     | Edit / comment / label / review a pull request.                  | **Explicit**      |
| `merge`       | Merge or squash a pull request.                                   | **Explicit**      |
| `release`     | Tag, publish, create a release, or upload release assets.         | **Explicit**      |

- `read`, `local_edit` and `commit` are the baseline an open **session scope
  contract** already authorizes (`.claude/sessions/<branch>.md`): edit the task
  worktree, commit the task branch. They never touch the shared `main`/`develop`
  checkout, and `commit` **never** carries a `Co-Authored-By: Claude` trailer
  (owner rule, overrides any harness default).
- `push` and everything below it leave the worktree. They are **refused by
  default**. When refused, the agent delivers the **exact command** for the owner
  to run — it does not run it, and it does not improvise an alternative route
  (no force-push, no direct push to a protected branch, no bypass).

## The three authorization states

Every request is judged in one of three states. A fixture, an audit, an
implementation and an integration must each be able to tell them apart.

1. **ABSENT** — no authorization granted. Only `read` / `local_edit` / `commit`
   (within an open scope contract) proceed. Every `push`/`pr_*`/`merge`/`release`
   is refused; hand the command to the owner.

2. **PARTIAL** — authorization exists for *a specific operation on a specific
   target*, and the request is for a **different** operation or a **different**
   target. It is refused. Authorization does **not** generalize:
   - a grant to `push branch:feat/x` does not authorize pushing `feat/y`, nor
     opening a PR, nor merging;
   - authorization to **integrate into `develop`** does **not** authorize cutting
     a release, a tag, or publishing artefacts
     (`engineering-process.md`, «Carrying a fix to both destinations»);
   - a single grant is single-use for its named operation+target; it is not a
     standing permission.

3. **EXPLICIT** — authorization naming **both** the operation and the target, and
   the request matches it. The act on that target proceeds — and only that act on
   that target.

The decision is therefore a function of `(operation, target, grant)`:
`read`/`local_edit`/`commit` → allow; otherwise allow **iff** the grant lists the
exact target under the exact operation, else refuse. The reference
implementation is pure logic — it never shells out to `git`/`gh`, so evaluating
authorization can never, itself, publish anything
(`scripts/authorization_gate.py`, tested by `scripts/test_authorization_gate.py`,
fixtures in `scripts/authorization_fixtures/`).

## MUST-NEVER (applies to every agent and skill)

1. Never `git push`, `gh pr create`, `gh pr merge`/`gh pr review`, `git tag`,
   `gh release create`, or `gh workflow run release.yml` without an **explicit**
   grant naming that operation and target. Absent a grant, deliver the command.
2. Never bypass a control to publish: no force-push, no direct push to a
   protected branch, no merging past a required check, no `--force` worktree/branch
   removal to hide pending work (`engineering-process.md`, «Integration»).
3. Never treat one grant as many: a push grant is not a PR grant, a PR grant is
   not a merge grant, and none of them is a release grant.
4. Never add a `Co-Authored-By: Claude` trailer to any commit.
5. A **local** control (git hook, `make gitflow preflight`) assists the person at
   the keyboard; it is advisory. The **authoritative** controls are the CI checks
   and — when the owner enables them — the GitHub branch protections. The harness
   cannot grant what GitHub enforces; it can only refuse earlier. State which side
   verifies a given rule when it matters (`engineering-process.md`, «The harness
   that enforces this»).

## How an agent uses this

Each spec in `.ai/agents/` names the **maximum collaboration operation** it may
*propose* and the fact that anything above `commit` is a stop-and-ask:

- **Auditor**, **Security Reviewer** — `read` only; never commit, push or operate PRs.
- **Builder/Tester**, **Gatekeeper** — up to `commit` on the task branch; `push`
  and above are the owner's, delivered as commands.
- **Release Manager** — may *prepare and propose* a `release`, never perform it;
  the owner runs every publish step.

Crossing above the granted state is a stop-and-ask, never a judgement call.
