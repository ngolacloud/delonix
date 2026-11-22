# 08 — Clusters (kind-mode) and kubeadm offline

SUT: `delonix 5.1.1` @ `bca34f6c3`. Image: `kindest/node:v1.34.0` (cache). Cluster: `relccluster`.

## Kind-mode (CLUSTER-1/2)

| # | Case (§12) | Observed | State |
|---|---|---|---|
| creation | `cluster create --name relccluster` (without Docker) | node in ~1m07s control-plane + kindnet; cluster `up`, API :6443, kubeconfig written, context `delonix-relccluster` | **PASSED** |
| node Ready | `kubectl get nodes` | `relccluster-control-plane Ready v1.34.0` (containerd 2.1.3, on the Delonix engine) | **PASSED** |
| **app + traffic** | nginx deployment + Service + **port-forward curl** | `rollout successfully`; pod `1/1 Running` (IP 10.244.0.5 kindnet); **Service HTTP 200 via port-forward** | **PASSED** |
| DNS pod→service | `wget http://relcweb-svc.default.svc.cluster.local/` from a pod | nginx page fetched; CoreDNS 2/2 Running | **PASSED** |
| removal | `cluster destroy relccluster --force` | nodes removed, network `dlx-relccluster` released, kubeconfig+context cleaned, disk reclaimed | **PASSED** |

> **§12 met where it matters:** I did not stop at "listing nodes" — I ran an application and proved
> traffic **two ways** (external port-forward + internal DNS pod→service).

## Notes

- **Method error (mine):** on a first creation, a cleanup of mine with `grep relc` accidentally
  removed the node `relc-k8s-control-plane` mid-`kubeadm init` → `exit 137` (SIGKILL).
  **That failure is mine, not the product's.** The 2nd run, clean, reached Ready without incident
  (control-plane 1m07s vs 4m20s aborted). Recorded so as not to contaminate the verdict.
- **UX (low):** `cluster destroy` requires `--force` outside a terminal (good, fail-safe). But
  `cluster delete <name>` (a natural guess) returns a confusing kubeconfig error instead of
  "unknown subcommand; maybe `destroy`". Minor note.
- **Loading local images:** `cluster load` exists (no registry) — not exercised.

## CLUSTER-2 (workers/mounts/portmap, real kind config) — PARTIAL

Single control-plane tested. `--workers`, mounts, port-mappings and the acceptance/rejection of a
real `kind: Cluster` config (`kindconfig.rs`) **not exercised** this round → partial coverage.

## CLUSTER-3 (kubeadm offline HA, §13) — **BLOCKED**

`cluster kubeadm` exists (provisions VMs from the golden image + kubeadm bootstrap), but it requires the
`k8s-golden` image and disk space for multiple VMs (host at 96%, 44 G free). **Not executable** in this
campaign without the golden image and without disk headroom. It is an environment gap, not N/A — it stands as
an **open risk** for the release assessment (HA, kube-vip, CNI profiles, CP failure, real offline mode).

## CLUSTER-4 (multi-cluster + preserved kubeconfig, §14) — NOT EXECUTED

A single cluster created (disk). Incidental observation: there are 3 old kubeconfig contexts
(`delonix`, `lindo`, `mbandi-saurimo-32`) with no live nodes — confirming the management-scale vs
execution-scale distinction that §14 asks for, but the preservation test on add/remove was not run.
