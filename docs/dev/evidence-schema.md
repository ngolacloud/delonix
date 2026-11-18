# Evidence schema — a result is only evidence if it says where it ran

A green console line is not evidence; a structured record is. The release gate
already binds each suite to the exact commit it shipped
(`scripts/release_gate.py`, `reports/release-validation/<tag>.json`), and `e2e.sh`
already writes PASS/FAIL/SKIP per leaf into `results.jsonl`. This page defines the
**structured evidence record** that ties those to the promotion policy with the
four states `PASS`/`FAIL`/`SKIP`/`BLOCKED`, and the rule that a **mandatory
profile is approved only by applicable, executed evidence**.
`scripts/evidence_gate.py` validates a record and decides promotion; it also
renders a human-readable view.

## The record (every field required)

One JSON object per suite-run:

| Field | Meaning |
|---|---|
| `sha` | the 40-hex commit the suite ran against |
| `suite` | the suite name (`cargo-test`, `e2e`, `chaos`, `critest`, a gate) |
| `version` | the version/tag under test (`v0.1.1`, or `develop`) |
| `environment` | where it ran / which client (`ci/ubuntu-latest`, `lab/qemu-rootless`) |
| `profile` | `pr-rootless` \| `lab-ephemeral` \| `release` (see below) |
| `command` | the exact command run |
| `exit_code` | the process exit code — **required when the state is PASS or FAIL** (it was executed); omitted only for SKIP/BLOCKED |
| `denominator` | the total the result is out of (leaves, cases) |
| `exclusions` | what was deliberately not covered (e.g. `["needs-root"]`) |
| `state` | `PASS` \| `FAIL` \| `SKIP` \| `BLOCKED` |
| `reason` | **required for SKIP/BLOCKED** — why it did not run/judge |
| `artifact_ref` | repo path to the raw artefact (e.g. `reports/.../results.jsonl`) |

## The four states

- **PASS** — ran, exit 0, the suite's criterion met. The only state that approves.
- **FAIL** — ran, did not meet the criterion. Blocks promotion of its profile.
- **SKIP** — deliberately not run (out of scope for this profile; a leaf the host
  cannot measure rootless). Not a pass; `reason` says why.
- **BLOCKED** — could not run because of a missing capability/client the executor
  cannot provide. Not a pass; `reason` says what was missing.

## The three profiles, and what they gate

- **`pr-rootless`** — what a PR proves on a hosted runner, rootless: build, unit,
  the gate suite, rootless E2E. Gates **merge to develop**.
- **`lab-ephemeral`** — a throwaway lab (rootful VM, Ceph, multi-node): the leaves
  a rootless runner can only SKIP. Produces evidence; does not itself gate a merge.
- **`release`** — the suites bound to a tag's commit (`release_gate.py`): cargo-test,
  e2e, chaos, critest. Gates **promotion to `main`/a release**.

## The promotion rule

A **mandatory profile** (the suites a given promotion requires) is approved only
when, for each required suite, there is an **applicable** record (same profile)
that is **executed and passing** (`state == PASS` and `exit_code == 0`). A suite
covered only by SKIP, BLOCKED or FAIL does **not** approve — `evidence_gate.py
promote` fails closed and names the gap. This mirrors `release_gate.py`'s refusal
to promote on suites that were not recorded green on the tagged commit; the
evidence record adds the profile, the denominator/exclusions, and the SKIP/BLOCKED
vocabulary so a reader sees *what was and was not actually run*.
