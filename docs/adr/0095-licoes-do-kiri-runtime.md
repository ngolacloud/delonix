# ADR-0095 — Lessons from the Kiri runtime: what we adopt, what we don't, and the only real gain

**Status:** Proposed (2026-11-18) — analysis of the **Kiri** runtime
(`github.com/libracoder/kiri`, Docker-compatible container runtime in Rust) at
the owner's request, to see what we can extract from its ~2 ms startup. Conclusion:
delonix **already implements, and well, all of Kiri's startup techniques that
respect its philosophy**; the only real gain missing is the **warm plane/pool**
— work for its own slice, not a drop-in. **Nothing to implement in this ADR.**
The owner's GO is what makes it Accepted. **Relates to:** ADR-0040 (multicall binary,
daemonless), ADR-0094 (isolated network by default), ADR-0056 (`volatile` overlay).

> **The decision, in one line:** we adopt **zero startup changes** from Kiri (the
> delonix already does the safe equivalent), we **reject** the warm-VM/persistent-agent
> model as the default (it breaks daemonless) and `clone3`+`CLONE_INTO_CGROUP`
> (risky rewrite for a sub-ms gain on an already-resolved race), and we **pursue**
> only the **warm plane/pool** (holder netns + IPAM + slirp) on the
> persistent paths (`serve cri`/`serve api`) and/or opt-in — which is already item nº 1 on the
> optimization list in the README.

## Context

Kiri advertises ~2 ms container startup and the owner asked us to extract lessons
for delonix. The analysis (real code from both) unpacks the number and separates what
transfers from what doesn't.

### What Kiri's ~2 ms REALLY measures

From Kiri's `bench/api.sh` (`hyperfine --warmup 3 --runs 30`, `alpine true` via
**Docker Engine API**):

| | Kiri | Docker Desktop | OrbStack |
|---|---|---|---|
| Docker Engine API | **2,55 ms** | 213 ms | 380 ms |
| docker CLI | 14,2 ms | 243 ms | 442 ms |

Read honestly:

- **The VM and the agent are already warm.** The microVM boot (~100 ms) and the image
  pull/unpack are **outside** the stopwatch. The 2,55 ms are `create+start+wait+delete`
  with the VM alive, the agent (PID 1, ~900 KB) alive and the layers already unpacked.
  The VM only sleeps after 60 s of inactivity.
- **It is the API path, not the CLI.** Kiri's CLI costs 14,2 ms — the overhead is the
  *`docker` binary itself*, not the runtime.
- Therefore it is **not directly comparable** to delonix's ~91 ms cold startup
  (`docs/measured-comparison.md`): delonix is **daemonless** (ADR-0040) —
  each `container run` is an ephemeral process that re-pays process startup +
  overlay mount + namespaces + supervisor fork + exec. Kiri reuses
  everything in a persistent process.

The **startup techniques**, those, do transfer — and delonix already has them.

### What Kiri does well and delonix ALREADY implements

| Kiri technique (`crates/kiri-init/src/linux/`) | Status in delonix |
|---|---|
| `clone` for namespaces, no runc/shim/dockerd | ✅ `clone_init`/`clone_flags` (`crates/adapters/delonix-linux/src/lib.rs`) — direct clone, zero runc/shim |
| Heavy work BEFORE the clone (CStrings, ordered mounts, pre-generated `/etc/*`) | ✅ `inputs`/`IoSpec` assembled in the parent before `clone_init` |
| Readiness pipes instead of polling (`root_ready`, `net_ready`) | ✅ `ready_w`, `sync`, `cg_sync` (`StartPipes`) |
| Cgroup placement race (parent writes `cgroup.procs` vs child exec) | ✅ **already resolved** by the `cg_sync` barrier — the child waits for placement before exec |
| Private cgroup view (`CLONE_NEWCGROUP`) | ✅ `unshare(CLONE_NEWCGROUP)` after placement (Docker/Podman `--cgroupns=private`) |
| Tuned overlay | ✅ `volatile` (ADR-0056); `mount_overlay_from_lowers` is minimal, no per-mount feature probing |
| Tiny init agent that only reaps/forwards signals | ✅ `supervise.rs` (ephemeral per-container supervisor, `conmon` model) |

## Decision

### Not to adopt

1. **Warm-VM / persistent agent as the default model.** It is the real source of
   Kiri's 2 ms, but it **breaks the «daemonless by design» guardrail** (ADR-0040): the
   delonix has no (nor wants an) always-present daemon on the CLI path. Kiri's
   persistent model is a different product choice; copying it as the
   default would trade delonix's philosophy for latency that the cold CLI does not
   need to have.
2. **`clone3` + `CLONE_INTO_CGROUP`.** It would place the container in the cgroup
   atomically, dispensing with the `cg_sync` barrier. But:
   - the race it would resolve is **already resolved** by `cg_sync` (a correctness fix, not a
     micro-optimization);
   - delonix uses `nix::sched::clone`, not `clone3` — adopting it means **rewriting the
     most dangerous path in the runtime** (userns/console/pidns/`cg_sync`) for a
     **sub-ms** gain irrelevant against the ~91 ms;
   - it is risk for almost nothing. **It stays out.**

### To pursue (future work, its own ADR + e2e-gated slice)

3. **Warm plane/pool** — holder netns + IPAM + `slirp4netns` pre-built and
   reused. This is where the real leap is (~91 ms host → ~216–344 ms isolated network
   cold, because rootless rebuilds veth/IPAM/slirp from scratch) and **it is already item
   nº 1 on delonix's optimization list in the README**. It respects daemonless if
   it is applied **only to paths that already have a live process** (`serve cri` for the
   kubelet, `serve api`) and/or as an **opt-in pool** — the cold CLI stays the same.
   **Measure first** (where the ~91 ms go) before designing the pool.

## Consequences

- **Zero code changes in this ADR.** delonix gains nothing by copying
  startup techniques it already has, and loses (risk) by rewriting the `clone`.
- The optimization path is left **named and justified** for a future session
  (warm plane), with the explicit philosophy boundary: never on the cold CLI.
- It serves the competitive narrative (adoption hole nº 1): delonix knows
  exactly where it stands against a fast competitor, and why, with evidence.

## Alignment with the guardrails

- **Daemonless by design:** reaffirmed — we reject the warm-VM-by-default because of this.
- **No silent failure / GO-NO-GO before a new privilege boundary:** the warm
  plane does not open a new boundary (it reuses infra that rootless already uses), but it enters via
  its own ADR + e2e-gated slice, not here.
