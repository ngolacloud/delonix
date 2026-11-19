# Delonix v0.1.1 — release decision

**Date:** 2026-10-31 · **Line:** `stabilize/0.1.1` (worktree, 20 commits, NOT pushed)
· **Base:** `origin/develop` `8ab783dd` (= `v0.1.0` + 39) · **v0.1.0 already published
and immutable** → these fixes are for **`v0.1.1`**.

## Verdict: **GO** within the explicitly validated scope (the only residual was root-caused and FIXED)

`v0.1.1` is an unequivocal improvement over the `v0.1.0` already in the field: it
closes 1 P1 integrity issue, reachable memory-safety advisories, 1 state TOCTOU, and 3
P2/P3 findings — all with a test **and** a live proof. The only residual (the custom-network
attach hang) was root-caused and FIXED: it was a self-deadlock that
P2-1 itself introduced (safe name-claim through the re-exec) + a systemic
pattern of `flock(LOCK_EX)` with no timeout; both handled. Custom-net attach
completes in ~1s, and the original DEF-003 (compose does not reconcile) was finally
testable and does NOT reproduce.

## 1. Fixes (all: fix + test + live proof)

| Item | Severity | Proof |
|---|---|---|
| F-1 poisoned layer cache on upgrade | P1 integrity | unit + A/B live (rabbitmq healthy on 1st use) |
| SEC-01 bump zstd 0.14.3 | Medium security | `cargo deny advisories`=ok; 6 image-reachable RUSTSEC eliminated |
| P2-1 concurrent `run --name` (TOCTOU) | P2 state | 4× race live → 1 created, 3 `DX-5000` |
| P3 version stamp + downgrade gate | P3 recovery | live: record format 99 refuses `start`/`rm` |
| S15-1 unbounded lock-files | P3 scale | 256-shard pool + GC on `system prune` (live: 20 reclaimed) |
| non-persisted restart_policy | P2 (new finding) | live: `--restart always` returns to Running; `no` stays Stopped |

Plus: `install.sh --uninstall`/`--purge`; P2-2 warning (restart does not survive reboot without `boot enable`).

## 2. Scope SUPPORTED for production (validated)

- **Rootless OCI containers/pods** (§7) — lifecycle/OOM/isolation proven; **OOM enforced live** (32M kills, 128M survives).
- **VMs** CH/libvirt — gated matrix **180/270 N2** (honest ceiling 247).
- **§8 network/gateway** — **proxmox (47 cells) and opnsense validated live against the appliances** (pve92/opnsense); evidence in the repo, gate-enforced (`every_supported_capability_cites_evidence_that_exists`).
- **§12 recovery** — no P1; crash reconcile, atomic writes, idempotency, backup/restore proven.
- **§13 install/upgrade/removal/data preservation** — **full cycle proven live on a clean Ubuntu 24.04** (apt family).
- **CRI endpoint** — `serve cri` + `crictl` handshake `runtime.v1`, **RuntimeReady=true**.
- **§16 artefact integrity/security** — minisign signature, SHA256SUMS, SBOM 402 pkgs, `release_verify` 6/6; SEC-01 fixed.

## 3. Experimental / unvalidated (do not operate without risk acceptance)

42 EXPERIMENTAL cells + 6 IMPLEMENTED-NOT-VALIDATED (see `contrato-suporte-producao.md`).

## 4. Residual — ROOT-CAUSED and FIXED (it was TWO defects)

The "custom-network attach hangs with no timeout" was **two** defects, both now
fixed and proven live:

1. **Self-deadlock introduced by P2-1.** `cmd_run` held the name-claim
   (namespace,name) through the ENTIRE function — including the custom-network
   path, which **re-exec**s a SECOND `cmd_run` and WAITS for it. The re-exec'd
   child re-acquired the SAME name-lock and blocked on its own parent. Fix: the
   claim + the uniqueness check only on the 1st step (`!reexec`) — the re-exec is the same
   logical create. **Custom attach now completes in ~1 s** (before: hangs). The
   P2-1 guarantee still holds (a user create is always the 1st step → 4×
   concurrent `create --name` = 1 created + 3 refused, re-proven).

2. **Systemic pattern of `flock(LOCK_EX)` with no timeout** (~7 sites). New shared
   helper `delonix_node::flock_exclusive` (NB + 30s ceiling, EINTR-safe)
   routed through ALL: `delonix-state` Store+secret, `delonix-sdn` infra +
   ExclusiveLock + IpamLock, and the bin locks ippool/ingress_proxy/
   container-start/hosts_file (the last one ignored the result). Each already had
   a refusal path (Err/None) → they now **fail cleanly at the ceiling, never
   hang** — this is also how #1 was diagnosed (the timeout named the lock).

**Live proof:** custom attach completes in 1 s; the P2-1 race holds; and the **original
DEF-003** (compose does not reconcile a stopped service), finally testable, does **NOT
reproduce** — `compose up -d` brings the stopped `proj-cache` back to Running.
Tests: delonix-node flock 20/20, state 22/22, sdn 325/325, node-api 14/14.

## 5. Breadth still to do (non-blocking)

- E2E install for dnf/zypper/pacman (apt proven; same script, +1 VM each).
- Full CRI critest (endpoint proven; still needs root + CNI + clean host).

## 6. Conditions to cut `v0.1.1`

1. Integrate the 24 commits of `stabilize/0.1.1` into `develop` (owner merge).
2. Cut/tag `v0.1.1` via the release pipeline (`release.yml` + `release_verify.py`); DO NOT touch `v0.1.0`.

(The §4 residual — the attach hang — was fixed, so it is no longer a
condition; a pass on a real production node with network churn is still recommended
to re-confirm the custom attach under concurrent load.)

## 7. Traceability

`stabilize/0.1.1` commits: F-1(`80e37e7c`,`2bebab71`) · §3(`a61f3ded`) · §12/§16(`081421d5`)
· SEC-01(`7ecd2f11`) · P2-1(`9eaa0f76`,`be81ee84`) · P3(`4fbd6beb`) · fmt(`455b3490`)
· §13 install(`dab971fe`,`7408805e`) · S15-1(lock-files) · P2-2(warning) · restart_policy
· uninstall · §8/install-E2E/env reports. Gates on the changes: green
(leaves/contract FAIL = baseline/environment). `version_gate` ok.
