# CRI conformance — measured result

> Suite: **cri-tools `critest` v1.36.0** (the upstream one, not ours).
> **Current measurement — 2026-10-24.** `delonix-cri` engine on the **v5.1.1**
> line (`main @ ab4751ac`), **rootless**, Linux 7.0 host, cgroup v2. This is the line of
> code that ships today as **delonix 0.1.0** — 0.1.0 is a version rebrand of
> 5.1.1, with no engine change; the number was **not** re-run against tag
> 0.1.0, but it's the same binary in behaviour. To reproduce: `scripts/critest.sh`
> (or `tests/compat/cri-conformance.sh`, the same suite).

## Result (2026-10-24)

```
Ran 104 of 122 Specs in 511.184 seconds
84 Passed | 20 Failed | 0 Pending | 18 Skipped
```

> Progression measured on suite v1.36.0: **79 (2026-08-25) → 84 (2026-10-24)**, with
> the fixes that landed after August (eviction/`writable_layer`,
> `ReopenContainerLog` following the inode, and the `ImageService` fixes #18/#19).
> The breakdown of the 20 failures as of 2026-10-24, classified by area (10 environment, 2
> rootless architecture, 1 deliberate divergence, 1 decision, 1 to investigate, 5
> real gaps of which 2 already fixed), is in
> `reports/cri-conformance-2026-10-24/00-audit-report.md`.

### Historical baseline — 2026-08-25 (engine v0.63.1), superseded

> The result below (**79/103**) is from **2026-08-25**, engine `delonix-cri`
> **v0.63.1** — twenty versions ago. It stays as a record of the progression and
> the per-area breakdown; the current number is the one above.

```
Ran 103 of 122 Specs in 777.277 seconds
79 Passed | 24 Failed | 19 Skipped
```

> Measured progression: **65 → 69 → 71 → 72 → 77 → 79**.

### The 2026-08-25 recount

This document used to say **77/103** and was from engine **v0.42.2** — twenty-one
versions ago. A conformance number is this project's most visible compatibility
claim, and one no one revalidates stops being a measurement and becomes a
citation.

Re-measured against the SAME suite and the SAME total: **79 passed, 24 failed**. It did not
regress — it improved by two.

**And the recount was, without meaning to, an independent verification.** The `README.rst`
and the site generator had already been publishing `79 Passed | 24 Failed` for a while; it was only
this document — the one with the detail — that had fallen behind. A blind run against
what was written here landed on exactly the same numbers as the public
page. That's worth more than the update itself: the number published to customers
was correct, and now the detail backs it up again.

**A methodological note that nearly ruined the comparison.** The first recount used
`critest` **v1.34.0**, and gave `70 Passed | 26 Failed` out of 96 specs. Comparing that
70 to the published 77 and concluding "regressed by seven" would have been false: they're
different suites, with different totals (113 specs vs. 122). The number only compares
against the same `critest`, which is why the suite version is in the header.

### The 24 that fail today, by area

| area | how many | note |
|---|---|---|
| AppArmor | 9 | the per-container profile is not translated from CRI; the engine has its own (`install.sh` writes `unconfined+userns`) |
| Container Mount Propagation | 3 | `rshared`/`rslave` are not propagated |
| Security Context | 4 | `PodPID`, `RunAsUserName`, nil `SeccompProfilePath`, `NoNewPrivs=false` |
| Image Manager | 2 | pull by digest, and `Uid`/`Username` in `image status` |
| Streaming | 2 | `portforward` and a second streaming case |
| Multiple Containers | 1 | |
| Networking | 1 | port mapping only with the container port |
| Container Mount Readonly | 1 | non-recursive read-only mounts |
| Container OOM | 1 | `OOMKilled`/137 — see the `AGENTS.md` note: the container's cgroup disappears with it, so post-mortem detection is impossible |

> **Always run on a CLEAN root**: with state accumulated across several runs,
> three "preserving attributes"/"Image Identifier Consistency" specs fail due to
> pollution, not conformance — confirmed to pass on a fresh root. The
> `scripts/critest.sh` already creates a new `DELONIX_ROOT` per run.
> **Always run on a CLEAN root**: with state accumulated across several runs,
> three "preserving attributes"/"Image Identifier Consistency" specs fail due to
> pollution, not conformance — confirmed to pass on a fresh root.

**This is not full conformance, and it is not worth writing it any other way.**
The published number is what gives the claim value: "serves a kubelet" is a
claim, "77 of 103 named specs" is a fact someone else can verify.

## The bug the first run found

The first run gave **0 of 122**. Not due to conformance: the server started up
with the default `root` (`/var/lib/delonix`), which a regular user cannot write to,
and every image pull died with `Permission denied (os error 13)`.
One cause, 103 symptoms.

Once the path was fixed, the next run gave **19 of 109** — and the dominant cause
was a real bug:

```
failed to create the ingress sandbox cri-cef8d63cf86e9f4d
```

`delonix-cri` was invoking **`delonix netns attach`**. The v0.30.0 CLI
reorganization moved that command to **`delonix net netns attach`**, with a clean
break and no aliases — a deliberate, documented decision — and this caller was never
updated. In other words: **rootless pod creation had been broken since
v0.30.0**, and CRI is this engine's strongest cloud-native bet.

Two things hid this for months, and the second is the worse one:

* `delonix_detached` sent **stderr to `/dev/null`** and returned a `bool`.
  The message that reached the kubelet named the victim and hid the killer.
  The real error — `unrecognized subcommand 'netns'` — would have said everything in
  the very first pod.
* There was no conformance run at all. A unit test doesn't catch this:
  the command is a `Command::new` for an external binary, and it still compiles fine.

Fixed on both axes: the correct argv, and `delonix_detached_why` propagating the
first stderr line into `Status::internal`. **19 → 65 passes with
one line fixed.**

## Second round — what closed, and the log bug underneath

Five specs closed: `ReadOnlyRootfs`, `MaskedPaths`, `ReadonlyPaths`,
`SupplementalGroups`, and `HostPID`.

**The underlying finding was a log bug, not a security one.** `ReadOnlyRootfs` and
`NoNewPrivs` both failed on "verify log contents": the file existed,
parsed, and the expected line wasn't there. The cause is that `log_shim`
was writing `format!("{ts} stdout {stream} …")` — with a **literal `stdout`**. The container's
two streams went through ONE pipe and all came out tagged `stdout`, and the
variable called `stream` was actually the F/P tag (full line/partial), a
confusion that helped the bug survive. A message like
`touch: /tmp/test: Read-only file system` is stderr by definition, and the suite
compares the stream.

Fixed with a **second pipe, CRI mode only** (the non-CRI format has no tag
and splitting there would just interleave things worse) and a `poll()` over both in the shim.
The old path stays byte-for-byte identical.

**New engine features**: `--group-add` (supplementary groups, applied even
with the container running as root), `--masked-path` (file covered with
`/dev/null`, directory with an empty read-only tmpfs — the runc
technique), `--readonly-path` (bind onto itself + remount RDONLY; passing
`MS_RDONLY` in the first bind is silently ignored by the kernel, the
classic trap), and `--security-opt no-new-privileges=false`.

**Three regressions I both opened and closed myself, all through measurement:**

1. **The fail-closed checker unconditionally required `NO_NEW_PRIVS`.** It was
   correct as long as the engine always set it; the moment a caller could turn it
   off, every such container aborted with 126 — and the symptom was four
   *seccomp* specs failing, nothing named NNP. A fail-closed check has to
   check the REQUESTED policy, or it becomes a second, undeclared policy.
2. **`seccompiler` turns on NNP unconditionally** (`lib.rs:347`), because the
   kernel only lets an UNPRIVILEGED process install a filter with NNP set.
   Installing the same program the raw way with CAP_SYS_ADMIN solves it — that's what
   libseccomp's `SCMP_FLTATR_CTL_NNP=0` does — but there are **two**
   filters (the `clone3` pre-filter and the main one), and routing only one changes nothing.
3. **A warning of mine ended up on `exec`'s stderr.** The fallback message "could not
   keep NNP off" was correct information written in the wrong place: the
   CRI `ExecSync` returns stderr verbatim and the suite compares it against an
   exact string. A well-intentioned diagnostic line failed a dozen unrelated
   specs.

**What was left open on purpose**: `no_new_privs: false` is honoured (measured:
`NoNewPrivs: 0` with `Seccomp: 2`), but the spec still fails for another reason —
it runs as uid 1000 and expects a setuid binary to escalate to 0, and the
engine's `chown_tree_once` hands the whole rootfs to the requested uid, including the
owner of the setuid binary. It's an old design trade-off (images like
Elasticsearch need a writable tree), not an oversight, and changing it
deserves its own decision.

## Third round — the CRI mounts did not exist

**`ContainerConfig.mounts` was read by no one.** Not a single line. A kubelet
mounting configMaps, secrets, emptyDirs, or hostPaths put nothing inside the
container — silently, because nothing errored. Five specs failed because of this and
read as five separate gaps instead of one missing feature.

The same pattern showed up right after in the sandbox's **`dns_config`** and
**`port_mappings`**: accepted by the API, thrown away. It's this layer's
signature — worth grepping for every `ContainerConfig`/`PodSandboxConfig` field
before assuming it's wired up.

Closed with: `Mount.propagation` in the engine (`:rprivate`/`:rslave`/`:rshared` in
`-v`, with the namespace root switched to `MS_SLAVE` **only** when some
mount requests it — an `MS_PRIVATE` cuts all propagation and would turn the flag into a
no-op), `--dns`/`--dns-search`/`--dns-option`, and translating all three in
`delonix-cri`.

**A kernel trap, the same family as `MS_RDONLY`**: the propagation flag cannot
be combined with `MS_BIND` in the same call — passed together it is silently ignored.
It has to be a separate `mount()` call, after the bind.

**A discovery that changes how to read the table**: the 3 propagation specs and the
non-recursive readonly one fail **in the harness itself**, mounting on the host with
`operation not permitted`. They need root for `critest`, not for the engine.
Added to the 9 from AppArmor, that's **13 of the 31 failures that aren't our
gaps**.

## Fourth round — three bugs hiding behind each other

**`exec` was not entering the container's IPC namespace.** The list was
`["user","uts","net","pid","mnt"]`. An `exec` should look like the container seen from
inside, and without IPC it sees the HOST's System V objects and the host's
`kernel.shm*`/`fs.mqueue.*` — precisely the knobs `--sysctl` touches. Measured: a
container created with `kernel.shm_rmid_forced=1` reported `0` through `exec`,
because the value is resolved in the ipc-ns of WHOEVER READS. It read as "the sysctl was not
applied" and sent me off to investigate the sysctl path, which had been correct from
the start. **Four one-line specs** (2 sysctls + 2 HostIpc).

**`apply_sysctls` was throwing away the write error** (`let _ = fs::write`). A
sysctl requested and silently not applied is the worst kind of failure this repo names:
the container runs, the record says the knob is set, and the value inside is
what it was before. Now it reports.

**A pod member carried TWO publication paths.** The netns is the pod's, not
its own — but the code only excluded the `--net <custom>` case, so a
member got a per-container slirp claiming the same port the ingress had already
published. Measured before fixing: the DNAT `10.0.2.100:12345 → 10.200.0.2:80`
was correct, nginx responded 200 from inside the holder, and `curl 127.0.0.1:12345`
from the host hung. Along the way the **fifth** occurrence of the same old
trap showed up: `c.ip` was never assigned to a pod member, so the
record described a container with no address.

**`NetworkReady` was a deadlock.** The condition was `infra.up`, and this engine is
daemonless: the infra netns comes up ON DEMAND. On a freshly booted node this
gave `NetworkReady: false` → the kubelet marked it NotReady → scheduled no pod
at all → nothing ever brought the infra up → permanently NotReady. The check existed
to catch a real SDN failure and instead described the engine's normal resting state.
It now distinguishes by ref-count: infra down WITH workloads is a failure,
down with none is idle.

## The 26 remaining failures, by area

| Area | # | Nature |
|---|---|---|
| AppArmor | 9 | **Environment** — loading profiles needs privilege rootless doesn't have |
| Mount propagation | 3 | **Environment** — `critest` cannot mount on the host without root |
| Non-recursive readonly mount | 1 | **Environment** — same reason |
| Streaming (portforward ×2) | 2 | **Real gap** |
| Seccomp profile path | 2 | One is a custom profile (fail-closed refusal); the other is a **deliberate divergence** — see below |
| Image Manager | 2 | One is `DeadlineExceeded` on a network pull — potentially environment |
| `PodPID` | 1 | `shareProcessNamespace` — touches `spawn()`, its own decision |
| Port mapping with container port only | 1 | **Architecture**: the pod's IP is unreachable from the host under rootless |
| NoNewPrivs | 1 | Honoured, but `chown_tree_once` takes ownership away from the setuid binary |
| Remaining (log reopen, per-label stats, OOM, multi-container) | 4 | **Real gap** |

**13 of the 26 are environment-related** (AppArmor + the 4 mount ones). Of the remaining 13,
two are design decisions still to be made and one is an architectural limit of rootless.

### Deliberate divergence: no profile ≠ no confinement

The "nil profile, which is unconfined" spec requires that a container with NO
declared profile runs with `Seccomp: 0`. That's what containerd does. **Not here**: with no
profile, the engine's built-in allowlist applies, and the value is `Seccomp: 2`.

We're stricter than the spec requires, and that is not going to change just to
win a spec. It's recorded as a known divergence, not a gap.

None of these is an architectural surprise; they're surface not yet written. Setting
aside AppArmor, which is environmental, the biggest remaining blocks are **mount
propagation** (3) and **networking** (3), and the next one with real value for a kubelet is
`shareProcessNamespace`, which was already identified as Phase 3 of the pod
work and touches the ~405-line `spawn()` this repo flags as a high-risk
function.

## What this number does NOT say

* It was not run as **root**. Several failures (AppArmor, HostPID) could flip
  sign there; it was not measured, so no claim is made.
* `critest` does not exercise a real kubelet. A kubeadm cluster booting on
  this engine is proven separately (see `AGENTS.md`, the "REAL KUBERNETES
  CLUSTER RUNNING" section), and it's a different kind of proof — narrower and
  deeper.
* 19 specs were **skipped** by the suite itself (features it detects as
  not advertised). They count neither as a pass nor as a fail.
* `ListPodSandboxMetrics`/`ListMetricDescriptors` (the generic metrics API,
  distinct from the Stats API above) went from empty stub to a real implementation
  AFTER this measurement — they reuse the same cgroup v2 reads
  `ContainerStats`/`PodSandboxStats` already used, with the metric-name ↔
  descriptor pairing verified by a test (the spec defines that a name with no
  matching descriptor is ignored by the caller). **`critest` v1.36.0 does not exercise
  either one** (confirmed by grep in this document) — the validation that
  exists is a real gRPC round-trip (`grpc_status.rs`), not a suite re-run.
  The number above (79/103) **does not change** because of this.
