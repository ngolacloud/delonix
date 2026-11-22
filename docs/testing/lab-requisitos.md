# Lab requirements — what is missing, by leaves it unlocks

> **How to read this.** Each line is a piece of infrastructure that **does not exist on
> this host**, and the class-`E` leaves it unlocks. Ordered by the number of leaves,
> not by difficulty: the decision to invest is the owner's, and what it needs is
> the return, not my opinion on the effort.
>
> **Measured** on 2026-10-18, commit `a24731e3`, with `scripts/lab_preflight.sh`
> (**rc=1**, two pieces missing) and direct probing of each tool.

## What this host ALREADY has

`lab_preflight.sh` passes 25 of 27 checks. Everything is here for the hard
part: unprivileged user namespaces, `/etc/subuid`+`/etc/subgid`
(`walter:100000:65536`), `br_netfilter` with `bridge-nf-call-iptables=1`,
`/dev/kvm` with read-write access, `virsh`, `qemu-img`, `qemu-system-x86_64`,
`cloud-hypervisor`, `cloud-localds`, `virt-customize`, the EDK2 `CLOUDHV.fd`,
`qemu:///system` reachable, `slirp4netns`, `nft`, `ip`, `conntrack`,
`newuidmap`, `protoc`, `wg`, and 138 GiB free.

Also present, and this matters: `critest` and `crictl` **v1.36.0** — exactly
the version of the two recorded baselines —, `kubeadm`, `kubectl`, and `kind`.

So the local lab covers containers, rootless networking, and **both** VM
backends. What is missing is everything external to the node, plus two
cgroup pieces.

---

## The table, by return

| # | missing piece | leaves it unlocks | who can put it in place |
|---|---|---:|---|
| 0 | **`socat`** (one package) | **5** CLI leaves, **with no line of code** | owner (`apt install socat`) |
| 1 | **Local OCI registry** | **5** CLI leaves | me, if you authorize it |
| 2 | **`docker` + `podman` in a lab VM** | the **whole of Phase 5** | owner (installation) |
| 3 | **Proxmox VE node** (API + credentials) | **~10** | owner |
| 4 | **Delegation of `cpuset` and `io`** | the cpuset/io limits | owner (needs root) |
| 5 | **A bootable OPNsense appliance** | **4** Kind cells plus `live.rs` | owner |
| 6 | **A bootable VM image in the isolated store** | **2** CLI leaves | me, if you authorize it |
| 7 | **`buf` + `protoc-gen-openapi`** | unlocks `contract_gate.py` | owner (installation) |
| 8 | **`kindest/node` image** | **1** leaf | me, if you authorize it |
| 9 | **Golden image + real VMs** | **1** leaf | me, if you authorize it |
| 10 | **Real LVM VG / zpool / btrfs filesystem** | **33** provider operations | owner |
| 11 | **TrueNAS appliance** | TrueNAS's `live.rs` (5 ops) | owner |
| 12 | **PowerDNS server + zone** | PowerDNS's `live.rs` (2 ops) | owner |
| 13 | **NAS with NFS** | share volumes | owner |
| 14 | **`cosign` / `syft`** | interoperability only | owner (installation) |
| 15 | **2nd Proxmox node + shared storage** | **4** live cases that have never passed | owner |

---

## 0. `socat` — **RESOLVED** on 2026-10-20

It was never a coverage gap: the battery **already had** the five
`net netns exec/detach/firewall/publish/unpublish` checks in `scripts/e2e.sh`,
and they judge things **on the wire** — two netns, A answers a token over
`socat`, B consumes it with `nc`. The block was skipping on a `command -v
socat`, and `socat` was not on the host; `nc` was.

The block landed in `5c62db13` (2026-10-11), **before** the trace in
`e5be0903` (2026-10-16). So the five leaves counted zero because the block
skipped, not because there were no checks.

Measured on 2026-10-20, with `socat` at `/usr/bin/socat`: the block runs, the
five checks PASS asserting effect on the wire, and the five leaves are now
**A**. The prediction written here was that the CLI would go from 254 to
**259** with no line of code, and that's exactly what happened — the current
trace's **263** are those 259 plus the four wave-2 leaves (`net l4guard
set/clear`, `system boot enable/disable`).

The alternative that was considered — rewriting the fixture's server with the
`nc` that already exists — was left undone, and rightly so: it would have
touched a check that works to work around a host gap, and would have swapped
a `socat` with `fork,reuseaddr` for an `nc -k` with different semantics.

## 1. Local OCI registry — 5 leaves, and it's the best return in the set

Unlocks `image push`, `image sign`, `vm image pull`, `vm image push`, and
`vm image ls-remote` — five of the nineteen leaves that no run exercises.

Neither `registry` nor `skopeo` is on the host. A local, TLS-less registry on
`127.0.0.1` is enough, stood up and torn down by the battery itself; it
doesn't need to be a permanent service or to have real credentials.

**And `image sign` does NOT need `cosign`**: the engine has its own
ECDSA-P256, with the key generated on first use at
`<DELONIX_ROOT>/keys/image-signing.key`. `cosign` would only serve to prove
signature interoperability (line 14).

## 2. `docker` + `podman` in a VM — the whole of Phase 5

Neither exists here, and installing them on **this** host is out of scope: it
is the development machine, and `measured-comparison.md` itself recorded on
2026-08-13 that it is not touched for exactly that reason. The comparison ran
in a `libvirt` VM created by the engine itself, and that's the path to
repeat.

**There is a second blocker, and it is independent.** `scripts/bench_gate.py`
is **per-machine** by design, and the only entry in `bench_baseline.json` is
from an **AMD Ryzen 9 8940HX, 32 threads** (2026-09-23, delonix 4.3.0). This
host is an **Intel i9-13900H, 20 threads** — an unknown machine, and the gate
**refuses to judge** (exits 3, never a silent green). So the bench must run
in the VM and record its own entry, or Phase 5 has no verdict.

## 3. Proxmox VE node — ~10 leaves

With credentials (`DELONIX_PROXMOX_TEST_URL` plus a token or username/
password) this unlocks: the 4 `NetworkZone` Kind cells (`apply`, `plan`,
`diff`, `drift`), `SystemContainer`'s `apply` (today only refusals), and the
5 routes the Proxmox matrix marks `supported+untested`.

## 4. Delegated `cpuset` and `io` — needs root

The only two preflight failures:

```
MISS  cgroup controller cpuset delegated — missing from
      /sys/fs/cgroup/user.slice/user-1000.slice/user@1000.service/cgroup.subtree_control
MISS  cgroup controller io delegated — idem
```

```bash
sudo mkdir -p /etc/systemd/system/user@.service.d
printf '[Service]\nDelegate=cpu cpuset io memory pids\n' \
  | sudo tee /etc/systemd/system/user@.service.d/delegate.conf
sudo systemctl daemon-reload && sudo systemctl restart user@1000.service
```

`daemon-reload` alone **is not enough**: a running `user@` does not change
controllers. This is measured in `AGENTS.md`, and `docs/lab.md` asks for all
five.

## 5. A bootable OPNsense appliance — 4 Kind cells

ADR-0071 already says it in writing: *«Not proven live: the lab OPNsense VM
does not boot»*. Until it boots, `NetworkGateway` stays with `apply`, `plan`,
`diff`, and `drift` at `E`, and OPNsense's `live.rs` (4 cases, 15 operations)
does not run.

## 6. A bootable VM image in the isolated store — 2 leaves

`vm pause` and `vm unpause` are real in all **three** backends
(cloud-hypervisor `:847`/`:851`, libvirt `:3254`/`:3268`, proxmox
`:6488`/`:6497`) and refuse a VM that is not running. All that's missing is a
running VM: the 2026-10-16 trace says «no VM images in the isolated store».

`vm image import` **is already exercised** by the battery, so the entry path
works — only the file is missing. With `virt-customize` present, the
alternative is to build it (line 9).

## 7. `buf` + `protoc-gen-openapi` — unlocks a gate that fails today

`python3 scripts/contract_gate.py` returns **rc=1** here:

```
FAIL  buf is not on PATH — the gate cannot check what it guards
FAIL  protoc-gen-openapi is not on PATH — the gate cannot check what it guards
```

**The behavior is correct** — fail closed, never pretend to measure. But it
means the node's contract (format, lint, breaking changes, HTTP mappings,
OpenAPI) **is not verifiable on this host**, and Phases 4/5 touch it.

## 10. Real LVM VG / zpool / btrfs — 33 operations with zero coverage

The three crates (`delonix-provider-lvm`, `-zfs`, `-btrfs`) **have no
`tests/`**. Each one's ~11 operations (`probe`, `required`, `allocate`,
`release`, `usage`, `create_pool`, `created_here`, `contents`, `destroy_pool`)
only have unit tests with a fake `Run` — they have never run against a real
VG, zpool, or btrfs filesystem.

And they are shipped and **linked** (`delonix-storage-helper/src/main.rs:120-133`,
`cmd/storage_pool.rs:125-134`) while `capability-matrix.md:196-197` declares
`storage.lvm-thin` and `storage.zfs-btrfs` **`not-implemented`**: the matrix
and the code disagree, and neither one is proven.

A loopback file (`losetup`) is enough for LVM and btrfs without dedicated
hardware; ZFS needs the kernel module.

## 15. 2nd Proxmox node + shared storage — 4 cases that have never passed

`delonix-proxmox/tests/live.rs:33-50` states it plainly: nine cases have
never passed against a node. Four need a second node and shared storage — the
three VM `move` cases and the system container's `move_to`. So
`VmBackend::move_to_node` and `SystemContainerProvider::move_to` are
implemented and **not proven**.

The other five need: a guest with a guest agent (IP, vCPUs/memory), a DNS
zone (`DnsProvider::prepare_zone`/`observe`), and a gateway freed up by hand
(the repair).

---

## A note on the label the mission asks for

The brief requires that «test resources carry the label
`delonix-test-run=<uuid>`». **It does not exist**: `grep -rn
'delonix-test-run'` across the repository returns **0 occurrences**, across
all nine providers. It is not a lab requirement — it is code work, and it
belongs in the Phase 4 plan, not here. What exists today are names derived
from the PID (TrueNAS, PowerDNS, libvirt), a fixed name (`dlxhp-live`, Cloud
Hypervisor), per-case names (Proxmox), and OPNsense's `fresh_mark()` — which
is a **production** ownership mark (ADR-0059 D1.5), not a test label.
