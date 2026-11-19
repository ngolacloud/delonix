# CRI PortForward — critest conformance (A/B, same base)

critest **v1.36.0**, `-ginkgo.focus "portforward"`, against an isolated
`delonix-cri` (its own WORK + `DELONIX_ROOT`, free network holder), rootless host.
A/B on the **same base** (`origin/main` @ `ab4751ac`): only the binary changes.

Specs in focus:
- `runtime should support portforward [Conformance]` (normal netns)
- `runtime should support portforward in host network`

## Before (main binary, without the fix) — `before.log`

```
FAIL! -- 0 Passed | 2 Failed | 0 Pending | 120 Skipped
```

- **in host network** → `error upgrading connection: unable to upgrade connection: pod sandbox has no live netns`.
  Code bug: a `hostNetwork` pod has no infra/netns of its own, and
  `portforward_upgrade` required the pid of the `pod-cri-<id>` infra (which does not exist).
- **[Conformance]** → fails BEFORE the port-forward, at the prerequisite
  "ensure the web server is serving": `dial tcp 10.200.0.2:80: i/o timeout`
  (networking.go:305). This is reaching the **pod IP** from the host — a **rootless**
  topology (slirp not routable from the host), not the port-forward code.

## After (binary with the fix) — `after.log`

```
FAIL! -- 1 Passed | 1 Failed | 0 Pending | 120 Skipped
```

- **in host network** → **PASSES** (green). The forward now connects at
  `127.0.0.1:<port>` in the host netns (`PfNetns::Host`) instead of doing `setns`
  into a pod netns that never existed.
- **[Conformance]** → still fails at the SAME environmental prerequisite
  (`dial 10.200.0.2:80 i/o timeout`), identical before/after — independent of the
  port-forward. Exercising the data path of the normal port-forward requires an
  environment with a routable pod IP (root/CNI or a kubeadm node); that is out of scope for this fix.

## The fix

`crates/interfaces/delonix-cri/src/`:
- `streaming.rs` — `portforward_upgrade` branches on `sandbox_is_host_network`:
  host-network → `PfNetns::Host`; normal pod → infra pid (the "no live
  netns" error is kept for a dead infra, which is the correct case).
- `spdy.rs` — `PfNetns { Pid(i32), Host }` + `connect_pf`: `Host` connects directly,
  `Pid` does `setns` as before.
- `runtime_svc/lifecycle.rs` — `pub fn sandbox_is_host_network(base, id)`.

---

## PortForward [Conformance] (normal netns) — A/B on the ROOT + CNI path

The `portforward [Conformance]` spec never went green on the rootless host because
it failed BEFORE the port-forward, at the "web server is serving" prerequisite (the pod
IP is not routable from the host under slirp/pasta). To exercise it you need an environment
with a **routable pod IP**. In CI it fails AFTER the prerequisite, in the forward itself
(`streaming.go`) — so there is a code bug hidden behind the environment.

### Routable environment (no root on the host)

Sudo and root on the host are off-limits to the harness. The clean path: run `delonix-cri`
as **root INSIDE a `--privileged` container** (`docker` group), where it
takes the ROOT + CNI path (real bridge, veth, `host-local` IPAM) and pod IPs are
routable in the container's netns. Harness: `scripts/critest-root-cni.sh` (base
`ubuntu:24.04` to match the host's glibc 2.39; binaries+critest+crictl+CNI plugins
mounted; bridge+portmap conflist; `DELONIX_ROOT` on a **volume** to escape
overlay-on-overlay; cgroup v2 delegation dance for the `memory` controller).
A/B on the **same base** (`cab3fcdf`): only the binary changes (the diff of this report).

### Before (without the named-netns fix) — `before-root-cni.log`

```
FAIL! -- 0 Passed | 1 Failed | 0 Pending | 121 Skipped
```

Prerequisite **PASSES** (`the IP:port is http://10.88.0.2:80` → `check port
mapping succeed` — routable pod IP, no i/o timeout). The forward **FAILS**:
`start port forward` → `error upgrading connection: ... pod sandbox has no live
netns` (`streaming.go:391`). This is the CI failure, reproduced live.

### After (with the fix) — `after-root-cni.log`

```
SUCCESS! -- 1 Passed | 0 Failed | 0 Pending | 121 Skipped
```

The data path runs end-to-end: `start port forward` connects, and
`check if we can get nginx main page via localhost:12001` → `Check port forward
URL ... succeed`. The bytes crossed the pod's named netns and came back.

### Root cause and the fix

On the **ROOT + CNI** path, the sandbox is a NAMED netns on the host
(`/run/netns/cri-<id>`, recorded in `SandboxRec.cni_netns`) and the pod's containers
join it with `--net host` — **there is no `pod-cri-<id>` infra** with a pid.
But `portforward_upgrade` resolved the netns ONLY by the infra pid
(`pod_sandbox_pid`), which on that path never exists → `CONFLICT "pod sandbox has no
live netns"` for a perfectly live pod.

- `spdy.rs` — `PfNetns` gains `NsPath(PathBuf)`; `connect_in_netns` now opens
  a **netns path** (`/proc/<pid>/ns/net` OR `/run/netns/cri-<id>`) and
  `setns`-es into it; `connect_pf` maps `Pid`→`/proc/<pid>/ns/net`,
  `NsPath`→the named file, `Host`→direct. (`PfNetns` is no longer `Copy`.)
- `lifecycle.rs` — `pub fn sandbox_cni_netns(base, id) -> Option<String>` reads
  `cni_netns` (empty on every other path).
- `streaming.rs` — `portforward_upgrade` branches: host-network → `Host`; else
  `cni_netns` present → `NsPath`; else infra pid (with the real "no live netns"
  of the dead infra left intact).
