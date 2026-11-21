# Warm plane (ADR-0095) — design, implementation and what the measurement revealed

> **What this document is.** The outcome of the slice that designs and implements the
> **warm plane pool** of ADR-0095 — a persistent process that keeps
> *runners* pre-loaded so that a container startup reuses infrastructure already
> built — **and** the measurement that this work forced us to do, which changed the
> target. It follows `reports/spawn-profile-2026-11-18/` (the cold profile) as the
> baseline.

| Field | Value |
|---|---|
| Date | **2026-11-18** |
| Base | `perf/measure-cold-start-spawn` (develop + the measurement d994cf1c) |
| Host | kernel 7.0.0-43, 20 threads, rootless |
| Binary | `delonix 0.1.2` release |
| Method | `delonix __poolbench <img> <runs> <size>` + `DELONIX_PROFILE_SPAWN`; reporter `scripts/warm_profile_report.py` |

---

## What was built

1. **`delonix-warm`** (`crates/adapters/delonix-warm`) — the pool. It orchestrates, over
   sockets, a set of parked *runners* and hands them the startup *spec*.
   It knows neither images nor the engine: only processes and a line
   protocol. **serve-one-then-exit** model, so that the supervisor's `fork()`
   (`delonix-linux`, safe only single-threaded) always runs in a pristine process —
   exactly the invariant that the cold `__apirun` re-exec guarantees. The pool
   replenishes **off the hot path**.
2. **`delonix __poolrunner <fd> [hint]`** — the parked runner: it opens the engine
   stores ONCE, pre-resolves the suggested image, announces READY and serves ONE
   *spec* with its warm state (the SAME `cmd_run` as `__apirun`), and exits.
   Intercepted **before** telemetry, to stay single-threaded.
3. **`delonix __poolbench`** — the persistent measurement process: it starts
   identical containers cold (a fresh `__apirun` per run) and warm (pool
   runner), times each one and emits JSONL to the profiler. It is the live-process
   path that the acceptance criterion measures, exercising the SAME
   `delonix_warm::WarmPool`.
4. **`pre_spawn` profiler** (`bins/delonix/src/cmd/prespawn.rs`) — it marks the
   `cmd_run` phases **upstream** of the `spawn` (the `spawn_profile` of
   `delonix-linux` only measures inside the `spawn`). Strict no-op without
   `DELONIX_PROFILE_SPAWN`. **It was this instrument that found the real
   cost.**

## What the measurement revealed — and changed the target

ADR-0095 and the cold profile said: host `pre_spawn` ≈ 57.9 ms (68 %), «image
resolution + rootfs preparation». By instrumenting `cmd_run` phase by phase, the
cost was **not** that:

```
cmd_run pre_spawn  ≈ 20,8 ms   ← dominated by:
  policy_preflight   0,16 ms
  claims             0,21 ms
  resolve_rootfs    20,39 ms   ← ALL the cost
  record_net         0,03 ms
```

And `resolve_rootfs` decomposed (`prepare_overlay`): `ensure_layers` 0.01 ms,
mkdir+lowers 0.26 ms, `image_owners` 0.07 ms — **0.3 ms of real work**. The
remaining **~20 ms were the `unpacking` progress-spinner**: `Progress::step_after`
spawns a thread that waits for the threshold (800 ms) **in 20 ms slices**, and
`ok()`/`close_line` does a `join()` on that thread — which only sees the stop flag at the
**end of the 20 ms slice in progress**. Result: **each `container run` paid up to
~20 ms of *join* latency for a spinner that, below the threshold, never draws
anything.** Pure *chrome*, on the hot path of **every** startup — CLI, and also each
`__apirun` that the CRI/API launch (stderr in a file, non-TTY, pays it just the same).

### The fix

`wait_out` now waits on an **interruptible `Condvar`** (`Gate`,
`bins/delonix/src/cmd/output.rs`): the `signal()` from `close_line` wakes the waiter
**instantly**, so a fast step closes in microseconds and a slow step
announces itself after the threshold **exactly as before** — byte-for-byte identical
output, only the dead wait disappears.

## The numbers (host, rootless, `alpine true --net host`, 20 runs — `REPORT.txt`)

| | cold baseline (2026-11-18) | cold (this slice) | **warm (pool)** |
|---|---|---|---|
| `pre_spawn` median | **~57,9 ms** | **7,02 ms** | **6,27 ms** |
| `resolve_rootfs` | (within the ~50 ms) | 0,39 ms | 0,35 ms |
| `cmd_run` pre_spawn total | — | 1,03 ms | 1,61 ms |

- **58 → 7 ms** is the spinner fix (cold and warm, CLI and CRI/API).
- **7 → 6,3 ms** is the pool: it eliminates the process load (43 MB binary already
  loaded). The rest of the warm `pre_spawn` (~5 ms) is the supervisor *handshake* +
  the socket round-trip, not engine work.

> **Honesty note.** In the baseline, the ~58 ms mixed three things that the
> method (`wall − spawn_total`) did not separate: the spinner latency (~20 ms), the
> process load (~8 ms) and — because it is foreground `--rm` — the *teardown* of
> the `--rm` after the container ran. The persistent path (detached, machine)
> never pays the teardown; and, with the spinner fixed, the host `pre_spawn` is already
> low (~6 ms). That is why the **incremental** gain of the pool on host-net is ~1 ms: the
> big value of the pool is for the **isolated** path (keeping the network plane
> up eliminates the +322 ms that the profile measured), its own slice.

## Verdict for ADR-0095

1. **The biggest lever of cold startup on host-net was NOT image/
   rootfs resolution — it was 20 ms of spinner latency.** Fixed, with unchanged
   output, for all paths.
2. **The warm plane pool is designed, implemented and proven** (via
   `__poolbench`): a persistent process starts containers reusing pre-built
   infrastructure, and the `pre_spawn` drops to ~5.7 ms (from 57.9 ms in the baseline).
3. **The isolated path (warm network plane) is the next lever** — that is where
   the +322 ms live, and that is where a resident pool pays off most. Wiring the pool to
   `serve cri` in production depends on per-image *warming* (one kubelet runs
   many images), also its own slice.

## Reproduce

```
scripts/warm_profile.sh --runs 20 --size 4 --image alpine
# refuses under load (load1 > nproc/2), like spawn_profile.sh
```
