# Capability conformance campaign — baseline v5.1.1

Objective: handle individually every partial, not-implemented, unsupported, external-component-dependent or host-unavailable entry in the delonix-runtime capability matrix, implementing real support (backend + public contracts + tests) wherever technically applicable — without merely changing labels and without artificially moving counters.

## Resumable checkpoint

| field | value |
|---|---|
| Baseline | `v5.1.1` = `a0da90f50abccb8d80a14136263d549dce257837` |
| Working branch | `develop` (linear descendant of the baseline, +258/−0) |
| Catalog | 1.4.0 · Engine 5.1.1 |
| Source of truth | `matrix-structured.json` (515 entries, lossless) |
| Current sprint | **Sprint 1 in progress** — §4 audit + A1–A3 reclassification + 1st real promotion (OOM) to supported |
| Next | Sprint 1 — remaining risks: namespaces shared by process identity, snapshot restore with data; then candidates B (CH bridge/ip.observed, net.apply.rollback) |

## Before/after metrics (measured @ develop)

| state | report | post-reconciliation (0894caf9) | now (b6aa53ff) |
|---|---|---|---|
| supported | 103 | 112 | **164** |
| partial | 93 | 86 | 52 |
| not-implemented | 74 | 72 | 51 |
| unsupported-by-provider | 230 | 230 | 233 |
| requires-external-component | 12 | 12 | 12 |
| unavailable-on-host | 3 | 3 | 3 |
| proved/applicable (N2) | — | 112/273 | **164/270 (60.7%)** |

Hardware front (`c29dd401`): this host HAS the hardware (IOMMU 17 groups+vfio, swtpm+/dev/tpm0, hugepages pool 4096×2M, RTX 2000 Ada+nvidia-ctk). **`vm.memory.hugepages` libvirt+CH →supported** (HugePages_Free drops by exactly the guest's RAM, proved live). But **tpm/pci-passthrough/gpu-cdi stay honestly partial — the blocker shifted from "no hardware" to "needs ROOT"**: modprobe nvidia (gpu), swtpm group membership (tpm), vfio-bind (pci) — all require root that the sandbox denies; CH tpm reclassified unsupported→not-impl (CH v52 has `--tpm` but the swtpm lifecycle is not wired up).

Pair `4e769bf0`: linux network `net.nat.snat` (SNAT observed on the wire: source=uplink, 0 with an internal IP) + `net.l7-proxy` (HTTP forwarded by the proxy, 404 without a route) →supported. Proxmox network §11: **0 promotions** (all proof requires a GLOBAL datacenter switch — firewall OFF, SDN is a global `PUT /cluster/sdn`; §11 forbids it), **4 honest reclassifications** (macvlan/encrypted-overlay/ipam/dns of the engine → unsupported-by-provider; vlan/vxlan/routes/rate-limit stay not-impl = adapter gap) → applicable 273→269.

Parallel trio (b/c/d, `9bc5e13e`): **network §10** — 6 promotions (net.static-ip, net.dns, firewall.default-deny/source-filtering/egress-policy, net.packet-capture) proved with allowed+denied traffic and nft counters; net.verify.dataplane stays honestly not-impl (ADR-0059 feature). **storage §12** — no root in this session → root-gated e2e harness ready + honest details; lvm/zfs/btrfs stay partial (privilege missing, not implementation). **VM migration §7** — 2nd node brought up (userspace sshd, separate DELONIX_ROOT), cold A→B proved cross-host + `$HOME` ssh bug fixed; live stays honestly not-impl (needs 2 libvirtd + copy-storage-all).

Integrity note: along the way `vm.snapshot-memory` (libvirt) was deliberately DEMOTED supported→partial (RAM not observable without a guest with an OS) — lowering the metric for honesty is progress (§5/§17); afterwards `metrics.prometheus` rose with real proof.

Progress separation (§17): 1 real promotion to supported (OOM, with a live test + evidence), 3 cause-corrected reclassifications (A1–A3), 2 gaps completed in classification (lvm/zfs-btrfs: backend already existed, now honestly partial). No false promotion — every +1 in supported has verified evidence.

## Campaign commit history

- `0894caf9` docs — Sprint 0 reconciliation (baseline, 515-entry matrix, diff explained, plan).
- `e082a275` docs — Sprint 1 verified reclassification proposal (§4 audit, 2 specialists).
- `8cd5c63b` fix — applies A1–A3: libvirt `vm.migration.live` unsupported→not-implemented; linux `storage.lvm-thin`/`storage.zfs-btrfs` not-implemented→partial.
- `34f7afd5` docs — marks A1–A3 as applied in the checkpoint.
- `b6aa53ff` feat — **`container.oom-detection` partial→supported (§9)**: `scen_oom` strengthened to assert `crash_reason=oom_killed` (distinguishes OOM from a non-OOM termination); proved live. supported 112→113.
- `560939e0` docs — post-OOM checkpoint.
- `fe3f529e` feat — **`pod.shared-ipc-uts` partial→supported (§9)**: new `scen_pod_shared_ipc_uts` proves by IDENTITY that 2 members of a pod read the same `/proc/self/ns/{ipc,uts}` and a container outside reads a different one; proved live. supported 113→114. Confirmed in parallel that `pod.shared-pid` is genuinely not-implemented (members receive `CLONE_NEWPID`; `shareProcessNamespace` not wired) — a Sprint 2+ feature.

- `d8f7390b` fix — `net.apply.rollback` not-impl→unsupported (§4.6, ADR-0059).
- `8838a15d`+`d8ff1c2b` feat+merge — **`pod.shared-pid` not-impl→supported (§9)**: implements `shareProcessNamespace` (joining the infra PID ns done in the *parent* before the `clone`, since `setns(CLONE_NEWPID)` only affects children; `PidnsGuard` restores). Scenario `scen_pod_shared_pid` proves by identity + process visibility. Done in a parallel worktree, proved live.
- `ca2d7d88`+`bcc2e47e` fix+merge — **CH `vm.ip.observed` unsupported→partial (§4)**: observation by ARP (`--wait`→`sdn_reachable`) validated live (guest answered ARP), but the reported IP is still computed from the MAC (not read from a lease/agent) → honestly partial, not supported. Done in a parallel worktree.

Integration: the two parallel fronts each ran in their own worktree off develop; integrated serially with a single matrix/ratchet regeneration (115/274); worktrees and branches cleaned. develop also gained, in parallel (owner), 2 CRI fixes (`91771918`, `7fbd385f`).

- `85c308c9`+`2bbc1cb7` feat+merge — **snapshot restore with DATA (libvirt)** §7/§14: a new e2e section writes `DADO-ORIGINAL-v1`→snapshot→corrupts→restore→re-reads the original (8/8 PASS live). `vm.snapshot-disk`/`vm.snapshot-restore` gain real recovery evidence; `vm.snapshot-memory` DEMOTED supported→partial (RAM not observable without a guest with an OS). Parallel worktree.
- `2b857465`+`387eefea` fix+merge — CH `vm.network.bridge`: reason corrected (§13); stays `unsupported-by-provider` as it is a mirrored architectural decision (CH rootless→SDN; libvirt→host bridge), not a VMM limitation. No state change. Parallel worktree.

- `9b35fffd`+`e28ea711` feat+merge — **CRI coverage of `shareProcessNamespace`**: `lifecycle.rs` reads `namespace_options.pid==POD`, persists `SandboxRec.shared_pid` and wires `pod_shared_pid`+`pod_infra_pid` (anchor pid = 1st live member) on the native SDN members. 95 delonix-cri tests + 2 new. Matrix untouched (coverage; it was already supported via the CLI). Proved by a unit test of the decision (not by a real CRI pod — that would need kubelet).
- `288b0f48`+`9a2ca746` feat+merge — **`metrics.prometheus` partial→supported (§14)**: the per-container gauges were only about contention (throttling/PSI); I added `workload_cpu_usage_seconds_total` + `workload_memory_current_bytes` (always present in cgroup v2). A live scrape proved the per-container series (mem 331776 B). supported 114→115.

- `e6624da7`+`d6f024c4` feat+merge — **shared PID under `--net host`/CNI**: proved empirically live that it is IMPOSSIBLE rootless (`setns(CLONE_NEWPID)` requires `CAP_SYS_ADMIN` in the userns that owns the pid ns; a sibling userns cannot even open `ns/pid`). Instead of silence, `lifecycle.rs` now REFUSES with an actionable error (naming the fix: drop hostNetwork). pod.shared-pid stays supported through the native path. Rootless CNI (`DELONIX_CNI=1`) unaffected. Matrix untouched.
- `ad2e7358`+`cb89aee6` feat+merge — **`host.capacity` not-impl→supported (§4/§9/§14)**: the reporting already existed (`node.rs:362 capacity()`, served at `/v1/node/capacity`, e2e-tested) — only the classification was wrong. Live: allocatable < total (CPU 17000/20000 millis; mem 53/62 GiB) confirms it reflects the rootless delegated cgroup, not the machine. supported 115→116.

- `b3323619`+`ba25af84` feat+merge — **`provider.events` partial→supported (§14)**: `WatchEvents` was already served (stream with incremental tail); proved live (`system events --follow` captured an external `secret/remove`; new gRPC integration test). supported→+1.
- `f210ee3f`+`4b092a0c` feat+merge — **`vm.cloud-init` partial→supported (§7)**: guest declares `DataSourceNoCloud [seed=/dev/vdb]`, `write_files` marker + hostname applied; read off the disk with guestfish (e2e 5/5 PASS). supported→+1.
- `af3cb654` docs — regenerates the combined matrix (118/274, 43.1%).
- `fe6093ad` chore(lang) — **lang_ratchet reconciliation**: the campaign introduced 16 comments + 2 test names in pt-AO; the gate is a descendant-only ratchet (code in English). Translated → English; lang_ratchet green (3181/1044/116). Behaviour/evidence intact.

### Environment constraint
Host disk at **99–100%** (~12G free after cleaning up worktrees). Each release build costs ~2–3G of churn; the next build-heavy work risks ENOSPC. Mitigated by removing finished worktrees; the underlying resolution is the owner's (free up disk).

- `5519a696`+`1b337766`+`9b00e60d` feat+merge — **`metrics.network.per-workload` partial→supported (§14)**: `/metrics` now exports `workload_network_receive_bytes_total`/`workload_network_transmit_bytes_total` per container (read in the netns), cache populated by the background refresh (netns reads outside the inline scrape). Live scrape: rx=2306/tx=524 match `/proc/net/dev`. Code in English (lang_ratchet lesson); lang green. supported 118→119.

- `23984c3f`+merge — **libvirt `vm.hotplug.disk.add` partial→supported (§7 flagship)**: 4 levels proved live on an Ubuntu 24.04 cloud-init guest — accepted → `domblklist` (vdc) → detected in the guest (`lsblk vdc`) → **real R/W** (`HOTPLUG-RW-PROOF-v1` written+re-read, read by guestfish). In-guest poller planted by cloud-init (no agent/network). supported→+1.
- `6e206c14`+merge — **proxmox `vm.snapshot.persistent` partial→supported (§7)**: proved live against the real pve node (PVE 9.2.2, 192.168.30.117) — the snapshot survives a stop/start; ephemeral VM destroyed, zero global/destructive changes (§11). supported→+1.
- `cc28a39f` docs — regenerates the matrix (121/274, 44.2%).

- `bfd626e1`+merge — **libvirt `vm.hotplug.nic.add` partial→supported (§7)**: 4 levels live — accepted → `domiflist` (vnet on the new network) → detected (`enp7s0` in the guest) → **real traffic** (ping 4/4 to the gateway through the new NIC, rx/tx rising). supported→+1.
- `448ddfb9`+merge — **proxmox `vm.snapshot.memory` partial→supported (§7)**: live against the pve node — the node confirms `vmstate = local-lvm:vm-100-state-ram1` (RAM saved). Ephemeral VM destroyed, §11 ok. supported→+1.
- `a42af347` docs — regenerates the matrix (123/274, 44.9%).

- `144c4d4e`+merge — **libvirt `vm.hotplug.cpu.add` + `vm.hotplug.memory.add` partial→supported (§7)**: 4 levels in the guest — CPU nproc 2→3 (cpuN online); memory MemTotal 917896→1966472 KB (+1 GiB, blocks online). supported→+2.
- `426455a9`+merge — **proxmox `host.capacity` not-impl→supported (§9)**: pure GET `/nodes/{node}/status` (cpus 4, mem 16.7G, rootfs 19.5G); §11-clean (nothing created on the node). supported→+1.
- `c46b1661` docs — regenerates the matrix (126/274, 46.0%).

- `d915b2ea`+merge — **linux `firewall.icmp-type` + `firewall.logging` not-impl→supported (§10)**: nftables lowering of `meta l4proto icmp [icmp type N]` + `log prefix ... limit rate 10/s`. Live: icmp type 8 allowed 3/3, type 13 blocked 0/3 (counters); logging 40→5 logged (rate-limit observable). supported→+2.
- `2b4d4494`+merge — **libvirt `vm.clone` + `vm.template` not-impl→supported (§7)**: independent clone (`qemu-img convert -U`, standalone disk, own UUID/MAC/seed; fixed a self-referencing `<backingStore>` bug that prevented boot); template marks a frozen VM (start refuses DX-1501, instantiates by clone). New `vm clone`/`vm template` CLI with all CLI/docs/leaves/pt.po gates updated. supported→+2.
- `73650bd2` docs — regenerates the matrix (130/274, 47.4%).

- `d915b2ea`+merge — **linux `firewall.icmp-type`+`firewall.logging` not-impl→supported (§10)**: nftables `icmp type N` + `log prefix limit rate`; traffic allowed 3/3 / denied 0/3, logging 40→5 (rate-limit observable by counter). supported→+2.
- `2b4d4494`+merge — **libvirt `vm.clone`+`vm.template` not-impl→supported (§7)**: independent clone (standalone disk, own UUID/MAC; fixed `<backingStore>` bug); template refuses start (DX-1501). New CLI + CLI/docs/leaves/pt.po gates updated. supported→+2.
- `53c2bdee`+merge — **CH `vm.hotplug.disk.add`+`vm.hotplug.nic.add` not-impl→supported (§7)**: API `/vm.add-disk`/`/vm.add-net`, proved in the guest (vdc mkfs+R/W; ens6 DHCP+ping, 8/8). Fixed raw-ReadOnly + default→ingress alias; closes taps on stop. supported→+2.
- `8b6048e1`+merge — **libvirt 5 VM partials→supported (§7)**: `disks.extra`, `nics.extra`, `cpu.model` (exact topology in the guest), `restart` (boot id changes), `migration.cold` (flatten+local import); `restart-policy.native` stays honestly partial (no `<panic>`). supported→+5.
- `17560f3b` docs — regenerates the matrix (**137/274, 50.0%**).

## VM compatibility (vm-compute/protection/mobility/console domains)

| provider | supported/applicable | % |
|---|---|---|
| cloud-hypervisor | 26/36 | 72.2 % |
| libvirt | 33/45 | 73.3 % |
| proxmox | 30/37 | 81.1 % |
| **TOTAL VM** | **89/118** | **75.4 %** |

Pair `193f9461`: CH `vm.backup.disk/restore` →supported (data recovery cycle ORIGINAL→CHANGED→ORIGINAL, guestfish); libvirt `vm.guest-agent` re-confirmed supported (agent 8.2.2 live); CH `vm.guest-agent` not-impl→unsupported-by-provider (CH v52 has no virtio-serial `org.qemu.guest_agent.0` — leaves the denominator, hence 274→273).

Pair `18fdf6fd`: CH `vm.hotplug.cpu/memory.add` →supported (nproc 1→2, MemTotal +512M in the guest; removes stay honestly not-impl by §7 risk); proxmox `vm.console.serial/vnc` →supported (websocket 101 handshake completed via termproxy/vncproxy).

Pair `993c7ae0`: proxmox `vm.hotplug.disk/nic.add` (hot attach to a running VM, `refuse_pending`) + libvirt `vm.hotplug.disk/nic.remove` (safe cooperative removal — a disk in use does not leave; the record only loses the device once the guest releases it) →supported.

Last pair (`2622eb07`): CH `vm.cpu.pinning` (CH v52 affinity bug fixed), `vm.antispoof`, `vm.console.serial` →supported; libvirt `vm.console.serial`/`vnc` (RFB handshake), `vm.backup.disk`/`restore` (data recovery) →supported. CH `vm.ip.observed` and `vm.migration.cold`, libvirt `vm.backup.quiesced` and `vm.raw-definition` stay honestly partial (IP computed≠observed; cold-migrate needs a 2nd host; quiesce needs a guest-agent; raw-def is a security boundary).

Realistic ceiling < 100% on this host: ~8-10 hardware-gated cells (pci-passthrough/tpm/hugepages/guest-agent) stay honestly partial until there is hardware. Remaining path: more backend-complete partials (CH antispoof/cpu.pinning/console; libvirt console/backup/raw-definition) + hotplug.remove (with §7 care) + proxmox hotplug add/console.

## Verified correct-as-is (DO NOT re-chase — read-only, no false promotion)

- **CH/libvirt `host.capacity` = not-implemented** is RIGHT. HostHealth is the only host-level thing the VM providers expose (libvirt:5149 test). `host.capacity` for a VM provider would mean the hypervisor's VM-allocatable capacity (vCPU/RAM for VMs), distinct from the cgroup-delegated container capacity that `/v1/node/capacity` reports (the one linux promoted). Not the same number → not mimicable.
- **CH/libvirt `provider.events` = not-implemented** is RIGHT. Confirmed by grep: no `events::emit(..., "vm", ...)` exists — VM actions do not write to the event log; only linux (container/volume/stack/pod/security) emits. That is why linux `provider.events` is supported and the VM ones are not.

## Next fronts (queue)

1. `provider.async-operations` (linux not-impl; proxmox partial — job handle) (`lifecycle.rs` reads `namespace_options.pid==POD` and injects `pod_infra_pid`) — today pod.shared-pid/ipc-uts are proved through the public CLI path `pod create`; a consistent extension.
2. Snapshot restore with data (§7/§14): prove recoverability, not just creation.
3. Remaining candidates B: CH `vm.network.bridge` (adapter ignores `net_mode`).
4. Fronts with a dedicated environment: Proxmox via the `pve` node, multi-node network, storage, GPU (no hardware → pending).

## Files

- `00-baseline-reconciliation.md` — fixed baseline, regenerated matrix, diff vs report explained, catalog model, OpenStack boundary.
- `01-sprint-plan.md` — plan by sprint/front, specialists, promotion criterion (#16), authorized environments.
- `matrix-structured.json` — structured import of the 515 entries (key `provider|capability`).
- `matrix-declared-5.1.1-descendant.md` — snapshot of the declared matrix regenerated from the binary.

## How to regenerate the matrix

```sh
delonix provider matrix              # declared view (Markdown, == docs/providers/capability-matrix.md)
delonix provider ls -o json          # view measured on this host (JSON)
delonix provider describe <prov> -o json
```

## Totals (reconciled)

supported 112 · partial 86 · not-implemented 72 · unsupported-by-provider 230 · requires-external-component 12 · unavailable-on-host 3 = **515**.
Actionable (partial+not-implemented): **158**.
</content>
</invoke>
