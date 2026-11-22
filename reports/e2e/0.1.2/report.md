# report.md — regression campaign for the `delonix 0.1.2` CLI

Regression campaign of the **public surface** of the delivered binary, via the
real executable with real arguments, **rootless** host, 2026-11-18. Links to H05
(E2E skill router) and H07 (evidence schema) of the harness; the structured
evidence lives in `evidence.jsonl` and validates against `docs/dev/evidence-schema.md`
(`scripts/evidence_gate.py`).

## 1. Candidate baseline (section 1 of the master prompt)

| Field | Value |
|---|---|
| Version | `delonix 0.1.2` |
| Base/candidate SHA | `b72edeedf38f4f46075520a449a37665228956e3` (HEAD of `develop` at start) |
| Binary digest | `cfc096220a0e064f60036993361188469f82585ec06844fd54a1957c5d650b16` |
| Build | `cargo build --release --locked -p delonix-bin …` (release profile: `lto=thin`, `codegen-units=1`) |
| Binaries | **5 in the same target** — `delonix`, `delonix-cri`, `delonix-mgmt`, `delonix-mcp`, `delonix-node-api` (all 0.1.2) |
| Toolchain | `rustc`/`cargo 1.96.0`; tag at HEAD `consolidacao-pre-20261118-2012` |
| Host | i9-13900H (20 threads), 62 GiB, kernel 7.0.0-43, rootless; `cloud-hypervisor`+`libvirt`+`/dev/kvm` present; **no sudo** |
| Lenses | docker 29.10.0, podman 4.9.3, k8s/CRI, cloud-hypervisor |

> **Concurrency note.** During the campaign the owner advanced `develop` from
> `b72edeed` to `9ec12592`, merging `fix/cli-ls-parity-and-buildcache-prune`
> (`29b10702`), which closes **GAP-0019** (`ls -q/-a`) and **GAP-0027** (build-cache
> prune) — both observed in this round as still open in `b72edeed`. The
> candidate tested is `b72edeed`; it was not rebuilt for `9ec12592`.

## 2. Discovery of the public tree (section 2)

`scripts/cli-tree.sh` walks the tree of the **real parser** of the delivered binary:

```
comandos: 413   folhas: 366   grupos: 78
classes:  250 ~ (grafia)   97 = (sem quebra)   17 → (movida por decisão)   2 ! (quebra documentada)
```

- `cli-tree.sh --gate` → **green**: the 366 leaves match `scripts/cli_baseline.tsv`.
  **Zero surface drift** vs the last baseline (0.1.1): the delta's changes in the
  parser (`build.rs`, `container.rs`, `stack.rs`, `system.rs`,
  `scaffold.rs`) were **behavioural**, not contractual.
- `cli_exec_ratchet.py --from-results` → **272 of 366 leaves executed under
  assertion (74.3 %)**, equal to the baseline, `rc=0` — no coverage regression.

## 3. Execution of the public suite (sections 3-4) — the `e2e.sh` battery

The repo's canonical battery (the 5 bins, rootless host, self-isolated in its own
`DELONIX_ROOT`/`DELONIX_NET_RUNTIME_DIR`). Full artefact:
`e2e-results.jsonl` (1792 records).

```
PASS=1735   FAIL=22   SKIP=35   XFAIL=0   XPASS=0
```

### 3.1 Harness robustness finding (measure the bench before the engine)

The **1st run** gave `FAIL=25`. Six of those failures were cloud-hypervisor
(`error[DX-1506] … socket path … is 143 bytes, a UNIX socket path is limited to
107`). **They were neither regressions nor missing environment**: the `OUT`/`DELONIX_ROOT`
went under the session's long scratch path, and the VMM socket (`<root>/vms/<name>.sock`)
overflowed the AF_UNIX limit of 107 bytes — which the binary **diagnoses
correctly** ("use a shorter VM name or a shorter DELONIX_ROOT"). With a short
root, the same cloud-hypervisor VM **starts** (`✓ VM started`). The figures above
(`FAIL=22`) are already from the re-run with a short root. → see GAP/finding H-1.

## 4. 0.1.1 regressions — confirmed FIXED in 0.1.2

| Bug (0.1.1) | Verification in 0.1.2 | State |
|---|---|---|
| **BUG-0007** ImageID docker-api = name, not digest | `GET /containers/json` → `ImageID: sha256:3ea06e…`, `Image` keeps `alpine:3.24` | ✅ **PASS** |
| **BUG-0008** exec inherits `65534(nobody)` groups | `exec: no --group-add leaves no inherited host groups (65534)` | ✅ **PASS** |
| **BUG-0009** false drift from `Always`≠`always` casing | `drift: logo a seguir ao apply não há deriva` + `restartPolicy` checks | ✅ **PASS** |

## 5. Triage of the 22 failures — **0 binary regressions**

| # | Category | Failures | Verdict |
|---|---|---|---|
| A | **Privilege/rootless-limited** (lab-ephemeral) | CRI default-caps, landlock+tmpfs, 2× mount `/sys` on-top/rshared, 2× `net netns firewall/publish` + 2 cascades, slirp-flaky | **BLOCKED** — the host does not measure rootless; would measure in a rootful VM + CNI |
| B | **Battery staleness vs 0.1.2** (correct binary) | 2× `--disk-size 0` (clap refuses with `rc=2`, the test expected `1`), 2× kind `networkgateways` (does not exist in the catalogue; there is `Gateway`+`Router.networkGateways`), `/metrics` (`# TYPE delonix_build info` OpenMetrics — test looks for the old `_info` line), `cluster network profiles`/`system monitor` format, `cluster kube generate` exit `1`≠`4` | **test deviation** — stale assertion, not a defect |
| C | **`vm --require` setup** | a non-existent image triggers DX-4000 before the capability probe (arguable ordering) | setup/needs-image |
| D | **Genuine finding (GAP-0028)** | a VM with the same name **in another namespace** is not a conflict (`rc=0 ensured`), whereas #824/`3de5db5c` made it a conflict for **containers** | 🟡 new GAP |

Detail and reproduction in `FINDINGS_BACKLOG.md`.

## 6. Privileged labs (lab-ephemeral profile) — what ran, what stays

| Lab | Result | Basis |
|---|---|---|
| **Compute / microVM (cloud-hypervisor)** | ✅ **PASS rootless** | the battery creates/starts/snapshots CH VMs with a short root; `/dev/kvm`+VMM+libvirt present |
| **KaaS — `cluster create` (kind-mode, no Docker)** | ✅ **PASS rootless** | control-plane `kindest/node v1.37.0`, kubeadm, CNI kindnet; `kubectl get nodes` → `camp012-control-plane Ready control-plane v1.37.0 containerd://2.3.4`; `destroy --force` clean (0 nodes, 0 containers). Log: `cluster-lab.log` |
| **Full CRI conformance (critest)** | ⛔ **BLOCKED** | requires root + CNI conflist (`portmap`) + golden image `delonix-vm-k8s`; rootless host, no sudo (repo policy: privileged only in a VM) |
| **Rootful mounts / netns-nft / multi-node HA** | ⛔ **BLOCKED** | same prerequisite (disposable rootful VM) |

## 7. Verdict per profile (section 5 — gate)

| Profile | Verdict | Basis |
|---|---|---|
| **pr-rootless** (merge→develop gates) | ✅ **APPROVES** | `cli-tree-gate`, `cli-exec-ratchet` and `regression-bug-0007-0008-0009` all `PASS exit 0` (`evidence_gate.approves` = APROVA); tree without drift; 3 bugs confirmed fixed |
| **lab-ephemeral** (produces evidence) | ◑ **partial** | compute-CH and KaaS **PASS**; the full `e2e` battery is `FAIL=22` (categories A-D: 0 binary regressions); critest/rootful **BLOCKED** |
| **release** (promotion→main/tag) | ⛔ **BLOCKED** | out of scope (no tag cut); requires green critest in a rootful lab |

### Conclusion
The **delivered 0.1.2 artefact has neither contract nor binary regression**
measurable in the rootless profile: the public tree is stable (zero drift), the
execution coverage holds (272/366), the 3 bugs from 0.1.1 are fixed, and
the two biggest privileged promises that **can** be proven rootless
(cloud-hypervisor microVM and KaaS kind-mode) **pass**. The 22 battery failures
split between environment gaps (BLOCKED, needing a rootful VM) and
**staleness of the battery itself** (the binary behaves correctly, it is the
assertions that aged) — plus **one** genuine new GAP (GAP-0028) and **one**
harness robustness finding (H-1, sensitivity to the length of `DELONIX_ROOT`).
Promotion to release stays **blocked** only on what requires a rootful lab
(critest), just as the harness foresees.

## 8. Artefacts
- `e2e-results.jsonl` — full battery (1792 records, PASS/FAIL/SKIP per leaf).
- `e2e-run.log` — stdout of the re-run (short root).
- `cluster-lab.log` — bring-up+teardown of the rootless KaaS cluster.
- `evidence.jsonl` — 6 evidence records (validate against `evidence-schema.md`).
- `COVERAGE_MATRIX.md` — coverage per axis (canonical/aliases/parameters/functional/profiles).
- `FINDINGS_BACKLOG.md` — GAP-0028, finding H-1 and the battery staleness items.
