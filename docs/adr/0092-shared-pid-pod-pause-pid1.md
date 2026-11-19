# ADR-0092 — A `shareProcessNamespace` pod needs a pause process as PID 1

**State:** Proposed — **IMPLEMENTED and PROVEN** (2026-11-08). The `critest` PodPID
passes on rootful-VM+CNI with the pause as PID 1; the suite rose to **102/103** (only
`rshared` remains, ADR-0093). Awaiting owner GO for Acceptance. Identified and measured in
the CRI conformance campaign that took `critest v1.36.0` from **84 → 102/103** on
rootful-VM+CNI
(`reports/cri-conformance-0.1.1/00-baseline-develop.md`). The PID-sharing path
on root-CNI was already wired (unnumbered ADR in this campaign: `apply_shared_pid_rootful`
in `lifecycle.rs`, which closed HostNetwork×2/HostIpc×2/portforward-host), but the
**PodPID** spec failed for a distinct architectural reason described here — now
**implemented and proven** (see "RESOLVED" at the end). Awaiting owner GO for Acceptance
(adds a per-pod shared pause process + a persisted state field).
**Relates to:** ADR-0020 (pod envelope/semantics), ADR-0093 (`rshared`, the only
spec remaining), `reports/cri-conformance-0.1.1/00-baseline-develop.md`.

## Context

The `critest` **PodPID** spec (`cri-tools` v1.36, `pkg/validate/security_context_linux.go`)
creates a pod sandbox with `namespace_options.pid = POD` (k8s `shareProcessNamespace`),
starts an nginx container, creates a second container (which inherits the pod's POD mode),
and inside it runs:

```
cat /proc/1/cmdline
```

The assertion is:

```go
Expect(o).ToNot(ContainSubstring("master process"))
```

In other words: **in a shared-PID pod, the namespace's PID 1 may NOT be a container's own
process** (nginx would be `nginx: master process …`). It's the contract the k8s field itself
documents: *"the first process in each container will not be
assigned PID 1"*. In `containerd`/CRI-O that PID 1 is the **pause process** (the
"sandbox container") that creates and holds the shared PID namespace; the
real containers join it and get higher PIDs.

Delonix **has no** per-pod pause process. By rootless design, the *ingress
holder* is per-UID and serves MANY netns, so the code chooses as the "anchor" the
**first live container** of the pod (`sandbox_shared_pid_anchor` in `lifecycle.rs`,
comment at ~1880: *"There is no per-pod 'pause' process here … so the anchor is
the first started container"*). Consequence: in the shared-PID pod, PID 1 is the
init process of the **first container** (nginx) → `cat /proc/1/cmdline` returns
`nginx: master process …` → the spec fails.

Sharing itself **works** (the second member joins the anchor's namespace via
`enter_pod_pidns`, proven: 0 refusals of `shareProcessNamespace` on the VM, was 6). What
fails is **who is PID 1**.

## Decision

For pods with `shareProcessNamespace` (`namespace_options.pid == POD`), introduce
a **minimal per-pod pause process** that:

1. creates the shared PID namespace (`CLONE_NEWPID`) and sits as **PID 1** in it,
   doing nothing else (a `pause`: blocks on a signal, reaps orphans);
2. is the **anchor** that pod members now join — `pod_infra_pid` is no longer
   "the first live container" but **the pause's PID**;
3. has a lifecycle tied to the **sandbox** (born in the shared pod's `RunPodSandbox`,
   dies in `RemovePodSandbox`), surviving any member — the
   kernel keeps the namespace alive while the pause stays in it, so containers
   coming and going don't collapse it (this also resolves the fragility of
   anchor-is-container: if the first container died, the namespace survived only by chance).

Members keep joining with the EXISTING plumbing (`pod_shared_pid` +
`pod_infra_pid` → `enter_pod_pidns`, `setns(CLONE_NEWPID)`); only **the anchor's
identity** changes (pause instead of first container) and **who creates it**
(`RunPodSandbox`, not the first `StartContainer`). The fix already made for
root-CNI (`apply_shared_pid_rootful`, root joins without a userns holder) stays and
composes: the pause is the `setns` target in both modes.

### Scope and modes

- **Rootful (Kubernetes node):** the pause runs as root; `setns(CLONE_NEWPID)` is
  privileged without any userns trick. This is the case `critest`/kubelet exercise.
- **Rootless:** the pause runs in the pod's userns (as the re-exec'd member does
  today), so `setns` keeps the privilege within the namespace the pause owns.

### What does NOT change

Pods **without** `shareProcessNamespace` (the default, per-container PID) don't gain a pause
— each container has its own `CLONE_NEWPID`, as today. The pause's cost is paid only
by pods that ask for PID sharing.

## Alternatives considered

1. **Anchor = first container (current).** Zero extra processes, but PID 1 is the
   container → violates the CRI contract and fails PodPID. Rejected for conformance.
2. **Don't support `shareProcessNamespace` (refuse it).** Honest but regresses
   a real k8s feature (sidecars signalling their peer). Rejected.
3. **Per-pod pause (this decision).** Aligns with `containerd`/CRI-O; cost of a
   trivial process per shared-PID pod; reuses all the existing `setns` plumbing.

## Consequences

- **+** Conforms to PodPID and gives the correct PID 1 semantics (no container
  process is PID 1); makes the shared namespace robust to members coming and going.
- **−** One extra process per pod with a shared PID (minimal/ephemeral memory; it's a
  `pause`). New lifecycle to manage in the sandbox (create/tear down, reconciliation
  after a crash — the pause must be reaped and recreated with the sandbox).
- **Compat:** no public contract (CLI/JSON/gRPC/state) changes shape; it's
  internal behaviour of the shared-pod path. The persisted `pod_infra_pid`
  now points to the pause — migration note: shared pods created by the
  previous version (anchor=container) don't persist across restarts (pod state is
  ephemeral), so there's no incompatible state to migrate.

## Validation required before Acceptance

- **Behaviour:** `critest` PodPID passes (PID 1 = pause, not nginx), **and** the
  positive path stays green (the second container SEES nginx at a PID > 1).
- **Unit:** the anchor resolves to the pause; `enter_pod_pidns` joins the pause's PID;
  non-shared pods don't gain a pause.
- **Recovery:** tearing down/recreating the pause with the sandbox; no member left
  orphaned in a dead namespace.

## Implementation notes (GO investigation, 2026-11-08)

Survey of the code the build has to touch, and the confirmed fork:

- **Native (non-CNI) path:** there's already an infra container `pod-cri-<id>`
  ("pause"-style) that holds the netns (`run_pod_sandbox`, `lifecycle.rs` ~1155; its
  PID is read at ~1514). A candidate to ALSO hold the shared PID namespace
  — but today it serves networking, not PID.
- **Root-CNI path (the one `critest` exercises):** **there is no per-pod process** —
  the network comes from CNI plugins on the host; there's nowhere to anchor PID-1.
- **Rootless-CNI:** the netns is served by the per-UID ingress holder (shared
  across many pods), so it also doesn't serve as a per-pod PID anchor.

**Implementation decision:** a **dedicated, uniform pause** per sandbox with
`shared_pid` (not reusing `pod-cri-<id>`, which only covers the native path and
mixes networking/PID responsibilities). The pause is a minimal process (`CLONE_NEWPID` +
a child that `pause()`s), spawned in `RunPodSandbox`, torn down in `RemovePodSandbox`.

**Surfaces to touch (and the care each requires):**
1. **PERSISTED STATE — `SandboxRec` gains `pause_pid: Option<i32>`** (and maybe the
   PID-ns inode): a serialization change → **migration note** (`#[serde(default)]`,
   old sandboxes without the field = no pause, previous behaviour) and compat with the
   on-disk state. It's a product invariant.
2. **Spawning the pause** in `RunPodSandbox` (a new primitive — there's no
   "spawn a process that holds a PID-ns" today; the closest pattern is the build's
   keep-alive `sleep infinity`, `bins/delonix/src/cmd/build.rs`).
3. **Anchor** = `pause_pid` instead of `sandbox_shared_pid_anchor` (first
   container) — `apply_shared_pid`/`apply_shared_pid_rootful` now receive the pause's PID.
4. **Teardown** in `RemovePodSandbox` + **reaping**; **post-crash reconciliation** (the
   pause must be recreated/cleaned up with the sandbox, without leaving the namespace orphaned).
5. **Validation:** `critest` PodPID (PID 1 = pause) + the positive case (member sees nginx
   at a PID > 1) + recovery, on a rootful VM.

**Recommendation:** since it changes persisted state and touches pod isolation, it's a
front of its own (slice by slice, behaviour test before removing the previous mechanism
— ADR-0088 pattern), not a patch at the end of a campaign. The path is
nailed down here; what's missing is dedicated engineering time + the validation critest run.

### Implementation status (2026-11-08, branch `feat/cri-conformance-rootful`, WIP)

DONE and validated: (1) the **pause primitive** `delonix_linux::run_pid_namespace_anchor`
+ hidden arg `delonix-cri __pid-anchor` — a live smoke test on the VM confirms it is
**PID 1 in a PID-namespace distinct** from the host (`NSpid: <host> 1`), with stdio
disconnected (otherwise `Command::output()` hangs waiting for EOF); (2) lifecycle —
`SandboxRec.pause_pid` (serde(default)), spawn in `RunPodSandbox` (shared_pid +
rootful), teardown (`SIGKILL`) in `RemovePodSandbox`; (3) anchor = `pause_pid` for
ALL members. critest **100/103** (up from 99).

**NAILED-DOWN BLOCKER — PodPID still fails** (`/proc/1` is still nginx):
`Launch::pod_infra_pid()` and `pod_shared_pid()` (`launch.rs:68-81`) are **gated on
`second_pass`** — the PID join only fires on RE-EXEC (native `--pod` path, which
enters the holder's userns). The **root-CNI** path runs `delonix __apirun` in a
**single pass** (`second_pass=false`, `container.rs:4040/5708`), so the join never
fires: `clone_flags` gives the member `CLONE_NEWPID` and nginx becomes PID 1. The
pause is spawned but **nobody joins it**.

**Precise fix (still to do):** it's NOT enough to remove the gate — `pod_infra_pid` also
controls IPC/UTS (`clone_flags`: `join_pod` suppresses `NEWIPC`/`NEWUTS`; RunSpec
`has_own_ipcns`), so ungating it would regress IPC/UTS isolation on root. A
**PID-only dedicated field** `pod_pidns_pid: Option<i32>` is needed, ungated, threaded
`RunOpts → Launch → launch_spec → RunSpec`, used ONLY in (a) `clone_flags`
(suppress `CLONE_NEWPID` when `Some`) and (b) `enter_pod_pidns` (`setns` on that pid),
without touching IPC/UTS. `apply_shared_pid_rootful` now sets `opts.pod_pidns_pid` instead
of `pod_shared_pid`/`pod_infra_pid`. **High blast radius** (`clone_flags` affects ALL
container startup) → requires a full critest re-validation on a VM. Files:
`run_opts.rs`, `launch.rs` (+ constructions in `bins/delonix/src/cmd/container.rs:4040,5708`),
`launch_spec.rs`, `lib.rs` (RunSpec + clone_flags + enter_pod_pidns).

### RESOLVED (2026-11-08) — `pod_pidns_pid` field + validation

The `second_pass` blocker was closed with the dedicated field **`pod_pidns_pid`**
(PID-only, decoupled from `pod_infra_pid`/IPC/UTS), threaded `RunOpts → Launch
(ungated accessor) → launch_spec → RunSpec` and honoured in `clone_flags` (suppresses
`CLONE_NEWPID`) + `enter_pod_pidns` (`setns` on the pause). `apply_shared_pid_rootful`
now sets `pod_pidns_pid`. **PROVEN:** `critest` PodPID **passes** on rootful-VM+CNI
(`/proc/1` = the pause `delonix-cri __pid-anchor`, not nginx), suite **102/103**
(only `rshared` remains, ADR-0093). Tests: compute `launch` 6/6, cri 4/4.
