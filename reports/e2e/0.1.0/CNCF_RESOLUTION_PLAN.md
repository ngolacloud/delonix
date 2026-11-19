# CNCF gap resolution plan — Delonix

> Derives from `CNCF_ACCEPTANCE_MASTER.md` (2026-11-12). Ordered by **return
> towards CNCF admission**, not by area number. Each wave declares dependencies,
> effort, reuse of existing code and a «done» criterion (a verifiable effect,
> not «it compiles»). GAP IDs aligned with `BACKLOG.md`.

## Ordering principle

1. **Admission first, technical quality after.** Area 7 (governance) is the
   only *entry* blocker; it runs in parallel to everything because it is the
   owner's decision, not code.
2. **Highest code ROI = supply-chain** (3+4): a single primitive (the
   in-toto/OCI attachment) unlocks almost two areas at once by reusing
   `sbom_spdx`+`sign.rs`+`registry.rs`.
3. **Cheap before expensive.** Area 8 (policy over the already-existing OSV) and
   area 2 (kube interop) before area 5 (multi-arch build, XL).
4. **Do not degrade invariants** (rootless-first, fail-closed, CLI/JSON/schema
   contracts) — each wave tests behaviour + a migration note.

## Wave 0 — Governance (area 7) — UNLOCKS ADMISSION · parallel

Owner's decision, not engine engineering. Without it, no wave below moves the
entry verdict.

| Item | Action | Done |
|---|---|---|
| GAP-0014a | Make the repo **public** (clean Apache-2.0, headers, DCO/CLA) | public repo + audited LICENSE/NOTICE |
| GAP-0014b | Documented neutral governance (`GOVERNANCE.md`+`MAINTAINERS.md` with real roles, decision process) | docs with ≥2 maintainers and a process |
| GAP-0014c | Signed + reproducible releases (`release_verify.py`, cosign/PGP, attached SBOM) | signed, verifiable v0.1.x release |
| GAP-0014d | Community signals (live security policy, public roadmap, CoC enforcement) | public SECURITY.md + roadmap |

> **Admission gate:** until Wave 0 closes, the state remains **NO-GO** for
> Sandbox, regardless of the rest.

## Wave 1 — Supply-chain anchor (areas 3+4) — HIGHEST CODE ROI

### 1A · GAP-0009 — in-toto/OCI attachment of attestations · **Compl. M** · dep. —
The pivot. It delivers almost all of area 4 and half of area 3 at once.
- **Reuses:** `sbom_spdx()` (SPDX ready), `sign.rs` (key+signature),
  `registry.rs::push_signature` (push of an OCI artefact in the same repo).
- **Implement:** wrap the SPDX in a **raw in-toto Statement** (`_type:
  https://in-toto.io/Statement/v1`, `predicateType: https://spdx.dev/Document`);
  publish a `artifactType=application/vnd.docker.attestation.manifest.v1+json`
  manifest with `subject=<image>`, an `application/vnd.in-toto+json` layer + an
  `in-toto.io/predicate-type` annotation; reference it in the index with
  `platform=unknown/unknown` + `vnd.docker.reference.type=attestation-manifest`/`.digest`
  annotations.
- **CLI:** `delonix build --sbom`; `delonix image sbom <ref>` exports SPDX.
- **Done:** `cosign verify-attestation` / `docker buildx imagetools inspect`
  reads the attached attestation; `push→pull→verify` round-trip against a local
  registry.

### 1B · GAP-0008 — SLSA provenance in the build · **Compl. L** · dep. 1A
- **Reuses:** the 1A attachment; `build.rs` already knows base/materials/steps.
- **Implement:** instrument the build to emit a
  `https://slsa.dev/provenance/v1` predicate (`buildType`, `buildDefinition{externalParameters,
  resolvedDependencies with checksum}`, `runDetails{builder.id, metadata}`); a
  `--provenance mode=min|max` flag (default max, like BuildKit). `byproducts` may
  stay empty (BuildKit itself does not fill it).
- **Done:** the built image carries a verifiable provenance v1; `mode=min` omits
  build-args/secrets values, `mode=max` includes Dockerfile+sourcemaps.

## Wave 2 — Mature scanning (area 8) · **Compl. M** · dep. — (1A helps GAP-0017)

- **GAP-0013** — declarative `policy` + `scan compare` + CI gating. Leverages the
  already-existing OSV+`--fail-on`; add a policy file (severity per
  package/CVE allowlist) and `delonix scan compare <imgA> <imgB>` (CVE delta).
- **GAP-0017** (Podman lens) — signature trust via policy: extend
  `delonix-security/policy.rs` with per-registry/repo rules (sigstore+key),
  applied at `pull`/`run`. Reuses the Wave 1 attestation format.
- **Done:** `delonix scan --policy p.json` fails CI per the rules; `run` refuses
  an unsigned image under policy.

## Wave 3 — Kubernetes interop (completes area 2) · **Compl. L** · dep. —

- **GAP-0015** — `delonix kube play <k8s.yaml>` runs a Pod/Deployment in
  `apiVersion: v1`/`apps/v1`; `delonix kube generate <ref>` emits valid k8s YAML.
  Podman's biggest CNCF differentiator (k8s is the centre of the ecosystem). A
  k8s↔envelope converter in `delonix-compute`.
- **Done:** `generate → play` round-trip of a 2-member pod; YAML accepted by
  `kubectl --dry-run=client`.

> The engine's CRI conformance (area 2) **is already** at 103/103 — this wave
> closes the *interop* dimension, not the conformance one.

## Wave 4 — Compose convergence (area 6) · **Compl. M–L** · dep. —

- **`${VAR}`/`.env` interpolation** (the largest gap, normative) — a
  substitution pass before parsing: `$V`, `${V}`, `${V:-def}`, `${V-def}`,
  `${V:?err}`, `${V?err}`, nested, `$$` escape; auto-load of `./.env`. A new
  module before `serde_yaml`.
- **GAP-0005** — `compose up` restarts a stopped service (lifecycle semantics).
- **GAP-0011** — implement the long tail of ~35 attributes on demand (priority:
  `pull_policy`, `cgroup_parent`, `pids_limit`, `oom_*`, `gpus`, `blkio_config`);
  top-level `include`.
- **Target reframe (M-DOC):** the «100%» criterion for area 6 becomes **validate
  against `compose-spec.json` + `compose-go` parity** (there is no suite). Fix
  `BACKLOG.md`.
- **Done:** real Docker Compose files with `${VAR}` come up without editing; `up`
  restarts stopped ones; schema validation green.

## Wave 5 — Cloud-native build (area 5) · **Compl. L–XL** · dep. —

Lowest *admission* weight (it is Incubating tier, not Sandbox); do it later.

- **GAP-0010a** — distributed cache `--cache-to/--cache-from type=registry` +
  `mode=min|max`. Reuses `registry.rs` (blob push/pull + cross-repo mount U6);
  the cache tag + mode serialisation is missing. **Compl. L.**
- **GAP-0010b** — `--platform a,b` → **emit an `ImageIndex`**. `registry.rs`
  already *decodes* an index; the *write* direction is missing (assembling an
  index from N per-arch builds). **Compl. XL.**
- **GAP-0016** — `delonix generate systemd/Quadlet` for user workloads (reuses
  `serve.rs`). **Compl. M.**
- **Done:** `--cache-to/from` round-trip between two builds; a multi-arch index
  `imagetools inspect` shows amd64+arm64; a working systemd unit.

## Wave 6 — Runtime lineage (area 1) · **Compl. XL** · dep. CRI BUGs (closed)

- **GAP-0012** — a lineage ADR (position the Rust runtime against containerd/runc;
  justify the own path) + run the OCI image/runtime/distribution suites for
  documented proof. CRI conformance (103/103) is already an input.
- **Done:** published ADR + attached report of green OCI suites.

## Waves → areas → verdict map

| Wave | Areas | Effort | Moves the admission verdict? |
|---|---|---|---|
| 0 Governance | 7 | decision | **YES — unlocks the NO-GO** |
| 1 Supply-chain | 3+4 | M→L | strong technical signal for Sandbox |
| 2 Scanning | 8 | M | reinforces supply-chain |
| 3 Kube interop | 2 | L | k8s = CNCF centre |
| 4 Compose | 6 | M–L | visible correctness |
| 5 Build | 5 | L–XL | Incubating tier |
| 6 Lineage | 1 | XL | narrative/proof |

**Recommended sequence:** Wave 0 in parallel (owner) **+** Waves 1→2→3 (code,
highest return) → 4 → 5 → 6. Admission depends on 0; technical
*competitiveness* rises monotonically with 1→6.

## How to execute (harness)

Each GAP becomes a session prompt driven by the `delonix-backlog-driver` skill
(it picks the next item by dependency→tier→CNCF weight→WIP cap) and validated by
the `delonix-runtime-e2e-quality` skill before closing. One worktree per wave off
`develop`; contracts in `.claude/sessions/`. Waves 1–2 (supply-chain) touch
`delonix-oci`+`delonix-scanner`+`delonix-security` — declare ownership of those
crates to avoid concurrent edits.
