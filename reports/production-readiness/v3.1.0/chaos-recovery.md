# Chaos and recovery — measured against `fa8c6297`

`scripts/chaos.sh --bin <fa8c6297 debug>`, sandbox `/tmp/dlx-chaos` (its own root and runtime dir),
load 1m 2.93 / 32 threads, 2026-09-15, 177 s. Log: `raw/chaos.log`.

**29 PASS · 0 FAIL · 1 SKIP**

| Area | Scenarios | Result |
|---|---|---|
| Holder / control plane | holder-kill, full-holder-death, control-restart, holder-wedge, slirp-kill, idempotent-up | PASS — pin dies without costing workload PIDs; total death rebuilds and reports |
| Ownership across roots | pin, teardown, cross-root, proxy-cross-root | PASS — a pid recycled from another root is neither read nor killed |
| Resources | oom, concurrent-attach (8), scale (30), scale-cleanup (0 MiB), abrupt-kill, aggregate-ceiling, delegated-scope, cgroup-netns | PASS |
| Isolation | namespace-isolation, pod-namespace-isolation, pod-holder-respawn | PASS |
| Durability | disk-full, write-failure (rc=77) | PASS **partial**: the real disk exhaustion was skipped (90.5 GiB free) |
| Desired state | stack-converge ×3, stack-netroute ×2, stack-partial-apply | PASS |
| Remote storage | truenas-destroy | **SKIP** — no credentials |

## Not exercised

- Real disk exhaustion (needs a small filesystem).
- `truenas-destroy`.
- Host reboot, rootful, in-place upgrade with an old holder, SELinux.

## Note

In CI this same harness was **skipped** for this commit (`chaos.yml` run 34988281401). This is the
first chaos result against `fa8c6297`.
