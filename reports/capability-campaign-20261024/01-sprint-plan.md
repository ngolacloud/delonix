# Sprint plan — capability conformance campaign (baseline v5.1.1)

Derived from `matrix-structured.json`. **158 actionable entries** (partial + not-implemented); the remaining 357 states are structural (230 unsupported-by-provider, 12 requires-external-component, 3 unavailable-on-host, 112 already supported).

## Distribution of the 158 actionable entries

By domain: vm-compute 45 · network 40 · metrics 13 · firewall 13 · inventory 12 · protection 7 · console 7 · access 6 · storage 5 · containers 4 · system-containers 4 · mobility 2.

By provider: proxmox 48 · libvirt 39 · linux 32 · cloud-hypervisor 27 · opnsense 12.

`not-implemented` (the cleanest gaps, 72): concentrated in vm-compute (CH 8, libvirt 6, proxmox 6), network (proxmox 12, opnsense 11, linux 2), metrics (proxmox 4), inventory and console. See the detail in `matrix-structured.json`.

## Promotion rule (criterion #16, non-negotiable)

Changing a capability to `supported` requires, cumulatively: functional implementation wired to the public paths (consistent CLI/API/Desktop) · precondition validation · actionable errors · persistence/recovery where applicable · **real test** (the evidence `check:`/`e2e:`/`live:`/`test:`/`chaos:` must exist and be verifiable) · documentation · evidence tied to commit+environment. Without an environment, implementation+local tests are concluded but it stays **"implemented, not validated"** on the evidence dimension (remains `partial` with a written limit, never falsely `supported`).

## Dependency sequence

### Sprint 0 — reconcile (THIS checkpoint) ✅
Baseline, binary, lossless 515 matrix, explained diff, plan. Delivered on `develop` (docs-only).

### Sprint 1 — isolation, data and classification risks (highest risk first)
Before adding features, fix what misleads or risks data:
- **Wrong classification**: adapter refusals marked as upstream absence; capabilities of one domain attributed to the wrong provider; ADRs treated as a permanent blocker. Reclassify with justification (does not count as a new feature).
- **Isolation**: shared namespaces (IPC/UTS/PID) verified by real process identity, not by `setns` having been called; OOM termination reason distinct from other terminations.
- **Data**: snapshots/backups that only prove file creation — require a restore+consistency test before any promotion.
- Specialists: Rust + security + testing. Low environment risk (almost everything testable on the rootless live host).

### Sprint 2 — complete what exists but is not validated
`partial` entries whose backend already exists and only lack live proof:
- vm-compute libvirt/CH: operations tested only by XML/args/mock → re-test with real guests (disk hotplug with R/W in the guest, NIC with traffic, snapshot restored with data).
- network linux: IPAM/reservations/DNS/static with dataplane verification (traffic allowed/denied), not just rule existence.
- Specialists: virtualization + networking + testing. Environment: live host (libvirt/CH proven here); Proxmox via node `pve` on virbr0.

### Domain sprints (3+) — implement gaps per front, in parallel when independent
- **Compute CH/libvirt** (mandate §7): events, async, clone/templates, resize, hotplug/unplug (4 levels: accepted→applied→detected in the guest→usable), extra disks/NICs, CPU model/pinning, hugepages, TPM/PCI passthrough, cloud-init, guest agent, observed IP, snapshots, backup/restore, migration, console, metrics/capacity.
- **Proxmox** (§8): jobs/UPIDs, templates, disk resize, static IP, storage pools, persistent snapshots, serial/VNC console, metrics; system containers (import, network, firewall, backup/restore, exec/logs/exit via a supported mechanism — without inventing endpoints; LXC ≠ OCI).
- **linux network** (§10): VM DHCP in the IPAM ledger, reservations without collision under concurrency, static, DNS, end-to-end IPv6, VXLAN, encrypted overlay (≥2 nodes), L7 proxy, L4 LB, healthchecks, drift/dataplane, firewall logging with limits.
- **Proxmox + OPNsense network** (§11): SDN zones, VLAN/VXLAN, subnets/IPAM/DHCP/DNS/SNAT/IPv6, VM/CT firewall with proven traffic; OPNsense rules with identity, multi-WAN, VPN, NAT 1:1, NPT, rollback, dataplane — without enabling global switches nor installing plugins without specific authorization.
- **Storage** (§12): NFS/CIFS/WebDAV/LVM-thin/ZFS/Btrfs/TrueNAS/Ceph, pools, replication, HA — distinguish lack of privilege from lack of implementation; restricted helpers instead of global root; HA and replication have their own contracts (local restart ≠ HA).
- **Containers/pods** (§9): streaming events, async, GPU/CDI (only with hardware), OOM, shared namespaces, metrics, capacity, secret resolution.

### Final sprint — integrated regression + new inventory
Re-run `delonix provider matrix`/`ls`, regenerate `matrix-structured.json`, produce a before/after matrix with all 515 entries and the separate counts (newly implemented / completed / validations / reclassifications / integrated dependencies / environment blocks / proven upstream limits). Progress is **not** measured only by the reduction of negative states.

## Harness per front

One branch+worktree per front, from `develop` (features/maintenance) or `main` (fixes/bugs), via `scripts/gitflow.py start`. Separate review before integrating. Integration into develop after green gates (`make gitflow gates`). Safe cleanup at the end. Independent fronts in parallel (distinct worktrees); changes to common contracts (catalogue `capability.rs`, protos) coordinated before distributing. Never two batteries disputing the same test resource (one `pve` node, one GPU host).

## Known authorized environments

- Rootless live host (this one): OCI containers, pods, linux network, libvirt+CH VMs, delegated cgroup2 cpu/memory/pids. io/cpuset not delegated.
- Proxmox node `pve` pre-installed on virbr0 (root@pam / default public) — create/snapshot/destroy proven by the provider.
- No confirmed GPU/TPM/PCI-passthrough hardware → those capabilities stay "implemented, not validated".
- OPNsense/multi-node overlay: to be confirmed per front before promising live validation.
