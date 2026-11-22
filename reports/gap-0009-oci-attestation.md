# GAP-0009 — in-toto/OCI attestation attachment (Wave 1A, CNCF areas 3+4)

> Pivot of the CNCF acceptance campaign. Delivers almost the whole of area 4
> (interoperable SBOM) and the attachment mechanism for area 3 (SLSA, GAP-0008).
> Base: develop @ `67ddef41`. Reference contract: `moby/buildkit`
> `exporter/containerimage/writer.go` + `util/attestation/types.go`.

## Objective
`delonix build --sbom` attaches an SPDX SBOM wrapped in in-toto to the image;
`delonix image sbom <ref>` exports the SPDX. The attestation is discoverable by a
standard consumer (`cosign verify-attestation`, `docker buildx imagetools inspect`).

## Contract to implement (BuildKit literals)
- **Raw in-toto statement** (NOT DSSE): `_type: https://in-toto.io/Statement/v1`,
  `predicateType: https://spdx.dev/Document`, `subject: [{name, digest:{sha256}}]`,
  `predicate: <SPDX-2.3>`.
- **Layer** of the attestation manifest: `mediaType application/vnd.in-toto+json`,
  annotation `in-toto.io/predicate-type = https://spdx.dev/Document`.
- **Attestation manifest**: `artifactType =
  application/vnd.docker.attestation.manifest.v1+json`, `config =` empty
  (`application/vnd.oci.empty.v1+json`), `subject =` descriptor of the target
  image's manifest.
- **Discovery**: OCI 1.1 **referrers** via `subject` (push by digest). Optional
  (phase 2): also reference it in an index with `platform=unknown/unknown` +
  annotations `vnd.docker.reference.type=attestation-manifest` /
  `vnd.docker.reference.digest=<img>` (buildx convention).

## Graft points (reuse)
- `delonix-scanner/lib.rs::sbom_spdx()` — SPDX-2.3 already ready → just wrap it.
- `delonix-oci/registry.rs::push_layer_annotated()` (:2548) — builds a manifest
  with `artifact_type` + annotated layer; **`.subject(...)` is missing** → add a
  `push_attestation(subject_descriptor, …)` variant.
- `delonix-oci/sign.rs` — pattern for resolving the digest of the image's manifest
  (reuse to assemble the `subject`).
- `bins/delonix/src/cmd/build.rs:1410` — already calls `write_sbom` per build;
  the hook point for `--sbom` to attach in addition to saving locally.

## Steps
1. **in-toto wrapper** (pure fn `intoto_statement(spdx, image_name, image_digest)`
   in `delonix-scanner` or `delonix-oci`) + shape tests.
2. **`push_attestation`** in `registry.rs`: resolves the target descriptor, builds
   the manifest with `subject`+`artifactType`, pushes by digest.
3. **CLI**: `delonix image sbom <ref> [--attach]`; `build --sbom` wires the
   attachment.
4. **Tests**: round-trip against a local registry (reuse `serve_anon_registry`),
   verify referrers/subject and the `predicate-type`.

## Done (verifiable effect) — DONE 2026-11-12

Implemented:
- in-toto wrapper (`delonix-scanner`: `intoto_statement`, `sbom_intoto_statement`).
- OCI attachment (`delonix-oci/registry.rs`: `push_attestation` — `subject` +
  `artifactType` + `vnd.in-toto+json` layer + `predicate-type` annotation),
  **with a referrers-tag-schema fallback** (`sha256-<hex>` index) for registries
  without a Referrers API.
- Native reader (`list_referrers` + `Referrer`; CLI `delonix image attestations`),
  with the tag-schema fallback when the API returns 404.
- CLI `delonix image sbom <ref> --attach`.

Proof:
- **Unit** (mock registry, real in-process HTTP): `push_attestation_binds_to_the_
  image_as_a_referrer` — push image → attach → Referrers API lists the attestation
  (subject=digest, artifactType, predicate-type); blob = exact Statement. +2 in the
  scanner. Suites: 196 (oci) + 30 (scanner), green.
- **Live** (real registry:3 / distribution, real alpine:3.20 image):
  1. `delonix image attestations` lists `sha256:5099…
     application/vnd.docker.attestation.manifest.v1+json  https://spdx.dev/Document`;
  2. `curl` to the tag-schema index returns the referrer with the annotation;
  3. **`docker buildx imagetools inspect`** (third party) reads the same index.
  This registry's native Referrers API returned 404 → the tag-schema fallback was
  what made the attestation discoverable (as buildx/cosign/oras do).

Notes:
- **Raw, not DSSE** statement (BuildKit shape). `image sign` (cosign `.sig`)
  intact — complementary mechanisms.
- `cosign verify-attestation` does NOT validate this: it requires cosign's own
  DSSE signature. Signing the attestation (DSSE + cosign) is the next step
  (GAP-0017/trust).

---

# GAP-0008 — SLSA provenance at build time (Wave 1B, CNCF area 3) — DONE 2026-11-12

Reuses the GAP-0009 mechanism entirely (in-toto wrapper + `push_attestation` +
tag-schema fallback). It only adds: generating the **SLSA v1 predicate** during
the build.

Implemented:
- `delonix-scanner`: `PREDICATE_TYPE_SLSA`, `ProvenanceMode{Min,Max}`, `Material`,
  `ProvenanceFacts`, `slsa_provenance_predicate()` (pure; BuildKit shape
  `buildDefinition{buildType,externalParameters,internalParameters,
  resolvedDependencies}` + `runDetails{builder,metadata}`; `byproducts` omitted
  as in BuildKit). Min withholds build-arg values; Max includes them.
- `build --provenance[=min|max]` (default max when present): `write_provenance`
  re-parses the Dockerfile, resolves the `FROM` bases in the store →
  `resolvedDependencies` (materials), and caches to
  `<root>/images/<id>.provenance.json`.
- `image provenance <ref> [--attach]`: prints, or wraps in in-toto (SLSA
  predicate) and attaches via `push_attestation`.

Proof:
- **Unit**: `slsa_max_includes_build_arg_values_and_materials`,
  `slsa_min_withholds_build_arg_values_but_keeps_keys` (scanner 32 green).
- **Live** (registry:3 + real build `FROM alpine:3.20`):
  1. `build --provenance=max` generated an SLSA v1 predicate with material
     `alpine:3.20` @ `bf8527eb…` (real digest), builder id, finishedOn,
     invocationId;
  2. `image provenance --attach` published it;
  3. `image attestations` lists **both** attestations on the same image:
     `https://spdx.dev/Document` **and** `https://slsa.dev/provenance/v1`;
  4. independent curl to the tag-schema index: 2 referrers, distinct
     predicate-types.

CNCF result: **area 3 (SLSA) from ✗ → ◑ strong** (predicate emitted + attached,
verifiable); **area 4 (SBOM) → ✔ on the mechanism** (generated + attached +
discovered). Clippy `-D warnings` clean; folhas.tsv updated (image
attestations/provenance).

Follow-up (not in this wave): sign the attestations (DSSE+cosign) for
`cosign verify-attestation` — fits into GAP-0017 (trust by policy).

---

# GAP-0013 — Scanner policy + compare + CI gating (Wave 2, CNCF area 8) — DONE 2026-11-12

Leverages the existing OSV + `--fail-on`. Adds the maturity layer that Docker
Scout has and Delonix did not.

Implemented:
- `delonix-scanner` (pure): `Policy{fail_on,ignore,ignore_unfixed}` + `Policy::load`,
  `evaluate_policy() -> PolicyOutcome{kept,waived,worst,failed}`,
  `compare_findings() -> ScanDiff{added,removed}` (key `(id,package)`).
- CLI `image scan`:
  - `--policy <file>`: declarative gate (threshold + waivers with audit trail +
    `ignore_unfixed`); precedes `--fail-on`; exits 1 on failure.
  - `--compare-to <image>`: CVE delta between two images (like `scout compare`).
  - `findings_for` helper (reuses extract_sbom + load_advisories + scan without
    printing).

Proof:
- **Unit** (scanner 35 green): `policy_waives_by_id_and_judges_the_rest_against_fail_on`,
  `policy_ignore_unfixed_drops_findings_with_no_fix`,
  `compare_reports_added_and_removed_by_id_and_package`.
- **Live** (alpine:3.20 busybox 1.36.1-r31; forged advisory fixed r40, High):
  - (A) `scan --fail-on high` → **exit 1** (CI gate fails on the finding);
  - (B) `scan --policy` (CVE waiver) → "policy waived 1 finding(s)" → **exit 0**;
  - (C) `scan alpine:3.20 --compare-to debian:12-slim` → `- removed CVE … busybox High`
    (the apk advisory does not match on dpkg — correct delta).
- Clippy `-D warnings` clean; no new CLI sheet (only flags on `image scan`).

CNCF result: **area 8 (scanning) from ◑ → ◑ strong / nearly ✔** — declarative
policy, compare and CI gating, all on top of the existing OSV. What remains
(GAP-0017) is signature trust by policy, which reuses this campaign's attestation
attachment.

---

# GAP-0017 — Signature trust by declarative policy (Wave 2, CNCF areas 4/8) — DONE 2026-11-12

Closes the supply-chain axis. Reuses `sign.rs::verify_signature` (cosign by key) +
the admission-on-pull pattern (like `admission_scan_on_pull`).

Implemented (CLI, `scan.rs`):
- `repo_matches(pattern, reference)` — pure: matches the repo (without
  :tag/@digest) by exact or wildcard prefix (`host/ns/*`).
- `admission_trust_on_pull(images, reference)` — reads `$DELONIX_TRUST_POLICY` or
  `<root>/trust.json`; if a rule matches the repo, it verifies the cosign signature
  against the trust key; `mode:"enforce"` (default) REFUSES unsigned,
  `mode:"warn"` only warns. No policy / no matching rule → no requirement
  (explicit, never a silent allow for a repo under policy).
- Hook in `image pull`: trust BEFORE the scan-on-pull ("who it is" before "what it
  carries").

`trust.json` format:
`{ "mode":"enforce", "rules":[ {"repo":"ghcr.io/ngolacloud/*", "key":"/path/key.pub"} ] }`

Proof:
- **Unit**: `repo_matches_exact_wildcard_and_drops_the_tag` (bin green).
- **Live** (registry:3 + delonix `image sign`):
  - (A) pull of the **signed** image → "→ trust: valid signature …" → **exit 0**;
  - (B) pull of the **unsigned** one (enforce) → "refused: not signed by a key
    trusted for this repo per the trust policy …" → **exit 1**;
  - (C) pull of the unsigned one (mode=warn) → warning + image pulled → **exit 0**.
- Clippy `-D warnings` clean; no new CLI sheet (env+file on the pull path).

CNCF result: **areas 4/8 reinforced** — declarative signature trust per repo,
fail-closed, applied at pull. The supply-chain axis (3+4+8) is now coherent:
SBOM+provenance attached (0009/0008), scanning with policy/compare/gate (0013),
and trust by policy (0017).

Follow-up (external interop, blocked on the host): sign the attestations in DSSE +
cosign `.att` for `cosign verify-attestation` to validate them (cosign not
installable here). Trust-by-policy is already verifiable natively.

---

# GAP-0015 — Kubernetes YAML interop: kube play/generate (Wave 3, CNCF area 2) — DONE 2026-11-12

Podman's biggest CNCF differentiator (k8s is the centre of the ecosystem).
Maximum reuse: Delonix's `PodSpec`/`PodContainer` ALREADY uses k8s field names
(serde rename) — the only difference from a k8s v1 Pod is the `apiVersion`.

Implemented:
- `delonix kube play <k8s.yaml>` (`kube.rs::cmd_play`): parses standard k8s YAML
  (multi-doc), deserializes the `spec` directly into `PodSpec` and calls
  `pod::create_pod` (made `pub(crate)`). `kind: Pod` (apiVersion v1); Deployment/
  ReplicaSet/StatefulSet/DaemonSet run as 1 pod from `spec.template.spec`; unknown
  kinds warned and skipped.
- `delonix kube generate <ref>`: already existed (under `cluster kube`);
  **fixed** the pod-detection bug (it filtered by `.pod`, which stores the NETNS
  name `pod-<name>`; now it filters by the `delonix.io/pod` label).
- Top-level command `delonix kube {play,generate}` (main.rs) — parity with
  `podman kube play/generate`. `cluster kube generate` is kept.

Proof:
- **Unit**: kube (12 green, includes pod_manifest).
- **Live** (rootless host, short net-runtime-dir for SUN_LEN):
  1. `kube play` of a **standard** k8s Pod (`apiVersion: v1`) → `played pod
     demo-kube`, pod Running (ip 10.200.0.2), container running `sleep 3600`;
  2. `kube generate demo-kube` → valid k8s YAML (apiVersion v1, kind Pod,
     metadata, containers name/image/command/args/resources);
  3. independent structural validation (python yaml) → correct Pod v1;
  4. **round-trip**: `kube play` of the GENERATED YAML → `played pod demo-rt`.
  (kubectl --dry-run=client fails only because there is no cluster/openapi — not a
  defect of the YAML.)
- Clippy `-D warnings` clean; folhas.tsv +2 (kube play/generate).

CNCF result: **area 2 (k8s interop) from ✗ → ◑ strong** — runs and generates
standard Kubernetes workload YAML (the interop dimension, distinct from CRI
conformance which was already at 103/103). Note remains: advanced k8s fields
(probes, lifecycle) are refused by `POD_CONTAINER_FIELDS` — future incremental
expansion.

---

# CNCF area 6 — Compose `${VAR}` interpolation + `.env` (Wave 4) — DONE 2026-11-12

The biggest NORMATIVE gap in Compose (`12-interpolation.md`), which Delonix did
not have (it read the YAML directly via serde_yaml, without substitution or
`.env`).

Implemented (`compose.rs`, pure + wired):
- `interpolate(text, lookup)` — shell-style pass BEFORE the YAML parse:
  `$VAR`, `${VAR}`, `${VAR:-def}`, `${VAR-def}`, `${VAR:?err}`, `${VAR?err}`,
  `${VAR:+rep}`, `${VAR+rep}`, nesting in the argument, and `$$` → literal `$`.
  Works on bytes (multibyte UTF-8 passes through intact). Bare unset → empty;
  `:?`/`?` unset → error with message.
- `parse_dotenv` — `.env` KEY=VALUE (comments/quotes/`export` handled).
- `load_compose`: reads the `.env` from the project dir, process env OVERRIDES the
  `.env` (docker compose precedence), interpolates each file before the parse.

Proof:
- **Unit** (compose 92 green): `interpolation_forms_and_escape` (all forms + `$$`),
  `interpolation_required_unset_is_an_error`, `dotenv_parsing_strips_quotes_and_comments`.
- **Live** (`compose config`):
  - `.env` with `TAG=3.20` → `image: alpine:3.20`;
  - `${PORT:-9000}` (PORT absent) → `ports: ["9000:80"]`;
  - `TAG=9.9` in the shell → `image: alpine:9.9` (env beats the `.env`).
- 89→92 compose tests without regression; clippy `-D warnings` clean.

Note (correction of the area 6 target): there is NO Compose conformance suite; the
target is to validate against `compose-spec.json` + `compose-go` parity.
Interpolation was the missing normative piece — now present. What remains is
GAP-0005 (up restart, already fixed earlier) and the long tail of attributes
(GAP-0011).

CNCF result: **area 6 (Compose) from ◑ → ◑ strong** — common surface + normative
interpolation + fail-closed allowlist.
