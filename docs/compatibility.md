<!--
  GENERATED FILE — DO NOT EDIT BY HAND.
  Source: tests/interop/results/*.jsonl + tests/interop/LIMITS.md.
  Regenerate: python3 scripts/gen_compatibility.py
-->

# Delonix — CNCF ecosystem compatibility

This matrix is **generated** from the measured interoperability records in `tests/interop/`. Every cell is a claim with a tool version, a mode and a proof — or it says **not tested**. A claim is never written as "compatible" without a test that proves it (engine doctrine; see `AGENTS.md`, "Engine identity and boundary").

**Modes.** Mode 1 = Delonix *under* a real Kubernetes, via the CRI. Mode 2 = Delonix *without* Kubernetes, open interfaces only.

**Result.** ✅ pass (proven) · ◑ partial (measured, incomplete) · ✗ fail (measured not to work) · ☐ not tested (no proof yet).

**Totals:** ✅ pass: 1 · ◑ partial: 8 · ✗ fail: 0 · ☐ not tested: 21 · records: 30

## H0 — SPIFFE identity

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `spiffe-spire` | v1.16.0 | 0.1.2 | 2 (no-K8s) | rootless | — | ✅ pass | [reports/interop/h0-spiffe/README.md](../reports/interop/h0-spiffe/README.md) |

<details><summary>Notes</summary>

- **`spiffe-spire`** — spire-server v1.16.0 mints the node X.509-SVID spiffe://example.org/node off its own CA; delonix-spiffe builds rustls server+client configs from it and completes a mutual-TLS handshake, verifying the peer by its spiffe:// URI-SAN against the SPIRE bundle. The own-CA (rcgen) fallback and the node contract's AttestWorkload RPC (PID->workload+digest+labels) land in the same phase (ADR-0102). Identity only: NOT wired onto a live node TCP listener (A1/ADR-0096), no SPIRE agent/Workload API/rotation (A2).

</details>

## H1 — Kubernetes (certifiable, Mode 1)

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `kubeadm` | 1.37 | 0.1.2 | 1 (K8s) | root | privileged, hostNetwork | ◑ partial | [reports/ha-multinode-0.1.2/report.md](../reports/ha-multinode-0.1.2/report.md) |
| `kubernetes-conformance` | — | — | 1 (K8s) | root | — | ☐ not tested | — |
| `kubernetes-cri` | v1.36.0 | 0.1.2 | 1 (K8s) | root | privileged | ◑ partial | [reports/cri-conformance-0.1.2/report.md](../reports/cri-conformance-0.1.2/report.md) |

<details><summary>Notes</summary>

- **`kubeadm`** — HA control plane with stacked etcd brought up and Ready; this is a bootstrap proof, not a conformance claim.
- **`kubernetes-conformance`** — No sonobuoy certified-conformance run yet. Target: the three supported Kubernetes minor releases, published per release alongside critest.
- **`kubernetes-cri`** — ~101/103 effective; 2 real gaps CRI-F1 (simultaneous RemoveImage) and CRI-F2 (drop ALL caps). The cncf-acceptance summary claims later closure to 103/103, but no fresh clean critest console is in-tree yet. H1 produces the clean run.

</details>

## H2 — Demanding DaemonSets (Mode 1)

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `argo-cd` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |
| `cilium` | — | — | 1 (K8s) | root | privileged, hostNetwork, eBPF | ◑ partial | [reports/cilium-d7-rerun-2026-10-27/FINDINGS.md](../reports/cilium-d7-rerun-2026-10-27/FINDINGS.md) |
| `coredns` | — | — | 1 (K8s) | root | — | ◑ partial | [reports/ha-multinode-0.1.2/report.md](../reports/ha-multinode-0.1.2/report.md) |
| `falco` | — | — | 1 (K8s) | root | privileged, eBPF, hostPID | ☐ not tested | — |
| `flux` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |
| `istio-ambient` | — | — | 1 (K8s) | root | privileged, hostNetwork | ☐ not tested | — |
| `linkerd` | — | — | 1 (K8s) | root | hostNetwork | ☐ not tested | — |
| `prometheus-node-exporter` | — | — | 1 (K8s) | root | hostPath | ☐ not tested | — |
| `rook-ceph` | — | — | 1 (K8s) | root | privileged, hostPath, mountPropagation | ☐ not tested | — |

<details><summary>Notes</summary>

- **`argo-cd`** — Mode 1 only (limit: Delonix is not a kube-apiserver). Not tested.
- **`cilium`** — default resolved to cilium; both nodes Ready in 2m12s. Hubble and kube-proxy-replacement not exercised; cilium version not pinned in the report (so not a PASS).
- **`coredns`** — Runs embedded in the kubeadm cluster; no dedicated interop test and no pinned version.
- **`falco`** — Modern eBPF driver + CRI-socket metadata not tested on a Delonix cluster.
- **`flux`** — Mode 1 only. Not tested.
- **`istio-ambient`** — ztunnel + Istio CNI not tested.
- **`linkerd`** — Not tested.
- **`prometheus-node-exporter`** — Not tested as a DaemonSet on a Delonix cluster.
- **`rook-ceph`** — Rook-Ceph DaemonSet / CSI (RBD/CephFS) not tested on a Delonix cluster. The native Ceph provider is proven separately (see h7-nokube: ceph).

</details>

## H3 — Delonix under containerd / CRI-O

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `containerd-shim` | — | — | 1 (K8s) | root | — | ☐ not tested | — |
| `cri-o` | — | — | 1 (K8s) | root | — | ☐ not tested | — |
| `oci-runtime` | — | — | 1 (K8s) | rootless | — | ◑ partial | [reports/conformance/B1-oci-runtime-baseline.md](../reports/conformance/B1-oci-runtime-baseline.md) |

<details><summary>Notes</summary>

- **`containerd-shim`** — No containerd-shim-v2 for the sandbox (microVM) runtime class yet. The CRI serves the kubelet directly; the shim is greenfield.
- **`cri-o`** — Equivalent CRI-O runtime-handler configuration not written or tested.
- **`oci-runtime`** — delonix oci-runtime create/start/state/kill/delete present (ADR-0080); runtime-tools suite wired. A full green runtime-tools result is not yet recorded as the campaign proof.

</details>

## H4 — Observability (Modes 1 and 2)

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `fluent-bit` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |
| `fluentd` | — | — | 2 (no-K8s) | rootless | — | ☐ not tested | — |
| `jaeger` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |
| `opentelemetry` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |
| `prometheus` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |

<details><summary>Notes</summary>

- **`fluent-bit`** — CRI log format /var/log/containers is written. Fluent Bit collection of Delonix logs not tested in either mode.
- **`fluentd`** — Standalone log drivers (journald, otlp, fluentd-forward) not proven. The lab _labs/logging uses Vector -> 3 sinks; no fluentd-forward interop test.
- **`jaeger`** — Direct OTLP reception by Jaeger not exercised.
- **`opentelemetry`** — OTLP present in delonix-telemetry. No OpenTelemetry Collector ingestion test; engine-operation spans (pull/build/run/vm create) and container.*/k8s.* semantic conventions not verified.
- **`prometheus`** — Engine exposes Prometheus metrics (DELONIX_MGMT_METRICS_ADDR) and CRI container stats; cAdvisor names are partial (container_memory_working_set_bytes present, container_cpu_usage_seconds_total missing). No dashboard-ingestion interop test yet, and ListContainerStats/ListPodSandboxMetrics completeness unverified.

</details>

## H5 — Policy and runtime security

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `falco` | — | — | 2 (no-K8s) | root | eBPF, hostPID | ☐ not tested | — |
| `opa-rego` | — | — | 1 (K8s) | rootless | — | ☐ not tested | — |

<details><summary>Notes</summary>

- **`falco`** — Mode 2 via the delonix-cri socket metadata not tested. Acceptance: a Falco event carries the Delonix container name and image.
- **`opa-rego`** — No Rego admission gate. The phase decision is in-process regorus vs external OPA over HTTP, fail-closed. Native admission rules exist; Rego is greenfield. Reference acceptance: a Rego policy refuses an unsigned image.

</details>

## H6 — Registry and supply chain

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `cosign` | — | — | 2 (no-K8s) | rootless | — | ◑ partial | [reports/gap-0009-oci-attestation.md](../reports/gap-0009-oci-attestation.md) |
| `harbor` | — | — | 2 (no-K8s) | rootless | — | ☐ not tested | — |
| `oci-distribution` | — | — | 2 (no-K8s) | rootless | — | ◑ partial | [reports/conformance/A1-oci-image-distribution.md](../reports/conformance/A1-oci-image-distribution.md) |

<details><summary>Notes</summary>

- **`cosign`** — image sign present; SPDX-2.3 SBOM emitted. The in-toto wrapper + OCI attachment (GAP-0009) and SLSA provenance predicate (GAP-0008) are pending.
- **`harbor`** — Matrix against Harbor (token, robot accounts, proxy cache, replication), referrers API, cosign verify and SPDX SBOM visibility not tested.
- **`oci-distribution`** — Client passes a baseline of the OCI Distribution spec. The full conformance-as-client matrix and the referrers API are not complete.

</details>

## H7 — Interoperability without Kubernetes (Mode 2)

| Tool | Version | Delonix | Mode | Privilege | Host capabilities | Result | Proof |
|---|---|---|---|---|---|---|---|
| `ceph` | — | — | 2 (no-K8s) | root | — | ◑ partial | [reports/ceph-live/cluster-access.md](../reports/ceph-live/cluster-access.md) |
| `envoy-xds` | — | — | 2 (no-K8s) | rootless | — | ☐ not tested | — |
| `helm` | — | — | 2 (no-K8s) | rootless | — | ☐ not tested | — |
| `kube-play` | — | — | 2 (no-K8s) | rootless | — | ☐ not tested | — |

<details><summary>Notes</summary>

- **`ceph`** — Native Ceph provider proven live (RADOS/RBD/CephFS/RGW, ADR-0081). Consuming a Rook-managed cluster as external Ceph through the native provider is not tested.
- **`envoy-xds`** — Envoy as an xDS L7 data plane for HTTPRoute is an ADR decision, not implemented. Envoy appears today only as a Cilium workload, not as an engine component.
- **`helm`** — 'helm template | delonix kube play' not proven. In Mode 1 Helm works through the real Kubernetes; that path is covered by H1.
- **`kube-play`** — delonix kube play/generate exist (podman-style, Pod subset, fail-closed allowlist) in bins/delonix/src/cmd/kube.rs. The closed subset for interop (Deployment, Service, ConfigMap, Secret, PersistentVolumeClaim) and 'helm template | delonix kube play' are not proven. Acceptance: a simple Helm chart runs in Mode 2; an unsupported field is refused by name.

</details>

## Limits (part of the contract)

The limits below are part of the contract, not gaps to be closed. They say where a
compatibility claim deliberately stops.

- **Delonix is not a kube-apiserver.** Helm, Argo CD and Flux only work end-to-end in
  **Mode 1** (under a real Kubernetes). In **Mode 2** only the closed `kube play`
  subset applies. GitOps of the engine itself over Kubernetes is done by an external
  operator, never inside the engine.
- **No service mesh in Mode 2.** Istio and Linkerd only run over Kubernetes (Mode 1).
- **`kube play` has no controllers.** No rollout, no ReplicaSet, no HPA. What does not
  map to the closed subset (Deployment, Service, ConfigMap, Secret,
  PersistentVolumeClaim) is **refused by the field name**, never silently ignored.
- **Rootless refuses privileged workloads.** Cilium, Falco, Rook-Ceph, the Istio CNI and
  anything that needs privileged eBPF, `hostNetwork` or `privileged` run in **root mode
  only**; under rootless they are refused with "not implemented in rootless" — never
  "impossible".
- **The sandbox runtime class is not for privileged DaemonSets.** Privileged DaemonSets
  and `hostNetwork` workloads do not run inside a microVM; the request is refused.
- **Architectures:** only the ones a release publishes (today `amd64` and `arm64`).
- **Vitess, TiKV and a customer's etcd are workloads, not engine capabilities.** They run
  on the engine; the engine does not claim to operate them.
- **CNCF Sandbox admission depends on governance** (public repository, neutral
  governance), which is an owner's decision outside this campaign's scope — reported,
  not decided. See `reports/cncf-acceptance-2026-11-12/report.md` and
  `CNCF-SANDBOX-READINESS.md`.

