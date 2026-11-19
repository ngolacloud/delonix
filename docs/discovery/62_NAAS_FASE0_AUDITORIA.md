# 62 — NaaS / Network Provider: Phase 0 and Phase 1 (audit before code)

Date: 2026-09-27. Measured baseline: `delonix-runtime` `origin/main` `a4d671a4` (v4.4.0 + 65),
`delonix-paas` `origin/main` `15fd72e`. Canonical document governing this:
`PROMPT_MASTER_DELONIX_RUNTIME_NETWORK_PROVIDER_NAAS.md` (provided by the owner, 2026-09-27).

This document fulfils §42 of that prompt: inventory, real flow, ownership, capability matrix,
bypasses, P0–P3 gaps, first increment, validation commands and blocking questions.
**There is no code in this step.** Every statement carries its location; "static" means read in the
code and not yet reproduced; "verified" means measured.

## 0. Translation of the prompt to this repository's boundary

The prompt describes a NaaS with tenant, IAM, quotas, billing, NetworkClass and approvals. This
repository's `AGENTS.md` forbids the engine from knowing a consumer, a tenant, a plan or a quota.
The two things reconcile like this, and this is the reading that decides what gets refactored here:

| Concept from the prompt | Where it lives | In the engine's vocabulary |
|---|---|---|
| Tenant API, IAM, quotas, NetworkClass, approval, billing | control plane (other repository) | — (the engine doesn't know them) |
| NetworkServiceProfile, ProviderBinding persisted per tenant resource | control plane | the engine returns the **capability report** and accepts a **named target**; it does not choose per tenant |
| Provider contracts per role, capability discovery, validate/plan/apply/observe/verify | **engine** | ports in `delonix-sdn`/`delonix-compute`, network Kinds, `delonix provider` |
| Policy compiler (IR → nft / Proxmox / OPNsense) | **engine** | `ContainerFw`/`FwRule`, `NetworkPolicy`, `NetworkAccessRule` |
| Transactional IPAM for a single node | **engine** | `delonix-sdn::ipam` |
| Global IPAM, per-region pools | control plane | — |

Practical consequence: no session delegated from this repository writes
`tenant`, `NetworkClass` or consuming-repository names into `crates/`, `bins/` or
`proto/` (`scripts/arch_fitness.py` refuses it).

## 1. Inventory of network components

### 1.1 Engine (`delonix-runtime`)

| Piece | Location | Role |
|---|---|---|
| holder (pin + control + slirp), veth/nft, DNS, DHCP, RA, NetDef, routes, services | `crates/adapters/delonix-sdn/src/infra.rs` | rootless dataplane |
| Per-prefix IPAM | `delonix-sdn/src/ipam.rs` (`allocate` 157, `reserve` 213, `release` 255, `reap_orphan_leases` 398) | `id→ip` leases |
| `NetworkStore`, slirp per container | `delonix-sdn/src/lib.rs` | declarative registry |
| CNI | `delonix-sdn/src/cni.rs` | CRI pod network |
| WireGuard, bpf | `delonix-sdn/src/{wg,bpf}.rs` | encrypted overlay, flows |
| pure rules (`Cidr`, `bridge_name`, prefix IPAM) | `crates/foundation/delonix-net-rules` | no dependencies |
| `NetworkProvider`, `VmNetwork` ports | `crates/contexts/delonix-compute/src/ports.rs:110,128` | attach/detach/publish/firewall |
| `GatewayProvider` port | `delonix-sdn/src/gateway.rs:126` | perimeter alias/rule/commit |
| `NetworkZoneProvider` port | `delonix-sdn/src/network_zone.rs:66` | remote zone/VNet |
| `VmBackend::apply_firewall/read_firewall` | `delonix-compute` (VM port) | per-VM remote firewall |
| OPNsense | `crates/providers/delonix-opnsense` | `GatewayProvider` (ADR-0051) |
| Proxmox SDN + VM firewall | `crates/providers/delonix-proxmox/src/{sdn,network_zone,vm_firewall}.rs` | `NetworkZoneProvider`, ADR-0052 |
| libvirt | `crates/adapters/delonix-vm` (`ensure_libvirt_network`, `net-update`, nwfilter) | `default` network, DHCP reservations |
| capability catalogue | `delonix-compute` `capability.rs` (ADR-0050) | `delonix provider ls/describe/matrix` |
| network Kinds | `Network`, `NetworkRoute`, `NetworkPolicy`, `NetworkAccessRule`, `Dependency`, `Service`, `IPPool`, `HTTPRoute`, `Ingress`, `Gateway`, `NetworkGateway`, `NetworkZone` | `crates/contexts/delonix-stack/src/kinds.rs` |
| local API | `crates/interfaces/delonix-mgmt/src/lib.rs:249-265` (`/v1/net/firewall`, `/egress`, `/attach-extra`) | frozen by ADR-0041 D4 |
| OpenStack | **no code exists** — ADR-0039 `Proposed`; `type: openstack` is refused in `providers_config.rs:570` | — |

### 1.2 Control plane (`delonix-paas`, read-only)

| Piece | Location | Note |
|---|---|---|
| NaaS model and `NetworkProvider`/`NetworkBackend` ports | `crates/delonix-net-model/src/{provider,backend}.rs` | its own, parallel to the engine's |
| `/v2/net/*` API | `delonix-api/src/netapi*`, registered in `ui/startup_reconcile.rs:544-576` | tenant |
| KV store (`net` bucket) | `delonix-net-store/src/repo_core.rs:7` | desired state |
| reconciler | `net_reconciler.rs:55-130` (no leader election), `net_sweeper.rs` | |
| adapters | `delonix-net-linux` (uses the v3.1.0 engine), `delonix-net-libvirt` (virsh), `delonix-net-proxmox` (**orphaned**), `delonix-image-proxmox` (2nd Proxmox client) | |
| engine pin | `Cargo.toml:50,82-90` → `rev 6680745` = **v3.1.0** | v0.56.0 only on a local branch |

## 2. Real flow (today)

```
control plane /v2/net ──► reconcile ──► NetworkProvider (env DELONIX_NET_PROVIDER, OnceLock)
      │                                   └► delonix-net-linux ──► engine v3.1.0 infra::network_create
      │                         e ALWAYS right after ► DelonixNetBackend (networks_subnets.rs:790)
      ├─ compute_sched ──► delonix-net-libvirt (virsh, fixed, outside the funnel)      [2nd realization]
      ├─ ui/* and overlay ──► delonix_net::infra::* directly                 [port bypass]
      └─ 30 s timer ──► reap_orphan_hostfwds(live only from the control plane's store) [concurrent writer]

engine (CLI / stack apply / node API)
      ├─ Kind Network ──► NetworkStore + NetDef ──► holder (lazy bridge on 1st attach)
      ├─ NetworkPolicy/NAR/ingress/egress ──► container registry (flock) ──► nft -f (atomic)
      ├─ delonix-mgmt /v1/net/firewall ──► nft WITHOUT registry                      [2nd writer]
      ├─ NetworkGateway ──► GatewayProvider (native refuses | OPNsense)
      └─ NetworkZone ──► NetworkZoneProvider (Proxmox; chosen by provider COUNT)
```

> ⚠️ **Design / not implemented** — the env var `DELONIX_NET_PROVIDER` (above) describes
> a planned provider selection; **0 readers in the Rust code as of 2026-10-28**
> (`grep` under `crates`/`src` = 0). Do not read it as an actual feature.

## 3. Ownership per resource (a single writer, or the conflict that exists)

| Resource | Writer(s) today | Verdict |
|---|---|---|
| per-container nft rules | registry+nft (CLI/Kinds) **and** `delonix-mgmt` (nft only, primary IP only) | **conflict** — mgmt bypasses the registry |
| slirp ingress hostfwds | engine **and** control plane (30 s timer, partial list) | **cross-cutting P0 conflict** — the same class as the outage already documented in `infra.rs` |
| holder / `dlxing` tables | engine binary **and** control plane's `delonix-engine` (re-exec as holder) | conflict if they share root (not verified) |
| NetDef (`ingress/networks`) | `network_create` (with lock); `network_create_with_gateway`, `network_remove`, `update_netdef_egress` **without lock** (the last in the holder process) | **internal conflict** |
| /16 allocation | `NetworkStore::create` (10.200–254) and `infra::network_create` (10.201–254) don't see each other; `validate_subnet` accepts overlapping 10.200/16 and 10.0.2/24 | **internal conflict** |
| IPAM leases | `ipam` (flock) **and** VM DHCP by MAC hash outside of IPAM (`dhcp_lease_ip` 2400) | two authorities |
| `httproute/manual.json`, `config.json` | `update_manual` read-modify-write without lock; `fs::write` not atomic | race |
| Proxmox cluster SDN | engine's `apply_sdn` (`PUT /cluster/sdn`) **and** control plane's `delonix-net-proxmox`; both apply EVERYTHING pending, including operator edits | latent conflict |
| OPNsense objects | engine; identity = name/description; `commit` applies third-party pending changes | adoption by name |

## 4. Current capability matrix (role × provider)

I = implemented and reachable via Kind/CLI; P = partial; L = library-only + live test; A = absent.

| Role | Native Linux | Proxmox | OPNsense | libvirt | Cloud Hypervisor | OpenStack | CNI |
|---|---|---|---|---|---|---|---|
| fabric/overlay | P (VXLAN/WG, single node) | L (fabrics, sdn.rs:1046-1317) | A | A | A | A | — |
| segment | I (bridge in the holder) | I/P (`NetworkZone`: zone+VNet) | A | P (`default`, nat/bridge/user) | I (tap) | A | I (CNI bridge) |
| port | I (`NetworkProvider::attach`) | P (`net0`; VM→VNet not wired) | A | P (extraNics XML only) | I | A | I |
| IPAM | I | L (`create_sdn_ipam`, `vnet ips`) | A | P (`ip-dhcp-host`) | I (derived from MAC) | A | I (host-local) |
| subnet | P (`Cidr`) | L | A | A | A | A | — |
| route | I (`NetworkRoute`) | L (read-only) | A | A | A | A | — |
| gateway/perimeter | P (`native` refuses everything) | A (excluded, ADR-0049 D3) | I (alias+rule, no update) | A | A | A | — |
| per-workload firewall | I (nft) | I/P (`scope: vm`) | A | P (anti-spoof nwfilter) | P (anti-spoof on tap) | A | — |
| NAT | I (masquerade, DNAT publish) | A | A | I (NAT network) | via SDN | A | portmap not wired as root |
| LB | P (`Service` by DNS) | A | A | A | A | A | — |
| DNS | P (`*.delonix.internal`) | L (`create_sdn_dns`) | A | A | A | A | — |
| telemetry | P | L | A | A | P | A | — |
| IPv6 | refused by default (fail-closed) | — | — | — | — | — | — |

The catalogue (ADR-0050) **has no** `gateway.*`, `nat.*`, `lb.*`, `dns.*` entries besides `net.dns`,
and `ProviderKind` only knows Compute/Network/Storage/Image. OPNsense does not appear in
`delonix provider ls`, and `providers.yaml` (ADR-0054) does not configure network providers.

## 5. Bypasses and duplications

1. `delonix-mgmt /v1/net/firewall|egress|attach-extra` writes nft without the container registry.
2. Three Proxmox clients in the ecosystem (engine's `delonix-proxmox`; control plane's
   `delonix-net-proxmox`, orphaned, and `delonix-image-proxmox`, TLS always off).
3. The control plane calls the engine's `infra::*` directly, outside its own port (≥20 sites).
4. The control plane realizes the same network twice (provider **and** `DelonixNetBackend`; and
   libvirt via `compute_sched`, with no delete).
5. Two `NetworkProvider` ports with the same name and different contracts (engine and control plane).
6. Public functions with no caller in production: `set_service_lb`/`clear_service_lb`,
   `import_iptables`, `alloc_ip`, `container_ip6`, `firewall_summary`, `network_routes_live`,
   `create_with_base`; and all of Proxmox SDN beyond zone/VNet (subnet, IPAM, DNS, fabric, IPs).

## 6. Prioritized gaps

### P0 — fail-open or loss of isolation

| # | Finding | Location | State |
|---|---|---|---|
| P0-1 | **Source egress bypassed.** `fwcont` does `ip daddr vmap @fwmap` before `ip saddr vmap @fwmap`, and the elements are `jump`. An `accept` in the DESTINATION chain terminates the base chain and the SOURCE chain is never evaluated: an `egress deny` has no effect for destinations that accept. | `infra.rs:775-778`, `4387` | static, nft semantics confirmed; to be reproduced |
| P0-2 | Rules that aren't `nft_safe` are **silently skipped** in the holder: an invalid `deny` disappears and the traffic falls through to the policy (which may be `allow`). | `infra.rs:4112-4115` | verified in code |
| P0-3 | Namespace isolation is a **warning**, not an error, on creation and on `start`. | `compute/network.rs:88`, `container.rs:3698` | static |
| P0-4 | `ingress rm/clear` that empties the rules runs `clear_firewall` and the container **loses namespace isolation**. | `firewall.rs:656,1168` | static |
| P0-5 | Networks with a CIDR outside 10.200–10.254: the holder refuses `firewall`/`ns_set_join` (`is_ingress_ip`) and those containers end up **without isolation**. | `infra.rs:3825` | static |
| P0-6 | `delonix-mgmt` writes firewall rules without the registry and only on the primary IP — the next reapply undoes it, and multi-homed stays open. | `mgmt/lib.rs:249-265,1098-1142` | static |
| P0-7 | (control plane) 30 s timer reaping hostfwds with a partial list. | `delonix-paas startup_reconcile.rs:279-307` | static; same class as the already-measured outage |

### P1 — leaks, races, adoption by name

- IPAM: network CIDR lease written to `<cidr>.json` and released into `10.X.json` (`allocate` uses
  the raw prefix, `release` via `key_for_ip`→`registry_key`) → **lease leak**; `reserve` fail-open
  (213); stop releases the lease against the `restore_lease` premise; extra-network leases not
  released; VM DHCP outside of IPAM.
- NetDef with no lock across 3 writers; two /16 allocators that don't see each other.
- `network rm` doesn't refuse with containers attached; `do_netdel` doesn't remove `@dlxbr`/`@netpair`
  nor stops DHCP; `publish_port` and `vm_attach` have no rollback; `httproute` has no lock/atomicity.
- OPNsense: identity by description (a manual rule "adopted" and then deleted), `commit` applies
  third-party pending changes, no update and no content drift detection.
- Proxmox SDN: `apply_sdn` reloads the operator's pending changes; `ensure_*` adopts by name.
- Holder control line: `attach`'s `ip`/`gateway`, `netrate`'s `burst`, `wg-up`/`wg-peer` fields
  without validation; `cni::resolve_plugin` accepts `/` and `..`; `vm bridge
  --vm-subnet` without validation and without checking root before `--apply`.

### P2 — contract and capabilities

- No per-role ports for remote IPAM/DNS/LB/subnet/fabric; no uniform `plan`/`observe`/`verify`;
  no stable error envelope (`unsupported_capability`, `stale_plan`, `partial_apply`…).
- Catalogue missing `gateway/nat/lb/dns`; OPNsense outside the catalogue and `providers.yaml`;
  `NetworkZoneProvider` chosen by count rather than by name; node API doesn't publish gateway/zone.
- `NetworkGateway`/`NetworkZone` not covered by the E2E battery.
- ADR-0024 (`selector` in policy) accepted and not yet implemented; ADR-0013 with IPAM/`Cidr` not wired.

### P3

- IPv6 (the current refusal is correct); OpenStack Neutron (ADR-0039 Proposed, zero code); real LB;
  authoritative DNS; topology; SLOs.

## 7. Refactoring decided by this document, and who it goes to

Each row is a local session, in its own worktree under
`~/workspace/ngolacloud/.worktrees/<repo>/<task>`, one PR per session.

| Session | Repo | Scope | Needs approval before code? |
|---|---|---|---|
| **S1 firewall fail-closed** | engine | P0-1…P0-5: split the dispatch into two base chains (egress and ingress) so `accept` isn't terminal; refuse the whole ruleset when a rule isn't `nft_safe`; namespace isolation as an error; `rm/clear` preserves isolation; isolation on CIDR networks | no — fixes behaviour, doesn't change ownership |
| **S2 transactional IPAM and NetDef** | engine | single lease key, fail-closed `reserve`, extra-network leases, lock on NetDef writers, a single /16 allocator, `validate_subnet` against reserved prefixes | no |
| **S3 lifecycle and single dataplane writer** | engine | `network rm` with dependents, complete `do_netdel`, rollback for `publish_port`/`vm_attach`, lock+atomicity in `httproute`, `delonix-mgmt` going through the registry or refusing (P0-6) | not for the engine; mgmt's route touches ADR-0041 D4 — decide to refuse rather than extend |
| **S4 privilege surface** | engine | validate every holder control-line token, `cni::resolve_plugin`, `vm bridge` | no |
| **S5 network provider contract (ADR)** | engine | new ADR: per-role ports, network/gateway capability report, validate/plan/apply/observe/verify lifecycle, stable errors, selection by name, `providers.yaml` for network providers. **ADR and spike only**; implementation after acceptance | **yes** (changes boundary) |
| **S6 ownership on remote providers** | engine | immutable ownership marker on OPNsense and Proxmox SDN, refusal to adopt by name, "no third-party pending changes" precondition before `commit`/`apply_sdn`, E2E for `NetworkGateway`/`NetworkZone` | no; coordinate with the active "Proxmox API interface assessment" session (worktree `sdn-rest`) |
| **C1 single writer in the control plane** | control plane | P0-7, double realization, `compute_sched` outside the funnel, unpersisted binding, leaderless reconciler, orphaned/duplicated Proxmox client, engine pin | **yes** (ownership) |

Order: S1 and S4 first (isolation); S2 and S3 in parallel (different files: `ipam.rs`/NetDef
versus `network.rs`/`httproute`/mgmt — **S2 and S3 both touch `infra.rs`**, so S3 rebases
onto S2); S5 after S1 (the ADR cites the corrected IR); S6 after S5 is accepted on the
contract side, but the ownership marker can start already.

## 8. First proposed vertical increment

"Isolated private network on the Linux provider, fail-closed": `kind: Network` + two workloads in
different namespaces + egress `NetworkPolicy` set to `deny` + ingress `NetworkAccessRule`.
Exit criterion: `stack apply` → `stack plan --detailed-exitcode` = 0; allowed traffic passes,
cross-namespace and denied egress **don't** pass (measured with packets, nft counters read);
`stack destroy` → zero leases, zero `@fwmap`/`@dlxns` elements, zero NetDef. This is exactly what
S1+S2+S3 close out, and it passes the battery check (`scripts/e2e.sh`) and a chaos scenario.

## 9. Validation commands and environments

```bash
cargo test -p delonix-sdn -p delonix-net-rules -p delonix-opnsense -p delonix-proxmox -p delonix-compute
python3 scripts/arch_fitness.py && python3 scripts/lang_ratchet.py
OUT=/tmp/dlxn DELONIX_ROOT=$OUT/root DELONIX_NET_RUNTIME_DIR=$OUT/net scripts/e2e.sh   # BOTH roots
scripts/chaos.sh                                                                       # isolation
```

Result measured in this audit (worktree `naas-audit`, empty `RUSTC_WRAPPER=`):
**19 suites, 484 tests, 0 failures, 0 ignored** in `delonix-sdn`, `delonix-net-rules`,
`delonix-opnsense`, `delonix-proxmox` and `delonix-compute` (unit, failure-injection and
doc-tests). Live Proxmox/OPNsense tests were not run (they need the
lab). None of this proves P0-1: the existing tests don't measure cross-egress against a destination
that accepts, and that is the first test S1 writes.

Environments: development host with the two isolated roots (never the real root — there is live
production); Proxmox lab `pve`/`pve2` and an OPNsense appliance for S6; no OpenStack environment exists.

## 10. Blocking questions

1. **C1**: should the control plane start consuming the engine's ports (and drop its
   `delonix-net-*` adapters), or keep its own and have the engine act purely as the node's dataplane?
   This decides who is the writer for Proxmox SDN and OPNsense.
2. **S3/mgmt**: do `delonix-mgmt`'s network routes refuse (fail-closed) until `delonix-node-api`
   replaces them, or do they go through the registry? (ADR-0041 D4 freezes the surface.)
3. **S5**: does the network capability report go into the existing catalogue (ADR-0050, new
   `ProviderKind::Gateway`) or a separate per-role document?
4. **OpenStack**: does it stay out until an environment exists (recommended), or does the ADR-0039
   spike open up?
