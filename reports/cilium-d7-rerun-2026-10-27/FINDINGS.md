# D7 re-run — `cni: default` → Cilium on a multi-node kubeadm cluster (2026-10-27)

Host: this rootless host. Binaries: `delonix`/`delonix-cri` built from the tree
`develop @ 9177cd6c` plus the `default`→cilium orchestration change (uncommitted).

## Command

    delonix cluster kubeadm --name d7cil --control-plane 1 --workers 1 \
        --memory 4G --vm-image k8s-golden

`--cni` was left at the default (`default`). **No `--experimental-cni`.**

## What WORKED (scope of #799 — orchestration)

- `default` resolved to **cilium** (not the node-local bridge): the plan announced
  "Installing the cilium pod network", not the single-node step. — `create.log`
- Bootstrap: kubeadm init → installs cilium (helm template + kubectl apply) →
  worker join → wait. **Exit 0.**
- `cilium agent Ready on 2/2 node(s); 2/2 node(s) Ready` in 2m12s; cluster ready.
- `kubectl get nodes`: **both Ready** (cp1 control-plane, w1), runtime
  `delonix://5.1.1`. — `kubectl-verify.txt`
- cilium DaemonSet 2/2, operator 2/2.
- `cilium status --brief` = OK; `cilium-health`: **Cluster health 2/2 reachable**
  (Node 1/1 + Endpoints 1/1 on both nodes). — `cilium-health.txt`
- **Confirms the bpffs #869 fix (c95a3b19) live** — Cilium STARTS where the
  original ADR-0078 D7 died with "multiple mount points detected at /sys/fs/bpf".
  Pending item #1 of ADR-0078 is resolved.

## What did NOT work (outside the scope of #799 — CRI defects)

### 1. Pod-to-pod traffic BETWEEN NODES blocked (CNI_ARGS missing in the CRI)

Two busybox pods pinned to distinct nodes (`ping-cp1` 10.244.0.89,
`ping-w1` 10.244.1.122): ICMP and TCP (httpd/wget) fail in both directions.
— `crossnode-connectivity.txt`

Root cause (agent logs): the endpoints of normal pods stay in
**`reserved:init`** (identity 5) with policy enforcement Enabled, so the init
policy blocks them. The logs show `k8sPodName=/` and `ciliumEndpointName=/`
**empty** — the agent does not associate the CNI endpoint with the Kubernetes
pod, so it does not fetch the labels from the apiserver and the identity never
resolves. — `diag-identity.txt`, `diag-apiserver.txt`

The delonix-cri **never passes `CNI_ARGS`** to the CNI plugin:
`crates/adapters/delonix-sdn/src/cni.rs:294 build_env()` sets only
`CNI_COMMAND/CONTAINERID/NETNS/IFNAME/PATH`; the `Target` does not even carry the
pod namespace/name. The CNI contract requires `CNI_ARGS` with
`K8S_POD_NAMESPACE`/`K8S_POD_NAME`/`K8S_POD_INFRA_CONTAINER_ID` — which plugins
with identity/policy (Cilium, Calico) read. Pure plugins (bridge, host-local,
flannel) do not need it, which is why the single-node `default` and the simple
overlay work. Flannel, having no notion of identity, should work cross-node
WITHOUT this fix.

**This is a defect of delonix-cri/sdn, not of the orchestration (#799).**
A candidate for the new pending item #1 of ADR-0078 (succeeding bpffs, already
fixed).

### 2. cilium-envoy in CrashLoopBackOff (/dev/shm missing)

`panic: cannot open shared memory region /envoy_shared_memory_0 — No such file
or directory` (`hot_restart_impl.cc`). The cilium-envoy container (L7 proxy)
has no `/dev/shm` (tmpfs). This affects only L7 features (L7 policy/ingress), not
the core agent/dataplane nor node readiness. — `envoy-crash.txt`.
Second narrow CRI defect.

## Fix applied and RE-PROVEN (CNI_ARGS in the CRI)

The owner asked for the CNI_ARGS to be fixed in this session. Done:
`crates/adapters/delonix-sdn/src/cni.rs` (`Target.args` + `build_env` emits
`CNI_ARGS`; `add`/`attach_named_netns` pass it through) and
`crates/interfaces/delonix-cri/src/runtime_svc/lifecycle.rs` (`cni_k8s_args`
assembles `IgnoreUnknown=1;K8S_POD_NAMESPACE=…;K8S_POD_NAME=…;
K8S_POD_INFRA_CONTAINER_ID=…;K8S_POD_UID=…` from the `PodSandboxConfig`, on the
ROOT path). Rebuilt, cluster d7cil **destroyed and recreated**.

Result (`crossnode-connectivity-FIXED.txt`), same command
`cluster kubeadm --control-plane 1 --workers 1 --memory 4G` (cni default):

- Cluster ready, `cilium agent Ready on 2/2 node(s); 2/2 node(s) Ready` in 2m07s.
- Pod identity RESOLVED: the endpoint of `x-w1` (10.244.0.51) gains
  `identity 5728  k8s:app=xt`, policy enforcement Disabled — no longer
  `reserved:init`. **0 endpoints in `reserved:init`** on cp1.
- **Pod-to-pod BETWEEN NODES: OK in both directions** — `x-cp1`(@cp1, 10.244.1.92)
  ⇄ `x-w1`(@w1, 10.244.0.51), HTTP via busybox httpd (`OK-w1`/`OK-cp1`).
- **Service/DNS**: `nslookup kubernetes.default` via ClusterIP 10.96.0.10
  resolves 10.96.0.1 — kube-proxy + coredns + cross-node datapath end-to-end.

## cilium-envoy FIXED (two CRI mount defects)

The `cilium-envoy` (L7 proxy) stayed in CrashLoopBackOff because of TWO defects,
one hiding the other:

1. **`/dev/shm` missing** (`envoy-crash.txt`): `setup_dev`/`setup_dev_userns`
   never mounted the `/dev/shm` tmpfs. Fix: `mount_dev_shm` (`mode=1777,
   size=65536k`) in `delonix-linux/src/lib.rs`, both paths. Proven in a rootless
   container (`/dev/shm` tmpfs 64m writable).
2. **nested mount ordering**: the pod mounts `/var/run/cilium/envoy/` RO and
   `.../sockets` RW; the kubelet lists the RW child before the RO parent, and
   `setup_rootfs` applied them in arrival order → the RO parent buried the RW
   child → "cannot bind '.../admin.sock': Read-only file system". Fix: stable
   sort by target depth (`mount_target_depth`), like runc.

Proven live (cluster d7env): **cilium-envoy Running 1/1 on both nodes**,
`/dev/shm` and `/run/cilium/envoy/sockets` both `rw` — `envoy-FIXED.txt`. Cilium
comes up COMPLETE (agent + envoy + operator 2/2).

## Conclusion

Full chain PROVEN live: `default`→cilium orchestration (#799) +
bpffs #869 fix (cilium starts) + CNI_ARGS fix (pod identity resolves →
pod-to-pod cross-node and services work). A multi-node kubeadm cluster comes up
functional with a single `cluster kubeadm --workers 1`, without opt-in. Resolves
pending item #1 of ADR-0078 (bpffs) and adds the new pending item it revealed
(CNI_ARGS), now fixed.
