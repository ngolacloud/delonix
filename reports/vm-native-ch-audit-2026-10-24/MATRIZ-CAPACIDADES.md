# Compatibility and capability matrix — native Cloud Hypervisor backend

**Reference** configuration (the only one validated; promise nothing beyond it).
Raw discovery dump: `evidence/ch-capabilities-raw.txt`.
Host facts: `evidence/reference-config.txt`.

## Pinned reference configuration

| Axis | Value |
|---|---|
| CPU / arch | Intel Core i9-13900H, x86_64, flag `vmx`, nested=`Y` |
| Host kernel | 7.0.0-38-generic |
| KVM | `/dev/kvm` present, access via group `kvm` (rootless) |
| VMM | Cloud Hypervisor **v52.0** (`/usr/local/bin/cloud-hypervisor`) |
| Firmware | EDK2 `CLOUDHV.fd` (4 MiB, `/usr/local/share/delonix/`) |
| Delonix | 5.1.1 / commit `f49d3570c` |
| Guest | Ubuntu 24.04 (`k8s-golden`), kernel 6.8.0-134, cloud-init |
| Image format | qcow2 (overlay with backing file, `image_type=qcow2`) |
| Network | rootless SDN: infra netns + `slirp4netns`, tap on bridge `ingress` |
| Storage | per-VM qcow2 overlay + NoCloud seed ISO |
| Mode | **rootless** (no root for the proven path) |

> Nothing outside this matrix is promised. Non-x86 architectures, other CH
> versions, direct-kernel boot, other formats (raw/vmdk/…) and hosts without
> libvirt installed were **not** validated in this session.

## vm-compute capabilities of the CH backend (from discovery) and harness proof

State legend = the state **declared today** by the catalogue. "Harness" = what
`harness-ch-lifecycle.sh` proved live in this session (28/28).

| Capability | State today | Harness proved | Proposed action |
|---|---|---|---|
| vm.create | supported | ✔ A | — |
| vm.start | supported | ✔ A/B | — |
| vm.stop | supported | ✔ B | — |
| vm.resume-same-identity | supported | ✔ B (stable machine-id) | — |
| vm.disk.resize | supported | (covered by provider tests) | — |
| vm.namespace-isolation | supported | (existing chaos) | — |
| **vm.destroy** | partial | ✔ H (zero orphans) | → graduate w/ `evidence:` |
| **vm.restart** | partial | ✔ C (uptime resets) | → graduate |
| **vm.pause** | partial | ✔ D | → graduate |
| **vm.resume** | partial | ✔ D | → graduate |
| **vm.resize.cold** | partial | ✔ E (guest sees 3 vCPUs) | → graduate |
| **vm.cloud-init** | partial ("never logs into the guest") | ✔ A (hostname+key, real login) | → graduate (login proof) |
| vm.hotplug.cpu.add | partial | not tested (needs `--vcpus-max`) | keep partial; test w/ headroom |
| vm.hotplug.memory.add | partial | not tested (needs `--memory-max`) | keep partial |
| vm.cpu.pinning | partial | not booted | keep partial |
| vm.memory.hugepages | partial | unit only | keep partial |
| vm.device.pci-passthrough | partial | no IOMMU on host | keep partial |
| vm.antispoof | partial | `dlxspoof` table exists; forgery not tested | keep partial |
| vm.clone / vm.template | not-implemented | — | do not promise |
| vm.hotplug.{cpu.remove,memory.remove,disk.*,nic.*} | not-implemented | — | do not promise |
| vm.disks.extra / vm.nics.extra | unsupported-by-provider | — | correct refusal (1 disk+1 tap) |
| vm.device.tpm / vm.cpu.model / vm.raw-definition | unsupported-by-provider | — | correct refusal |
| vm.restart-policy.native | unsupported-by-provider (engine supervisor handles it) | — | correct refusal |

**vm-compute count:** 6 supported · 12 partial · 8 not-implemented · 6
unsupported-by-provider. The harness gives live evidence for **6** of the 12
`partial`; graduating them (audit queue #2) moves backend maturity forward
without lying.

## Snapshot (Protection domain)

CH snapshots **only a stopped VM** (the running VMM holds the disk
exclusively; CH has no live-disk snapshot API) — documented in
`vm snapshot --help`. Harness: create+ls+rm of a stopped VM **passed**.
Snapshot of a running VM / with memory: **not supported** by this backend
(that is a libvirt feature) — refuses with a message, not silence.

## What NOT to promise (rejected with error, absent from discovery as "supported")

live VM migration, full snapshot (memory) in CH, GPU passthrough without
IOMMU, disk/nic hotplug, extra disks/nics, vTPM, CPU-model selection,
raw definition. All appear as `not-implemented` or `unsupported-by-provider`
in discovery — never as `supported`.
