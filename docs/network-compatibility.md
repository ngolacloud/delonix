# Cluster network profiles — compatibility matrix and what was measured

> **Read this first.** The three profiles (`cilium`, `calico`, `flannel`) are
> **EXPERIMENTAL** in this release, and the 2026-10-15 two-node lab showed
> why: Cilium **FAILS** on this CRI, with the root cause identified
> (§3.2). Everything up to the dataplane — golden image, preflight,
> artefacts, installation, waiting for readiness — was measured and passes;
> the Cilium dataplane does not. This page says exactly what was measured,
> with what, and what was not. A conformance number nobody re-validates
> stops being a measurement and becomes a citation.
>
> Decision and trade-offs: [ADR-0078](adr/0078-selectable-pod-network-profiles.md).

Measured on **2026-10-15**, against base commit `67a0ba97`, on the
development host (Linux 7.0.0-38-generic, cgroup v2, KVM). This is the **v5.1.1**
line (see the `delonix://5.1.1` of stage L7); it is the same engine that ships today as
**0.1.0** (version rebrand, no network code change). **Historical baseline:
the profiles were not re-measured against tag 0.1.0** — re-measuring the network
matrix is new work (see §5, B13).

## 1. Pinned versions

Every artefact is pinned by **version and content SHA-256**. A URL that starts
answering different bytes fails closed (DX-6704) instead of installing something
else. `latest` does not appear on any reproducible path — there is a test that
checks this per profile.

| component | version | sha256 | digest origin |
|---|---|---|---|
| `containernetworking/plugins` (amd64) | 1.9.1 | `b98f74a0f8522f0a83867178729c1aa70f2158f90c45a2ca8fa791db1c76b303` | publisher's `.sha256` sidecar + local `sha256sum` |
| `containernetworking/plugins` (arm64) | 1.9.1 | `56171987d3947707c3563db2f4001bccaf50fd63468611b9f3cbecb1375ee7ec` | publisher's `.sha256` sidecar |
| Cilium chart | 1.20.2 | `b2afd87b7f75f875f92a14559f14f59b7babbb479d968e3fd625a20bf30ec20e` | `sha256sum` of the obtained tarball |
| Calico manifest | 3.33.0 | `2de8f47595fb9c41b3f47d7b767a1f8e72ecf84057af834738ff12689a234da5` | `sha256sum` of the obtained manifest |
| `kube-flannel.yml` | 0.28.10 | `393bdbce53eb50c8eb97dd513b218518b7d6e7985dc7b889be956e112a711084` | `sha256sum` of the obtained manifest |
| Multus daemonset | 4.3.1 | `bd618d23a957729071ec81dcef740ffc03a5916102137afa6114d31a5d187843` | `sha256sum` of the obtained manifest |

**Architecture.** Only `amd64` is in the golden image's path (ADR-0018). `arm64`
has a pinned digest for the plugins; any other architecture is **refused**
instead of being downloaded unverified.

**What the pins do not cover.** The container images *inside* the Calico and
Flannel manifests are pinned by **tag** upstream, not by digest. The Cilium
chart is the best case, verified at render time, its three images all come
with `@sha256:`:

```
quay.io/cilium/cilium:v1.20.2@sha256:2939231d0d3e3ebddcd80fffa168b7ddcc78fdf0dc864d1c8c126ff523c54f01
quay.io/cilium/operator-generic:v1.20.2@sha256:64d8798350e8569b8e7622563fed6e44dce2625f311e4651b774816516c744fc
quay.io/cilium/cilium-envoy:v1.37.6-...@sha256:af7382699576b9e65e9184efa52eeca0b58aea70ad6e511bf260c91d9f740463
```

Resolving the other two to digests is an open item, not a detail.

## 2. Capability matrix by profile

| capability | cilium | calico | flannel | default | none |
|---|---|---|---|---|---|
| support level | **EXPERIMENTAL — FAILS on this CRI** (§3.2) | **EXPERIMENTAL** | **EXPERIMENTAL** | supported | supported |
| nodes | several | several | several | **single only** | — |
| rendered dataplanes | `vxlan` (default), `geneve` | `vxlan` (default), `ipip` | `vxlan` | — | — |
| kube-proxy modes | `iptables` (default), `ipvs`, `none` | `iptables` (default), `ipvs` | `iptables` (default), `ipvs` | `iptables` | `iptables` |
| `NetworkPolicy` **implemented by the profile** | yes | yes | **no** | no | no |
| `NetworkPolicy` **tested here** | **no** | **no** | n/a | n/a | n/a |
| automatic MTU (1500 → ) | 1450 | 1450 (`vxlan`) / 1480 (`ipip`) | 1450 | — | — |
| plugins the chain runs | `loopback`, `portmap` | `loopback`, `portmap`, `bandwidth` | `loopback`, `bridge`, `host-local`, `portmap` | `loopback`, `bridge`, `host-local`, `portmap` | — |
| kernel modules | `vxlan` | `vxlan`/`ipip`, `ip_tables`, `nf_conntrack` | `vxlan`, `ip_tables`, `nf_conntrack` | — | — |
| images pinned by digest | **yes** | no (tag) | no (tag) | n/a | n/a |
| Multus as an extension | yes | yes | yes | no | no |
| Hubble | yes | n/a | n/a | n/a | n/a |

> **Flannel does not deliver isolation.** A `NetworkPolicy` in a Flannel
> cluster is accepted by the apiserver and **enforced by nobody**. It is a
> lab and CRI/CNI path diagnostic profile, and it is not equivalent to the
> other two. `delonix cluster network profiles` says so on every run.

### Kubernetes versions

`DEFAULT_K8S_VERSION` = **1.36**. The engine's validation accepts any numeric
version (`1.36`, `1.36.2`) and truncates to `major.minor` only on the apt
repository path. **There is no measured profile × Kubernetes-version matrix
here**: what is declared above is the profile version this binary installs,
not a claim that it was exercised against every supported Kubernetes
version. Building it needs the §5 lab.

## 3. What was measured — and with what

### PASS — measured in this session

| # | what | how | result |
|---|---|---|---|
| 1 | Digest of each artefact | `delonix cluster network fetch` + independent `sha256sum` | **PASS** — 4 artefacts obtained and verified in 3.0 s; all four digests confirmed by `sha256sum` outside the engine |
| 2 | CNI plugin digest | `curl` of the `.sha256` sidecar + local `sha256sum` of the tarball | **PASS** — `b98f74a0…b303` in both |
| 3 | Plugin tarball contents | `tar tzf` | **PASS** — the 10 reference plugins present; 18 members total |
| 4 | Cilium chart render with the engine's values | `helm template` with the exact `cilium_helm_values` list | **PASS** — 22 objects; `routing-mode: tunnel`, `tunnel-protocol: vxlan`, `mtu: 1450`, `cluster-pool-ipv4-cidr: 10.244.0.0/16`, `kube-proxy-replacement: false`, `/opt/cni/bin` and `/etc/cni/net.d` — all present in the ConfigMap |
| 5 | Substitution against the **real** manifests | test `the_substitution_lands_on_the_real_upstream_manifests` against the cached artefacts | **PASS** — and **found a real bug**: see §4 |
| 6 | Node preflight, all 5 profiles | `sudo sh dist/delonix-node-preflight.sh <profile>` on the real host | **PASS** — each profile produces distinct, correct verdicts; detected active swap, absence of `delonix-cri`, and confirmed bpffs/cgroup v2/kernel 7.0/vxlan |
| 7 | Preflight fails closed without root | run as uid 1000 | **PASS** — `FAIL` + rc=1, never rc=0 |
| 8 | Preflight refuses incorrect usage | invalid argument | **PASS** — rc=2 |
| 9 | `shellcheck` of the preflight | `shellcheck dist/delonix-node-preflight.sh` | **PASS** — clean |
| 10 | The 10 refusals, end-to-end through the real CLI | `delonix cluster apply -f` with each invalid manifest | **PASS** — DX-1701/1702/1703/1704/1705/1706 and the unknown-field warning, all with rc=1 |
| 11 | New valid example | `delonix manifest validate -f examples/cluster/kubernetescluster.yaml` | **PASS** |
| 12 | Unit and contract tests | `cargo test -p delonix-runtime-bin --bin delonix` (affected suites) | **PASS** — 212 tests |
| 13 | Cilium agent CRI requirements | inspection of the rendered DaemonSet | **PASS by reading** — see §3.1 |
| 14 | `fmt`, CLI surface gate, published schema, PT translations | `cargo fmt --check`, `scripts/cli-tree.sh --gate`, drift tests | **PASS** — 273 leaves match the baseline |
| 15 | **Real golden image build** (was B1) | `delonix vm image build -t k8s-golden --k8s-version 1.34 --offline`, isolated root `/var/tmp/delonix-lab`, this branch's release binaries, under an 8 GiB disk floor | **PASS** — `rc=0`, 608 MiB, 49 `virt-customize` steps, never hit the floor |
| 16 | Golden image's `/etc/machine-id` | `virt-ls -l -a … /etc/` | **PASS** — **0 bytes**. The 2026-07-17 golden had **33** (`6bd6963d409a14986c04b3b6eed4c818`) — this is DX-6702 measured on the artefact |
| 17 | The 10 reference plugins, and only them | `virt-ls -a … /opt/cni/bin` | **PASS** — `bandwidth bridge firewall host-device host-local ipvlan loopback macvlan portmap tuning`. The 8 excluded ones and `LICENSE`/`README.md` absent — see §4.1, because at first they were NOT absent |
| 18 | Preflight installed as a COMMAND | `virt-ls -l -a … /usr/local/bin` | **PASS** — `delonix-node-preflight`, `0755`, alongside `delonix` and `delonix-cri` |
| 19 | Provenance in the image | `virt-cat -a … /etc/delonix-image-release` | **PASS** — `DELONIX_CNI_PLUGINS="v1.9.1 (amd64, sha256:b98f74a0f8522f0a)"` and `DELONIX_NET_PROFILES="cilium:1.20.2,calico:3.33.0,flannel:0.28.10,multus:4.3.1"` |
| 20 | Profile modules and sysctls persisted | `virt-cat` of `/etc/modules-load.d/delonix-cni.conf` and `/etc/sysctl.d/delonix-cni.conf` | **PASS** — the 9 modules (`vxlan geneve ipip ip_tables nf_conntrack ip_vs ip_vs_rr ip_vs_wrr ip_vs_sh`) and the 4 sysctls |
| 21 | Network diagnostics in the golden image | `virt-ls` of `/usr/bin` and `/usr/sbin` | **PASS** — `tcpdump`, `ethtool`, `conntrack` |
| 22 | The golden image carries no conflist | `virt-ls -l -a … /etc/cni/net.d` | **PASS** — only `.kubernetes-cni-keep`, 0 bytes, from the `.deb`. Not a `*.conflist`/`*.conf`, and `cni::list_conf_files` filters by extension, so it is invisible to the CNI layer |

### 3.2 The two-node lab — what ran, and where it stopped

Run on **2026-10-15**, isolated root `/var/tmp/delonix-lab`, golden image built
in this session, `cluster kubeadm --control-plane 1 --workers 1 --cni cilium
--experimental-cni` on libvirt/NAT (virbr0).

| # | step | result |
|---|---|---|
| L1 | Provisioning the 2 VMs | **PASS** — 16.1 s + 19.4 s |
| L2 | **DISTINCT addresses** (DX-6702) | **PASS** — `labcil-cp1` 192.168.122.114, `labcil-w1` 192.168.122.15, with different Client-IDs in the dnsmasq leases. The July golden gave **one** shared lease |
| L3 | Preparing the 2 hosts (recipes + CRI + CLI + preflight) | **PASS** — 3.6 s; detected and replaced the older CLI the golden image carried |
| L4 | `kubeadm init` | **PASS** — 32.4 s, after fixing the version bug (§4.0) |
| L5 | **Profile installation** (the step that did not exist) | **PASS** — 1.0 s, between `init` and join, as designed |
| L6 | Worker `kubeadm join` | **PASS** — 6.3 s |
| L7 | **Both nodes register on the cluster** | **PASS** — `kubectl get nodes` shows the 2, `v1.34.12`, and **`CONTAINER-RUNTIME: delonix://5.1.1`** — this CRI serving a real kubelet on two nodes |
| L8 | The Cilium DaemonSet schedules | **PASS** — `DESIRED 2, CURRENT 2` |
| L9 | **The Cilium agent starts** | **FAIL** — `CrashLoopBackOff` on both nodes |
| L10 | The readiness wait **fails honestly** | **PASS** — `✗ … 7m00s` and `DX-6704`: "its agent is Ready on 0/0 node(s) and 0/2 node(s) report Ready … nothing was deleted or rolled back". Did not report success, did not delete the evidence, and named the diagnostic commands — but **those commands did not run**, see the correction after the table |
| L11 | Cross-node, DNS, ClusterIP, MTU, NetworkPolicy | **NOT RUN** — depends on L9 |

**Correction to L10, measured on 2026-10-20 and already fixed in the code.** What
L10 observed is still true: the wait fails, with the counts, without deleting
anything. What nobody tried in that session was **running** the two commands
DX-6704 names. They did not run: `delonix cluster <name> -n <ns> describe
daemonset <ds>` resolves the cached kubeconfig
(`<root>/clusters/<name>-kubeconfig.yaml`), and `apply_ssh` fetched it on the
**last** step — after profile installation and after the wait. The only run
that prints the advice was, by construction, the run that returned before the
file existed. Reproduced on 2026-10-20 on a cluster of 3
control-planes + 2 workers with `--cni flannel`: `DX-6704` at 420s and then
`error[DX-1000] … no kubeconfig for cluster 'ngolacloud'`, with the cluster
directory containing only `id_ed25519`, `id_ed25519.pub` and
`network-profile.json`.

The kubeconfig is now saved **right after `kubeadm init`**, which is the
first moment `admin.conf` exists; it costs nothing on the success path, and on
failure paths it is the difference between one piece of advice and a second
error. Two tests hold the order in place: the plan's own
(`bootstrap_plan`) and the real calls inside
`apply_ssh` (`tests/architecture.rs`).

**The lesson, the same as §4's:** naming diagnostic commands is not the
criterion — running them is. A "PASS" given to a message's wording never saw
half of what the message promises.

**L9's root cause, measured:**

```
level=info  msg="Mounting BPF filesystem" bpffsRoot=/sys/fs/bpf
level=fatal msg="Unable to mount BPF filesystem" error="multiple mount points detected at /sys/fs/bpf"
```

And the node host has **exactly one** bpffs, already `shared` (good for
propagation):

```
35 24 0:30 / /sys/fs/bpf rw,nosuid,nodev,noexec,relatime shared:11 - bpf bpf rw,mode=700
```

So the duplication is **built inside the container**, by this engine. The
hypothesis the code supports: a `privileged` container gets `/sys` via a
recursive bind from the host (`delonix-linux`, the path taken when a fresh
`sysfs` is refused in a userns) **and** the pod requests `/sys/fs/bpf` as a
hostPath — two entries at the same point, which is exactly what Cilium
refuses. The container's log confirms the pair `"source": "/sys/fs/bpf"` /
`"target": "/sys/fs/bpf"`.

**This is an engine defect, in the CRI, not the profile's.** It is recorded
as its own pending item: it is the first thing to fix for Cilium to leave
experimental status, and it is not fixed in this change because it lives in
the privileged-container mount path, which serves much more than CNIs.

**What the failure PROVES, and that could not be proven by reading:** the
`EXPERIMENTAL` gate was right. A profile whose artefact is pinned, verified,
rendered with the correct values and successfully applied **still does not
work** — and the only reason this is known is that it was run. Declaring
Cilium supported based on the code looking right would have been exactly the
account DX-1705 exists to prevent.

**Isolation with flannel: NOT RUN.** The attempt (the same pair of VMs, a
profile that does not need bpffs) stopped because `kubeadm reset` left the
apiserver holding port 6443 on the control-plane, and deeper cleanup was
blocked by this session's environment. It is the next step and would isolate
the Cilium failure from the rest of the chain.

### 3.1 Cilium agent CRI requirements — PASS by reading, not measured live

Extracted from the rendered DaemonSet (what the agent actually asks for):

| the agent asks for | this engine | where |
|---|---|---|
| `privileged: true` (×1) | implemented, and the bootstrap **raises the ceiling** on its nodes | `cluster.rs` (`DELONIX_CRI_CAP_CEILING=all`, drop-in) |
| `SYS_ADMIN` ×5, `NET_ADMIN` ×4, `SYS_MODULE`, `SYS_RESOURCE`, `SYS_PTRACE`, `SYS_CHROOT`, `NET_RAW` | covered by the `all` ceiling | `cap_ceiling.rs` |
| `mountPropagation: Bidirectional` (×1) | implemented → `rshared` | `lifecycle.rs` → `delonix-linux` (`MS_SHARED\|MS_REC`, its own `mount()`) |
| `mountPropagation: HostToContainer` (×4) | implemented → `rslave` | the same |
| `hostNetwork: true` (×3) | implemented; the CNI is skipped | `lifecycle.rs` |
| bpffs at `/sys/fs/bpf` | **checked** by the preflight (not configured) | `dist/delonix-node-preflight.sh` |
| unified cgroup v2 | **checked** by the preflight | the same |

**Every piece exists. None was exercised together.** That is all, and only
that, the `EXPERIMENTAL` level means.

### BLOCKED — not run, with the reason

| # | what | why | unblocked by |
|---|---|---|---|
| B1 | Real golden image build | **5 GiB free out of 938 GiB (100% full)**. `virt-resize`/`virt-customize` expands the qcow2 and it does not fit; filling the root disk of a shared machine is not an acceptable trade-off | ~40 GiB free |
| B2 | 1 CP + 1 worker cluster | **ALREADY RUN** for cilium — see §3.2. The cluster comes up, the nodes register, the agent does not start | fixing the duplicated bpffs |
| B3 | Pod creation/removal, DNS, `ClusterIP`/`NodePort`/`hostPort`, external egress | needs B2 | §5 |
| B4 | Same-node and **cross-node** communication | **NOT RUN** — depends on an agent that starts (§3.2 L9) | fixing the duplicated bpffs, or isolating with flannel |
| B5 | MTU with large packets | needs B2 | §5 |
| B6 | `NetworkPolicy` ingress and egress | needs B2 | §5 |
| B7 | Agent and node restarts | needs B2 | §5 |
| B8 | Partial `ADD` failure and cleanup by `DEL` | needs B2 + `crictl` on the node | §5 |
| B9 | Absence of IPAM/interface/rule leaks after cycles | needs B2 | §5 |
| B10 | End-to-end offline mode | needs B2 (the **fetch** check is already PASS, #1) | §5 |
| B11 | Multus: additional interfaces and their **isolation** | needs B2 | §5 |
| B12 | Bootstrap repetition (live idempotency) | needs B2 | §5 |
| B13 | `critest` re-measured | not run in this session | `scripts/critest.sh` |

### NOT RUN

- **Profile × Kubernetes-version matrix.** Only the default version (1.36) is
  named; no combination was exercised.
- **Providers other than libvirt.** Nothing here was run against Proxmox or
  Cloud Hypervisor, and **nothing is extrapolated** from one provider to
  another.
- **Network policies.** See B6. Installing Cilium **is not** delivering
  isolation, and this page does not claim it.

## 4. Bugs found by these tests

### 4.0. `--kubernetes-version=v1.34` — kubeadm refuses it, and ALL the examples used it

Found on the **first real two-node run**, once the VMs existed and the hosts
were prepared:

```
kubeadm init --kubernetes-version=v1.34
error: version "v1.34" doesn't match patterns for neither semantic version nor
labels (stable, latest, ...)
```

The engine built the flag as `v{spec.k8sVersion}` verbatim. kubeadm requires
**full semver**. `valid_version` accepts major.minor, the flag's help says
"minor version", and the five shipped examples use `k8sVersion: "1.34"` —
so **every documented invocation of this path died on `kubeadm
init`**.

It is the **exact mirror** of the bug already recorded by
`k8s_repo_version`: there, a version with a patch broke the apt repo URL,
which only exists at the minor level, and the fix truncated. Nobody checked
the other direction — the same field's two consumers want opposite shapes.

**Why the flag is omitted instead of inventing `.0`:** measured on the golden
image this session built, the installed kubeadm is `1.34.12` and
`preseed_k8s_images` cached `v1.34.12` images. Asking for `v1.34.0` would make
kubeadm want images the node **does not have** — turning a working offline
node into one that silently reaches out to the network, or fails. Omitting
makes kubeadm use its own version, which by construction is the one the
golden installed and the one pre-seeded. A full semver passes through
intact: whoever pinned a patch meant to pin it.

### 4.1. The 8 plugins I said would not ship, shipped

The first golden image built carried **all 18** plugins plus `LICENSE` and
`README.md`. The `kubernetes-cni` `.deb` arrives as a kubelet dependency and
owns 21 paths in `/opt/cni/bin`, so extracting ten verified binaries on top of
its own left the other eight intact — while the module header and ADR-0078
asserted they would not ship, citing attack surface as the reason.

**No unit test could catch this:** the tests check the contents of
`REFERENCE_PLUGINS`, and the extra files come from a `.deb` no test runs. The
image was the only place that claim was verifiable.

Fixed with a prune, and verified on a rebuilt golden image. The cost is
written in the code: `dpkg` still believes it owns the removed files.

### 4.2. The Flannel MTU substitution matched nothing

`kube-flannel.yml` v0.28.10 **has no `MTU` key** — its `net-conf.json` is
`Network`, `EnableNFTables` and `Backend`. The code substituted a `"MTU":`
line that does not exist, so Flannel would have auto-detected the **node**'s
MTU and every pod would pay the 50-byte VXLAN header twice: large packets
silently dropped, which reads as unexplained slowness.

It was exactly the failure the overhead calculation exists to prevent,
defeated by a substitution written from an assumption. **The snippet tests
always passed**, because the snippet was written with the same assumption.
Fixed: the MTU is **inserted**, anchored to the `"Network":` line, and the
test now parses the real document as YAML and the `net-conf.json` as JSON,
instead of searching for a substring.

### 4.3. The nodes' network could be the cluster's own pod CIDR

Reported on a live cluster, not found by these tests. A cluster's nodes sit on
a network of their own (`dlx-<name>` in kind mode, the cluster's network in
`cluster kubeadm` without `--network`), and its `/16` was picked from the
network name's hash over `10.201`–`10.254` — a span that contains
`10.244.0.0/16`, kubeadm's default pod CIDR. About one cluster name in 55
landed on it (`c25` does). Inside each node that range is routed to the CNI, so
replies to anything addressed on the node network went to the pods: the API
answered from inside the node and **hung from the host**, with nothing refused
at creation time. The workaround was an explicit `--pod-subnet`.

Fixed in the allocator, which now takes ranges to keep off
(`NetworkStore::create_avoiding`): both paths hand it the cluster's pod and
service CIDRs (`netops::ensure_node_network`). A node network that already
exists on one of them — left by an older binary, or by a failed create — is
refused with DX-1704 before anything is created, naming the
`delonix network rm` that clears it.

Kind mode also checks its pod and service CIDRs against the engine's OTHER
networks now, as `cluster kubeadm` always did: a pod CIDR on a network the host
already uses is refused with DX-1704 before the cluster's network, state or
image exist.

## 5. Lab procedure — how to unblock B1-B12

Prerequisites: ~40 GiB free, `/dev/kvm`, `libvirt`, `virt-customize`,
`helm`, and `scripts/lab_preflight.sh` passing.

```bash
# 0. Isolated root, so as not to touch the host's shared state
export DELONIX_ROOT=/var/tmp/delonix-lab
export DELONIX_NET_RUNTIME_DIR=/var/tmp/delonix-lab-net

# 1. The golden image, with the plugins pinned and the preflight inside (B1)
delonix vm image build -t k8s-golden --k8s-version 1.36 --offline
virt-cat -a "$DELONIX_ROOT/vm-images/k8s-golden.qcow2" /etc/delonix-image-release
#   confirm: DELONIX_CNI_PLUGINS=v1.9.1 (amd64, sha256:b98f74a0f8522f0a)
#            DELONIX_NET_PROFILES=cilium:1.20.2,calico:3.33.0,flannel:0.28.10,multus:4.3.1
#   and that /etc/machine-id is EMPTY (0 bytes) — cluster identity baked into the image is DX-6702

# 2. Verified artefacts (already PASS; repeating here proves the offline cache)
delonix cluster network fetch

# 3. One cluster per profile (B2). Repeat with --cni calico and --cni flannel.
delonix network create lab-net
delonix cluster kubeadm --name lab-cilium --control-plane 1 --workers 2 \
  --network lab-net --cni cilium --experimental-cni --copy-kubeconfig
#   CRITERION: finishes on its own, with no manual step, and the last line says
#   "agent Ready on 3/3 nodes; 3/3 nodes Ready". A timeout is FAIL, not "slow".

export KUBECONFIG="$DELONIX_ROOT/clusters/lab-cilium-kubeconfig.yaml"

# 4. State and drift (operation)
delonix cluster network status lab-cilium
#   CRITERION: desired and observed equal; rc=0.

# 5. Pods, same node and ACROSS NODES (B3, B4) — the test that decides support
kubectl create deployment web --image=nginx:1.27 --replicas=4
kubectl rollout status deployment/web --timeout=180s
kubectl get pods -o wide        # confirm they landed on DIFFERENT nodes
A=$(kubectl get pod -l app=web -o jsonpath='{.items[0].metadata.name}')
B_IP=$(kubectl get pod -l app=web -o jsonpath='{.items[-1:].status.podIP}')
kubectl exec "$A" -- curl -sS --max-time 5 "http://$B_IP" >/dev/null && echo "PASS cross-node"

# 6. DNS, ClusterIP, NodePort, hostPort (B3)
kubectl expose deployment web --port 80 --name web-cip
kubectl run probe --rm -i --restart=Never --image=busybox:1.36 -- \
  sh -c 'nslookup web-cip.default.svc.cluster.local && wget -qO- http://web-cip'
kubectl expose deployment web --port 80 --type NodePort --name web-np
#   plus a pod with hostPort, which exercises the portmap (ADR-0074) under this profile

# 7. External egress (B3)
kubectl run egress --rm -i --restart=Never --image=busybox:1.36 -- \
  sh -c 'wget -qO- --timeout=5 http://example.com >/dev/null && echo PASS egress'

# 8. MTU with large packets (B5)
kubectl exec "$A" -- ip link show eth0 | grep -o 'mtu [0-9]*'   # expected: mtu 1450
kubectl exec "$A" -- ping -c3 -M do -s 1422 "$B_IP"             # 1422+28 = 1450
kubectl exec "$A" -- ping -c1 -M do -s 1500 "$B_IP" && echo "FAIL: should fragment"

# 9. NetworkPolicy ingress AND egress (B6) — cilium and calico
kubectl create ns locked
kubectl -n locked run target --image=nginx:1.27
kubectl -n locked apply -f - <<'EOF'
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata: {name: deny-all, namespace: locked}
spec: {podSelector: {}, policyTypes: [Ingress, Egress]}
EOF
#   CRITERION: access that worked is now REJECTED. If it keeps working,
#   the profile does NOT deliver isolation and that must be written in the matrix.

# 10. Restarts (B7)
kubectl -n kube-system rollout restart ds/cilium
kubectl -n kube-system rollout status ds/cilium --timeout=180s
delonix vm stop lab-cilium-w1 && delonix vm start lab-cilium-w1
kubectl wait --for=condition=Ready node/lab-cilium-w1 --timeout=300s

# 11. Partial failure in ADD and cleanup by DEL (B8), on the node
#   crictl runp with a conflist whose chain fails on the 2nd plugin; confirm that
#   no IPAM lease nor netns survives (`ls /var/lib/cni/networks`,
#   `ip netns list`).

# 12. Leaks after cycles (B9)
for i in $(seq 1 20); do kubectl run churn-$i --image=busybox:1.36 --restart=Never -- true; done
kubectl delete pod -l run --all --wait
#   CRITERION: netns, veth, IPAM lease and nft rule counts return to their
#   starting value. Method: scripts/net-lifecycle-leaks.sh.

# 13. Offline (B10)
#   On a host WITHOUT reachability: copy $DELONIX_ROOT/net-profiles/ and run step 3
#   with --cni-offline. CRITERION: installs without network; without the cache, DX-6704.

# 14. Idempotency (B12)
delonix cluster apply -f examples/cluster/kubernetescluster.yaml
#   CRITERION: converges without duplicating anything, and rc=0. With ANOTHER profile:
#   DX-6703, and the cluster stays untouched.

# 15. Multus (B11)
#   --multus, a macvlan NetworkAttachmentDefinition, a pod with two
#   interfaces. CRITICAL CRITERION: confirm whether the secondary interface allows
#   BYPASSING the primary interface's NetworkPolicy. If it does, document it as a
#   limitation before any use.

# 16. Cleanup
delonix vm rm -f lab-cilium-cp1 lab-cilium-w1 lab-cilium-w2
delonix network rm lab-net
```

**Exit condition from `EXPERIMENTAL`, by profile:** B2-B9 and B12 at PASS, plus
B6 for profiles that declare they implement `NetworkPolicy`. Any FAIL
keeps the profile experimental and is written on this page with the result.

## 6. Known limitations

1. **No profile is proven against this CRI across several nodes.** It is item 1
   of ADR-0078's pending items.
2. **`ipFamily` only `ipv4`.** `ipv6` and dual-stack are refused (DX-1706) as
   **not implemented**, with ADR-0065 named. For dual-stack today:
   `cni: none` and your own CNI.
3. **Routed dataplanes absent.** `native` and BGP require the surrounding
   network to carry pod routes; this engine does not configure it.
4. **`kubeProxy: none` is a creation-time decision.** It changes `kubeadm init`
   (`skipPhases: addon/kube-proxy`) and cannot be toggled afterward.
5. **Switching CNI on a live cluster is not supported** (DX-6703). The cluster
   stays untouched; the way out is creating another cluster and migrating the
   workloads.
6. **`helm` is a host prerequisite for the Cilium profile.** Its absence is
   a refusal that names the alternatives, never a silent fallback.
7. **The Calico and Flannel images are pinned by tag**, not by digest.
8. **`cni: default` serves one node.** It was not extended: a node-local
   bridge has no way to route between nodes.
9. **LoadBalancer, BGP and L2 announcements are out of this delivery.** ADR-0066
   (L4 load balancer) is accepted and not built; nothing here wires BGP or
   L2 announcements, and nothing here creates a second owner of an address
   pool. The boundary holds: pod network (this page), VM network and firewall
   (`guia-rede.md`), apiserver VIP (the HAProxy `cluster kubeadm`
   provisions), and `LoadBalancer` Services (not delivered).
10. **`critest` was not re-measured** in this session (2026-10-15). **Since
    re-measured on 2026-10-24** on v5.1.1 → **84/20** (`84 Passed · 20 Failed · 18
    Skipped`, critest v1.36.0); `docs/cri-conformance.md` was updated and no
    longer lists the 79/103 of 2026-08-25 as the current number. See
    `reports/cri-conformance-2026-10-24/`.

## 6.1. Method note: why these conclusions do not depend on the binary on disk

A parallel session warned — and it was confirmed — that a SHARED
`CARGO_TARGET_DIR` between worktrees on different commits produces **mislabelled**
binaries: the build-script fingerprint does not include the git HEAD, so the
`commit:` `delonix version` prints can be from a different commit than the
code actually inside it. Measured at 21:12 on 2026-10-15: the binary at
`target/release/delonix` said `commit: ce3582329` (this series') and **did
not contain the string `cilium`** — it was from a different branch, written
over 4 minutes after §3.2's measurement ended.

**What this invalidates: nothing in §3.2 — and the reason matters.** The
conclusions do not rest on `delonix version` nor on the file currently at
that path, they rest on the BEHAVIOUR recorded in the run log, and that can
only come from this code:

- `Installing the cilium pod network` — a step that only exists here;
- `Waiting for the cilium pod network …` and `DX-6704` with the wording
  "nothing was deleted or rolled back";
- `--experimental-cni` accepted as a flag;
- and the version fix itself changing the observable: `--kubernetes-version=v1.34`
  present in the 1st run, **absent** in the next, with kubeadm answering
  `Using Kubernetes version: v1.34.12` on its own.

**The lesson, worth more than the episode:** a `cargo build` that ends
`exit 0` does not prove the binary is from the commit one thinks it is, when
`target/` is shared. Any measurement from this repository must be anchored to
observable behaviour (a string only that code emits, a flag only it accepts)
and not to the version label. To measure with your own `target/`:
`CARGO_TARGET_DIR=<your-own-path> cargo build --release` — costs ~10 GB and a
build from scratch, and this host's disk was at 100% three times in the same
day.

**One variable §3.2 could rule out for free.** Up to commit `f945b1c7`
(another branch), `--network <name>` was accepted, recorded and **discarded**
in the libvirt provider: every node landed on the `default` network. The §3.2
run used `--network default`, so the outcome was the same with or without
that bug — and both nodes received, measured, **distinct addresses and
client-ids** (`192.168.122.114` and `.15`). Moreover: the Cilium agent dies in
`Mounting BPF filesystem`, the FIRST thing it does, before touching the
network at all. Addressing, libvirt network choice and lease sharing cannot
explain a mount-namespace construction failure.

## 7. Diagnosis and recovery

**The cluster does not reach `Ready`.** The bootstrap already failed with the
counts (`agent Ready on A/D nodes; N/E nodes Ready`) and **deleted nothing**:

```bash
delonix cluster network status <name>              # desired vs observed
delonix cluster <name> -n kube-system get pods -o wide
delonix cluster <name> -n kube-system describe ds <agent>
delonix cluster <name> -n kube-system logs -l k8s-app=<agent> --tail=100
ssh delonix@<node> sudo delonix-node-preflight <profile>   # what is missing ON THE NODE
```

**Retrying is how to resume.** `kubectl apply` is idempotent and the
installation converges; `delonix cluster apply -f <manifest>` with the
**same** profile resumes where it stopped. With another profile, DX-6703 and
nothing changes.

**Pods `Pending` with no address.** Almost always a pod CIDR too small for the
nodes — refused today at validation time (DX-1704), but a cluster created
before this release may be in that state.

**Unexplained slowness with large packets.** It is MTU. `ip link show eth0`
inside a pod must show the profile's MTU (1450 on VXLAN over 1500), and
`ping -M do -s <mtu-28>` must succeed while `-s 1500` fragments.

**Updating a profile.** There is no supported path to change a profile's
version on a live cluster in this release. The procedure is the upstream
project's own (`helm upgrade`/`kubectl apply`), outside the engine, and
remains **outside** what this page guarantees.
