# CRI conformance audit — delonix-runtime

> Independent measurement against `cri-tools critest` **v1.36.0** (upstream),
> engine `delonix-cri` **v5.1.1**, **rootless**, Linux 7.0 host, cgroup v2.
> Date: **2026-10-24**. Reproduce with: `scripts/critest.sh`.

This report **re-measures** conformance (the published `docs/cri-conformance.md`
claimed 79/103 and dated from 2026-08-25 / engine v0.63.1 — twenty versions ago)
and **fixes** two real `ImageService` failures.

## 1. Compatibility matrix (reference configuration)

| Axis | Value |
|---|---|
| delonix-runtime | v5.1.1 (`main` @ `ab4751ac`; fix in `fix/cri-image-repodigests-and-user`) |
| CRI contract | `runtime.v1` (`k8s.io/cri-api`), **34 RPCs** (RuntimeService + ImageService) |
| Suite | `cri-tools critest` **v1.36.0** (binary at `/usr/local/bin/critest`, `crictl` v1.36.0) |
| Target Kubernetes | 1.36.x (real kubeadm cluster proven separately — VM `delonix-vm-k8s:1.36`, k8s 1.36.4) |
| Host | Linux 7.0.0, x86_64 |
| cgroups | v2, `cgroupfs` driver (advertised to the kubelet) |
| Mode | **rootless** (delegated uid; cgroup2 cpu/memory/pids delegated; io/cpuset not) |
| CNI | native SDN (slirp+per-UID holder); CNI mode available but not exercised here |
| Storage | rootless per-layer overlay + CAS under `$DELONIX_ROOT` |
| Cap ceiling | `DELONIX_CRI_CAP_CEILING=all` (the suite creates privileged containers) |

**Mode NOT measured**: rootful. Several failures (AppArmor, mount propagation,
HostPID) may flip sign as root and on a VM with the privilege for the harness to
mount on the host — **it was not measured, so no claim is made**. That is the
matrix decision to follow this audit.

## 2. Measured result (baseline, before the fix)

```
Ran 104 of 122 Specs in 511.184 seconds
84 Passed | 20 Failed | 0 Pending | 18 Skipped
```

Progression against the published document (same v1.36.0 suite): **79 → 84**. It
did not regress; it improved by five, thanks to fixes that landed after
2026-08-25 (eviction/`writable_layer`, `kubeadm reset`, `ReopenContainerLog`
following the inode). The old document was out of date — this is exactly why
re-measuring was the first step.

> `critest` is **not** a CNCF certification and does **not** exercise a real
> kubelet. It is a reproducible measurement of named specs, nothing more. The
> five dimensions (CRI compatibility · critest coverage · Kubernetes integration
> · robustness · security) are reported separately; this audit covers the first
> two.

## 3. The 20 failures, classified

| # | Spec | Area | Class | Note |
|---|---|---|---|---|
| 1-3 | mount propagation `rprivate`/`rshared`/`rslave` | Mount | **Environment** | `critest` mounts on the HOST (`operation not permitted`): needs root, not the engine |
| 4-9 | AppArmor (deprecated/new/prefer, 6 specs) | Security | **Environment** | loading profiles requires a privilege rootless does not have |
| 10 | Image Identifier Consistency (different registries) | Image | **Real gap** | init container exits 126: `could not mask '/proc/sysrq-trigger': ENOENT` — masking fails fail-closed when the path does not exist (runc tolerates). Candidate for the next session |
| 11 | Multiple Containers — should support network | Network | **Architecture** | timeout; reaching the pod IP from the host under rootless |
| 12 | Streaming — portforward in host network | Streaming | **Real gap** | portforward |
| 13 | Networking — port mapping with container port only | Network | **Architecture** | pod IP unreachable from the host under rootless |
| 14 | Security Context — HostPID | Security | **To investigate** | may flip as root |
| 15 | Security Context — PodPID (`shareProcessNamespace`) | Security | **Decision** | touches `spawn()`; Phase 3 of the pods work |
| 16 | SeccompProfilePath — nil profile unconfined | Security | **Deliberate divergence** | engine applies an allowlist (`Seccomp:2`) instead of `0`; more restrictive on purpose |
| 17 | Streaming — portforward [Conformance] | Streaming | **Real gap** | portforward |
| 18 | Image Manager — digest pulled and removed | Image | **Real gap → FIXED** | see §4 |
| 19 | Image Manager — Uid/Username non-empty | Image | **Real gap → FIXED** | see §4 |
| 20 | Container Mount Readonly — non-recursive | Mount | **Environment** | the harness mounts on the host |

Summary: **10 environment** (3 mount + 6 AppArmor + 1 readonly), **2 rootless
architecture** (network), **1 deliberate divergence**, **1 decision** (PodPID),
**1 to investigate** (HostPID), **5 real gaps** → of which **2 fixed** here
(#18, #19) and **3 queued** (portforward ×2, masking ENOENT #10).

No environment failure is reclassified as "not applicable": either the
environment is fixed (measure rootful on a VM) or the result for that axis is
declared **incomplete**.

## 4. Applied fix (#18, #19)

Root cause confirmed by live reproduction (`crictl` against an isolated
`delonix-cri` — see `02-evidence.md`), not by reading:

- **#18** — a digest reference (`name@sha256:…`) was returned in `repo_tags`,
  and `repo_digests` came back ALWAYS empty. The engine's `ImageStore` keeps all
  references in a single list; the separation is the CRI boundary's job, because
  the proto has distinct fields that the kubelet reads separately. Fix:
  `partition_refs` splits the list by `@` (a reference with `@` is a digest).
- **#19** — for a `USER` of the form `name:group`, the `username` came back with
  the group attached (`www-data:1004`); the spec compares against the name alone
  (`www-data`). Fix: drop the group with the same `split(':').next()` the numeric
  path already used.

Both in `crates/interfaces/delonix-cri/src/lib.rs`, with regression tests
(`image_user_*`, `partition_refs_*`) and mapping the `Image` shared between
`list_images`/`image_status` so they do not diverge again.

**Proof**: focused `critest -focus "Image Manager"` → **10 Passed | 0 Failed**
(the two previously red specs now green). Regression: full suite re-run — see
`01-postfix-result.md`.

## 5. Follow-ups logged (one session each)

1. **Matrix decision**: re-measure rootful on a disposable VM (flips the sign of
   ~10 environment failures), or declare the rootful axis incomplete.
2. **portforward** (#12, #17) — real streaming gap.
3. **masking ENOENT** (#10) — masking should ignore non-existent paths like runc
   does, instead of aborting the container's startup.
4. **`shareProcessNamespace`** (#15) — touches `spawn()`.
5. **Model digests in the `ImageStore`** — persist the manifest-digest per image
   (the #18 fix is correct at the boundary, but the store still treats the whole
   reference as a "tag"); and **`blob_path` should use the service's `base`**
   instead of `default_root()` (they only coincide because `DELONIX_ROOT` is in
   the environment).
