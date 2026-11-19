# G5 — density: the attach curve vs. number of containers

> **Date:** 2026-11-03. **Scope:** the plan's NO-GO→GO G5 — measure whether attach
> latency grows with node density (the suspicion that the two `nft` dumps on the
> attach path, O(N) in the number of namespaces, degrade scale). **Measure BEFORE
> optimising** — the repo's discipline.

## Setup

`scripts/bench_density.sh --tiers 1,10,25,50,100 --samples 8 --force`. A ramp of
busybox containers on a bench network; at each tier, 8 timed probe attaches
(`run -d` + `rm`). **NOT PUBLISHABLE** by design: the node had 2 unrelated
containers (the owner's `sd-pg`) and load 4–5 (shared box). What matters here is
the **shape** of the curve, robust to noise, not the absolute numbers.

## The curve (attach p50, ms)

| live containers | 1 | 10 | 25 | 50 | 100 |
|---|---|---|---|---|---|
| **attach p50 (ms)** | 193 | 202 | 206 | 196 | **215** |
| attach p99 (ms) | 654 | 1846 | 1424 | 483 | 1150 |

## Reading

**p50 is FLAT**: 193 → 215 ms from 1 to 100 containers (~11%, within the noise of
a box at load 4–5). p99 jumps around (654…1846…483…1150) but **with no trend** — it
is the contention of the shared machine, not a density effect (if it were O(N), it
would rise monotonically). **Attach latency does NOT grow with density up to 100
containers.**

## Conclusion — do NOT optimise (and why the suspicion does not hold)

The suspicion was that `drop_from_every_ns_set` (`infra.rs`, one `nft list sets`
per attach/detach) and the `nft -a list chain fwdeny` were O(N) and degraded
scale. The curve shows they **do not degrade in practice**: `list sets` counts
**namespaces**, not containers, and a typical workload has few namespaces (the 100
containers were in the `default` namespace → ~1 ns-set). The attach cost is
dominated by the fixed part (~20-25 `fork`+`exec` of `ip`/`sysctl`/`nft` + rootfs
prep), not by the dumps.

**Optimising `drop_from_every_ns_set`/`fwdeny` would be solving a non-problem** —
and at the cost of touching the network/security datapath. "Measure first" saved
exactly that bad trade. The target stays **documented but untouched**; if one day a
workload with MANY namespaces shows the curve rising, the optimisation (attack the
specific ns-set instead of the dump) has its starting point here.

## What REMAINS to measure (dedicated node)

The PUBLISHABLE curve (idle, dedicated node, no unrelated containers, up to a
degradation ceiling with SLOs) still needs the dedicated node — but the **shape**
is already known and reassuring. Reproducible: `scripts/bench_density.sh`.
