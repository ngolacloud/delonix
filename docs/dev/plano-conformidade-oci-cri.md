# OCI + CRI conformance plan — aligning Delonix with Docker/Podman

> **Goal.** Make Delonix pass the **same interoperability contracts** that
> containerd/CRI-O (CRI) and any OCI runtime (image/distribution/runtime) pass, to be
> drop-in for the kubelet and for registries — and, on a separate track with a caveat,
> cover the Docker Engine API that third-party tools call via `DOCKER_HOST`.
>
> **Measurement principle.** Each phase closes against an **upstream suite with a number**, not
> "looks done". Never silent divergence (explicit refusal + reason). A test that fails without the
> fix and passes with it, like the rest of the project. Where a test does not apply to the model
> (daemonless, rootless), it is named **N/A** with the reason — the percentage is never inflated.
>
> **Scope and the `paridade-docker-podman.md §5` caveat.** OCI (image/distribution/runtime) and
> CRI are interop contracts where full conformance is worth it — they make Delonix a real
> drop-in replacement. The Docker Engine API (Track D) is ecosystem convenience; the project
> itself advises against chasing full Docker CLI parity, so Track D covers only what
> automated tools call, not interactive `docker run`.

## Measured baseline (binary `ee615f3b`, 2026-10-24)

| Contract | Status | Source |
|---|---|---|
| CRI — critest v1.36.0 | **RE-MEASURED 2026-10-24 on v5.1.1 (`main @ ab4751ac`): 84/104 (`84 Passed \| 20 Failed \| 18 Skipped`)** — beats the 79/103 from v0.63 (2026-08). The line that ships today as 0.1.0 | `reports/cri-conformance-2026-10-24/00-audit-report.md` · `scripts/critest.sh` |
| OCI Image | **interop proven on v5.1.1**: delonix→docker **and** delonix→podman + docker→delonix, by digest (IMG-1 fixed 2026-10-24) | `reports/conformance/A1-oci-image-distribution.md` |
| OCI Distribution | push/pull cross-engine OK on v5.1.1; the `distribution-spec` suite is server-side → **N/A** (Delonix is a client) | A1 |
| OCI Runtime — bundle | **v5.1.1**: ociVersion 1.3.0 ✅, seccomp ✅, `runc run` rootless rc=0 ✅ (RT-1/2/3) — `oci-runtime-tool validate` OK | B1/B2 reports |
| OCI Runtime — runc CLI | **does not exist** (the one real runtime gap) — RT-4 / ADR-0080 | B1 |
| Docker Engine API | **15/26** routes (measured live on v5.1.1) | `delonix serve docker-api --matrix` |

> **Provenance warning (important).** This plan's first version inherited the gap matrix from the
> **v3.1.0** report (`fa8c6297`). We are on **v5.1.1**: re-measuring against current code showed
> that several "gaps" are **stale** — mount propagation already works (B2), a layer's `.tmp` is
> already cleaned in-process (G-035), and `tag@digest` is a valid format, not a bug (G-033). **This
> plan is driven by measurements on v5.1.1, not by the v3.1.0 matrix.** A "gap" only counts if it
> REPRODUCES on v5.1.1.

**The CRI's 20 failures (2026-10-24 measurement), by area:** AppArmor 6 · Mount Propagation 3 ·
Security Context 3 (HostPID, PodPID, SeccompProfilePath) · Networking 2 · Streaming 2 (portforward) ·
Image Manager 2 (both already fixed, #18/#19) · RO-mount 1 · Image Identifier Consistency 1.
(The old breakdown — 24 failures from v0.63, AppArmor 9 — stays in `cri-conformance.md`'s history.)

## Owner decisions (2026-10-24)

1. **Priority: CRI first** (kubelet drop-in — the container+VM+k8s differentiator).
2. **`NoNewPrivs=false`:** honor the CRI's request (passes conformance) **while keeping NNP
   always-on by default on the CLI/normal path**. The default guarantee does not change; only
   the CRI may relax it.
3. **OOM post-mortem:** **invest in the supervisor** — the per-container supervisor (which
   already exists for `--restart`/`wait`) reads the cgroup BEFORE teardown and records
   `OOMKilled`/137. Not N/A.

---

## Track C — CRI (PRIORITY) · gate: re-measure on v5.1.1, then → ~100/103 in rootful

**Step zero: re-measure critest on v5.1.1 — DONE (2026-10-24): 84/104** (`scripts/critest.sh`;
report in `reports/cri-conformance-2026-10-24/`). The 79/103 was from v0.63 (2026-08); the
failure breakdown below has been updated for the 2026-10-24 run. The `ImageService` fixes
#18/#19 (C2, worktree `cri-image-repodigests-and-user`) have already landed; portforward
(C3, `cri-portforward-conformance`) remains. The phases below still stand as a map of the AREAS.

Each phase = one area of critest failure, with the failing spec named.

| Phase | Area (# specs) | What to do | Gate |
|---|---|---|---|
| **C1** | Security Context (4) | `PodPID` namespace; `RunAsUserName`; `SeccompProfilePath` by path (wires into the custom-seccomp path that already errors instead of ignoring); **`NoNewPrivs=false`** (decision 2 — relaxable only by the CRI) | +4 |
| **C2** | Image Manager (2) | pull by digest via CRI; `Uid`/`Username` in `ImageStatus` | +2 |
| **C3** | Streaming (2) | CRI streaming server: `PortForward` + the `Exec`/`Attach` base with hijack (**shared with D3**) | +2 |
| **C4** | Networking (1) + Mount Propagation (3) | port mapping with container port only; `rshared`/`rslave` (**comes from Track B2**) | +4 |
| **C5** | RO-mount (1) + Multiple Containers (1) | non-recursive read-only mounts; multi-container case | +2 |
| **C6** | OOM (1) | supervisor captures OOM before cgroup teardown (decision 3) | +1 |
| **C7** | AppArmor (9) | translate the CRI's per-container profile; run the suite **rootful** (loading profiles needs privilege — N/A in rootless, named); prove the ones that are not environment-dependent | +0 to +9 depending on rootful/rootless |

> **Honest N/As to name in the final report:** the AppArmor specs that require loading a
> profile (privilege rootless doesn't have) stay N/A in rootless and are proven in rootful. No
> others.

**Track B prerequisite:** C4 depends on B2 (mount propagation). Do B2 at the start of Track C.

---

## Track B — OCI Runtime Spec (enables CRI)

| Phase | What to do | Gate |
|---|---|---|
| **B1** | Run `opencontainers/runtime-tools` validation against (a) the bundle from `image export` + runc/crun and (b) Delonix's own init (config.json → namespaces/caps/seccomp/mounts/hooks/rlimits). Reproducible script. | published baseline |
| **B2** | Mount propagation `rshared`/`rslave` (**closes 3 of CRI's — C4**) | runtime-tools + critest |
| **B3** | Recursive readonly + masked/readonly paths (decision G-009) | runtime-tools |

## Track A — OCI Image + Distribution (image interop foundation)

**Re-baseline on v5.1.1 (2026-10-24): this track is largely DONE — the rest is optional.**
OCI image interop was **proven live** across all three engines (IMG-1 fixed: `push` publishes
an OCI manifest, pullable by both docker AND podman). The v3.1.0 report's "gaps" G-033/035/039
**fell away** on re-measurement:

- **G-033** (tag+digest) — **NOT a bug**: `repo:tag@digest` is a valid and common OCI/Docker
  format (the kind uses it); refusing it would break the kind. If one day a real *conflict*
  (the tag resolving to a different digest) needs detecting, that's a local representation
  decision — out of scope until requested.
- **G-035** (layer `.tmp`) — **already cleaned in-process** (`overlay.rs:492-508`). Minimal
  residue: only an orphan `.tmp` from a process that DIED midway, which `prune` doesn't sweep —
  a small improvement, groupable into a future `prune` pass, not a conformance gap.
- **G-039** (LEGACY docker-save archive `<hash>/layer.tar`) — real but **only affects
  pre-OCI archives**; modern `docker save` produces the `blobs/sha256/` layout that `load`
  already accepts. Low priority; the fix (path→digest map) has subtlety around layers.

| Optional | What remains | Gate |
|---|---|---|
| A-sbom/scan/sign | `image sbom`/`scan`/`verify`/`sign` — measure real coverage on v5.1.1 (not measured in this campaign) | published baseline |
| A-manifest-list | multi-arch on `push` (today 1 arch per invocation) | multi-arch round-trip |

> `oci-image-tool` (image-spec) **does not compile** (breakage in `Sirupsen/logrus`) and
> `distribution-spec` is server-side — so the v5.1.1 image proof is **cross-engine interop**,
> not a suite.

## Track D — Docker Engine API (ecosystem; §5 caveat)

| Phase | Route(s) | Why |
|---|---|---|
| **D1** | `GET /containers/{id}/logs` (stdout/stderr framing) | unblocks testcontainers' `wait_for_logs` — the biggest isolated value |
| **D2** | `/events`, `/build`, `/networks`, `/volumes`, `GET …/stats` | the surface compose/kind/tools touch |
| **D3** | `POST …/exec` + `…/attach` (HTTP hijack) | **shares C3's streaming server** |

> Deliberately out of scope: interactive `docker run` against the server (internal Go CLI
> behavior), and `--format`/CLI parity, which `§5` advises against.

---

## Sequence and dependencies

**Status on v5.1.1:** Track A (image) **proven** (IMG-1); Track B RT-1/2/3 + B2 **done** — only
**RT-4** (runc CLI, ADR-0080, GO-gated) remains. The real remaining work is RT-4, Track C (to
re-measure), and Track D.

```
[v5.1.1 done]  A (interop) ✅   B2 mount-prop ✅   RT-1/2/3 bundle ✅
[real remaining]
   RT-4 (ADR-0080, GO) ─► runtime-tools validation suite against Delonix
   C0 re-measure critest ─► C1 ─► C2 ─► C3 ─┬─► C4 ─► C5 ─► C6 ─► C7
                                          └─(C3 streaming)─► D3 ─► D1 ─► D2
```

1. **Track C**: re-measure critest on v5.1.1 (C0), then close area by area — the strategic
   return (underway by the owner: C2/C3).
2. **RT-4** (when the owner gives GO): unblocks measuring the runtime through the upstream suite.
3. **Track D** last, with the caveat; D3 comes for free with C3 (same streaming server).
4. **Track A**: only the optional items (sbom/scan/sign, manifest-list) — interop is already proven.

## How to measure (reproducible, per phase)

- CRI: `scripts/critest.sh` → `Ran N of 122 ... X Passed | Y Failed`. Publish the delta in X at each phase.
- OCI runtime: `runtime-tools` validation (install in A1/B1); publish pass/fail per spec section.
- OCI image/distribution: upstream `image-spec`/`distribution-spec` suites; publish baseline and deltas.
- Docker API: `delonix serve docker-api --matrix` (served/refused routes) + `tests/compat/docker_api_smoke.py` and a testcontainers run.
- Each phase brings a test that fails without the fix (the project's method).

## Known risks / tensions

- **NNP vs CRI** (decision 2): the path that relaxes NNP must only be reachable through the CRI, never the CLI.
- **AppArmor rootless** (C7): part of the 9 specs is a privilege limit, not a gap — measure rootful to tell them apart.
- **Daemonless OOM** (C6): the supervisor must read the cgroup in the window between death and teardown; a real race.
- **Host disk at 96%**: the OCI/CRI suites pull images and write artifacts — free up space before A1/B1.
