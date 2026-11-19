# Honest ceiling of the capability matrix — why 270/270 is not achievable

**Date:** 2026-10-27 · **Measured state:** 180/270 cells proven (66.7 %, level N2)

> **Bucket B +2 (2026-10-27): `host.capacity` CH + libvirt graduated.** The
> semantics were already fixed by Proxmox (`host.capacity` supported = the node
> reports CPU/memory/disk); for CH and libvirt the "node" is the LOCAL HOST where
> the VMs run. Real CODE: `delonix_node::host::capacity(disk_path) -> HostCapacity`
> (cpus+model from /proc/cpuinfo, mem/swap from /proc/meminfo, disk via statvfs;
> discipline "never a silent zero" — what is not measured is named in
> `unmeasured`). Each provider exposes a thin `host_capacity(vmdir)` + a test in
> ITS OWN crate (`host_capacity_reports_the_local_host`) that reads the real host
> and proves the invariants (cpus≥1, mem_total>0, used≤total for mem/swap/disk).
> Arms cite `test:…`. 7 unit tests green (4 in delonix-node + 1 per provider +
> parsers). Gates/ratchet green.

> **Bucket B started (2026-10-27):** I graduated `metrics.prometheus` (CH) —
> discovered that the per-VM-domain series (`delonix_workload_{cpu_usage_seconds,
> memory_current_bytes}{kind="vm"}`) ALREADY existed (read from the VMM's cgroup
> leaf in `collect_workload_series`); the "vms_running/total only" detail was
> obsolete. Proof = e2e section `metrics: /metrics exports per-VM cpu and memory
> series for a Cloud Hypervisor VM` (2/2 live: mem=104 MB, cpu=0.51s for
> `kind="vm"`). Real CODE work: I implemented `vm_tap_net_bytes` (delonix-sdn:
> reads the VM tap's rx/tx in the holder's netns, swap host→guest) + wiring in
> `dashstats::collect_workload_series` + a parser unit test — but
> `metrics.network.per-workload` (CH) STAYS `partial`: I could not get it green
> live because `ingress/holder.pid` would not resolve in this host state (the same
> thing that left container-net absent in the isolated run). Honest: code
> delivered and unit-tested, capability not asserted without end-to-end proof.
> `host.capacity` and libvirt `metrics.prometheus` still to do (libvirt has no qemu
> pid in the registry → cgroup_missing, seen live on `pve92`).
> · **Source:** `delonix provider matrix` + `delonix provider describe <p> -o json`
> (binary from `bins/delonix-runtime-bin`, `develop` tree).

> **Bucket A — CLOSED (2026-10-27):** **7 cells** graduated, all proven live:
> - **libvirt** `vm.destroy` + `vm.static-ip` + `vm.ip.observed` — one e2e battery
>   (`vm: a libvirt VM boots at its reserved static IP…`, 7/7 live, one VM);
> - **libvirt** `vm.cpu.pinning` — `vm: a libvirt VM's vCPUs are pinned to the
>   requested host cores, live` (3/3 live: `virsh vcpuinfo` → affinity `yy---…` on
>   the 2 vCPUs, only cores 0-1 of 20);
> - **cloud-hypervisor** `access.transport-verified` — `vm: the Cloud Hypervisor
>   control transport is a per-VM unix socket…` (4/4 live: unix socket, engine uid,
>   mode 700, no TCP);
> - **linux** `access.credential-in-vault` — `access: a --secret is injected from
>   the vault at spawn but never persisted…` (6/6 live: value present at runtime, 0
>   occurrences of the plaintext in the registry/inspect, secret inspect redacts).
>
> **Reclassifications from the live triage** (the 1st draft was optimistic):
> `vm.restart-policy.native` → **B** (crash→restart requires a nonexistent
> `<panic>`/pvpanic device); `vm.network.bridge` → **C** (enslaving a NIC to a
> bridge = root); **`access.transport-verified` (linux) → stays `partial`** —
> FINDING: the old claim "no TCP is ever bound" is **false**; the `delonix-cri`
> process binds a **loopback** TCP `127.0.0.1` (the CRI streaming server for
> exec/attach/port-forward). It is local-only (never 0.0.0.0) and the control
> sockets are unix+uid+mode 600, but deciding whether the loopback streamer counts
> as "supported" is the owner's call — I did not flip it; I corrected the detail to
> reflect reality. Real Bucket A = 7 done + 1 that stays honestly partial.

> The matrix is **proven, not asserted**. The gates
> `cmd::provider::tests::every_supported_capability_cites_evidence_that_exists` and
> `the_published_matrix_is_the_generated_one` refuse any `supported` whose evidence
> (`check:`/`e2e:`/`chaos:`/`test:`/`live:`) does not actually exist in the
> repository. Flipping an arm without real proof **fails the build** — it is not an
> option.

## Executive summary

Of the ~96–99 cells still unproven, **23 must NEVER change** (by design, security,
or honest provenance). So the **absolute honest maximum is 270 − 23 = `247/270`
(91.5 %)**, and only if *all* the engineering and infra ones get done. **270/270 is
categorically impossible** without lying in the evidence.

Breakdown of the unproven (curated cell-by-cell classification at the end):

| Bucket | Nº | What they are | Achievable? |
|---|---:|---|---|
| **A — Only-proof-missing** | 9 | The feature already exists and works on a usable provider on this host; only the cited battery is missing (this was the case of `vm.migration.cold`). | **Yes, already** — no new code |
| **B — Engineering** | 11 | Incomplete feature; requires real new code (export metrics, dataplane probes, label selectors). | Yes, but it is code work |
| **C — Infra** | 53 | Need absent infra: live PVE (37), live OPNsense (12), root + PCI device, 2nd node, swtpm, image with guest-agent. | Only after standing up the infra |
| **D — Principle** | 23 | `not-implemented` by design/security, or `partial` by honest provenance. | **No — they must stay** |

### The ceiling arithmetic

```
  270  applicable cells
− 171  already proven
───────
   99  to prove (≈96 classified here)

Honest ceiling  = 270 − 23 (bucket D)            = 247 / 270   (91.5 %)   ← theoretical maximum
Achievable here = 171 + 9 (A) + ~9 (B viable here) ≈ 189 / 270 (~70 %)   ← without new infra
Gap to the ceiling: 53 (C) + 2 (heavy B) = infra (PVE/OPNsense/2nd node/root) + engineering
```

Even by standing up **all** the infra (a multi-node PVE cluster, a live OPNsense, a
2nd delonix node, a spare PCI device with root, the provisioned swtpm, a guest
image with qemu-guest-agent) and doing **all** the engineering, we stop at the 23
of bucket D. The honest number to chase is **247**, not 270 — and the cheapest path
close to it is A now, then the Proxmox/OPNsense infra (which alone is worth 49
cells).

## Recommendation

1. **Now (no infra):** close bucket A (9 cells) — live batteries on libvirt
   (`vm.destroy`, `vm.network.bridge`, `vm.network.static-ip`, `vm.ip.observed`,
   `vm.restart-policy.native`, `vm.cpu.pinning`) and positive transport proofs
   (`access.transport-verified` CH/linux, `access.credential-in-vault` linux). Same
   pattern as `vm.migration.cold`: one `e2e.sh` section + `e2e:<title>` on the arm.
   → brings it to ~180/270.
2. **Next (CORRECTED 2026-10-27, measured live):** configuring the Proxmox node
   `pve` is NOT the easy lot this report estimated. DONE: `pve92` appliance booted
   (node `pve` @ 192.168.30.117), provider configured (AVAILABLE=yes), and the
   `live.rs` suite validated end-to-end (`cria_arranca_e_destroi_contra_um_no_real`
   PASSES against the PVE, 47s). FINDING: the **40 supported were already proven**;
   configuring the node re-confirms them but **unblocks ~0 new cells on its own**.
   The 12 partial / 9 not-impl of compute and 7/9 of network each need, cell by
   cell: **measurement in the dataplane/guest** (firewall "read back live, not
   measured on a packet"; cpu/mem hotplug "mock-tested, never run against a guest" →
   needs a guest with qemu-guest-agent), **engineering** (`async-operations`
   without a job handle; `vm.template` without an engine verb; `storage.pools`
   without listing; wiring vlan/vxlan/snat/ipv6/net.observe), or **multi-node**
   (migration/HA). `provider.availability` and `access.*` are honest provenance
   (bucket D). Conclusion: Proxmox is a real B+C campaign, not flips; OPNsense the
   same (needs the live appliance + dataplane). The "~230/270" estimate was
   optimistic — withdrawn.
3. **Engineering (bucket B):** per-VM/per-workload metrics exporters +
   `host.capacity`/`host.health`, `net.observe` with comparison to the registry,
   `net.verify.dataplane`, `firewall.workload-peer` label selectors.
4. **Never:** bucket D. Changing them empties the meaning of the word `supported`.

---

## Appendix — cell-by-cell classification

### Bucket A — Only-proof-missing (achievable ON THIS host, no new feature) — 9 cells

| Provider | Capability | State | What graduation would require |
|---|---|---|---|
| cloud-hypervisor | `access.transport-verified` | partial | the transport IS verified (unix socket/SO_PEERCRED); the positive battery is missing |
| libvirt | `vm.cpu.pinning` | partial | works on qemu:///system (present here); it only falls on session — the live battery is missing |
| libvirt | `vm.destroy` | partial | works on libvirt here; only the cited live battery is missing |
| libvirt | `vm.ip.observed` | partial | works on libvirt here; only the cited live battery is missing |
| libvirt | `vm.network.bridge` | partial | works on libvirt here; only the cited live battery is missing |
| libvirt | `vm.network.static-ip` | partial | works on libvirt here; only the cited live battery is missing |
| libvirt | `vm.restart-policy.native` | partial | works on libvirt here; only the cited live battery is missing |
| linux | `access.credential-in-vault` | partial | works (resolved from the vault, never persisted); the cited battery is missing |
| linux | `access.transport-verified` | partial | the transport IS verified (unix socket/SO_PEERCRED); the positive battery is missing |

### Bucket B — Engineering (new code to complete the feature) — 11 cells

| Provider | Capability | State | What graduation would require |
|---|---|---|---|
| cloud-hypervisor | `host.capacity` | not-implemented | export the missing metrics / build the capacity report |
| cloud-hypervisor | `metrics.network.per-workload` | partial | export the missing metrics / build the capacity report |
| cloud-hypervisor | `metrics.prometheus` | partial | export the missing metrics / build the capacity report |
| libvirt | `host.capacity` | not-implemented | export the missing metrics / build the capacity report |
| libvirt | `metrics.network.per-workload` | not-implemented | export the missing metrics / build the capacity report |
| libvirt | `metrics.prometheus` | partial | export the missing metrics / build the capacity report |
| libvirt | `vm.snapshot.memory` | partial | recovery of the in-guest memory CONTENT is not observed |
| linux | `firewall.workload-peer` | partial | label selectors (ADR-0024) to be implemented |
| linux | `net.ipam.dhcp` | partial | the CH tap address stays outside the IPAM ledger (audit 62 §3) |
| linux | `net.observe` | partial | read-back exists; comparison with the registry is missing (ADR-0059 F4) |
| linux | `net.verify.dataplane` | not-implemented | build an active dataplane probe + assertion |

### Bucket C — Infra (root / device / 2nd node / live PVE / OPNsense) — 53 cells

| Provider | Capability | State | What graduation would require |
|---|---|---|---|
| cloud-hypervisor | `vm.device.pci-passthrough` | partial | root + spare PCI device in an isolated IOMMU group |
| libvirt | `vm.backup.quiesced` | partial | offline guest image with qemu-guest-agent |
| libvirt | `vm.device.pci-passthrough` | partial | root + spare PCI device in an isolated IOMMU group |
| libvirt | `vm.device.tpm` | partial | swtpm running as the qemu identity + AppArmor permission (host provision) |
| linux | `container.gpu.cdi` | partial | live run needs a loaded NVIDIA driver |
| linux | `net.overlay.encrypted` | partial | inter-node routing needs a 2nd node |
| linux | `net.overlay.vxlan` | partial | inter-node routing needs a 2nd node |
| linux | `net.vlan` | partial | root only (802.1Q on a physical NIC) |
| opnsense | `firewall.icmp-type` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.apply.rollback` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.dns.authoritative` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.dns.records` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.gateway.multi-wan` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.gateway.update-in-place` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.gateway.vpn` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.ipam.dhcp` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.ipam.reservation` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.nat.npt` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.nat.one-to-one` | not-implemented | needs a live OPNsense appliance to drive and read back |
| opnsense | `net.verify.dataplane` | not-implemented | needs a live OPNsense appliance to drive and read back |
| proxmox | `access.credential-in-vault` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `access.transport-verified` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `firewall.default-deny` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `firewall.egress-policy` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `firewall.icmp-type` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `firewall.logging` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `firewall.per-workload` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `firewall.source-filtering` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `host.health` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `metrics.network.per-workload` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `metrics.prometheus` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.bridge` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.ipv6` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.nat.snat` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.observe` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.overlay.vxlan` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.rate-limit` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.routes` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.segment.remote` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.verify.dataplane` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `net.vlan` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `provider.async-operations` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `storage.pools` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `system-container.backup` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `system-container.firewall` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `system-container.network.bridge` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `system-container.oci-image` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `vm.hotplug.cpu.add` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `vm.hotplug.disk.remove` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `vm.hotplug.memory.add` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `vm.hotplug.nic.remove` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `vm.network.static-ip` | not-implemented | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |
| proxmox | `vm.template` | partial | Proxmox node NotConfigured — needs a live PVE (several need a multi-node cluster) |

### Bucket D — Principle: MUST stay unproven (design, security, provenance) — 23 cells

| Provider | Capability | State | What graduation would require |
|---|---|---|---|
| cloud-hypervisor | `provider.async-operations` | not-implemented | synchronous backend; nothing asynchronous to report |
| cloud-hypervisor | `provider.availability` | partial | honest provenance: available() is cheap; the deep probe is provider ls |
| cloud-hypervisor | `provider.events` | not-implemented | this backend has no event stream in the contract |
| cloud-hypervisor | `storage.pools` | not-implemented | it is a VM backend, not a storage-pool manager |
| cloud-hypervisor | `vm.device.tpm` | not-implemented | CH has no TPM device |
| cloud-hypervisor | `vm.hotplug.cpu.remove` | not-implemented | deliberately not wired (unsafe to force; goes via cold resize) |
| cloud-hypervisor | `vm.hotplug.memory.remove` | not-implemented | deliberately not wired (unsafe to force; goes via cold resize) |
| cloud-hypervisor | `vm.ip.observed` | partial | honest provenance: the IP is COMPUTED from the MAC, never observed |
| libvirt | `access.transport-verified` | partial | honest: local URIs only; a remote libvirt URI is never built |
| libvirt | `provider.async-operations` | not-implemented | synchronous backend; nothing asynchronous to report |
| libvirt | `provider.availability` | partial | honest provenance: available() is cheap; the deep probe is provider ls |
| libvirt | `provider.events` | not-implemented | this backend has no event stream in the contract |
| libvirt | `storage.pools` | not-implemented | it is a VM backend, not a storage-pool manager |
| libvirt | `vm.hotplug.cpu.remove` | not-implemented | deliberately not wired (unsafe to force; goes via cold resize) |
| libvirt | `vm.hotplug.memory.remove` | not-implemented | deliberately not wired (unsafe to force; goes via cold resize) |
| libvirt | `vm.migration.live` | not-implemented | out of scope (NBD live migration requires convergence the engine does not have) |
| libvirt | `vm.raw-definition` | partial | deliberately local-only escape hatch; never reachable via the node contract |
| linux | `provider.async-operations` | not-implemented | synchronous backend; nothing asynchronous to report |
| linux | `provider.availability` | partial | honest provenance: available() is cheap; the deep probe is provider ls |
| proxmox | `provider.availability` | partial | honest provenance: available() is cheap; the deep probe is provider ls |
| proxmox | `provider.events` | not-implemented | this backend has no event stream in the contract |
| proxmox | `vm.hotplug.cpu.remove` | not-implemented | deliberately not wired (unsafe to force; goes via cold resize) |
| proxmox | `vm.hotplug.memory.remove` | not-implemented | deliberately not wired (unsafe to force; goes via cold resize) |
