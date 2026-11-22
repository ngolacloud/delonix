# kubeadm offline/HA round — §13 of the dlxcamp-20261024 campaign

**Date:** 2026-10-24 · **Branch:** develop (HEAD 34f7afd5 at the end) · **Binary:** delonix 5.1.1 (+260 commits since v5.1.1, stable copy in the scratchpad) · **Host:** rootless-first, libvirt + Cloud Hypervisor, 20 CPU / 62 GiB.

## Verdict

The `delonix cluster kubeadm` path provisions and bootstraps a **full HA control plane** (3 control-planes with stacked etcd + 1 worker + HAProxy), but **no node reaches Ready**: a **bug in the CRI path** (`compose_command`) prevents any CNI that overrides the `command` of an image with an `ENTRYPOINT`. In addition, `--cni-offline` does **not** make the bootstrap offline. Criteria 1 and 2 are **blocked**; criterion 3 (HA failure/recovery + clean destroy) was **met**.

| Criterion | State | Note |
|---|---|---|
| 1. Nodes Ready, offline/egress blocked | ❌ BLOCKED | nodes NotReady (CNI bug); offline unfeasible (flannel images come from ghcr.io) |
| 2. HA endpoint + CNI + Pod-to-Service/DNS | ⚠️ PARTIAL | HAProxy active and serving the API ✔; CNI/DNS/app ✗ (CNI bug) |
| 3. CP failure + recovery + clean destroy | ✅ MET | API 8/8 with 1 CP down; etcd rejoin 3/3; virsh clean, owner's VMs preserved |

## What WORKS (proven live)

- Provisioning of 5 VMs from `k8s-golden` (3×CP, 1×worker, 1×HAProxy) on libvirt NAT (network `dlxha-cfdff4dd`, virbr4, 10.240.0.0/16 — no collision with pod/service).
- `kubeadm init` + **HA join of 3 control-planes** with **stacked etcd (quorum 3/3 Running)** + worker join. All nodes registered `v1.34.12`, runtime `delonix://5.1.1`.
- Control-plane components all `Running`: etcd ×3, kube-apiserver ×3, kube-controller-manager ×3, kube-scheduler ×3, kube-proxy ×4.
- **HA endpoint = HAProxy** (NOT kube-vip — this engine has no kube-vip; `ensure_haproxy` in cluster.rs:3973). kubeconfig server = `https://10.240.84.84:6443` (VM `dlxha-lb`). `kubectl`/`/readyz` served through it.
- **HA failure/recovery** (see `evidence/ha-failover.txt`): `delonix vm stop dlxha-cp1` → `/readyz` 8/8 OK via HAProxy (quorum 2/3); `delonix vm start dlxha-cp1` → comes back with the same IP, etcd rejoins (3/3), API available on recovery (7/8, 1 blip).
- **Clean destroy**: `cluster destroy --force` removed 5 VMs + network + kube context; `virsh` with no dlxha VMs/networks/disks; owner's VMs (opnsense, pve92) and `k8s-golden` intact.

## BUG 1 (BLOCKER) — CRI: the pod's `command` is appended to the image's ENTRYPOINT

**Symptom:** with flannel (`--cni flannel --experimental-cni --cni-offline`), all nodes stay `NotReady`. The `kube-flannel-ds-*` pods are stuck forever in `Init:1/2`: the `install-cni` init container (spec `command:[cp] args:[-f, /etc/kube-flannel/cni-conf.json, /etc/cni/net.d/10-flannel.conflist]`) **runs `/opt/bin/flanneld`** (the ENTRYPOINT of the `flannel:v0.28.10` image) in etcd mode, as a daemon that never exits → the init never completes → the node never becomes Ready.

**Minimal reproduction (exact kubelet→delonix-cri path):** Pod with `command:["echo"] args:["ENTRYPOINT_OVERRIDE_OK"]` over the `flannel:v0.28.10` image → ran `flanneld` (etcd mode), not the `echo`. The override was **appended to**, not a replacement for, the ENTRYPOINT.

**Root cause (live in develop HEAD):**
- `crates/contexts/delonix-compute/src/run.rs:294` `compose_command(entrypoint, cmd, user)`:
  ```rust
  let mut v = entrypoint.to_vec();          // começa SEMPRE pelo entrypoint da imagem
  if user.is_empty() { v.extend(cmd) }      // sem override → entrypoint + cmd da imagem
  else { v.extend(user) }                   // COM override → entrypoint + command do pod  (APENSA!)
  ```
- `run.rs:192`: `image_command = compose_command(&config.entrypoint, &config.cmd, &o.command)`.
- `crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs:1861`: `o.command = rec.command.chain(rec.args)` and the CRI **never** sets `o.entrypoint` (empty grep in delonix-cri/src).
- Result: effective argv = `image.entrypoint ++ pod.command ++ pod.args`.

**Correct (CRI/K8s contract):** the pod's `command` **replaces** the `ENTRYPOINT`; the `args` replace the `CMD`. The docker-like path (`build_record`/`--entrypoint`, run.rs:324-332) is correct; the defect is that the CRI does not map `pod.command → RunOpts.entrypoint` and `pod.args → RunOpts.command`.

**Impact:** any workload that overrides `command` on an image with an `ENTRYPOINT` runs the wrong program. critest does not catch it (it uses images where command==entrypoint or with no entrypoint). It blocks flannel and, very likely, the install steps for cilium/calico.

**Suggested fix (out of scope for this session — not applied):** in the CRI, `o.entrypoint = Some(rec.command)` when `rec.command` is non-empty and `o.command = rec.args`; `build_record` already handles `Some(argv)` correctly.

## BUG 2 — `--cni-offline` does not cover the CNI images

`delonix cluster network fetch --cni flannel` only brings the **manifest** (`net-profiles/kube-flannel-v0.28.10.yml`). The DaemonSet still references `ghcr.io/flannel-io/flannel-cni-plugin:v1.9.1-flannel3` and `ghcr.io/flannel-io/flannel:v0.28.10`, **pulled live from ghcr.io** at bootstrap (`Pulling`/`Successfully pulled` events, not "already present"). `k8s-golden` does **not** pre-load these images. So, with egress blocked, the CNI bootstrap **fails** — the "offline/egress blocked" criterion is unreachable by this path. `cluster network fetch` would need to also download the OCI images and the bootstrap to import them into each node's CRI store (as `cluster load` does in kind mode).

## Operational note — owner's rebuild race

The 1st attempt aborted at `install_cli` with `DX-1000 resolving this CLI's own binary path: No such file or directory`: `std::env::current_exe().canonicalize()` failed because the owner rebuilt `target/release/delonix` (mtime 15:24:07) **during** the bootstrap, replacing the in-use inode. Mitigation: always run from a **stable copy** of the binary (scratchpad), immune to rebuilds. Minor product fragility: `cluster kubeadm` aborts hard if its binary is moved/replaced midway (recorded, not fixed).

## Evidence pointers

- `evidence/nodes.txt`, `evidence/etcd-pods.txt`, `evidence/kubeconfig-server.txt`, `evidence/haproxy-lb-ip.txt`, `evidence/ha-failover.txt`
- `bootstrap.log` (1st attempt, binary failure), `bootstrap2.log` (2nd, reaches NotReady)
