# Delonix — CNCF acceptance: measurement + plan (report)

> **Campaign:** assess whether the CNCF would accept Delonix's entry, comparing
> against Docker and Podman, and design the plan to close the gaps.
> **Date:** 2026-11-12 · **Base:** develop @ `67ddef41`.
> **Method:** Delonix code cross-checked against the official code of
> BuildKit/buildah/compose-spec (via `gh`, literals at `file:path:line`).
> Doctrine: only the verified contract/effect counts.
>
> **Detail:** `CNCF_ACCEPTANCE_MASTER.md` (scorecard+evidence) and
> `CNCF_RESOLUTION_PLAN.md` (plan by waves), in `reports/e2e/0.1.0/`.

## 1. Verdict

- **Technically**: competitive for **CNCF Sandbox** on one strong dimension
  (CRI conformance **103/103 clean**, rootful+CNI) and halfway on
  supply-chain (cosign + OSV + SPDX already present).
- **Admission**: **NO-GO** until **governance (area 7)** is closed — private repo and
  single-owner org. The Sandbox criteria are mostly non-technical (public,
  neutral governance, IP without a single owner); the engine's quality does not offset that.
- **Conclusion**: engineering is not the *entry* bottleneck; the decision to open up
  is. The code waves raise *competitiveness* (towards Incubating), they do not
  unlock admission — that is Wave 0.

## 2. Scorecard of the 8 areas (today)

Legend: ✗ absent · ◑ partial · ✔ done. **Δ** = correction vs 08-Nov backlog.

| # | Area | Docker | Podman | **Delonix (measured)** | Est. | Δ |
|---|---|---|---|---|---|---|
| 1 | Runtime lineage | containerd+runc | crun+conmon | Rust runc-compat runtime, outside the containerd chain | ✗ | — |
| 2 | OCI/CRI conformance | 100% | containerd/cri-o | **CRI 103/103 clean**; k8s YAML interop missing | ◑→strong | **↑** was 90/13 |
| 3 | SLSA provenance | BuildKit v1/v0.2 | — | predicate not emitted; cosign present | ✗ | — |
| 4 | Interoperable SBOM | SPDX+in-toto | CycloneDX/SPDX | **SPDX-2.3 already emitted**; in-toto wrapper+attachment missing | ◑ strong | **↑** underestimated |
| 5 | Cloud-native build | 6-backend cache, multi-arch | --manifest, --cache-to | local cache+mount; single `--platform` | ✗ | — |
| 6 | Compose Spec | Compose Spec | kube play | fail-closed allowlist; **no `${VAR}` interpolation** | ◑ | target reframe |
| 7 | Governance | Moby open | LFX+PGP+LTS | private repo, own org | ✗ | — |
| 8 | CI scanning | Scout policy/compare | delegates | OSV+staleness+`--fail-on`; no policy/compare | ◑ | — |

**Material corrections this round:** (2) CRI rose 90/13 → **103/103** (closure of
`rshared`/OOM/AppArmor/HostNS/shared-PID); (4) the **SPDX-2.3 is already generated**
(`sbom_spdx`), it only needs wrapping+attaching; (6) **there is no Compose
conformance suite** — the target is to validate against the JSON Schema + `compose-go`
parity.

## 3. Sandbox admission criteria

| Criterion | Delonix |
|---|---|
| OSI licence (Apache-2.0) | ✔ |
| **Public repository** | **✗ private** |
| Neutral governance / IP without a single owner | ◑ |
| Code of conduct | ✔ |
| Adoption/community | ✗ |
| Cloud-native differentiation | ✔ (Rust rootless/daemonless + microVMs) |

## 4. Closure plan (by return-to-admission)

| Wave | Areas | Effort | Role |
|---|---|---|---|
| **0 Governance** | 7 | owner's decision | **unlocks the NO-GO** — parallel |
| **1 Supply-chain** | 3+4 | M→L | **pivot**: GAP-0009 (in-toto/OCI attachment) → GAP-0008 (SLSA) |
| **2 Scanning** | 8 | M | policy+compare over existing OSV |
| **3 Kube interop** | 2 | L | `kube play/generate` (Podman differentiator) |
| **4 Compose** | 6 | M–L | `${VAR}` interpolation + GAP-0005 (up restart) |
| **5 Build** | 5 | L–XL | registry cache + multi-arch index + systemd |
| **6 Lineage** | 1 | XL | ADR + OCI suites |

**Sequence:** Wave 0 in parallel (owner) + Waves 1→2→3→4→5→6 (code). Highest-ROI
point: **Wave 1A (GAP-0009)** — `Compl. M`, reuses `sbom_spdx`+`sign.rs`+
`registry.rs`, moves areas 3 and 4 at the same time.

## 5. Execution state

| Wave | Item | State |
|---|---|---|
| 1A | GAP-0009 in-toto/OCI attachment | **STARTING** (actionable ticket + worktree) |
| remaining | — | planned |

> Execution driven by the `delonix-backlog-driver` skill; validation by
> `delonix-runtime-e2e-quality` before closing each wave. One worktree per wave from
> `develop`.
