# CNCF gap analysis — Delonix × Docker (8 value areas)

> Reading of the official Docker/Moby code (via `gh`: `moby/moby`,
> `moby/buildkit`, `docker/scout-cli`, `docker/compose`, `compose-spec/compose-spec`,
> lineage `opencontainers/runc` + `containerd/containerd`) cross-checked against
> the Delonix code (`delonix-scanner`, `delonix-oci/sign.rs`, `build.rs`, CRI) and
> the 2026-11-08 tests. Goal: what is missing for **100% across the 8 CNCF value
> areas**. Impartial: includes where Delonix is **already** aligned.

## Where Delonix is already aligned (do not inflate the gap)
- **CVE via OSV** — `delonix-scanner` consumes **OSV** feeds (open CNCF format), offline. ✔
- **cosign/sigstore signature** — `delonix-oci/src/sign.rs` is compatible with **cosign** (ECDSA-P256, `.sig` artefact, simplesigning). ✔
- **Rootless-first** — philosophical parity with rootless BuildKit/Moby. ✔

## Matrix of the 8 areas → path to 100%

| # | CNCF value area | Docker (evidence) | Delonix today | Items for 100% | State |
|---|---|---|---|---|---|
| 1 | **Runtime lineage** | containerd (CNCF *graduated*) + runc (OCI ref.) | Rust runc-compat runtime, outside the containerd chain | GAP-0012 | ✗ |
| 2 | **OCI/CRI conformance** | containerd CRI 100%; Moby OCI image/dist ref. | CRI 84/19 rootless · 90/13 rootful-VM; OCI bugs | BUG-0003/04/05/06 · GAP-0001/0003 · GAP-0012 | ◑ |
| 3 | **SLSA Provenance in build** | BuildKit SLSA v0.2/v1.1 (min/max, builder-id, reproducible) | does not generate provenance in build | **GAP-0008** | ✗ |
| 4 | **Interoperable SBOM (in-toto/SPDX)** | BuildKit SBOM SPDX+in-toto, pluggable scanner (syft) | scanner generates SBOM for CVE, does not attach SPDX in-toto to the image | **GAP-0009** | ◑ |
| 5 | **Cloud-native build** | distributed cache (registry/S3/Azure/GHA), multi-plat, LLB frontends | COPY/`--link` ok; no dist. cache/multi-plat/frontends | **GAP-0010** | ✗ |
| 6 | **Compose as an open specification** | community Compose Specification | own compose, gaps (DEF-003) | GAP-0005 + **GAP-0011** | ◑ |
| 7 | **Open governance** | open Moby governance; donated runc/containerd | private repo, own org | **GAP-0014** | ✗ |
| 8 | **Mature CI scanning** | Scout: policy, `compare`, CI gating (`--exit-code`) | new OSV scanner, no policy/compare/CI | **GAP-0013** | ◑ |

Legend: ✗ absent · ◑ partial · ✔ done. **100% = all ✔.**

## Priority (CNCF lens)
1. **Build supply-chain** (areas 3+4) — GAP-0008, GAP-0009. Additive: `sign.rs`
   (cosign) and `delonix-scanner` (SBOM/OSV) already cover half the way; what is
   missing is emitting **in-toto/SPDX** + **SLSA provenance** and attaching it as
   an OCI attestation.
2. **Proven conformance** (areas 1+2) — GAP-0012 + the already-open CRI BUGs.
3. **Compose Spec** (area 6) — GAP-0005 + GAP-0011.
4. **CI scanning** (area 8) — GAP-0013.
5. **Advanced build** (area 5) — GAP-0010.
6. **Governance** (area 7) — GAP-0014 (owner's decision, not just code).

---

## New GAP sheets (backlog format)

### GAP-0008 — SLSA Provenance in `delonix build` (area 3)
- **Current:** `build.rs` builds the image (replicates BuildKit's COPY/`--link`) but does not emit provenance.
- **Expected (CNCF):** BuildKit emits SLSA provenance (v0.2/v1.1, min/max) as an in-toto attestation attached to the image.
- **Target 100%:** `delonix build --provenance[=mode=min|max]` produces a SLSA v1.1 predicate (builder.id, materials, invocation) wrapped in in-toto and attached to the OCI index.
- **Component:** `crates/adapters/delonix-oci/src/build.rs` + `.../sign.rs` (reuse) + `image_ops.rs`. **Compl.:** L. **Dep.:** GAP-0009 (they share the attestation attachment).

### GAP-0009 — SBOM SPDX in-toto attached to the image (area 4)
- **Current:** `delonix-scanner` extracts an SBOM (apk/dpkg) for CVE matching, but does not publish it as an image attestation.
- **Expected (CNCF):** BuildKit attaches an **SPDX** SBOM wrapped in **in-toto**, pluggable scanner.
- **Target 100%:** `delonix build --sbom` emits SPDX JSON (from the existing scanner) in an in-toto attestation, attached to the OCI index; `delonix image sbom <ref>` exports SPDX.
- **Component:** `crates/adapters/delonix-scanner` (SPDX emitter) + `delonix-oci` (attachment). **Compl.:** M. **Dep.:** —.

### GAP-0010 — Cloud-native build: distributed cache + multi-platform (area 5)
- **Current:** local build; no exportable cache nor native multi-arch images.
- **Expected (CNCF):** BuildKit: `--cache-to/--cache-from` (registry), multi-arch `--platform`, frontends.
- **Target 100%:** `delonix build --platform linux/amd64,linux/arm64` produces a multi-arch index; `--cache-to/--cache-from type=registry` round-trip.
- **Component:** `crates/adapters/delonix-oci/src/build.rs` + `registry.rs`. **Compl.:** XL. **Dep.:** —. (LLB frontends remain out of scope — architectural decision.)

### GAP-0011 — Convergence on the Compose Specification (area 6)
- **Current:** own compose with gaps (DEF-003, see GAP-0005).
- **Expected (CNCF):** adhere to the Compose Specification (`compose-spec/compose-spec`).
- **Target 100%:** conformance suite against the spec; common directives green; divergences documented.
- **Component:** `delonix compose`. **Compl.:** M. **Dep.:** GAP-0005.

### GAP-0012 — Proven OCI/distribution/CRI conformance + runtime position (areas 1+2)
- **Current:** own runc-compat runtime; partial CRI; no published pass of the OCI suites.
- **Expected (CNCF):** demonstrable conformance (containerd passes CRI 100%; OCI image/distribution/runtime suites).
- **Target 100%:** CRI ≥95% in rootful-VM (closes with BUG-0003/04/05/06 + GAP-0001/0003); run and publish **oci image-spec/distribution-spec/runtime-spec conformance**; document the "own runtime vs adopt containerd" decision in an ADR.
- **Component:** cross (`delonix-cri`, `delonix-oci`, `delonix-linux`). **Compl.:** XL. **Dep.:** CRI BUGs.

### GAP-0013 — Mature scanner in CI: policy + compare + gating (area 8)
- **Current:** `delonix-scanner` does SBOM+CVE (OSV), without policy/compare/CI.
- **Expected (CNCF):** Scout: policy engine, `compare` between images, `--exit-code --only-severity` for gating.
- **Target 100%:** `delonix scan <ref> --exit-code --only-severity critical,high`; `delonix scan compare A B`; declarable policy (kind).
- **Component:** `delonix-scanner` + CLI `scan`. **Compl.:** M. **Dep.:** —.

### GAP-0014 — Open project governance (area 7)
- **Current:** private repo, own org.
- **Expected (CNCF):** open governance (Moby), SECURITY.md, contribution process, open licence.
- **Target 100%:** owner's decision — go public with documented governance, SECURITY.md, CODEOWNERS, release policy. **Not just code.**
- **Component:** repo/project. **Compl.:** M. **Dep.:** strategic decision.
