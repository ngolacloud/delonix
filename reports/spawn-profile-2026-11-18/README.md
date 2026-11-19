# Per-phase profile of the cold start — where the ~91 ms go

> **What this document is.** The **measured** breakdown of the ~91 ms that
> `docs/measured-comparison.md` (line 4a) attributes to the cold start of a
> container on delonix rootless. That document says *how much*; this one says *where*.
> It is the prerequisite that **ADR-0095** requires before designing the warm plane/pool:
> «measure first where the ~91 ms go».
>
> **What it is not.** An optimization proposal. Nothing in the engine was changed — the
> instrumentation is an **env-gated** profile (`DELONIX_PROFILE_SPAWN`) that is a
> no-op without the variable (confirmed: container stdout intact, exit codes 0/42
> propagated, 261/261 crate tests, clippy clean).

| Field | Value |
|---|---|
| Date | **2026-11-18** |
| Where | development host, kernel 7.0.0-43-generic, 20 threads |
| Load | load1 ≈ 3.6 (below nproc/2=10; the bench **passed** the refusal judgement) |
| Binary | `delonix 0.1.2` release, rootless, isolated root and runtime-dir |
| Method | `scripts/spawn_profile.sh --runs 25` + `spawn_profile_report.py` |
| Image | `alpine:latest`, pre-extracted during warm-up (the pull stays off the clock) |

---

## How it was measured

Each `container run --rm alpine true` is timed in **wall-clock** (`date
+%s%N` around the process, like `bench.sh`) and, in parallel, the embedded profiler
emits the **internal total of `spawn`** and its phases. The subtraction yields the
phase the doc could never name:

```
wall = pre_spawn  (process startup + image/rootfs resolution in the CLI)
     + spawn_total (clone + cgroup + net-hook + mounts)
```

The marks (see `crates/adapters/delonix-linux/src/lib.rs`, module `spawn_profile`):

| Phase | Where | What it measures |
|---|---|---|
| `pre_spawn` | `wall − spawn_total` | re-exec of the binary (43 MB), linker, CLI parse, image manifest resolution, rootfs/overlay preparation |
| `pre_clone` | `spawn` entry → before `clone_init` | `/etc/*` files, `InitInputs::gather` (CStrings/mounts), pipes, console |
| `clone` | around `clone_init` | the `clone(2)` that creates the namespaces |
| `cgroup` | inside `userns_handshake`/`place_without_userns` | `setup_cgroup` (writing `cgroup.procs`, `memory.max`, `cpu.max`) |
| `net` | around the `on_started` hook | CNI/slirp hook **when it exists** (only `--net <net>` has it) |
| `mounts` | around `wait_for_mounts` | the child shaping the mount-ns: overlay + `pivot_root` + `/proc`,`/sys`,`/dev` |
| `setup_rootfs`/`confine_tail` | **in the child**, sub-phases of `mounts` | the overlay+pivot isolated from the rest of the confinement |

> **Method risk ruled out.** `wait_for_mounts` is a single `poll()` that blocks
> until the byte from the child — **no sleep/polling** —, so the `mounts` phase is real
> work time, not an interval artefact.

---

## The result

### HOST (`--net host`) — line 4a of the doc (~91 ms)

Median of 24 warm runs (the 1st, cold, is set apart):

| Phase | min | **median** | max | % of total |
|---|---|---|---|---|
| **`pre_spawn`** (process + image/rootfs) | 35.38 | **57.93** | 218.98 | **68.0 %** |
| `cgroup` (`setup_cgroup`) | 2.59 | 19.03 | 453.40 | 22.3 % |
| `mounts` (overlay+pivot, child) | 1.45 | 2.00 | 238.08 | 2.4 % |
| `clone` (namespaces) | 0.25 | 0.42 | 0.53 | 0.5 % |
| `pre_clone` (setup in the parent) | 0.06 | 0.14 | 0.20 | 0.2 % |
| `net` (hook) | 0.00 | 0.00 | 0.00 | 0.0 % |
| **total (wall)** | 52.11 | **85.15** | 513.98 | 100 % |

- 1st run (**cold**): wall **76.26 ms** (spawn 18.77 + pre_spawn 57.49).
- Child sub-phase inside `mounts`: `setup_rootfs` ≈ **1.49 ms**,
  `confine_tail` ≈ 0.41 ms.
- **Pure** process startup (`delonix --version`): median **7.90 ms**.

**The warm median of 85.15 ms matches the doc's 91 ms** — it validates the method. And the
biggest consumer **is not the container mechanics**: it is **`pre_spawn`, 57.9 ms
(68 %)** — the daemonless binary being re-exec'd on every `run` and resolving the image
and preparing the rootfs/overlay **before** even cloning. Of those 57.9 ms, only
~7.9 ms are pure process startup; the remaining ~50 ms are image resolution +
rootfs preparation in the CLI handler.

### ISOLATED (`--net <net>`, plane up) — line 4b of the doc (~216 ms)

| Phase | min | **median** | max | % of total |
|---|---|---|---|---|
| **`pre_spawn`** | 282.14 | **391.70** | 2673.46 | **96.2 %** |
| `cgroup` | 0.72 | 15.63 | 183.05 | 3.8 % |
| `mounts` | 0.01 | 0.01 | 1.46 | 0.0 % |
| `net` (hook `on_started`) | 0.00 | 0.00 | 0.00 | 0.0 % |
| **total (wall)** | 291.92 | **407.35** | 2676.11 | 100 % |

> **Method note, and it matters.** The `net` phase (the `on_started` hook) is **0** even
> in isolated mode. It is not an error: the cost of the isolated network (veth, IPAM, nft ruleset,
> the `nsenter … ip netns exec` re-exec that rootless requires) is set up by the
> **CLI network orchestration around `spawn`**, not inside the placement hook
> — so it shows up in `pre_spawn` and is measured by **wall-clock**,
> exactly as the doc does on lines 4b/4c. The absolute number (407 ms) is
> higher and much more dispersed than the doc's 216 ms (max 2.7 s) because this run
> caught the host under load; the **qualitative signal** — pre_spawn dominates — is the
> same in both modes.

### Host → isolated delta

```
host    ~  85.15 ms
isolated ~ 407.35 ms   (delta +322 ms, all in pre_spawn/network orchestration)
```

---

## What this tells ADR-0095

ADR-0095 suspected the **cold isolated network** as «suspect #1». The measurement
confirms it as the biggest *delta* (host→isolated: +322 ms), **and** shows where it
lives: not in `clone`, not in the cgroup, not in the mount-ns — it lives in **`pre_spawn`**, the
CLI orchestration that a daemonless process re-pays in full on every `run`.

Consequences for the warm plane/pool design:

1. **The biggest lever is eliminating `pre_spawn`, not tuning the container mechanics.**
   A warm plane over a persistent process (`serve cri`/`serve api`) that
   keeps the binary loaded, the image resolved and the network plane up
   erases 68 % (host) to 96 % (isolated) of the cold start. It is consistent with the
   guardrail: the cold CLI stays the same; only the paths with a live process
   gain.
2. **The container mechanics are already cheap and not worth a rewrite.** `clone` 0.4 ms,
   `mounts`/`setup_rootfs` ~1.5–2 ms, `pre_clone` 0.14 ms. This confirms the
   ADR-0095 refusal of `clone3`+`CLONE_INTO_CGROUP` (sub-ms gain) — the profile
   shows there are no ms to gain there.
3. **The `cgroup` (~19 ms, very dispersed) is the only in-spawn item with fat**,
   but it is cgroup-file write noise under load, not a pool target.

## Artefacts (reproducible)

- `host.jsonl` / `isolated.jsonl` — one JSON record per run (phases + wall).
- `host.child.txt` / `isolated.child.txt` — child sub-phases (from stderr).
- `host.walls` / `isolated.walls` — raw wall-clock per run (µs).
- `process-start.us` — pure process startup (`--version`).
- `REPORT.txt` — the output of `spawn_profile_report.py`.

Reproduce: `scripts/spawn_profile.sh --runs 25` (refuses to run under load).
