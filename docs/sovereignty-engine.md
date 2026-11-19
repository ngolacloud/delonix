# Sovereignty — the slice that belongs to the engine

> Measured: **2026-09-15**. Engine: `delonix` 3.1.0 (tree build, commit
> `6a9e1fe5b` — slightly behind the `HEAD` of this measurement; see the method
> note at the end). Host: Linux 7.0.0, cgroup v2, `cloud-hypervisor` and
> `libvirt` backends installed.

This document is not "N'GolaCloud's sovereignty report" — it is the slice of
it that `delonix-runtime`, alone, on one node, can prove. The eight testable
properties are defined in `ngolacloud-doutrina/references/soberania.md` (the
platform's workspace); four of them belong to this engine, four belong to the
platform above it (residency, keys, proof for the regulator, exit without
hostage to the managed service). Reading the two halves as if they were one
makes a sovereignty report read stronger than it is.

The platform already has substantial work done on the "WAN unplugged"
exercise — five passes, 2026-08-18, at the scale of the whole cluster
(`ngolacloud-doutrina/references/soberania.md`). This document does not
repeat it; it measures the same property one level down, at the engine.

## Report format (`ngolacloud-doutrina` §"When reporting sovereignty")

```text
Operational independence (engine): PROVEN on 2026-09-15 —
    container run/exec/stop of an already-cached image: zero network touch.
    vm create/boot of an already-cached image (cloud-hypervisor): IP confirmed by
    ARP, zero network touch. kubeadm init offline: unproven (reason: would require
    SSH into the VM and a 2nd/3rd node, on a shared host — not a justifiable
    expense outside a session dedicated to it).
Foreign (engine): boot 0 (local image) · first-use 1 (ghcr.io,
    golden VM/image) · kubeadm 0-1 (conditional on `--offline build`, not
    re-validated on this pass) · image-build 6 (all build-time)
Keys / supply chain: out of scope for this pass
Client exit: out of scope for this pass
```

## The test: operational independence, at engine scale

**Protocol**: instead of cutting the machine's network (a shared host, with
real containers from other sessions running — not a lab, and a real network
cut would affect everyone), the engine's process ran entirely
inside ITS OWN isolated netns (`unshare --user --net`, no privilege),
with **zero interface besides loopback** and DNS failing by design — the same
"no WAN" effect a real cut would have, without the cost of doing it.

```text
$ unshare --user --map-root-user --net -- sh -c 'ip addr show; getent hosts docker.io'
1: lo: <LOOPBACK> mtu 65536 qdisc noop state DOWN ...
(no other interface)
getent hosts docker.io → failed (no DNS, as expected)
```

### Container

| Case | Result |
|---|---|
| `container run` of an **already-cached** image, `--net none` | **rc=0** — `alpine 3.24.1` responding inside the container |
| `container run` of an **already-cached** image, default SDN network (which would normally have a NAT uplink to the internet) | **rc=0** — the container comes up and runs; the absence of an outbound route does not block startup |
| `container run` of a **non-cached** image | **rc=1 in ~1s** — an explicit failure naming the URL (`registry-1.docker.io/v2/...`), never a hang |

### VM (cloud-hypervisor)

```text
$ unshare --user --map-root-user --net -- \
    delonix vm create sov-test --backend cloud-hypervisor \
    --disk delonix-vm-k8s:1.34 --wait --boot-timeout 60
...
info vm 'sov-test' is up — ip 10.200.254.29
```

`is up` here is the value **confirmed by ARP** (`infra::sdn_reachable`), not the
address computed from the MAC that `AGENTS.md` already documents as the
trap to avoid (see the section "The `--wait` of a CH VM was waiting for a
number it already knew") — the difference between "the engine said it's up"
and "I proved it responded". A 3.5 GiB qcow2 (679 MiB actually on disk, image
`delonix-vm-k8s:1.34`) booted, got an IP on the internal SDN, and responded,
with not a single outbound route available in the process that created it.

**Side finding, unrelated to sovereignty, recorded for method discipline**:
the first attempt failed with `Fatal error: ... path must be
shorter than SUN_LEN` — the test `DELONIX_ROOT` path (under a deep
scratchpad directory) exceeded the kernel's length limit for a Cloud
Hypervisor Unix socket. Fixed by using a short `DELONIX_ROOT`
(`/tmp/sov`); not a defect in the engine or the network.

**Environment cleaned up after the test**: `delete vm sov-test`, confirmed no
leftover `cloud-hypervisor` process, test `DELONIX_ROOT` removed.

### What was NOT proven on this pass, and why

`kubeadm init` itself did not run inside the isolated VM. It would require
SSH into it (one more network hop inside the isolation being
built) and, more relevantly, a second/third node for a real
control-plane bootstrap — on a shared host with load from other sessions,
that resource expense isn't justified outside a session dedicated to the
exercise.

It stays as `AGENTS.md` itself already documents (section "Pre-seeding
`kubeadm`'s images"): a `vm image build --offline` fetches Kubernetes' 7
core images (apiserver, controller-manager, scheduler, etcd, coredns, pause)
on the build HOST and injects them into the `ImageStore` embedded in the
qcow2 — **if** the published golden image was built through that path. The
image used in this test (`delonix-vm-k8s:1.34`) was obtained by `vm pull`
from `ghcr.io/angolardevops/delonix-vm-k8s:1.34`, and `pull` **does not
recover metadata** saying whether the source build was `--offline` (a gap
already documented in `AGENTS.md`) — so this line stays **conditional**, not
confirmed.

## The count of foreign dependencies (code, not estimate)

Measured by direct source-code reading (`grep` of hardcoded domains/registries
in `crates/`), classified by the same discipline as
`ngolacloud-doutrina` (build-time vs run-time; blocking vs degradable vs
convenience):

| Path | Domain(s) | Class | Blocking? |
|---|---|---|---|
| **Boot** (`container run`/`exec`/`stop` of an already-local image) | none | — | **No** — confirmed above |
| **Boot** (`container run` without the image cached) | `registry-1.docker.io` (default when the client doesn't qualify the registry) | run-time, **explicit client action** | Only if the client asks for a new image — not an automatic "phone home" by the engine |
| **First `vm pull`/`cluster kubeadm --vm-image` with no cache** | `ghcr.io/angolardevops/delonix-vm-*` | run-time, first use | **Yes**, for a node creating its FIRST cluster/VM; local after that |
| **`kubeadm init`** | `registry.k8s.io` | run-time — conditional on the golden image's `--offline build` | Conditional on the build mode; not guaranteed by default (see section above) |
| **Golden image build** (`vm image build`) | `cloud-images.ubuntu.com`, `cloud.debian.org`, `dl.rockylinux.org`, `download.fedoraproject.org`, `pkgs.k8s.io`, `archive.ubuntu.com` | **build-time**, not run-time | No — the doctrine already classifies build-time as outside the critical count |
| **Cloud Hypervisor firmware** (`install.sh`) | `github.com/cloud-hypervisor/edk2` (releases) | one-time host install | No — per-host, not per-VM |

**What this does not cover**: Rust's own build dependencies (`crates.io`
in `Cargo.lock`) — the doctrine already classifies these as build-time and
outside the critical count (confirmed in the platform's 2026-08-18 sweep: 68
occurrences of `crates.io`, all build). Not recounted here as it's already
resolved elsewhere.

## Method note

The binary used (`target/release/delonix`, commit `6a9e1fe5b`) is
slightly behind the `HEAD` used to design this test (`e66ae8da`
local / `adae8343` on `origin/main` at the time of writing this document) —
both are documentation commits (`AGENTS.md`), with no engine code change
between them. A full rebuild was not done since this is a test of a
structural property (whether the network is touched or not), not the
validation of a specific feature within that commit window.

## Links

- The eight properties and the two full exercises:
  `ngolacloud-doutrina/references/soberania.md` (the platform's workspace).
- What remains for "boot" to close at 100%: exercising `kubeadm init` inside
  an isolated VM, and confirming (or denying) that an `--offline` golden image
  actually produces a control plane without touching `registry.k8s.io`.
- Sovereignty as a positioning axis against AWS/GCP:
  `delonix-engine/SKILL.md` §2.
