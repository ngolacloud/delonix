# Delonix × Docker × Podman — CNCF acceptance master sheet

> Consolidation of the 2026-11-12 measurements (develop @ `67ddef41`). Cross-checks
> the Delonix code against the official BuildKit/buildah/compose-spec code (via
> `gh`, literals confirmed in `file:path:line`). Supersedes the numbers in
> `CNCF_GAP_DOCKER.md`/`CNCF_GAP_PODMAN.md`/`BACKLOG.md` (2026-11-08) where this
> round corrected them. Doctrine: only the *verified effect/contract* counts;
> implemented ≠ tested ≠ proposed.

## 0. Provenance

| Area | Delonix source | Reference source |
|---|---|---|
| 2 CRI | `reports/cri-conformance-0.1.1/03-full-suite-103.md` | `critest` v1.36.0 |
| 3 SLSA | `delonix-oci/build.rs`, `sign.rs` | `moby/buildkit` `solver/llbsolver/provenance/types/types.go`, `frontend/attestations/parse.go` |
| 4 SBOM | `delonix-scanner/lib.rs` (`sbom_spdx`) | `moby/buildkit` `exporter/containerimage/writer.go`, `frontend/attestations/sbom/` |
| 5 Build | `delonix-oci/build.rs`, `registry.rs`, `bins/delonix/src/cmd/build.rs` | `moby/buildkit` `cache/remotecache/*`, `exporter/containerimage/writer.go`; `containers/buildah` `pkg/cli/common.go`, `cmd/buildah/manifest.go` |
| 6 Compose | `bins/delonix/src/cmd/compose.rs` | `compose-spec/compose-go/schema/compose-spec.json`, `compose-spec/compose-spec/12-interpolation.md` |
| 8 Scan | `delonix-scanner/lib.rs` | `docker/scout-cli`, `containers/buildah` `--sbom` |

## 1. Scorecard of the 8 CNCF value areas (state today)

Legend: ✗ absent · ◑ partial · ✔ done. **Δ** = correction vs the 2026-11-08 backlog.

| # | Area | Docker | Podman | **Delonix (measured)** | Est. | Δ |
|---|---|---|---|---|---|---|
| 1 | Runtime lineage | containerd+runc | crun+conmon | Rust runc-compat runtime, outside the containerd chain | ✗ | — |
| 2 | OCI/CRI conformance | 100% | via containerd/cri-o | **CRI rootful-VM 103/103 clean** · rootless 84/19 · k8s YAML interop missing | ◑→strong | **↑** was 90/13 |
| 3 | SLSA Provenance | BuildKit v1/v0.2 | — | predicate not emitted; `sign.rs` cosign exists | ✗ | — |
| 4 | Interoperable SBOM | BuildKit SPDX+in-toto | buildah CycloneDX/SPDX | **SPDX-2.3 already emitted**; missing in-toto wrapper + OCI attachment | ◑ strong | **↑** backlog underestimated |
| 5 | Cloud-native build | 6-backend cache, multi-arch, LLB | --manifest, --cache-to | local cache + cache-mount; single `--platform`; auto SBOM | ✗ | — |
| 6 | Compose Specification | Compose Spec | kube play | fail-closed allowlist, common surface; **no `${VAR}` interpolation** | ◑ | target reframe |
| 7 | Open governance | open Moby | LFX+PGP+LTS | private repo, own org | ✗ | — |
| 8 | Mature CI scanning | Scout policy/compare | delegates | OSV+staleness+`--fail-on`; no policy/compare | ◑ | — |

**Already aligned (do not redo):** rootless-first, daemonless, native pods,
runtime `policy.json`, cosign/sigstore (`sign.rs`), OSV (CNCF format),
cache-mount, automatic per-build SBOM.

## 2. Key measurements (with evidence)

### 2.1 Area 2 — CRI 103/103 (material jump)
`reports/cri-conformance-0.1.1/03-full-suite-103.md`: golden rootful+CNI VM,
`critest` v1.36.0 → **103 Passed | 0 Failed | 19 Skipped** (single run, no
retries). Closes `rshared` (ADR-0093), OOM (cgroup barrier), AppArmor,
HostNetwork/HostIpc, shared-PID. `rshared` behind privilege (refused in rootless
with `DX-1117`). **Missing in area 2:** k8s YAML interop (`kube play/generate`,
GAP-0015) — distinct from CRI conformance.

### 2.2 Areas 3+4 — supply-chain: the in-toto/OCI attachment is the pivot
- **Delonix already has:** SPDX-2.3 (`sbom_spdx`, `delonix-scanner/lib.rs:605`),
  cosign signature (`sign.rs`), OCI artefact push to the same repo
  (`registry.rs::push_signature`), digest read/resolution.
- **Missing (shared by 3 and 4):** wrap in a **raw** in-toto Statement (not
  DSSE) and attach it to the image via the BuildKit mechanism:
  - layer `application/vnd.in-toto+json` + annotation `in-toto.io/predicate-type`
    (`writer.go:633,639`);
  - manifest `artifactType=application/vnd.docker.attestation.manifest.v1+json`,
    `subject=<image>` (`writer.go:51,675`);
  - descriptor in the index `platform=unknown/unknown` + annotations
    `vnd.docker.reference.type=attestation-manifest` /
    `vnd.docker.reference.digest=<digest>` (`util/attestation/types.go:4-7`).
- **Missing (area 3 only):** generate the SLSA predicate during the build —
  `predicateType https://slsa.dev/provenance/v1` (default), `buildType`,
  `buildDefinition`/`runDetails`, flag `--provenance mode=min|max` (default max).
  `build.rs` builds but does not instrument materials/invocation.

### 2.3 Area 5 — cloud-native build (the weakest)
- **Cache:** local only (`build.rs:623`) + cache-mount (`build.rs:24-47`).
  **No** `--cache-to/--cache-from` (BuildKit has 6 backends:
  `registry·local·inline·gha·s3·azblob`, `buildkitd/main.go:943-956`, `mode=min|max`;
  buildah has per-registry cache). Delonix is below even buildah here.
- **Multi-arch:** `--platform linux/<arch>` **single** (`build.rs:213-228`), cross
  via the host's binfmt/qemu (`build.rs:1122`). **Does not** accept a list nor
  **emit an `ImageIndex`** (BuildKit assembles it automatically, `writer.go:246-313`;
  `registry.rs` only *reads* an index).
- **LLB frontends:** out of scope (architectural decision — own Dockerfile parser).
- **Host/ops:** missing `generate systemd/Quadlet` for workloads (GAP-0016).

### 2.4 Area 6 — Compose (target reframe)
- **No Compose conformance suite exists** (≠ `critest`). Conformance =
  **JSON Schema `compose-go/schema/compose-spec.json`** + parity with the
  `compose-go` parser. The backlog's "green suite" target is invalid.
- Delonix: **fail-closed** allowlist (refuses with a reason, never ignores), 94
  tracked keys (`SPEC_SERVICE_KEYS`), 3 `depends_on` conditions, `extends`,
  `profiles`, `x-`, secrets/configs, deploy/resources, obsolete `version` handled.
- **Gaps:** `${VAR}`/`.env` interpolation **absent** (normative, `12-interpolation.md`)
  — the biggest; `include` refused (workaround `-f`); long tail of ~35 attributes
  refused; `up` does not restart a stopped service (GAP-0005/DEF-003); `develop.watch`/
  `models`/`jobs` absent (optional/niche).

### 2.5 Area 8 — scanning
OSV (array+ZIP, `lib.rs:895-1015`), `db_is_stale` (differentiator in our favour),
`--fail-on` by severity (primitive CI gate). **Missing:** declarative `policy`,
`compare` between images, trust-by-policy (GAP-0017, Podman lens).

## 3. The CNCF admission question

The technical table is **not** the entry criterion. The **Sandbox** level
assesses mostly the non-technical:

| Sandbox criterion | Delonix |
|---|---|
| OSI licence (Apache-2.0) | ✔ LICENSE present |
| **Public repository** | **✗ private** |
| Neutral governance / IP without a single owner | ◑ files exist; org and control belong to the owner |
| Code of conduct | ✔ |
| Adoption/community (no scale required) | ✗ |
| Cloud-native differentiation | ✔ Rust rootless/daemonless runtime + microVMs |

**Verdict:** technically Delonix is already competitive for Sandbox on a strong
dimension (CRI 103/103 — evidence that many candidates lack) and is halfway on
supply-chain. **The admission blocker is area 7 (governance): while the repo is
private and the org has a single owner, entry is NO-GO regardless of the engine's
quality.** Engineering is not the bottleneck; the decision to open up is.
