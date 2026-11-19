# CRI conformance — measured on `fa8c6297`

## Measurement

| Field | Value |
|---|---|
| Suite | cri-tools `critest` **v1.36.0** (upstream) |
| Engine | `delonix-cri` built from `fa8c6297` (debug), **rootless** |
| Kernel / cgroups / LSM | Linux 7.0.0-31 / v2 / AppArmor (no SELinux) |
| State | `DELONIX_ROOT` via `mktemp` + `DELONIX_NET_RUNTIME_DIR=/tmp/dxcri-n` isolated |
| Command | `NO_BUILD=1 DELONIX_NET_RUNTIME_DIR=/tmp/dxcri-n scripts/critest.sh` |
| Date / duration | 2026-09-15, 1 129 s |
| Logs | `raw/critest.log`, `raw/critest.log.server` |

```
Ran 103 of 122 Specs in 1129.479 seconds
FAIL! -- 82 Passed | 21 Failed | 0 Pending | 19 Skipped
```

**Comparable** with the published value (79/24 of 103, critest v1.36.0, v0.63.1): same suite,
same total. **+3 pass** — all on AppArmor (9 → 6 failures). The other 15 failures are the
same areas as before.

Rootful: **NOT RUN** (production host). SELinux: **ENVIRONMENT BLOCKED**.

## The 21 failures, by cause

| # | Spec | Message | Cause | Note |
|---|---|---|---|---|
| 1–6 | AppArmor (6 specs) | `InvalidArgument: AppArmor profile '…deny-write' is not loaded on the host` | **HOST ENVIRONMENT** | critest loads the profile with `apparmor_parser`, which requires root. The engine refuses an unloaded profile — **correct fail-closed behaviour**. Revalidate rootful. |
| 7–9 | Mount Propagation rprivate/rshared/rslave | `failed to mount "mntSource": operation not permitted` | **HOST ENVIRONMENT** | It is critest itself that fails to mount on the host without privilege; the engine is never exercised. |
| 10 | Mount Readonly non-recursive | `failed to mount tmpfs … operation not permitted` | **HOST ENVIRONMENT** | Same. Independently of this, rro is not supported (G-010). |
| 11 | Networking — port mapping with container port only | `dial tcp 10.200.0.2:80: i/o timeout` | **ROOTLESS LIMITATION** | critest connects to the pod IP from the host; in rootless that IP lives in the holder's netns. |
| 12 | Multiple Containers — should support network | timeout 77 s | **ROOTLESS LIMITATION** (inference) | Same reachability-from-host pattern; not confirmed line by line. |
| 13 | Streaming — portforward | timeout 60 s | **UNKNOWN** | The server log records nothing (77 lines total — observability gap). |
| 14 | Streaming — portforward in host network | `failed to start port forward …` | **UNKNOWN** | Same. |
| 15 | Security Context — PodPID | process list not shared | **MISSING FEATURE** | `shareProcessNamespace` is the documented "Phase 3". |
| 16 | Security Context — RunAsUserName | `failed to start container` | **DELONIX BUG / MISSING FEATURE** | Start fails with a user by name. |
| 17 | SeccompProfilePath nil ⇒ unconfined | `Seccomp: 2` (expected 0) | **DELIBERATE SECURITY DIVERGENCE** (to confirm by ADR) | The engine applies the default profile where the spec asks for unconfined. |
| 18 | NoNewPrivs=false allows escalation | `Effective uid: 1000` (expected 0) | **ROOTLESS LIMITATION** (inference) | setuid does not elevate in a single-mapping userns. |
| 19 | Container OOM → 137/OOMKilled | timeout 60 s | **MISSING FEATURE** | Documented: `oom_kill` is not captured live. |
| 20 | Image — pull by digest | image list does not match | **DELONIX BUG** | |
| 21 | Image status Uid/Username | `Image Username should be www-data` | **DELONIX BUG** | |

## Summary by cause

| Cause | N |
|---|---|
| HOST ENVIRONMENT | 10 |
| ROOTLESS LIMITATION | 3 |
| MISSING FEATURE | 2 |
| DELONIX BUG | 3 (includes RunAsUserName) |
| DELIBERATE SECURITY DIVERGENCE | 1 |
| UNKNOWN | 2 |

## Harness problems (not the product)

1. `scripts/critest.sh` isolates `DELONIX_ROOT` but **not** `DELONIX_NET_RUNTIME_DIR` — the
   half-isolation this repo has already documented as dangerous. Worked around from outside.
2. The `cleanup` calls `delonix netns down`, a command that no longer exists (it is `net netns down`);
   the error is swallowed by `|| true`. Verified afterwards: no process was left pointing at the
   isolated runtime dir.
3. The server does not log streaming/port-forward failures — diagnosing 2 of the 21
   failures is impossible from the logs alone.
