# Per-image warm-plane warming on the CRI (design note, ADR-0095)

Status: **design only.** No `serve cri` wiring ships in this slice — the warm
isolated path is proven in the `__poolbench` harness (`scripts/warm_profile.sh
--net <network>`). This note is the prerequisite the session contract requires
*before* any CRI wiring, because the naive single-hint pool does not fit a
kubelet.

## The problem the harness hides

`WarmPool::new(bin, hint, …)` pre-warms **one** image: each parked runner resolves
`hint` and keeps its layers hot before announcing READY. The bench is fair because
it runs one image (`alpine`) many times, so every start hits a runner warmed for
exactly that image.

A kubelet does not. One node runs dozens of distinct images concurrently (the
pause image, CNI/CSI side-cars, and every workload image). A pool warmed for image
A serving a start for image B gives back **only part** of the win:

- It still saves the 43 MB binary re-load and the holder residence (the isolated
  path's cold `netns run` child never happens — that is the +322 ms this work
  removed, and it is image-independent).
- It does **not** save B's image resolve + layer warm, because the runner warmed
  A. B's start re-resolves B in-process (page cache cold for B's layers).

So a single-hint pool on the CRI would leave the per-image half of the `pre_spawn`
on the table for every image that is not the one hint.

## What is per-image and what is not

Measured split on the isolated path (`run_spec_resident_isolated`):

| Cost | Scope | Warm plane removes it? |
|---|---|---|
| 43 MB binary re-load (cold `execve`) | per-start, image-independent | yes — runner is resident |
| cold `nsenter … netns run` child | per-start, image-independent | yes — runner `setns`'s in-process |
| `ImageStore`/`Store` open | per-start, image-independent | yes — resident stores |
| image record read + config parse | **per-image** | only if the runner warmed THIS image |
| layer page-cache warmth | **per-image** | only if the runner warmed THIS image |
| `attach_container` (IPAM + veth) | **per-container** | no — irreducible, runs host-side per start |
| `setns` + spawn mechanics | per-start | n/a (this IS the start) |

The decisive observation for the CRI: **warming is keyed by IMAGE, not by network.**
The host side runs `attach_container(id, net, …)` per container (its own veth +
leased IP) and the runner only `setns`'s into the holder-built netns. One
holder-resident runner warmed for image X therefore serves a start of X **on any
custom network** — there is no per-network pool. This keeps the fan-out to
`O(images)`, not `O(images × networks)`.

## Proposed shape

A keyed set of sub-pools, each a `WarmPool` warmed for one image and (for the
isolated path) launched with the holder's `nsenter` prefix:

```text
WarmPlane {
    isolated_prefix: Vec<OsString>,          // nsenter -t <pin> -U -m -n -- (once holder is up)
    pools: LruMap<ImageRef, WarmPool>,        // bounded; one sub-pool per hot image
    total_cap: usize,                         // ceiling on parked runners node-wide
}
```

- **Fill signal.** `PullImage` is the natural trigger: the image is now local and
  about to be run. Create/grow its sub-pool there. The pod spec (`RunPodSandbox`
  → container images) is a second signal for pre-fill.
- **Pick on start.** `CreateContainer`/`StartContainer` looks up the sub-pool for
  the container's image; a hit serves warm, a miss serves a cold start AND seeds a
  sub-pool for next time (warm-on-second-use), so an unknown image is never worse
  than today.
- **Bound + evict.** `total_cap` caps parked runners across all images; an LRU
  over `ImageRef` evicts the coldest image's sub-pool when a new hot image needs
  room. Parked runners cost one resident process each (binary + open stores + hot
  layers), so the cap is a real memory lever and must be configurable.
- **Refill off the hot path.** Each sub-pool refills on a background thread (the
  bench refills synchronously between timed runs; a server must not).
- **Holder lifecycle.** The isolated prefix depends on the pin PID
  (`infra::status().holder_pid`). The holder is ref-counted (`acquire`/`release`):
  a warm plane that parks runners in it must hold a ref for as long as the pool
  lives, or a last-container `release` could tear the netns out from under parked
  runners. Pin-restart (`ensure_up_locked`'s adopt path) changes the PID — the
  plane must re-derive the prefix and re-park, or detect the stale PID on a failed
  `setns` and rebuild the sub-pool.

## Open questions for the wiring session

1. **Sizing policy.** Fixed size per image, or demand-scaled from recent start
   rate? A pause-image pool wants depth; a one-shot job image wants none.
2. **Eviction vs. pin ref.** Evicting a sub-pool must `release` its holder ref and
   reap its runners cleanly (serve-one-then-exit means idle runners are just
   parked processes; killing them is a plain `kill` + `wait`).
3. **Digest vs. tag keying.** Key on resolved digest (correct across `:latest`
   re-tags) rather than the reference string, so a re-pulled `:latest` does not
   serve stale layers.
4. **Per-image hint cost.** Warming resolves the image eagerly; a burst of new
   images at pull time could stampede. Rate-limit sub-pool creation.

Until this is wired, the CRI keeps its current cold path; the warm plane lives in
the measurement harness only (ADR-0040 daemonless, ADR-0095 opt-in-on-live-process).
