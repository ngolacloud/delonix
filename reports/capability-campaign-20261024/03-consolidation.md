# Consolidation — capability campaign (checkpoint 126/274)

Date: 2026-10-25. Integrated verification over develop HEAD of the whole accumulated body (67 commits since `558e5f20`), before proceeding. Meets §17 of the mandate (before/after matrix + separate counts, not measuring progress only by the reduction of negatives).

## Verified state (all CI gates green at the same time)

- `make gates`: lang_ratchet **3181/1044/116**, arch_fitness (library_prints **82**, layer direction, env_writes 23, self_exec 29), version_gate (5.1.1=v5.1.1), adr_status (85 ADR/81 lines), dev_docs/handbook, leaves 353/570 — **all ok**.
- `capability_ratchet`: **126 of 274 (46.0 %)**.
- Published-matrix non-drift + evidence-exists (every `supported` cites a real test): **ok**.
- Campaign crate tests, together at HEAD: delonix-runtime-bin 233, delonix-proxmox 134, delonix-provider-libvirt 71, delonix-mgmt 36, delonix-telemetry 14, delonix-cri 95, delonix-compute 126, delonix-linux — **0 failures**.
- Campaign worktrees and branches: **clean**. Tag `v5.1.1`: **intact**.

During consolidation a cross-pair debt that the per-pair checks had let slip was caught (and fixed, `d7c517ca`): 3 debug `eprintln` in the proxmox live tests raised `arch_fitness.library_prints` 82→85. Lesson: run the **complete** `make gates` (not just the last line) per front.

## Before/after matrix

| state | initial report | now | Δ |
|---|---|---|---|
| supported | 103 | **126** | **+23** |
| partial | 93 | 78 | −15 |
| not-implemented | 74 | 67 | −7 |
| unsupported-by-provider | 230 | 229 | −1 |
| requires-external-component | 12 | 12 | 0 |
| unavailable-on-host | 3 | 3 | 0 |
| **total** | **515** | **515** | **0** |
| proved/applicable (N2) | — | **126/274 (46.0 %)** | — |

(Reconciled: the initial report measured 103 supported on a build prior to v5.1.1; the regenerated baseline gave 112. From those 112, the campaign took it to 126.)

## Separate counts (§17) — qualified progress

**New capabilities implemented (new backend):**
- `pod.shared-pid` (linux) — `shareProcessNamespace`: joining the infra PID ns in the parent before the clone + guard; wired to CLI and CRI.
- `metrics.network.per-workload` (linux) — rx/tx per-container series in `/metrics`.
- `metrics.prometheus` (linux) — per-container cpu/mem usage gauges (previously contention only).
- `host.capacity` (proxmox) — `host_capacity` method reading `/nodes/{node}/status`.
- `vm.snapshot.memory` (proxmox) — `snapshot_config` method exposes `vmstate`.

**Features completed + validated live (backend existed, proof was missing):**
- libvirt hotplug add: `disk` (R/W in the guest), `nic` (real traffic), `cpu` (nproc↑), `memory` (MemTotal↑) — 4 levels each.
- libvirt `vm.cloud-init` (guest consumes NoCloud), `vm.snapshot-disk`/`vm.snapshot-restore` (data recovery via guestfish).
- linux `container.oom-detection` (crash_reason=oom_killed), `pod.shared-ipc-uts` (ns identity).
- proxmox `vm.snapshot.persistent` (survives a stop/start, real node).

**Validations added:** chaos scenarios (`scen_oom`, `scen_pod_shared_ipc_uts`, `scen_pod_shared_pid`), e2e sections (6 new), integration tests (gRPC WatchEvents) and proxmox live tests (3).

**Classification errors corrected (cause, not feature):** libvirt `vm.migration.live` unsupported→not-implemented; linux `storage.lvm-thin`/`storage.zfs-btrfs` not-impl→partial; linux `net.apply.rollback` not-impl→unsupported; CH `vm.ip.observed` unsupported→partial; CH `vm.network.bridge` reason corrected; linux/proxmox `host.capacity`+`metrics` reclassified.

**Demotion for honesty (metric down, confidence up):** libvirt `vm.snapshot.memory` supported→partial (RAM recovery not observable without a guest with an OS).

**Coverage/robustness:** `shareProcessNamespace` wired on the CRI path (kubelet); actionable refusal of shared-pid under `--net host`/CNI (instead of silence).

**Environment blocks / proven upstream limits:** GPU/CDI — no hardware, stays "implemented not validated". CH/libvirt `host.capacity` and `provider.events` — verified correct as not-implemented (VM capacity ≠ cgroup; VMs do not emit events). Proxmox serial/VNC console — remote websocket, graduation requires a cross-cutting change (stays honestly partial/not-impl).

## Invariant maintained
Every `+1` in supported went through the gate that requires real evidence (an existing `check:`/`e2e:`/`live:`/`chaos:`/`test:` test) — zero label-only promotions. 15 net promotions to supported, all with proof; progress not measured only by the reduction of negatives.

---

## Checkpoint 2 (2026-10-25, HEAD 722956f1) — integrated verification repeated

After 6 more pairs of fronts (firewall §10, clone/template, hotplug disk/NIC/cpu/mem add+remove, consoles, backups, cold migration, cloud-init, proxmox snapshots), I re-ran the integrated verification over the whole accumulated body (**121 commits** since `558e5f20`):

- `make gates` COMPLETE green: lang_ratchet 3181/1044/116, arch_fitness (library_prints **82**), version, dev_docs, adr (85/81), leaves (353/572).
- `capability_ratchet`: **152 of 274 (55.5 %)**.
- Campaign crate tests, together at HEAD, **0 failures**: delonix-runtime-bin 233, delonix-proxmox 136, delonix-provider-libvirt 72, delonix-provider-cloud-hypervisor 40, delonix-mgmt 36, delonix-cri 99, delonix-compute 130, delonix-telemetry 14.

### Before/after (updated)

| state | initial report | now | Δ |
|---|---|---|---|
| supported | 103 | **152** | **+49** |
| partial | 93 | 64 | −29 |
| not-implemented | 74 | 55 | −19 |
| unsupported-by-provider | 230 | 229 | −1 |
| proved/applicable | — | **152/274 (55.5 %)** | — |

### VM compatibility (vm-compute/protection/mobility/console domains): **85/118 = 72.0 %**
cloud-hypervisor 23/36 (63.9 %) · libvirt 32/45 (71.1 %) · proxmox 30/37 (81.1 %).

The remaining path to the VM % is mostly **hardware-gated** (pci-passthrough/tpm/hugepages/guest-agent) and **architecturally honestly partial** (cpu/mem hot-remove unsafe → cold resize; live/cold migration between hosts needs a 2nd node; raw-definition is a security boundary). The invariant holds: every promotion to supported has real evidence verified by a gate; no label-only promotion.
</content>
