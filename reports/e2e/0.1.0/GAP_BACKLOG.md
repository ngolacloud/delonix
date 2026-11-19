# GAP_BACKLOG.md — 0.1.0 (@ 1898466b)

Missing or unmeasured functionality — not a defect in something that should
already work. Comparison lenses: Docker 29.9.2, Podman 4.9.3, CRI v1.36.0.

> GAP-0001..0007: Docker/Podman parity + CRI (test round).
> **GAP-0008..0014: 8 CNCF value areas** (see `CNCF_GAP_DOCKER.md` — a reading
> of the official Docker/Moby code). Goal: 100% across the 8 areas.

## P1

### GAP-0001 — PodPID / `shareProcessNamespace` refused in host-network/CNI pods
- **Current behaviour:** the path exists — `crates/contexts/delonix-compute/src/domain/pod.rs:149-150` (`share_process_namespace`), applied at `:406-435`; `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:100` (`wants_shared_pid`), `wire_shared_pid` `:1913-1945`. **But there is a refusal path for hostNetwork/CNI pods at `:1930-1945`**, and the critest pod falls into it.
- **Expected platform behaviour:** Docker/Podman/CRI support `shareProcessNamespace` — the containers of a pod see each other's PIDs.
- **Why it matters:** the sidecar pattern (process sharing, signal-handling). CRI conformance (spec «should support PodPID»).
- **Acceptance criteria:** the PodPID spec passes on rootful-VM.
- **Dependencies:** coupled to **BUG-0004** (the refusal is for hostNetwork/CNI). **Complexity:** M.

### GAP-0002 — `differential` coverage limited to 2 flows
- **Current behaviour:** `tests/cli-parity/run.sh differential` exercises only flow1 (interactive container) and flow2 (pod). 76 commands remain «recognised-to-be-tested» (parser) and 36 «to-do» with no measured functional equivalence vs docker/podman.
- **Expected platform behaviour:** a credible parity matrix measures volumes, networks and images against both reference engines, not just containers/pods.
- **Why it matters:** the differential «PASS» rests on 2 flows; the harness itself warns that it does not claim 100%. Without this, the parity claim is partial.
- **Acceptance criteria:** differential flows for volumes (create/rm/inspect round-trip), networks (create/connect/rm) and images (pull/tag/save/load) running on all 3 engines.
- **Dependencies:** none. **Complexity:** M. (= former M3)

### GAP-0003 — Streaming port-forward in host-network
- **Current behaviour:** `crates/interfaces/delonix-cri/src/streaming.rs:227-261` has the host-network branch (connects directly in the host netns, guarded by `sandbox_is_host_network`, `:239`); `spdy.rs:854` `handle_port_forward`.
- **Expected platform behaviour:** CRI supports portforward for pods on host network.
- **Why it matters:** CRI conformance (spec «portforward in host network»); kubectl port-forward for host-network pods.
- **Acceptance criteria:** the spec passes on rootful-VM.
- **Dependencies:** coupled to **BUG-0004** (sandbox_is_host_network). **Complexity:** S after B4.

## P2

### GAP-0004 — Docker Engine API only partial (logs + exec)
- **Current behaviour:** only `logs` and `exec` implemented from the Docker Engine API.
- **Expected platform behaviour:** Docker ecosystem clients/tools expect the API surface (containers, images, networks, volumes, events).
- **Why it matters:** ecosystem portability (Testcontainers, external Compose, SDKs).
- **Acceptance criteria:** roadmap per endpoint family; conformance measured against a real client.
- **Dependencies:** none. **Complexity:** XL.

### GAP-0005 — Compose with gaps (DEF-003)
- **Current behaviour:** `delonix compose` supported with known gaps (DEF-003).
- **Expected platform behaviour:** parity with `docker compose` v2 on the common directives.
- **Acceptance criteria:** close DEF-003; measured compose-directive matrix.
- **Dependencies:** none. **Complexity:** M.

## P3

### GAP-0006 — Contexts / remote-mgmt (`system connection`) out-of-scope
- **Current behaviour:** 22 `system`/context commands marked out-of-scope by ADR-0010.
- **Expected platform behaviour:** docker `context` / podman `system connection`.
- **Why it matters:** multi-host management. It is a deliberate architectural decision — record it as WONT_FIX until ADR-0010 reopens, not as silent debt.
- **Dependencies:** ADR-0010. **Complexity:** L.

### GAP-0007 — 36 «to-do» commands in the CLI matrix
- **Current behaviour:** includes `network prune` and `image`/`volume` families to be completed (see `tests/cli-parity/matriz-cli-parity.csv`).
- **Acceptance criteria:** each command moves up from «recognised» → «passed» or is classified out-of-scope with a reason.
- **Dependencies:** GAP-0002 (measurement). **Complexity:** varies (S–M per command).

---

# CNCF value areas (GAP-0008..0014)

Full rationale in `CNCF_GAP_DOCKER.md`. Already aligned: OSV (scanner),
cosign/sigstore (`sign.rs`), rootless-first.

## P1

### GAP-0008 — SLSA provenance in `delonix build` (CNCF area 3)
- **Current behaviour:** `delonix-oci/src/build.rs` builds the image but does not emit provenance.
- **Expected platform behaviour:** BuildKit emits SLSA provenance (v0.2/v1.1, mode min/max, builder-id, reproducible) as an in-toto attestation attached to the image.
- **Why it matters:** verifiable supply-chain — the signal CNCF today expects from a builder (SLSA). Additive: `sign.rs` (cosign) already provides the attestation mechanism.
- **Acceptance criteria:** `delonix build --provenance[=mode=min|max]` attaches a SLSA v1.1 predicate (builder.id, materials, invocation) in in-toto to the OCI index; verifiable by a third-party client.
- **Dependencies:** GAP-0009 (they share the attestation attachment). **Complexity:** L.

### GAP-0009 — SBOM SPDX in-toto attached to the image (CNCF area 4)
- **Current behaviour:** `delonix-scanner` extracts an SBOM (apk/dpkg) for CVE matching; it does not publish it as an image attestation.
- **Expected platform behaviour:** BuildKit attaches an SBOM in **SPDX** as **in-toto**, with a pluggable scanner (syft).
- **Why it matters:** supply-chain interoperability (consumable by Scout, cosign, SLSA chains).
- **Acceptance criteria:** `delonix build --sbom` emits SPDX JSON (from the existing scanner) in in-toto attached to the OCI index; `delonix image sbom <ref>` exports SPDX.
- **Dependencies:** —. **Complexity:** M.

### GAP-0012 — OCI/distribution/CRI conformance proven + runtime positioning (CNCF areas 1+2)
- **Current behaviour:** own runc-compat runtime; partial CRI (84/19 · 90/13); no published pass of the OCI suites.
- **Expected platform behaviour:** containerd (CNCF graduated) passes CRI 100%; Moby is the OCI image/distribution reference.
- **Why it matters:** CNCF values demonstrable conformance and reuse of graduated projects.
- **Acceptance criteria:** CRI ≥95% rootful-VM (closes with BUG-0003/04/05/06 + GAP-0001/0003); run+publish **oci image-spec/distribution-spec/runtime-spec conformance**; ADR to decide «own runtime vs adopt containerd».
- **Dependencies:** CRI BUGs. **Complexity:** XL.

## P2

### GAP-0011 — Convergence to the Compose Specification (CNCF area 6)
- **Current behaviour:** own compose with gaps (DEF-003, see GAP-0005).
- **Expected platform behaviour:** adhere to the Compose Specification (`compose-spec/compose-spec`, community-governed).
- **Why it matters:** manifest interoperability; the spec is community-governed.
- **Acceptance criteria:** conformance suite against the spec; common directives green; divergences documented.
- **Dependencies:** GAP-0005. **Complexity:** M.

### GAP-0013 — Mature scanner in CI: policy + compare + gating (CNCF area 8)
- **Current behaviour:** `delonix-scanner` does SBOM+CVE (OSV); no policy/compare/CI.
- **Expected platform behaviour:** Docker Scout: policy engine, `compare` between images, `--exit-code --only-severity`.
- **Why it matters:** supply-chain gating in pipelines — real cloud-native use.
- **Acceptance criteria:** `delonix scan <ref> --exit-code --only-severity critical,high`; `delonix scan compare A B`; declarable policy (kind).
- **Dependencies:** —. **Complexity:** M.

### GAP-0010 — Cloud-native build: distributed cache + multi-platform (CNCF area 5)
- **Current behaviour:** local build; no exportable cache or native multi-arch.
- **Expected platform behaviour:** BuildKit: `--cache-to/--cache-from` (registry), `--platform` multi-arch, LLB frontends.
- **Why it matters:** reproducible and shareable CI builds; multi-arch images.
- **Acceptance criteria:** `delonix build --platform linux/amd64,linux/arm64` → multi-arch index; `--cache-to/--cache-from type=registry` round-trip. (LLB frontends out of scope — architectural decision.)
- **Dependencies:** —. **Complexity:** XL.

## P3

### GAP-0014 — Open project governance (CNCF area 7)
- **Current behaviour:** private repo, own org.
- **Expected platform behaviour:** open governance — Moby (donated runc/containerd); Podman in LFX/Linux Foundation with **PGP-signed releases**, a security mailing list, **LTS**, a public roadmap, LFX Health Score.
- **Why it matters:** CNCF weighs openness and governance, not just code.
- **Acceptance criteria:** owner's decision — public with documented governance, SECURITY.md, CODEOWNERS, release policy, **signed releases (PGP/sigstore)**, a security channel, public roadmap. **It is not just code.**
- **Dependencies:** strategic decision. **Complexity:** M.

---

# Podman differentiators (GAP-0015..0017)

Rationale in `CNCF_GAP_PODMAN.md`. Delonix is already aligned on daemonless,
rootless, pods, runtime `policy.json`, cosign verify and the node-api systemd unit.

## P1

### GAP-0015 — Kubernetes YAML interop: `kube play` / `kube generate` (CNCF area 2)
- **Current behaviour:** own `compute.delonix.io/v1beta1` manifests; `lower_application` lowers the Application to pods; it neither reads nor emits standard Kubernetes workload YAML.
- **Expected platform behaviour:** `podman kube play <k8s.yaml>` runs a k8s Pod/Deployment; `podman kube generate` emits k8s YAML from an existing pod/container.
- **Why it matters:** Kubernetes is the centre of the CNCF ecosystem; running/generating k8s YAML is Podman's strongest interop differentiator.
- **Acceptance criteria:** `delonix kube play <manifest.yaml>` runs workloads in `apiVersion: v1`/`apps/v1` (Pod at minimum); `delonix kube generate <ref>` emits valid k8s YAML (round-trip verified).
- **Dependencies:** —. **Complexity:** L.

## P2

### GAP-0016 — Quadlet/systemd for user workloads (CNCF area 5)
- **Current behaviour:** `delonix serve` generates a unit only for the `node-api` (`cmd/serve.rs`), not for user containers/pods.
- **Expected platform behaviour:** Quadlet / `podman generate systemd` — run containers/pods as systemd units (boot, supervision).
- **Why it matters:** native integration with the host init — cloud-native operation on single-host nodes.
- **Acceptance criteria:** `delonix generate systemd <container|pod>` emits a working unit; optionally the declarative Quadlet format.
- **Dependencies:** —. **Complexity:** M.

### GAP-0017 — Signature trust via declarative policy (CNCF areas 4/8)
- **Current behaviour:** `sign.rs` verifies cosign by explicit key; `policy.json` exists but for admission/runtime, not per-registry signature trust.
- **Expected platform behaviour:** Podman's `containers-policy.json` — declarative trust per registry/repo (sigstore + GPG), applied at pull/run.
- **Why it matters:** an actionable trust chain in production (refuse an unsigned image by policy), not just ad-hoc verification.
- **Acceptance criteria:** extend `policy.json` with per-registry/repo signature trust (sigstore + key); `delonix container run`/`image pull` refuses an image outside the policy.
- **Dependencies:** GAP-0009 (attestation format). **Complexity:** M.
