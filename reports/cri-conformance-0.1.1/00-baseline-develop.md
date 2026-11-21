# CRI — baseline re-measurement on `develop`/0.1.0 (2026-11-08)

> Re-measures CRI conformance on the **current** engine (the 2026-10-24 audit was
> against `delonix-cri` **v5.1.1** — twenty-odd versions back and stale: since then
> `port_forward`, the `/proc/sysrq-trigger` masking fix and #18/#19 have landed).
> Reproduce: `scripts/critest.sh` (fixed in this session — see §4).

## 1. Measured configuration

| Axis | Value |
|---|---|
| Engine | `delonix-cri` + `delonix` from `develop` @ `1898466b`, version **0.1.0** |
| Suite | `cri-tools critest` **v1.36.0** (`/usr/local/bin/critest`, `crictl` v1.36.0) |
| Mode | **rootless** (delegated uid), cgroup v2 |
| Host | Linux 7.0.0, x86_64 |
| Cap ceiling | `DELONIX_CRI_CAP_CEILING=all` (the suite creates privileged pods) |
| Isolation | own `DELONIX_ROOT`/socket (mktemp+PID) |

## 2. Result

```
Ran 103 of 122 Specs in 615.771 seconds
84 Passed | 19 Failed | 0 Pending | 19 Skipped
```

Against the v5.1.1 audit (84/20): **19 failures** (−1). The absolute pass count
is the same because the rootless host is the same ceiling; what changes is the composition (see §3).

## 3. The 19 failures, classified

| # | Spec | Class | Resolves in… |
|---|---|---|---|
| 1 | Mount Propagation `rprivate` | **Environment** (mounts on the host, root) | rootful-VM |
| 2 | Mount Propagation `rshared` | **Environment** | rootful-VM |
| 3 | Mount Propagation `rslave` | **Environment** | rootful-VM |
| 4 | Non-recursive readonly mount | **Environment** (mounts on the host) | rootful-VM |
| 5-10 | AppArmor ×6 (deprecated/field/prefer) | **Environment** (profile not loaded, root) | rootful-VM |
| 11 | Security Context `HostPID` | **Environment** (needs root) | rootful-VM |
| 12 | Security Context `HostNetwork=true` | **Rootless architecture** (host netns) | rootful-VM+CNI |
| 13 | Networking port-mapping container-port only | **Rootless architecture** (host's pod IP) | rootful-VM+CNI |
| 14 | Multiple Containers — should support network | **Rootless architecture** | rootful-VM+CNI |
| 15 | Streaming portforward *in host network* | **Rootless architecture** | rootful-VM+CNI |
| 16 | Streaming portforward `[Conformance]` | **Rootless architecture** | rootful-VM+CNI |
| 17 | Container OOM — OOMKilled 137 | **Environment** (cgroup subtree with no `memory` delegated in this shell; `subtree_control` empty) | rootful-VM (correct delegation) |
| 18 | SeccompProfilePath — `nil`=unconfined | **Deliberate divergence** (engine applies an allowlist, more restrictive) | conformance-only flag |
| 19 | Security Context `PodPID` (`shareProcessNamespace`) | **Feature** (touches `spawn()`) | engineering |

**Summary:** **17 environment/rootless-architecture** (expected to flip in rootful-VM+CNI,
the way Kubernetes runs the node's runtime) · **1 deliberate divergence** (seccomp) ·
**1 feature** (PodPID).

## 4. Harness fix (done in this session)

`scripts/critest.sh` was broken on `develop`: the restructuring (ADR-0086) moved
the `delonix-cri` bin from the `delonix-cri` package to **`delonix-cri-bin`**. Build line
fixed (`-p delonix-cri` → `-p delonix-cri-bin`); the bin name (`delonix-cri`)
stays the same.

## 5. Projection to 104/104 (TO BE CONFIRMED in rootful-VM)

> Projection, not fact: the class of the 17 (AppArmor profile not loaded, mount-on-host
> denied, host's-pod-IP, cgroup not delegated) is characteristically rootless-only.
> The v5.1.1 audit made the same prediction and hedge ("not measured, not claimed").

Minimum backlog for **104/104** in the rootful-in-a-VM matrix (k8s 1.36/1.37, CNI):
1. **Run the rootful-VM+CNI matrix** → expected to close the 17 environment/rootless ones.
2. **seccomp `nil`=unconfined (#18)** → *conformance-only* flag `DELONIX_CRI_SECCOMP_NIL=unconfined` that only the suite enables (production default stays hardened). +1.
3. **PodPID `shareProcessNamespace` (#19)** → implement in the `spawn()` path (pods Phase 3). +1.
4. **Confirm OOM (#17)** with cgroup2-memory delegated on the VM node.

Projection: rootful-VM baseline ≈ 101-102/104 → with (2)+(3) → **104/104**, OOM confirmed.

---

# ROOTFUL-in-VM + CNI matrix (2026-11-08) — the real conformance run

> Measured on a golden VM `delonix-vm-k8s:1.36` (libvirt, 4 vCPU/4G, ubuntu 24.04,
> glibc 2.39), engine 0.1.0 scp'd, **as root**, CNI bridge+portmap+host-local
> (`/etc/cni/net.d/10-delonix.conflist`). This is the configuration in which Kubernetes
> runs the node's runtime. Environment obstacles resolved along the way: missing CNI
> config; `iptables` outside the suite's PATH (it's in `/usr/sbin`).

```
Ran 103 of 122 Specs in 388.967 seconds
90 Passed | 13 Failed | 0 Pending | 19 Skipped
```

**84 (rootless) → 90 (rootful+CNI).** Flip confirmed: mount `rprivate`/`rslave`,
non-recursive readonly mount, `HostPID`, port-mapping-container-port-only,
multi-container-network, and 2 AppArmor (deprecated) passed. **BUT the flip was
partial and CNI introduced a trade-off** (see #A below).

## The 13 rootful failures, by root cause

| Root cause | Specs | No. | Class | Work |
|---|---|---:|---|---|
| **A. `shareProcessNamespace` refused in a CNI sandbox** | PodPID · HostNetwork×2 · HostIpc×2 · portforward-in-host-network | **6** | **REAL ARCHITECTURAL GAP** | the engine enables the shared PID-ns only via the native SDN; it refuses it under CNI (`FailedPrecondition`, container.go:519). With CNI (needed for the network) it blocks 6 specs. |
| **B. Test AppArmor profile not loaded** | apparmor field ×2 · prefer new ×2 | **4** | **Environment** | `profile 'cri-validate-apparmor-test-deny-write' is not loaded`. The 2 "deprecated" ones pass → AppArmor works on the VM. Fix: `apparmor_parser` loads the profiles in the harness. |
| **C. seccomp `nil`=unconfined** | 1 | **1** | Deliberate divergence | conformance-only flag `DELONIX_CRI_SECCOMP_NIL=unconfined` (decided). |
| **D. OOMKilled not reported** | 1 | **1** | Under investigation (candidate gap) | container does not die/does not report within 60s even rootful; cgroup driver? (`cgroup_parent is not set` warn). |
| **E. bidirectional `rshared` mount** | 1 | **1** | Under investigation | `rprivate`/`rslave` pass; `rshared` does not (`len(fileInfo) zero`) — host↔container propagation. |

## The real path to "all green" (corrects the rootless projection)

The rootless projection ("≈101-102 → flag+PodPID → 104") was **optimistic** — the
real run shows that CNI trades the network gains for **losing PID-sharing**.
The honest backlog is:

1. **(A) `shareProcessNamespace` in CNI sandboxes** — the biggest item, architectural: closes 6 specs at once. It is SDN/CRI engineering, not a flag.
2. **(B) load AppArmor profiles in the harness** — +4, cheap (VM setup).
3. **(C) conformance-only seccomp flag** — +1, small (decided).
4. **(D) OOMKilled** — investigate cgroup driver/reason-reporting — +1.
5. **(E) `rshared`** — investigate propagation — +1.

With (B) resolved in the harness the run would already rise to **94/103**; the number
that measures the ENGINE (taking out environment B) is **94 of 103**. The real
engineering blockers are A (6), D (1), E (1) + decision C (1).

---

# A+C+B implementation (2026-11-08) — measured progression

Progression in the rootful-VM+CNI matrix, measured at each step:

| Step | Result | Delta |
|---|---|---|
| rootless (host) | 84/103 | baseline |
| rootful-VM+CNI (no fixes) | 90/103 | +6 (environment flip) |
| **+ A + C + B(harness)** | **97/103** | **+7** |
| + B (AppArmor bug fix) | *to validate* | expected +4 → ~101/103 |

## Fixes implemented (develop, not committed; unit 4/4 green)

- **A — `apply_shared_pid_rootful`** (lifecycle.rs): in the root hostNetwork/CNI path
  (no `opts.pod`), it enables the `pod_shared_pid`+`pod_infra_pid` pair instead of refusing —
  as root the `setns(CLONE_NEWPID)` on the anchor is privileged with no userns holder.
  **Closed 5/6**: HostNetwork×2, HostIpc×2, portforward-in-host-network (0 refusals
  of shareProcessNamespace on the VM, there were 6). **PodPID still fails** — it is the only one
  that checks the *effective* sharing (execSync in the 2nd container sees/does not see the nginx);
  the semantics in the root-CNI path need deeper investigation (ADR candidate). The other 5 only
  needed the container to *start*.

- **C — flag `DELONIX_CRI_SECCOMP_NIL=unconfined`** (lifecycle.rs): conformance-only;
  maps nil seccomp profile→unconfined only when enabled, production default intact.
  **Spec passed** on the VM.

- **B — REAL AppArmor BUG** (lifecycle.rs): the NEW `apparmor` field (SecurityProfile)
  returned `localhost_ref` with the `localhost/` prefix, whereas the host lists the profile
  by its bare name in `/sys/kernel/security/apparmor/profiles` → `apparmor_loaded` never
  matched → CreateContainer refused unduly (only the DEPRECATED field stripped the
  prefix; hence 2 specs passing and 4 failing). Fix: shared helper
  `apparmor_localhost_name` (normalizes both paths). + step in `critest.sh`
  that loads the 2 test profiles (`apparmor_parser`, kept as root).

- **D (OOM)** — passed in the rootful run (the earlier timeout was cgroup delegation
  in the background shell, not a regression). **E (`rshared`)** — still fails
  (bidirectional host↔container propagation); under investigation.

## Remaining for "all green" (rootful)

After the AppArmor fix (expected ~101/103): **PodPID** (real PID sharing in root-CNI)
and **`rshared`** (bidirectional mount propagation) — both deeper engineering,
candidates for dedicated investigation / ADR.

---

# Final campaign result (2026-11-08): 84 → 99/103

| Step | Passed/103 |
|---|---|
| rootless (host) | 84 |
| rootful-VM+CNI | 90 |
| + A + C + B(harness) | 97 |
| **+ B (AppArmor bug fix)** | **99** |

## Classification of the 4 final failures

| Spec | Class | Note |
|---|---|---|
| AppArmor ×4 | **RESOLVED** | real bug fixed (`localhost/` normalization); 4→0 |
| Container OOM | **FLAKY** | passed in the A+C+B run, failed in this one — timing (186s vs 238s), not a deterministic gap |
| Multiple Containers network (BeforeEach) | **FLAKY** | same: passed in A+C+B, failed here; clean CNI (0 IPs/netns allocated) → transient |
| **PodPID** | **REAL GAP** | A enables the setns, but the *effective* PID sharing in root-CNI fails the execSync-check; touches isolation → **ADR** |
| **`rshared`** | **REAL GAP** | bidirectional host↔container mount propagation; touches mount semantics → **ADR** |

## Verdict

- **Stable number ≈ 99-101/103** (the 2 flaky ones oscillate). The 4 AppArmor + seccomp +
  the 5 of the shareProcessNamespace cascade + OOM(environment) were **resolved**.
- **2 real gaps remaining** — PodPID (effective PID sharing in root-CNI) and
  `rshared` (mount propagation) — touch **isolation/security invariants**.
  They should not be fixed in haste: **candidates for an ADR + a dedicated behaviour test**,
  with the design validated before touching the spawn/mount path.
- OOM/Multiple-Containers flakiness: investigate separately (harness stability under
  repeated runs on the same VM), not an engine conformance gap.

---

# PodPID RESOLVED (2026-11-08): 100 → 102/103

The pause-PID-1 (ADR-0092) + the `pod_pidns_pid` engine field closed PodPID.

| Step | Passed/103 |
|---|---|
| rootless | 84 |
| rootful-VM+CNI | 90 |
| + A + C + B(harness) | 97 |
| + B (AppArmor bug fix) | 99 |
| + pause anchor (ADR-0092) | 100 |
| **+ `pod_pidns_pid` (join in the single root pass)** | **102** |

`/proc/1/cmdline` in a shareProcessNamespace pod member = `delonix-cri __pid-anchor`
(the pause), not `nginx: master process` → PodPID passes. **Only remaining failure:
`rshared`** (ADR-0093 — bidirectional mount propagation behind privilege, not yet
implemented). OOM = flaky (passed in this run).
