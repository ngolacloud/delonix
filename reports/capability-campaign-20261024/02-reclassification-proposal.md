# Sprint 1 — Reclassification proposal (audit §4)

Date: 2026-10-24. Read-only front of Sprint 1. Two specialist auditors (compute; network/storage/gateway) cross-checked the 350 partial/not-implemented/unsupported entries of the 5 providers against the adapter code and the official documentation. **Reclassifying does not count as implemented functionality** (mandate §13) — it is a fix of the root cause (§4). Applying it in code is left for a worktree off develop (see cascade effects at the end); this document is the verified proposal.

Overall result: the matrix is well built. Of the ~350 negative states, only **3 have a strong basis to change state** (all self-verified below), plus a handful of medium/textual confidence that require live proof before being touched.

> **APPLIED in `8cd5c63b`**: A1, A2 and A3 integrated into develop with green gates (libvirt 71/0, volume 38/0, matrix no-drift, node-api mirror, ratchet 112/274, arch-fitness). Published matrix regenerated. B/C/D remain pending for Sprint 2.

## A. HIGH confidence — verified directly in this session

### A1. libvirt | `vm.migration.live` : `unsupported-by-provider` → `not-implemented`
- **Cause §4.1**: an architectural limitation of the engine treated as an upstream absence.
- **Code** (`crates/providers/delonix-provider-libvirt/src/lib.rs`, arm `C::VmMigrationLive`): `reason: "ADR-0031: needs shared VM storage or a privileged libvirt daemon listening on the network; neither is a default of this engine"`. The reason itself describes **existing paths** (shared storage *or* privileged daemon) — the capability belongs to libvirt, what is missing is the engine wiring it up.
- **Official source**: libvirt.org/migration.html — live migration without shared storage is supported via `--copy-storage-all` since libvirt 1.0.3.
- **Contrast that validates the criterion**: the same capability on cloud-hypervisor (`S::UnsupportedByProvider`, "CH migrates memory only, never the disk") **remains correct** — it matches `docs/live_migration.md` of CH exactly; there it is a genuine provider limit. The reclassification is for libvirt only.
- **Note**: `not-implemented` is more honest than `unsupported-by-provider`; a defensible alternative = `requires-external-component { component: "shared/replicated VM storage (ADR-0031)" }`, consistent with the sibling `C::VmReplication` that already uses that state. Decide during application depending on whether the engine intends to support `--copy-storage-all` natively (→ not-implemented) or require external storage (→ requires-external).

### A2. linux | `storage.lvm-thin` : `not-implemented` → `partial`
- **Cause §4.4**: not-implemented when it is already implemented and wired to the public path.
- **Code**: `crates/adapters/delonix-volume/src/provider_report.rs:101` → `C::StorageLvmThin => S::NotImplemented`. But the driver exists (`crates/providers/delonix-provider-lvm/src/lib.rs`, 621 lines, own tests), is **registered at startup** (`bins/delonix-runtime-bin/src/cmd/storage_pool.rs:134` via `register_drivers()`), is on the **allowlist** (`crates/contexts/delonix-storage/src/allowlist.rs:47` — `("lvm-thin","P4")`) and serves volume allocation through the public `storage pool` command. The architecture test `every_pool_driver_the_allowlist_accepts_is_registered` (`storage_pool.rs:1090`) proves the chaining.
- **Correct state** = `partial` with a written limit: *"driver implemented, registered and accepted by the allowlist; no e2e proof on a real LVM-thin pool (scripts/e2e.sh only exercises the `dir` driver)"*. It is not `supported` (missing live evidence — criterion #16).

### A3. linux | `storage.zfs-btrfs` : `not-implemented` → `partial`
- **Cause §4.4**: identical to A2.
- **Code**: `provider_report.rs:102` → `C::StorageZfsBtrfs => S::NotImplemented`. Drivers in `delonix-provider-zfs/src/lib.rs` (477 lines) and `delonix-provider-btrfs/src/lib.rs` (498 lines), both registered (`storage_pool.rs:125,130`), on the allowlist (`("btrfs","P2")`, `("zfs","P3")`), with unit tests.
- **Correct state** = `partial` with the same limit ("no e2e proof on a real pool").

## B. MEDIUM confidence — requires live verification before being touched (candidates for Sprint 2, not blind reclassification)

| provider\|capability | current → proposed | cause | what to verify first |
|---|---|---|---|
| cloud-hypervisor\|`vm.network.bridge` | unsupported → not-implemented | §4.1 — adapter ignores `net_mode` (never read); libvirt implements the same tap+bridge model as partial | confirm that the CH tap is indeed indifferent to the bridge and that only SDN wiring is missing |
| cloud-hypervisor\|`vm.ip.observed` | unsupported → partial | §4.1/§4.5 — ARP `--wait` path already exists (`lib.rs:471`), exercised in `vm.antispoof` | confirm that the ARP result is written to the "observed IP" field that `vm show`/`ls` reads, and not isolated in the anti-spoof |
| linux\|`net.apply.rollback` | not-implemented → unsupported-by-provider | §4.3/§4.6 — `nft -f` replaces the ruleset atomically; nothing is staged (ADR-0059) → rollback is inapplicable, not pending. Consistent with `net.apply.staged` already unsupported | confirm reading of ADR-0059 §152/§248 and product decision |
| linux\|`volume.webdav` | unavailable-on-host (state kept) — justification shared with NFS/CIFS technically dubious | §4.1 — `mount.davfs` is FUSE (setuid helper `fusermount`), does not need `CAP_SYS_ADMIN` like NFS/CIFS | confirm on the host whether `mount -t davfs` mounts rootless; if so, separate the WebDAV justification from the NFS/CIFS ones |

## C. Textual precision (does not change `state`, improves interface honesty — mandate §13)

- **proxmox | `net.gateway.*`** (firewall block ~`lib.rs:7131`): the text "the node's SDN has no perimeter filter of its own" is factually false — PVE has a Datacenter Firewall with a default perimeter policy. The decision not to manage it (administration, ADR-0049 D3) can stand, but the text should say "it exists, the engine does not touch it by design", not deny its existence.
- **proxmox | `vm.raw-definition`** (unsupported, ADR-0049 D6): raw `args:` exists in `qm`/API (a partial analogue to the `libvirtXml` that libvirt exposes as partial). Genuinely different scope — keep it unsupported but review whether the reason should cite decision D6 instead of denying the mechanism.

## D. Useful negative confirmations (NOT errors — they look like them at first sight)

- hotplug `*.remove` (cpu/mem/disk/nic) CH+libvirt `not-implemented`: upstream supports it (CH `/vm.remove-device`), but the adapter lacks the code → `not-implemented` correct.
- proxmox `vm.console.serial`/`vnc` `not-implemented`: the API has `termproxy`/`vncproxy`, the adapter does not call them → correct.
- proxmox `net.vlan/vxlan/ipam/dns/routes`, firewall `logging/icmp-type` `not-implemented`: PVE SDN/firewall supports everything, the adapter does not wire it (`sdn.rs:26-32` documents it) → `not-implemented` is the exact semantics; candidates for Sprint 2.
- opnsense `net.vlan/vxlan/ipv6/rate-limit/capture` `unsupported-by-provider`: the catalogue measures the network fabric of the engine's *workloads*, not the appliance's administrative config → different category, correct.
- opnsense `nat.one-to-one/multi-wan/vpn/dns/dhcp`, `firewall.icmp-type` `not-implemented`: the API supports it, the adapter does not touch it → real Sprint 2 gaps.
- linux `net.ipv6` unsupported: a written, deliberate security refusal (`infra.rs:3645`), not confusion → correct (mandate §10: do not enable IPv6 just by removing the block).
- linux `firewall.icmp-type`/`logging` not-implemented: `policy_nft.rs:73` refuses at lowering despite nftables supporting it → a real absence in the engine, `not-implemented` correct; Sprint 2 (with logging limits, mandate §10).

## Cascade effects of applying it in code (why it needs a worktree + gates)

Applying A1–A3 touches `provider_report.rs` and `libvirt/src/lib.rs` (3 arms), and triggers:
1. **Published matrix**: `docs/providers/capability-matrix.md` must be regenerated (`delonix provider matrix > …`) — the test `the_published_matrix_is_the_generated_one` fails otherwise.
2. **Cell ratchet** (`scripts/capability_baseline.json`): A1 (unsupported→not-implemented) **increases `applicable` by 1** (273→274) because not-implemented counts as applicable; A2/A3 do not change `applicable` (partial and not-implemented both applicable) nor `proved` (none becomes supported). `scripts/capability_ratchet.py --update` is required **in the same commit**.
3. **Counts**: not-implemented 72→69, partial 86→88, unsupported 230→229. `proved`/supported unchanged (no false promotion — consistent with §17: progress is not only about reducing negatives).
4. **Mirror copy** of the provider set in `crates/interfaces/delonix-node-api/src/providers.rs` (test `the_declared_providers_are_the_published_matrix`).
5. **pt.po translation** if any new reason/limit has text presented to the user.

Conclusion: A1–A3 are a small, verifiable, low-risk code front — the first to apply when a WIP slot opens. B/C/D enter Sprint 2 (implementation/validation), not here.
