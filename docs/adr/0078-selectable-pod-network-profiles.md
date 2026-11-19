# ADR-0078: A kubeadm cluster's pod network is a selectable PROFILE, pinned by content and refused before any machine exists

- **Status:** Accepted (2026-10-15); **amended 2026-10-27** — see
  "Update, 2026-10-27" at the end. D1 through D6 implemented. The D7 from
  2026-10-15 failed on Cilium (duplicate bpffs, a CRI defect); since then the
  two CRI defects blocking it have been **fixed and Cilium's D7 passes
  live** (nodes Ready + cross-node pod-to-pod + services). New decision: on a
  multi-node kubeadm cluster `cni: default` now **resolves to Cilium**
  instead of being refused (the old DX-6701), so a multi-node cluster comes
  up functional with no opt-in.
- **Date:** 2026-10-15
- **Deciders:** Walter Angolar
- **Relates to:** ADR-0074 (publishing `hostPort` on the CRI — its own
  "what was not measured" section is the reason for D2), ADR-0038 (the CRI
  follows the kubelet's resource model), ADR-0065 (IPv6 dataplane — the
  reason `ipFamily` is "not implemented" rather than "impossible"), ADR-0069
  D3 (CNI plugins have a deadline and a ceiling) and D7 (`KubernetesCluster`
  stays out of `stack apply`), ADR-0018 (VM images stay amd64),
  `docs/discovery/kaas-matrix-2026-10-08.md` (gap #1).

## Context

A `kind: KubernetesCluster`'s `spec.cni` accepted exactly two values:

| value | what it does | consequence |
|---|---|---|
| `default` | a node-local `bridge`+`host-local`+`portmap` conflist | **refused above ONE node** (DX-6701): does not route pods between nodes |
| `none` | installs nothing | every node stays `NotReady` until someone applies a CNI by hand |

In other words: **this engine could not create a functional multi-node
kubeadm cluster.** The refusal was honest — DX-6701 says so in full — but it
was still a refusal. The KaaS audit's gap #1 (2026-10-08) names it as the
most serious in the matrix, and notes that the "node `Ready`" state was
*structurally unreachable* down this path.

Three facts measured before designing, all against the code and not the
documentation:

1. **Nothing in the build named the CNI plugins.** The only supplier of
   `/opt/cni/bin` was the `kubernetes-cni` `.deb`, arriving as a `kubelet`
   dependency — and its version is **deliberately not pinned**
   (`download_k8s_debs`, whose `VERSIONED` covers only
   `kubeadm`/`kubelet`/`kubectl`, because `kubernetes-cni` has its own
   versioning, 1.7.x). The set of binaries on a node was whatever
   `pkgs.k8s.io` was publishing on build day. The only evidence in the
   repository that `portmap` even exists is an ADR-0074 measurement and a
   `scripts/e2e.sh` test that **skips** when the file is missing.

2. **`valid_cidr` was a CHARACTER-SET check.** It accepted `9`, `1.2.3.4`,
   `///` and `0.0.0.0/99` as a `podSubnet`, for a solid reason (the value is
   interpolated into a remote `bash -c`, and that is the defence against
   injection) — but the failure arrived later, from `kubeadm`, on a host.

3. **A real CNI agent's CRI requirements are all implemented, and were never
   exercised together.** Measured 2026-10-15 by rendering the pinned Cilium
   1.20.2 chart: the agent asks for `privileged: true` (×1),
   `SYS_ADMIN` (×5), `NET_ADMIN` (×4), `SYS_MODULE`, `SYS_RESOURCE`,
   `SYS_PTRACE`, `SYS_CHROOT`, `NET_RAW`, `mountPropagation: Bidirectional`
   (×1) and `HostToContainer` (×4), and `hostNetwork: true` (×3). This
   engine's bootstrap **already** raises the capability ceiling on its nodes
   (`DELONIX_CRI_CAP_CEILING=all`, in a visible drop-in), the CRI **already**
   translates `Bidirectional`→`rshared` and `HostToContainer`→`rslave`, and
   **already** skips the CNI under `hostNetwork`. None of these pieces had
   been proven together.

## Decision

### D1 — The profile is a contract value, with one table as the single source

`spec.cni` now accepts `default | none | cilium | calico | flannel`, and a
new `spec.network` block expresses *how* that profile is configured
(`dataplane`, `kubeProxy`, `mtu`, `nodeMtu`, `ipFamily`, `interface`, `multus`,
`hubble`, `experimental`, `offline`).

Everything decidable without touching a host is decided in
`cmd/net_profiles.rs` — pure, tested with no host — and the side that touches
things lives in `cmd/net_profiles_install.rs`. **The split is not cosmetic:**
it is what lets a bad cluster be refused before a machine exists, which is
why `cluster kubeadm` (the path that CREATES VMs) validates the whole network
request on its first line, before resolving the image.

No default value lives in two places. This repository's
`k8s_recipes::resolve_k8s_version` has a recorded cost of having had two: an
independent default once drifted **four minor versions** from the other, and
the build pulled from a repository the documentation never mentioned.

### D2 — A profile is not "supported" just because the manifest applies it. All three are born EXPERIMENTAL

Each profile carries a support level **with the reason**, and an
`Experimental` one is **refused** (DX-1705) unless the manifest names it
explicitly (`spec.network.experimental: true` / `--experimental-cni`).

All three are born experimental because ADR-0074's own "what was not
measured" section says so in writing: *"A real cluster CNI (Calico, Cilium)
declaring the capability differently, and more than one node, were not
measured."* Cilium is the profile this engine **wants** — and that is exactly
why it is not declared supported before being measured. Shipping them as
supported would be the "claims success on work it never did" failure this
project calls out in other people's code.

The gate is **declarative and visible**, not an environment variable: an
operator who chose an unmeasured profile has to have written it down.

### D3 — No silent fallback. A combination the engine cannot deliver is a refusal that names the field

Seven new refusals, each with its own code and each firing before any
machine exists:

| code | refusal |
|---|---|
| DX-1701 | a `cni` that does not exist — and the message **lists the ones that do** |
| DX-1702 | a knob the chosen profile does not read (including any knob under `cni: default`/`none`) |
| DX-1703 | an MTU that does not fit the encapsulation |
| DX-1704 | a malformed CIDR, with bits outside the prefix, overlapping itself, overlapping a network of this host, or too small for the nodes |
| DX-1705 | an experimental profile with no explicit consent |
| DX-1706 | an `ipFamily` other than `ipv4` — stated as **not implemented**, naming ADR-0065 |
| DX-6703 | changing the CNI of a live cluster (the cluster stays **untouched**) |
| DX-6704 | an artefact that could not be obtained, or whose bytes do not match the digest |

A cluster that asked for Cilium and got Flannel is worse than a cluster that
was never created.

**Why the MTU is a refusal and not a warning:** an MTU that does not fit does
not fail. The dataplane comes up and silently drops large packets, which
reads as unexplained slowness, not as an error. `docs/guia-rede.md` already
records this trap for the engine's own encrypted overlay — 1500 − 50 (VXLAN)
− 60 (WireGuard) ≈ 1390 — and the lesson is the same here.

**Why bits outside the prefix are refused rather than masked:**
`10.244.0.1/16` is almost always a typo for `10.244.0.0/16`. Accepting it
after masking means the manifest and the cluster disagree about what was
requested.

**Routed modes deliberately absent.** `native` and BGP require the network
around the cluster to carry the pods' routes. This engine does not configure
that network and will not assume it, so those modes are **absent**, not
accepted-and-broken. VXLAN is the first entry on every list because it is
the only mode that asks nothing of what sits between the nodes.

### D4 — The pin is the CONTENT, not the version

Each profile names a version **and the SHA-256 of the exact artefact**. A URL
that starts answering different bytes fails closed instead of installing
something else. `latest` appears on no reproducible path, and there is a
test verifying that for every profile.

Pinned on 2026-10-15, each digest obtained from the publisher's `.sha256`
sidecar **and** re-confirmed with a local `sha256sum` of the file that
arrived:

| artefact | version | sha256 (prefix) |
|---|---|---|
| `containernetworking/plugins` | 1.9.1 | `b98f74a0f8522f0a` (amd64), `56171987d3947707` (arm64) |
| Cilium chart | 1.20.2 | `b2afd87b7f75f875` |
| Calico manifest | 3.33.0 | `2de8f47595fb9c41` |
| `kube-flannel.yml` | 0.28.10 | `393bdbce53eb50c8` |
| Multus daemonset | 4.3.1 | `bd618d23a9577290` |

**What these pins do NOT cover, stated plainly:** the container images
*referenced inside* the Calico and Flannel manifests are pinned by TAG
upstream, not by digest — `docker.io/calico/node:v3.33.0` is only as
reproducible as Docker Hub decides. The Cilium chart is the best case:
verified at render time, its three images all carry `@sha256:`. Resolving
the other two to digests requires a registry round-trip per image and
**remains open**, recorded as such.

### D5 — Installing means waiting. `kubectl apply` returning 0 is not the criterion

A profile is considered installed when **its agent is `Ready` on every node
AND every node is `Ready`**, observed by polling, with a bounded deadline,
and with the observed state recorded. `kubectl apply` exiting 0 only says
the apiserver accepted the objects.

A profile **always waits**, regardless of `--copy-kubeconfig`: the flag
exists to preserve the historical timing of a path that never installed any
CNI, and a path that just installed one and did not check is the failure in
its purest form. A timeout is an **error** with counts
(`agent Ready on A/D nodes; N/E nodes Ready`), not a warning — and **nothing
is torn down or reapplied**: a half-installed CNI is the only evidence that
explains the failure.

Installation happens **between `kubeadm init` and the joins**, as the
kubeadm documentation asks: a node joining a cluster whose network is
already installed gets the agent as soon as it joins, instead of sitting
`NotReady` until someone notices.

**Idempotency:** this is not a `check` that skips, it is an apply that
converges. Repeating the same profile is safe and resumes a half-done
install. Repeating with ANOTHER profile is DX-6703, before the artefact is
even fetched.

### D6 — The image carries the prerequisites; what it does not configure gets verified

**In the golden image:** the pinned, verified reference subset of plugins
(ten, each with its reason written down, and the eight no profile reaches —
`dhcp`, `static`, `vlan`, `vrf`, `ptp`, `sbr`, `tap`, `dummy` — **do not**
ship: unreachable code on a machine handed to third parties is attack
surface, the same reasoning `k8s_node_debloat_steps` already applies); the
profiles' modules and sysctls, in the **shared** `k8s_recipes` catalog so the
golden image and a host prepared live cannot diverge; diagnostic tools the
cloud image does not carry (`ethtool`, `conntrack`, `tcpdump`); and
provenance (`DELONIX_CNI_PLUGINS`, `DELONIX_NET_PROFILES`) in
`/etc/delonix-image-release`.

**The golden image still ships no conflist at all.** This is deliberate and
matters: the kubelet picks the first file in `/etc/cni/net.d` in lexical
order, and a generic bridge sitting there would be chosen **before** the
intended CNI. The absence is the decision.

**What the image deliberately does NOT configure:** it does not mount bpffs
and does not touch the cgroup hierarchy. Both are already true on these
distributions (systemd mounts `/sys/fs/bpf`; the unified hierarchy has been
the default since 22.04), and writing over a system that already works just
to satisfy a checklist is how you break what did not need touching.

**Instead, it gets verified.** `dist/delonix-node-preflight.sh` measures, per
profile: swap, modules, sysctls, the CRI's service and socket, the binaries
that profile's chain executes, vxlan, and — for Cilium — bpffs, cgroup v2
and kernel ≥ 5.10; for Calico, `rp_filter` and the iptables modules. It
reports **all** failures (the same reasoning as `scripts/lab_preflight.sh`:
naming only the first one is the same lie one level up), and **never exits 0
because it failed to measure something** — an "I don't know" counts as a
failure. `prepare_host` runs it before kubeadm and **pushes** it to a node
whose golden image predates it.

### D7 — The live multi-node validation RAN, and Cilium failed

Run on 2026-10-15, two nodes (1 control-plane + 1 worker) on libvirt/NAT,
with the golden image built in the same session. The detail is in
`docs/network-compatibility.md` §3.2; the summary:

**Everything passed up to the dataplane.** The VMs with distinct addresses
(DX-6702, which the older golden image used to fail), both hosts prepared,
`kubeadm init` in 32.4s, **the profile install step in 1.0s** at its intended
spot (between `init` and the join), the worker joining in 6.3s, and both
nodes registered in the cluster with `CONTAINER-RUNTIME: delonix://5.1.1` —
this CRI serving a real kubelet on two nodes. The Cilium DaemonSet scheduled
2/2.

**The Cilium agent does not start:**

```
level=fatal msg="Unable to mount BPF filesystem"
            error="multiple mount points detected at /sys/fs/bpf"
```

The node's host has **exactly one** bpffs, already `shared`. The duplicate
is built **inside the container**, by this engine: a `privileged` container
gets `/sys` via a recursive bind and the pod asks for `/sys/fs/bpf` as a
hostPath — two entries at the same point. It is a **CRI defect**, in the
mount path for privileged containers, which serves far more than CNIs — so
it is not fixed in this change, and is pendency #1.

**The readiness wait failed honestly**, which is D5's decision being
exercised: `✗ … 7m00s` and DX-6704 with the counts ("agent Ready on 0/0
node(s) and 0/2 node(s) report Ready"), with nothing torn down and the
diagnostic commands named. A successful `kubectl apply` was not treated as
success.

> **Correction, 2026-10-20.** The commands DX-6704 names **did not run**
> at the moment they were printed: they resolve the cached kubeconfig, and
> `apply_ssh` only fetched it in the last step — after the profile and
> after the wait. Reproduced on a 3+2 cluster with `--cni flannel`:
> DX-6704 at 420s, immediately followed by
> `DX-1000 … no kubeconfig for cluster`. D5 still stands — what failed was
> the actionable half of the message, not the criterion. The kubeconfig is
> now saved right after `kubeadm init`; see
> `docs/network-compatibility.md`, correction to L10.

**What this proves, and what no code reading could have proven:** the
`EXPERIMENTAL` gate was right. A profile with a pinned artefact, verified,
rendered with correct values and applied successfully **still does not
work.** Declaring Cilium supported because the code looked right would have
been exactly the kind of claim DX-1705 exists to prevent.

**Calico and Flannel remain NOT RUN.** An attempt to isolate with Flannel
(same pair of VMs, a bpffs-free profile) stopped because `kubeadm reset`
left the apiserver holding port 6443. That is the next step, and it
separates Cilium's failure from the rest of the chain.

## Alternatives considered

**Installing with `helm install` instead of `helm template`.** Rejected: it
would leave release state inside the cluster, from which a second run could
diverge, adding a resident piece to an engine that has none. `helm template`
runs **on the host**, produces a plain manifest, and `kubectl apply` makes it
idempotent. The absence of `helm` is a refusal that names the alternative
(`--cni calico`, `--cni flannel`), never a silent fallback to another CNI.

**Embedding the manifests in the binary.** Rejected: it would tie the CNI's
version to the engine's version, and a CNI security fix would then require
an engine release.

**Pinning only the version and trusting the URL.** Rejected: that is what
`kubernetes-cni` already did, and it is how a node's plugin set ended up
being "whatever the publisher had that day."

**Accepting `native`/BGP because upstream supports them.** Rejected: they
require configuring the network around the cluster, which this engine does
not do. Accepting a value it cannot deliver is the exact definition of the
problem this ADR solves.

**Letting `cni: default` grow into multi-node.** Rejected: a node-local
bridge has no mechanism to route between nodes. Extending it would be
trading one failed promise for another, quieter one.

## Consequences

**Gained:** a functional multi-node kubeadm cluster for the first time;
refusals before a machine exists instead of failures on a host; artefacts
reproducible by content; and an observable state (`cluster network status`)
that reads the cluster, not our own record — so it can detect drift.

**Costs:** `helm` becomes a **host** prerequisite for the Cilium profile (in
the same family as `ssh`, `scp`, `qemu-img`, `virt-customize` and `gpgv`,
which this engine already relies on); the golden image gains ~70 MiB of
plugins and diagnostics; and three profiles sit behind explicit consent,
which is deliberate friction.

**Not gained, and not claimed:** no profile is proven against this engine's
CRI across several nodes. Network policies have **not** been tested —
`enforces_network_policy` describes what the profile implements, not what
was measured here, and Flannel explicitly says it delivers none. Multus is
implemented and its secondary interfaces were not exercised; whether a
secondary interface is isolated from the primary one's policies **was not
verified**, and is the lab's first item.

## Pendencies, in order of importance

> Pendency 1 (bpffs) was FIXED (#869/#871), the next blocker it revealed
> (CNI_ARGS) too, and `cilium-envoy` too — see "Update, 2026-10-27". Cilium
> now comes up COMPLETE (agent + envoy + operator), not just the agent.

1. ~~**The duplicate bpffs inside a privileged container**~~ — **FIXED**
   in #869 (`c95a3b19`, dedup of `/sys`'s recursive bind) + #871
   (`f43287c5`), both on `develop`. Re-proven live 2026-10-27: Cilium
   starts. `/sys` was entering via recursive bind AND the pod was asking
   for `/sys/fs/bpf` as a hostPath; the agent refused with "multiple mount
   points detected".
2. **Run D7 for Calico and Flannel**, and promote or block each on the
   result — **BOTH DONE and PROMOTED to `Supported`.**
   - **Flannel (2026-10-27)**: D7 on a 2-node cluster (`--cni flannel`,
     golden 1.37): nodes Ready v1.37.1, DaemonSet 2/2, cross-node
     pod-to-pod OK and ClusterIP DNS resolving
     (`reports/.../d7-flannel*`). No bpffs/identity involved, it isolated
     the datapath.
   - **Calico (2026-10-28)**: same D7 (`--cni calico`): nodes Ready
     v1.37.1, calico-node 2/2 + calico-kube-controllers Running, cross-node
     pod-to-pod OK and DNS resolving (`reports/.../d7-calico*`). The
     DATAPLANE is proven; Calico's NetworkPolicy ENFORCEMENT remains
     untested (pendency #3).
   - **Cilium (2026-10-28, deliberate owner promotion)**: proven end to
     end this campaign (nodes Ready, agent/operator/envoy Running,
     cross-node + DNS, 0 endpoints in `reserved:init`) after the three CRI
     fixes (bpffs #869/#871, CNI_ARGS, envoy `/dev/shm`+mount ordering).
     `Profile::Cilium.support()` → `Supported`. **All THREE addons are now
     `Supported`** — no profile is `Experimental`. The mechanism
     (`Support::Experimental` + DX-1705 + `--experimental-cni`) stays
     DORMANT (`#[allow(dead_code)]`), ready for the next unmeasured profile.
3. **Network policies tested** (ingress and egress) on the profiles that
   implement them. Installing Cilium does not mean isolation is delivered.
4. **Resolve Calico's and Flannel's images to digests** (D4).
5. **Isolation of Multus's secondary interfaces** — and who can create a
   `NetworkAttachmentDefinition`.
6. **`prevResult` on the CNI's `DEL` and the conflist persisted on the
   rootless path**: `cni::del` passes `None`, and the rootless `DEL`
   re-reads the current `/etc/cni/net.d` instead of the `sb.cni_conf` the
   `ADD` wrote. This is ADR-0069 Pending #8, and touches `portmap` cleanup.
7. **`--extra-package` is silently discarded on the offline path** of the
   golden image build (`k8s_customization_steps_offline` does not receive
   `extra_packages`), and the build records it in
   `/etc/delonix-image-release` as if it had been installed. Found by
   reading in this session, out of scope for this change.
8. **`k8s_repo_version` has an independent default** (`stable:/v1.31`) that
   disagrees with `DEFAULT_K8S_VERSION` (1.36). There is a fix in another
   session's working tree; it was not duplicated here to avoid a conflict.

## Update, 2026-10-27 — `cni: default` → Cilium on multi-node, and D7 passes

Two things changed since 2026-10-15, and one partly reverses the
"Letting `cni: default` grow into multi-node. Rejected." alternative above —
which made sense when NO profile worked cross-node.

**1. The two CRI defects blocking Cilium are fixed.**

- The duplicate bpffs (pendency #1) was fixed in **#869** (`c95a3b19`) and
  **#871** (`f43287c5`), already on `develop`. With these, Cilium STARTS.
- Running D7 then revealed the next blocker: **the CRI was not passing
  `CNI_ARGS`** to the CNI plugin. Without `K8S_POD_NAMESPACE`/`K8S_POD_NAME`,
  Cilium could not associate the sandbox with the Kubernetes pod, could not
  read its labels, and left every endpoint in the `reserved:init` identity —
  which its own policy isolates. Result: nodes Ready, agent Ready 2/2, but
  cross-node pod-to-pod blocked. **Fixed** in this session:
  `delonix-sdn/src/cni.rs` (`Target.args` → `build_env` emits `CNI_ARGS`;
  `add`/`attach_named_netns` pass it through) and
  `delonix-cri/.../lifecycle.rs` (`cni_k8s_args` builds the kubelet's keys
  from the `PodSandboxConfig`, on the ROOT path). The opt-in rootless path
  (`DELONIX_CNI=1`) **now also passes**: the holder's control line gained a
  FIFTH token — the `CNI_ARGS` in hex (`build_cni_add_cmd`/`parse_cni_add` in
  `delonix-sdn/infra.rs`, 4 OR 5 tokens accepted for host/holder
  compatibility), and `do_cni_add` forwards them to `attach_named_netns`.
  Round-trip tested; the live rootless end-to-end proof (crictl + fake CNI
  plugin + rootless delonix-cri) was not run — the identity mechanism is
  already proven on the root path and the downstream
  (`attach_named_netns`→`build_env`) is the same.

**2. `cni: default` resolves to Cilium on a multi-node kubeadm cluster.**

Owner decision (2026-10-27). `net_profiles::resolved_profile` (single
source) maps `default` → `Cilium` when `mode != kind` and `nodes > 1`; a
single node still gets the node-local bridge, and `mode: kind` still gets
kindnet. The old **DX-6701** (refusal) no longer fires — `default` now means
"give me a cluster that works." Since the engine is the one choosing, the
engine-default profile does not go through the `EXPERIMENTAL` gate
(DX-1705); naming `cilium`/`calico`/`flannel` explicitly still requires
`--experimental-cni`. The code lives in `cmd::net_profiles` and
`cmd::cluster` (orchestration), not in the CRI.

**D7 re-run and PASSES (2026-10-27)**, host with 31 GiB free (the
2026-10-15 one had 5). `delonix cluster kubeadm --control-plane 1 --workers 1
--memory 4G` (default cni, with NO opt-in):

- `cilium agent Ready on 2/2 node(s); 2/2 node(s) Ready`; cluster ready.
- Identities resolved (`k8s:app=…`), **0 endpoints in `reserved:init`**.
- **Cross-node pod-to-pod OK** both ways (pods pinned to cp1 and w1, distinct
  /24s, HTTP via busybox httpd).
- **Service/DNS OK**: `nslookup kubernetes.default` against the ClusterIP
  resolves — kube-proxy + coredns + cross-node datapath end to end.

Evidence in `reports/cilium-d7-rerun-2026-10-27/` (`create-with-cniargs.log`,
`crossnode-connectivity-FIXED.txt`, `cilium-health.txt`, `diag-identity.txt`).

**3. `cilium-envoy` comes up (two CRI mount defects, fixed).** After the
agent, the `cilium-envoy` sidecar still sat in CrashLoopBackOff — for TWO
reasons, one hiding the other:

- (a) **`/dev/shm` missing**: `setup_dev`/`setup_dev_userns` mounted `/dev`
  (tmpfs) but never `/dev/shm`, which every OCI runtime (runc) provides; Envoy
  aborted with "cannot open shared memory region /envoy_shared_memory_0".
  Fixed with `mount_dev_shm` (tmpfs `mode=1777,size=65536k`,
  `nosuid,noexec,nodev`) on both paths (`delonix-linux/src/lib.rs`). This
  serves any `shm_open` user (Envoy, Chromium, PostgreSQL DSM, tests).
- (b) **nested-mount ordering**: the pod mounts `/var/run/cilium/envoy/`
  READ-ONLY and `/var/run/cilium/envoy/sockets` READ-WRITE, and the kubelet
  lists the RW child BEFORE the RO parent; `setup_rootfs` applied them in
  arrival order, the RO parent buried the RW child, and Envoy died with
  "cannot bind '…/sockets/admin.sock': Read-only file system." Fixed with a
  stable sort by destination depth (`mount_target_depth`), the way runc does
  it — parents before descendants. Proven live 2026-10-27: `cilium-envoy`
  Running 1/1 on both nodes, `/dev/shm` and `…/sockets` both `rw`
  (`reports/cilium-d7-rerun-2026-10-27/envoy-FIXED.txt`). Cilium now comes
  up COMPLETE.

**Flannel and Calico: D7 DONE and PROMOTED to `Supported` (2026-10-27/28)** —
2-node cluster (`--cni flannel` / `--cni calico`, golden 1.37): nodes Ready
v1.37.1, DaemonSets ready, cross-node pod-to-pod OK and ClusterIP DNS
resolving. `Profile::{Flannel,Calico}.support()` moved to `Supported`, so
they no longer require `--experimental-cni`. Only Cilium remains
`Experimental`.

**Cilium, Calico and Flannel: all THREE `Supported` (2026-10-28).** No
profile is `Experimental`; the mechanism (`Support::Experimental` + DX-1705 +
`--experimental-cni`) stays DORMANT, ready for the next unmeasured profile.

**NetworkPolicy enforcement (Cilium): PROVEN (2026-10-28, pendency #3 for
Cilium).** Cilium cluster (golden 1.37), pods `srv`/`cli`: baseline OK →
`default-deny-ingress` on `app=srv` → `cli→srv` BLOCKED → `allow from
app=cli` → OK again (`reports/.../netpol-enforcement.txt`). Calico uses the
same k8s NetworkPolicy contract; not re-tested here.

**CNI_ARGS on the rootless path: PROVEN LIVE (2026-10-28).** Rootless
delonix-cri (`DELONIX_CNI=1`, its own root/socket) + `crictl runp` + a fake
CNI plugin under `/opt/cni/bin`: the plugin received, via the holder,
`IgnoreUnknown=1;K8S_POD_NAMESPACE=argns;K8S_POD_NAME=argtest-pod;
K8S_POD_INFRA_CONTAINER_ID=cri-…;K8S_POD_UID=…`
(`reports/.../rootless-cni-args-proof.txt`). The control line's 5th hex
token reaches the plugin end to end.

**Still to do:** Calico's NetworkPolicy enforcement (same contract, not
re-measured); egress policy; and Multus's secondary interfaces (pendency
#5).
