# CNCF gap analysis — Delonix × Podman (specific differentiators)

> Read from Podman's official code (via `gh`). **Org note:** the project
> renamed itself from `containers/*` to **`podman-container-tools/*`**
> (`podman`, `buildah`, `skopeo`); `crun`/`image`/`storage` remain under
> `containers/*`. Cross-referenced with the Delonix code.

## Positioning
Delonix is **philosophically closer to Podman** than to Docker:
daemonless, rootless-first, native pods. This analysis therefore focuses on what
Podman has that is **distinctive** and that Delonix does not yet have — it does not repeat what is already
in `CNCF_GAP_DOCKER.md`.

## Already aligned with Podman (do not redo)
- **Daemonless + rootless-first** — same architecture. ✔
- **Native pods** — Delonix has them (declarative, `kind: Pod`). ✔
- **Runtime `policy.json`** — `delonix-security/src/domain/policy.rs` (`<root>/policy.json`, `RuntimePolicy`, admission). ✔
- **systemd unit** — `delonix serve` generates `delonix-node-api.service` (but only for the service itself — see GAP-0016). ◑
- **cosign verify** — `delonix-oci/sign.rs`. ✔

## Podman differentiators → new GAPs

| Podman differentiator (evidence) | Delonix today | GAP | CNCF area |
|---|---|---|---|
| **`podman kube play` / `kube generate` / `kube apply`** — runs and generates **standard Kubernetes YAML** (docs `podman-kube-*.md`) | **own** kinds (`compute.delonix.io/v1beta1`), `lower_application`; does **not** consume/generate `apiVersion: v1`/`apps/v1` | **GAP-0015** | 2 (k8s interop) |
| **Quadlet / `generate systemd`** — runs the user's containers/pods as **systemd units** (docs `podman-quadlet-*`, `podman-generate-systemd`) | generates a unit only for `node-api`, not for user workloads | **GAP-0016** | 5 (cloud-native ops) |
| **Trust-policy verification** — `containers/image` `containers-policy.json` + sigstore + GPG (declarative trust per registry/repo) | has an **admission/runtime** `policy.json` and `sign.rs` (cosign verify by key), but **no** declarative signature trust per registry | **GAP-0017** | 4/8 (supply-chain) |
| **Mature LFX governance** — LFX Health Score, **PGP-signed** releases, security mailing list, LTS, public roadmap | private repo | reinforces **GAP-0014** | 7 (governance) |
| **Skopeo** — `copy`/`inspect`/`sign` between registries **without a local pull or privilege** | no registry→registry copy without pull | note (minor) | 5 |

## Impact on the 8 CNCF areas
The Podman differentiators **do not open a 9th area** — they refine the existing ones:
- **Area 2 (k8s conformance/interop):** GAP-0015 adds the "run/generate
  Kubernetes YAML of *workloads*" dimension, distinct from CRI conformance. It is Podman's strongest
  CNCF differentiator (k8s is the centre of the ecosystem).
- **Area 5 (cloud-native ops):** GAP-0016 (systemd/Quadlet) joins GAP-0010.
- **Areas 4/8 (supply-chain):** GAP-0017 (policy-based trust) joins GAP-0008/09/13.
- **Area 7 (governance):** GAP-0014 gains concrete criteria (PGP releases, LFX,
  security list, LTS, public roadmap).

## New GAP cards

### GAP-0015 — Kubernetes YAML interop: `kube play` / `kube generate` (area 2)
- **Current:** own manifests `compute.delonix.io/v1beta1`; `lower_application` lowers the Application to pods; it neither reads nor emits standard Kubernetes workload YAML.
- **Expected (Podman):** `podman kube play <k8s.yaml>` runs a k8s Pod/Deployment; `podman kube generate` emits k8s YAML from an existing pod/container.
- **Target 100%:** `delonix kube play <manifest.yaml>` runs workloads described in `apiVersion: v1`/`apps/v1` (Pod, at minimum); `delonix kube generate <ref>` emits valid k8s YAML.
- **Component:** `bins/delonix/src/cmd/` + `delonix-compute` (k8s↔envelope converter). **Compl.:** L. **Dep.:** —.

### GAP-0016 — Quadlet/systemd for user workloads (area 5)
- **Current:** `delonix serve` generates a unit only for `node-api` (`cmd/serve.rs`).
- **Expected (Podman):** Quadlet / `podman generate systemd` — run the user's containers/pods as systemd units (boot startup, supervision).
- **Target 100%:** `delonix generate systemd <container|pod>` emits a working unit; optionally a declarative Quadlet format.
- **Component:** `bins/delonix/src/cmd/` (reuses the `serve.rs` mechanics). **Compl.:** M. **Dep.:** —.

### GAP-0017 — Signature trust via declarative policy (areas 4/8)
- **Current:** `sign.rs` verifies cosign by explicit key; `policy.json` exists but for admission/runtime, not for signature trust per registry.
- **Expected (Podman):** `containers-policy.json` — declarative trust per registry/repo (sigstore + GPG), applied at pull/run.
- **Target 100%:** extend `policy.json` with signature-trust rules per registry/repo (sigstore + key); `delonix container run`/`image pull` refuses an unsigned image per the policy.
- **Component:** `delonix-security/domain/policy.rs` + `delonix-oci/sign.rs` + `registry.rs`. **Compl.:** M. **Dep.:** GAP-0009 (attestation format).
