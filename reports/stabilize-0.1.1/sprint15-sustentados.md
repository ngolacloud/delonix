# §15 — Stability and sustained performance

**Measured candidate:** `delonix 0.1.0` release (`8ab783dd`, the shipped candidate) for
the resource/latency numbers; P2-1 footprint measured with the `stabilize/0.1.1`
line binary (debug). **Date:** 2026-10-29. **Host:** Linux 7.0.0, rootless,
state root isolated in scratch (not the owner's).

## Workload and criteria (defined BEFORE measuring)

- **Churn:** 300× `run --rm alpine:3.20 -- true` (create→start→exit→rm cycle) +
  30× explicit lifecycle `create→start→stop→rm`, with 1 long-lived container
  (`sleep 1000`) acting as a supervisor-under-observation.
- **Pass:** no monotonic growth of supervisor fds/RSS; state records do not
  accumulate (`--rm`/`rm` clean up); `dlx-*` cgroup leaves do not
  accumulate; 0 orphans post-churn (back to baseline = only the holder); per-cycle
  latency stable (no monotonic rise).

## Results

| Metric | Baseline | @300 churn | @+30 lifecycle | Verdict |
|---|---|---|---|---|
| supervisor fds | 3 | 3 | 3 | ✅ no fd leak |
| supervisor RSS | 832 kB | 832 kB | 832 kB | ✅ no growth |
| `.json` records | 1 | 1 | 1 | ✅ `--rm`/`rm` clean up |
| `dlx-*` cgroup leaves | 11 | 11 | 11 | ✅ **no leaf-leak** (the historical concern does NOT reproduce here) |
| remaining containers | 1 (holder) | 1 | 1 | ✅ 0 orphans |
| latency/cycle | ~0.07 s steady-state (first 2 ~0.4 s = cold warmup) | stable | — | ✅ no degradation |

**Idle:** the supervisor of a stopped container consumes **832 kB RSS / 3 fds**
(very light daemonless profile). Network "down" until needed.

## Finding S15-1 — UNBOUNDED accumulation of lock files (P3)

The lock files in `containers/` are **never deleted** (by design:
deleting them would open a window in which two processes lock different inodes).
Measured:

- **Per-id `.{id}.lock`** (PRE-EXISTING, before this campaign): 330 containers →
  **330** lock files; they grow 1:1 with containers already created.
- **`.name-{ns}.{name}.lock`** (ADDED by my **P2-1** fix): 30 containers →
  **30** name-locks. Since the auto-generated names of `run --rm` are unique, P2-1
  **~doubles** the accumulation rate (one id-lock + one name-lock per container).

**Impact:** on a high-turnover, long-lived node (e.g. a PaaS creating
millions of containers), the `containers/` directory accumulates millions of
0-byte files. Each `Store::list()` (used by `ps`, by name checking, etc.)
does a `read_dir` of ALL entries → the scan degrades linearly with the total
container history, not with the live ones. It is unbounded resource consumption (§6#9) and
sustained latency degradation (§15). Severity **P3** (0 bytes each, no
data loss; only bites at scale and in the long term).

**Proposed mitigations (owner's design decision):**
1. **Single shared lock** for P2-1: instead of one `.name-*.lock` per name,
   serialize the name-check+record-write in a single `.names.lock`
   (1 file, permanent) held only around the *short* critical section (not the
   heavy image/rootfs work). Eliminates the per-name name-locks; minimal
   contention. Requires moving the `save` inside that lock after the heavy work.
2. **Safe GC of dead lock files:** a pass (in the style of `startup-cgroup-gc`)
   that, under a global lock, prunes `.{id}.lock`/`.name-*.lock` with no live record nor
   process holding them. Also closes the legacy per-id case.

## Coverage and honest limits

- **Covered:** the daemonless CLI path (create/run/stop/rm) — fd/memory/cgroup/state/process
  leaks and latency, over 330 cycles.
- **NOT covered:** (a) the **long-lived servers** (`delonix-cri`,
  `delonix-node-api`, `delonix-mgmt`) that run continuously on a node —
  they need their own sustained battery (server startup + churn via
  CRI/gRPC + RSS/fd over hours); (b) real long duration (hours / 10⁴–10⁶
  cycles) — this test is 330 cycles (minutes). **No "no leaks" claim**, but
  rather "no leak observed over 330 cycles, with fds/RSS/leaves/records flat".
- Comparison with the v5.1.1 baseline under equivalent conditions: not done (left for
  the sustained E2E on a dedicated node).
