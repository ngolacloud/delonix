# Delonix Runtime — Validation report (CLI parity + CRI conformance + installation)

> Focused scope, not a full E2E. This round measures three axes that underpin
> the impartial Delonix × Docker × Podman comparison: (1) installation from the
> tree, (2) CLI/behaviour parity against docker and podman, (3) real CRI
> conformance (critest), in rootless and in rootful-VM. Groups not exercised are
> marked BLOCKED/SKIPPED with a reason — never omitted.

## Executive Summary

| | |
|---|---|
| Version | `delonix 0.1.0` |
| Commit | `1898466bb` (develop; +241 commits since v0.1.0) |
| Date | 2026-11-08 |
| Environment | Shared host Linux 7.0.0, x86_64, cgroup v2; **under load** (QEMU cri-rootful + kind cluster + Ceph + the owner's Odoo running in parallel) |
| Engines compared | Docker 29.9.2 (rootful, daemon), Podman 4.9.3 (rootless), Delonix 0.1.0 (rootless) |
| Mode | **rootless-first**; CRI also measured in **rootful-VM** (KVM, real root) where the suite requires it |
| Suites | `cri-tools critest` v1.36.0; harness `tests/cli-parity` (fast + differential) |

| Axis | Result |
|---|---|
| Installation `make install` | **FAIL → worked around** (packaging bug B1); 0.1.0 binary installed |
| Parity `fast` (parser/TAB/errors, rootless) | **49 OK / 1 FAIL** |
| Parity `differential` (docker×podman×delonix) | **PASS** (container flow + pod flow) |
| Static CLI matrix (198 commands, Nov 6) | 63 passed · 76 recognised-to-be-tested · 36 to-do · 14 out-of-scope · 9 divergence |
| CRI critest **rootless** | **84 ✓ / 19 ✗ / 19 skip** (Ran 103/122, 616 s) |
| CRI critest **rootful-VM** | **90 ✓ / 13 ✗ / 19 skip** (Ran 103/122, 389 s) |

## 1. Installation (Core Runtime / build axis)

`make install` aborted: `error: target/release/delonix-cri is missing`. Cause:
`PACKAGES` in the `Makefile` (line 44) lists the **library crate** `delonix-cri`,
not the binary package `delonix-cri-bin` that produces the `delonix-cri`
executable (ADR-0086 moved the bin; `scripts/critest.sh` was fixed in the
owner's session, the `Makefile` was not). Worked around with
`cargo build --release -p delonix-cri-bin --bin delonix-cri` + a fresh `make install`.
→ **BUG-0001**.

## 2. CLI and behaviour parity

### 2.1 Tier `fast` (parser, TAB, errors — rootless, no engine)

`tests/cli-parity/run.sh fast target/release/delonix` → **49 OK / 1 FAIL**.
All top-level aliases (`ps`, `logs`, `images`, `rmi`, `cp`, `port`, `info`,
`events`, `wait`…) align with the long form `container|image|system …`. The only
failure — `same_behaviour "stats all"` — is **NOT a product bug**: verified live,
`delonix stats` and `delonix container stats` are **byte-identical**
(they share the `StatsArgs` struct and the same handler, `bins/delonix/src/main.rs:217`
→ `cmd::container::run`). The FAIL is in the harness itself: `same_behaviour`
compares a **volatile sample** of `stats` (CPU/mem) and, with a live container on
the host (the owner's Odoo) between the two calls, the values diverge. →
**IMPROVEMENT M-TEST-02** (not a BUG). With the host free of live containers,
`diff` = identical.

### 2.2 Tier `differential` (docker × podman × delonix, real engines)

`tests/cli-parity/run.sh differential` → **PASS**. Proves real behavioural
parity, not just parser parity:

- **Flow 1 — interactive container:** the three engines return **identical**
  results — `run --rm -it` (marker + `test -t 0` + exit 37), `ps -a`
  (`--rm` cleaned up), `run -i` without TTY, stdout/stderr on separate streams,
  `stats` single-sample (no stream).
- **Flow 2 — two-member pod:** `delonix` and `podman` OK (localhost comms,
  listing members, inspect/describe, stop/start/restart, rm). `docker` omitted
  (it has no pods). **Deliberate** divergence: podman joins members with an
  imperative `run --pod`; delonix is **declarative-first** (`pod create -f`,
  `kind: Pod`) and returns an *actionable error* to the imperative `run --pod`.

> Coverage: `differential` only exercises 2 flows; volumes/networks/images
> remain at the parser-recognition level ("to-be-tested"). → **GAP-0002**.

## 3. CRI conformance (critest v1.36.0)

### 3.1 Rootless (owner's fresh baseline, same commit) — 84/19

19 failures: classified as 17 environment/rootless-architecture + 1 deliberate
divergence (seccomp `nil`) + 1 feature (PodPID).

### 3.2 Rootful-VM (KVM, real root) — 90/13

Rose 84→90: as predicted, `mount rprivate/rslave`, non-recursive `mount readonly`
and 2 of the 6 AppArmor *flip*. **But the ~104/104 projection is NOT
confirmed** — **13 failures with real root remain**:

| # | Spec failed in rootful-VM | Class | Item |
|---|---|---|---|
| 1 | AppArmor — enforce `apparmor_profile` (blocking writes, field) | BUG | BUG-0003 |
| 2 | AppArmor — permissive deprecated profile | BUG | BUG-0003 |
| 3 | AppArmor — prefer new field, blocking writes | BUG | BUG-0003 |
| 4 | AppArmor — prefer new field, work with profile | BUG | BUG-0003 |
| 5 | Security Context HostNetwork = true | BUG | BUG-0004 |
| 6 | Security Context HostNetwork = false | BUG | BUG-0004 |
| 7 | Security Context HostIpc = true | BUG | BUG-0004 |
| 8 | Security Context HostIpc = false | BUG | BUG-0004 |
| 9 | Mount Propagation `rshared` (host↔container) | BUG | BUG-0005 |
| 10 | Container OOM — exitCode 137 / OOMKilled | BUG | BUG-0006 |
| 11 | Streaming port-forward in host network | GAP | GAP-0003 |
| 12 | Security Context PodPID (`shareProcessNamespace`) | GAP | GAP-0001 |
| 13 | SeccompProfilePath — `nil` = unconfined | Deliberate divergence | (conformance-only flag) |

**Narrative correction (M1):** of the 13, **1** is a deliberate divergence; the
other **12 are real engineering work** (AppArmor, host-namespaces, OOM, rshared,
PodPID), not "environment". The honest number to publish is
**rootless 84/19 · rootful-VM 90/13**.

## 4. Groups not exercised in this round (coverage honesty)

BLOCKED/SKIPPED with a reason: **VM/microVM, Storage (Ceph), Kubernetes, IaC,
GitOps, PaaS, MCP, Providers** — not re-tested here; the host was saturated with
the owner's workloads and the scope of this round was parity+CRI+installation.
The rootless CRI baseline was **not re-run** because it is deterministic at the
same commit and because of the host saturation (the owner was running
critest-rootful-VM concurrently). Backlogs in `BUG_BACKLOG.md`,
`GAP_BACKLOG.md`, `BACKLOG.md`.

## 5. Evidence provenance

- Fast/differential parity: runs made in this session against `target/release/delonix`
  (0.1.0 @ 1898466b) with the host's real docker/podman.
- CRI rootless/rootful-VM: owner's baselines from 2026-11-08 in
  `reports/cri-conformance-0.1.1/` (`00-baseline-develop.md`, `rootful-vm.out`),
  same commit — validated and re-classified here.
