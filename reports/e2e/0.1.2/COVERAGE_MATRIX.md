# COVERAGE_MATRIX.md — coverage of the `delonix 0.1.2` public surface

Coverage split per axis (section 5 of the master prompt), against the delivered
binary `0.1.2 @ b72edeed`. Source: `cli-tree.sh` (inventory), `e2e.sh`
(execution, `e2e-results.jsonl`), `cli_exec_ratchet.py` (execution under assertion).

## Axis 1 — canonical paths (invocable leaves)

| Metric | Total | Note |
|---|---|---|
| Discovered (real parser) | **366 leaves** / 413 commands / 78 groups | `cli-tree.sh --count` |
| Applicable (rootless+lab profile) | 366 | all groups are reachable on the host |
| With mapped target (gate) | **366** | `cli-tree.sh --gate` green; 0 orphan leaves |
| Executed under assertion | **272 (74.3 %)** | `cli_exec_ratchet.py`, == baseline, `rc=0` |
| Help verified (`--help`) | **366 (100 %)** | the battery checks the `--help` of each leaf by construction |
| Approved (rootless profile) | see §4 | 1735 PASS / 22 FAIL(triaged) / 35 SKIP |

> **Numerator caveat (documented in the harness):** the ratchet **undercounts** —
> a leaf invoked from inside a shell function does not resolve the leaf. That is
> why `exec`, `logs`, `kill`, `cp`, `port` appear at 0 % even though the battery
> exercises them (e.g. the BUG-0008 regression runs `exec … id` and passes). The 74.3 % is
> a floor, not a ceiling.

## Axis 2 — execution coverage per group (top)

| Group | exec/leaves | % | Group | exec/leaves | % |
|---|---|---|---|---|---|
| net | 36/36 | 100 | vm | 33/39 | 84.6 |
| system | 20/20 | 100 | cluster | 19/22 | 86.4 |
| stack | 11/11 | 100 | network | 18/21 | 85.7 |
| secret | 9/9 | 100 | container | 27/35 | 77.1 |
| backup | 6/6 | 100 | manifest | 3/4 | 75.0 |
| provider | 6/6 | 100 | volume | 11/16 | 68.8 |
| systemcontainer | 6/6 | 100 | image | 16/28 | 57.1 |
| compose/gitops/serve/workload/mcp/config | 100 | | pod | 6/19 | 31.6† |
| oci-runtime | 0/5 | 0‡ | bucket | 0/3 | 0‡ |

† `pod`/`image` low: many leaves exercised via a shell helper (undercount).
‡ `oci-runtime`/`bucket`: exercised via `--help` but without effect-asserted
execution in the rootless profile (oci-runtime requires a bundle; bucket requires an S3 backend).

## Axis 3 — functional contracts per family (section 2 of the prompt)

| Family | Key scenarios exercised | Rootless verdict |
|---|---|---|
| Containers/pods/OCI | id+name lifecycle, signals, TTY, exec, cp, logs, port, freezer, shared pod netns | ✅ PASS |
| VM / hybrid compute | CH create/start/snapshot/resize, joint inventory, namespaces | ✅ PASS (short root) |
| Images/registry/secrets | build FROM+RUN, cache, digest, save/load, commit, login, encrypted secret | ✅ PASS |
| Network/storage | published port, internal DNS, L3, multi-homed IPAM, volume bytes/mount, hosts | ✅ PASS |
| Kubernetes/protocols | **cluster create→Ready (kind-mode)**, serve docker-api/cri, mcp negotiation | ✅ PASS (KaaS); CRI-root ⛔ |
| IaC/workflows | idempotent plan/apply/diff/drift/delete, GitOps, compose, wait, 33 Kinds | ✅ PASS |
| Operation/experience | backup/restore, completion, man, dashboard(PTY), init, policy, version, events | ✅ PASS |

## Axis 4 — aliases and parameters/variants

- **Aliases:** `ps`/`container ls`, `images`/`image ls`, etc. — covered in the
  inventory (`cli-tree` classes); `ls -q/-a` had a divergence (GAP-0019),
  **closed by the owner in `9ec12592`** during the round.
- **Parameters:** positive/negative cases, default-when-omitted, rejection and
  limits are exercised by the battery (e.g. `--restart always:2` refused;
  `--disk-size 0` refused in the parser; `restartPolicy` enums
  Always/OnFailure/Never normalised). Build-conditioned variants do not
  apply (a single release artefact).

## Axis 5 — profiles (gate governance, H07)

| Profile | Discovered | Applicable | Mapped | Executed | Approved |
|---|---|---|---|---|---|
| **pr-rootless** | 366 | 366 | 366 | 272 (+help 100%) | ✅ merge gates APPROVE |
| **lab-ephemeral** | — | VM/KaaS/CRI/rootful | — | VM-CH ✅, KaaS ✅ | ◑ critest/rootful BLOCKED |
| **release** | — | critest/chaos/cargo-test | — | — | ⛔ out of scope (no tag) |

Structured evidence: `evidence.jsonl` (6 records, validate against
`docs/dev/evidence-schema.md`).
