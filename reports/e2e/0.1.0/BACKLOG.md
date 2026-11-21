# BACKLOG.md (master) — Delonix 0.1.0 @ 1898466b — 2026-11-08

Interleaves BUG/GAP/IMPROVEMENT by severity (P0→P3). Produced by the
parity+CRI+installation round (`report.md`). To groom → queue, use
`.ai/skills/delonix-bug-triage`.

> **Guiding rule:** rootless-first. Several items (BUG-0003/04/05/06, GAP-0001/03)
> only manifest/resolve with real root → the fix target is the **rootful-VM
> matrix**, which materialises "rootful where required". Do not degrade the
> rootless default to close them.

## P0
_(none — nothing fully blocks the product in this round)_

## P1

| ID | Type | Title | Component | Compl. | Dep. |
|---|---|---|---|---|---|
| BUG-0001 | BUG | `make build/install` does not produce `delonix-cri` | `Makefile:44` | XS | — |
| BUG-0003 | BUG | AppArmor not enforced (4 specs) | `delonix-linux` apply/ensure_apparmor + `delonix-cri` lifecycle | M | — |
| BUG-0004 | BUG | HostNetwork/HostIpc unsupported (4 specs) | `delonix-cri` lifecycle + `delonix-linux` launch_spec (IPC setns missing) | L | enables G1,G3 |
| GAP-0001 | GAP | PodPID refused in host-net/CNI | `delonix-compute` pod.rs + `delonix-cri` lifecycle:1930 | M | BUG-0004 |
| GAP-0002 | GAP | `differential` covers only 2 flows | `tests/cli-parity` | M | — |
| GAP-0003 | GAP | Port-forward host-network | `delonix-cri` streaming.rs:227 | S* | BUG-0004 |
| GAP-0008 | GAP | **CNCF 3:** SLSA Provenance in build | `delonix-oci` build.rs + sign.rs | L | GAP-0009 |
| GAP-0009 | GAP | **CNCF 4:** SBOM SPDX in-toto on the image | `delonix-scanner` + `delonix-oci` | M | — |
| GAP-0012 | GAP | **CNCF 1+2:** proven OCI/CRI conformance + runtime ADR | cross | XL | CRI BUGs |
| GAP-0015 | GAP | **CNCF 2 (Podman):** k8s YAML interop `kube play`/`generate` | `delonix` cmd + `delonix-compute` | L | — |
| M-DOC-01 | IMPROVEMENT | Conformance narrative: 84/19 rootless · 90/13 rootful-VM (not ~104/104) | docs/README/reports | XS | — |
| M-TEST-03 | IMPROVEMENT | Gate that fails the build if a `BINARIES` binary is missing (would have caught BUG-0001) | `Makefile` + `scripts/` | S | — |

## P2

| ID | Type | Title | Component | Compl. | Dep. |
|---|---|---|---|---|---|
| BUG-0005 | BUG | Mount Propagation `rshared` | `delonix-linux` lib.rs:2504 | M | — |
| BUG-0006 | BUG | OOMKilled / exit 137 not reported | `delonix-linux` died_of_oom + `delonix-cri` exit_reason | M | — |
| GAP-0004 | GAP | Partial Docker Engine API (logs+exec) | docker API | XL | — |
| GAP-0005 | GAP | Compose (DEF-003) | `delonix compose` | M | — |
| GAP-0011 | GAP | **CNCF 6:** converge on the Compose Specification | `delonix compose` | M | GAP-0005 |
| GAP-0013 | GAP | **CNCF 8:** scanner policy + compare + CI gating | `delonix-scanner` + CLI `scan` | M | — |
| GAP-0010 | GAP | **CNCF 5:** distributed build cache + multi-platform | `delonix-oci` build.rs + registry.rs | XL | — |
| GAP-0016 | GAP | **CNCF 5 (Podman):** Quadlet/systemd for user workloads | `delonix` cmd (reuses serve.rs) | M | — |
| GAP-0017 | GAP | **CNCF 4/8 (Podman):** signature trust by policy | `delonix-security` policy.rs + `delonix-oci` sign.rs | M | GAP-0009 |
| M-TEST-02 | IMPROVEMENT | `same_behaviour "stats all"` gives a false positive with a live container (volatile sample) | `tests/cli-parity/frequent-verbs.sh:112` | XS | — |

## P3

| ID | Type | Title | Component | Compl. | Dep. |
|---|---|---|---|---|---|
| GAP-0006 | GAP | Contexts/remote-mgmt (ADR-0010) — WONT_FIX candidate | `system` verbs | L | ADR-0010 |
| GAP-0007 | GAP | 36 "to-do" commands in the matrix | various | S–M | GAP-0002 |
| GAP-0014 | GAP | **CNCF 7:** open project governance (owner's decision) | repo/project | M | strategic |
| M-DOC-04 | IMPROVEMENT | Document rootful-VM as the official path for host-ns/AppArmor/OOM | docs | S | — |

## Progress of the 8 CNCF value areas (target: all ✔)

Source: `CNCF_GAP_DOCKER.md`. ✗ absent · ◑ partial · ✔ done.

| # | CNCF area | State | Closed by | → 100% |
|---|---|---|---|---|
| 1 | Runtime lineage | ✗ | GAP-0012 | runtime ADR + conformance |
| 2 | Conformance/k8s interop | ◑ | BUG-0003/04/05/06 · GAP-0001/0003 · GAP-0012 · **GAP-0015** | CRI ≥95% + OCI suites + `kube play/generate` |
| 3 | SLSA Provenance in build | ✗ | GAP-0008 | in-toto SLSA v1.1 attached |
| 4 | Interoperable SBOM | ◑ | GAP-0009 · **GAP-0017** | SPDX in-toto + trust by policy |
| 5 | Cloud-native ops (build+host) | ✗ | GAP-0010 · **GAP-0016** | cache/multi-arch + systemd/Quadlet |
| 6 | Compose Specification | ◑ | GAP-0005 + GAP-0011 | green conformance suite |
| 7 | Open governance | ✗ | GAP-0014 | public + governance + signed releases |
| 8 | Mature CI scanning | ◑ | GAP-0013 · **GAP-0017** | policy + compare + gating + trust |

**Already aligned (do not redo):** OSV (scanner), cosign/sigstore (`sign.rs`),
rootless-first, **daemonless, native pods, runtime `policy.json`, node-api systemd
unit** (philosophical parity with Podman). Sources: `CNCF_GAP_DOCKER.md`,
`CNCF_GAP_PODMAN.md`.

## Suggested sequence (dependencies)
1. **BUG-0001** + **M-TEST-03** (unblocks a clean install; XS/S, independent).
2. **BUG-0004** (host-ns) → unblocks **GAP-0001** (PodPID) and **GAP-0003** (port-forward) in the same batch.
3. **BUG-0003** (AppArmor), **BUG-0005** (rshared), **BUG-0006** (OOM) — independent, parallelisable.
4. **M-DOC-01** as soon as (2)+(3) close, to republish the real numbers.
5. **GAP-0002** (broaden differential) underpins the measurement of **GAP-0007**.

## CNCF sequence (supply-chain first — highest weight in the assessment)
6. **GAP-0009** (SBOM SPDX in-toto) → **GAP-0008** (SLSA provenance) → **GAP-0017** (trust by policy): they reuse `sign.rs`+scanner+`policy.json`.
7. **GAP-0013** (scanner policy/compare/CI): leverages the existing OSV.
8. **GAP-0015** (k8s interop `kube play/generate`): Podman's biggest CNCF differentiator.
9. **GAP-0012** (proven conformance): consumes the closed CRI BUGs + runs the OCI suites.
10. **GAP-0005 → GAP-0011** (Compose Spec).
11. **GAP-0010** (multi-arch build + cache), **GAP-0016** (systemd/Quadlet) and **GAP-0014** (governance — owner's decision).
