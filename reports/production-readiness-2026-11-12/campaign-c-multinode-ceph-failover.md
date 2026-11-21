# Campaign (c) — critical production level 2: multi-node · Ceph · failover

> The next level of readiness validation (Round 1 in `findings-host.md` covered
> single-node). Objective: validate Delonix under the conditions that **only appear
> in distributed production** — scheduling across nodes, shared persistent
> storage (Ceph), and recovery from **whole-node** failure (not just
> process failure). Doctrine: only the verified effect counts.

## Available harness (do not reinvent)
- **In-engine multi-node:** `delonix cluster create --workers N --control-planes M`
  (kind-mode: `kindest/node` nodes run IN the engine, `kubeadm` bootstrap, no Docker).
- **Multi-node on real VMs:** `kubeadm` HA (5 VMs + 3 CP + etcd + HAProxy) — already
  proven (see memory `delonix-kubeadm-ha-offline-findings`; CNI was a blocker).
- **Ceph:** proven infra — native MicroCeph 19.2.3 (RADOS/RBD/CephFS/RGW) +
  `ObjectStorage` backend `cephRgw` + Ansible roles microceph / Helm rook_ceph.
- **Scripts:** `e2e_overlay_multinode.sh`, `e2e_cluster_kind.sh`,
  `e2e_node_network_ops.py`, `chaos.sh`.

## Proofs to run (in order of findability)

### C1 — Multi-node: scheduling across nodes ⏳ (in progress)
`cluster create prodtest --workers 2` → 1 CP + 2 workers Ready. Scale a
Deployment of N replicas and verify that they **spread across the nodes** (implicit
anti-affinity). **Effect:** pods on ≥2 distinct nodes, all Running.

### C2 — Node failover (the central critical-production test)
With the C1 cluster: `delonix vm stop`/kill a worker (simulates node failure) →
verify that k8s marks the node NotReady and **re-schedules the pods** onto the
surviving node. **Effect:** replicas return to Running on another node within the
pod-eviction-timeout; no pod is stuck in Terminating/Unknown.

### C3 — Overlay networking across nodes
`e2e_overlay_multinode.sh` — a pod on node A reaches a pod on node B (cross-node CNI).
**Effect:** pod-to-pod connectivity across nodes, routable IP.

### C4 — Persistent Ceph storage (survives node failure)
`kind: ObjectStorage`/`Volume` with Ceph backend (RBD/CephFS). Write data in a
pod, kill the node where it runs, verify that the re-scheduled pod **re-reads the same
data** from Ceph. **Effect:** persistence survives the loss of the node — the criterion
that distinguishes "stateful in production" from "demo". (Needs a live Ceph — reuse
MicroCeph or a minimal stand-up.)

### C5 — Control-plane HA
`cluster create --control-planes 3` → kill 1 CP → the API continues (etcd quorum).
**Effect:** `kubectl` responds with 1 CP down.

## Where this breaks (hypotheses to confirm/refute)
- Memory: **CNI was a blocker on kubeadm HA** (5 VMs) — C3 is the direct test.
- Failover (C2): pod-eviction + re-schedule depends on the kubelet/kube-controller;
  Delonix is the CRI runtime underneath — it validates that the CRI reports the node as dead
  in time and cleans up the orphaned pods.
- Ceph (C4): the CSI/driver coupling is the typical fragile point.

## C1 — first attempt (host under heavy contention): findings
`cluster create --name prodtest --workers 2` (kind-mode): the **3 nodes
started** (`prodtest-control-plane`+`worker`+`worker2` as `kindest/node:v1.37.0`
containers, `cluster ls` → state **up**, API at 127.0.0.1:6443,
k8s v1.37), **BUT the create failed** with "timeout waiting for containerd on node
'prodtest-worker2' (90s)" and **did not export the kubeconfig** → cluster inaccessible.
Three findings (under the shared host's load: VM + benches + 3 nodes):
1. **Resilience BUG:** the containerd wait (90s, no retry/backoff) fails the
   whole create over ONE slow node, even with the nodes up — it should wait with
   retry or report "nodes up, kubeconfig pending", not abort.
2. **Partial-create cleanup BUG:** the nodes were left **orphaned** (not registered
   as a cluster) — `cluster <name> delete` does not find them; a manual
   `container rm -f` was needed. A failed create should clean up or register.
3. **UX:** `cluster delete prodtest` errors — the syntax is `cluster <name> delete`
   (name first). Clear message, but inverted relative to docker/podman.

> Note: the host contention (shared laptop, many loads) exaggerates (1).
> **Re-running in a clean environment / dedicated VM** to separate the resilience
> bug from the load. The nodes COME UP — the block is the readiness gate + the
> kubeconfig.

## Status
| Proof | Status |
|---|---|
| C1 multi-node scheduling | **PARTIAL** — nodes come up, create aborts on the containerd wait under load (3 findings above); re-run clean |
| C2 node failover | after C1 |
| C3 cross-node overlay | after (`e2e_overlay_multinode.sh`) |
| C4 persistent Ceph | needs a live Ceph (reuse MicroCeph) |
| C5 HA control-plane | optional (`--control-planes 3`) |

> This is a **multi-round** campaign (each proof is non-trivial and some
> require VMs/Ceph). The host has KVM/qemu/virsh; the kind-mode cluster runs in the engine.
