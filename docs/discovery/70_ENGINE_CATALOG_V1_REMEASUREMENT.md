# 70 — Catalog v1 and engine gaps: re-measurement with evidence

- **Measured at:** `develop == main == 64a5953a` (2026-11-03). The prompt's
  `0f172f5` point (#958, 2026-10-29) is an ancestor of this one — the
  measurement reflects TODAY's state, a few commits ahead.
- **Method:** 6 parallel reading fronts over `crates/`, `bins/`, `proto/`,
  `docs/`, `scripts/e2e.sh`, `scripts/chaos.sh` and
  `docs/providers/capability-matrix.md`. Each point has a **CLOSED / PARTIAL /
  OPEN** verdict with `file:line` or section.
- **Rule:** CLOSED items are skipped in the build phases. A PARTIAL says
  what is missing. This is the table that Phase 1 of the prompt asks for as
  Deliverable #1.

> **Baseline warning (out of scope for this campaign, but blocking).** The CI
> run at tip `64a5953a` is **FAILURE**: `contract gate`, `generated-docs`
> (handbook), `test` (arm64, `cargo test --workspace`) and
> `cli-execution-ratchet`. The last green battery is `b5e153b3` (Oct 11,
> 1345/0/13 + chaos 53/0/2). The non-negotiable «green gates» rule has to be
> restored BEFORE any phase of this campaign closes — otherwise the campaign
> inherits a red baseline and masks its own regressions.

---

## Corrections to the prompt's premises (measured, not assumed)

1. **Groups: there are 9, not 7.** Besides `core/compute/networking/gateway/
   storage/artifact/infrastructure`, there are also `security.delonix.io`
   (`RuntimePolicy`) and `gitops.delonix.io` (`GitOpsSource`). All in
   `v1alpha1`; there is no `v1` at all despite the name «catalog v1».
2. **The per-Kind gRPC services are ALREADY served.** The prompt (C1) assumes
   that `VirtualMachineService/ContainerService/PodService/ImageService/
   StackService` «are in the `.proto` but not served». Measured: the **10
   services are registered and implemented**
   (`delonix-node-api/src/service.rs:948-993`), since Sprint 13. The premise is
   stale — what C1 proposes is to *replace* a served contract, not finish one
   that was never served.
3. **There is no «Pod/VM namespace exception».** The prompt (A3) wants to «end
   the exception ADR-0069 documented». Measured: `VirtualMachine/Container/
   Pod` are already `Namespaced::Always` (`kinds.rs:489`). ADR-0069 Pending #2
   decided something else — **a VM's name is unique on the NODE** (conflict
   across namespaces), not «unique within the namespace». That is the real
   contradiction to resolve (see the list below), not a namespace-field
   exception.
4. **Bug N3 (`set_network` on VM creation) is CLOSED**
   (`delonix-node-api/src/lib.rs:128-143` + regression test
   `tests/vm_network_composition.rs`). C1 does not need to fix it.
5. **Two lines of `capability-matrix.md` are ahead of / false against
   develop** — they violate the «only supported with a live test» rule:
   - `storage.ceph` (line ~198): says «driver the engine does not ship» when
     the `delonix-provider-ceph` crate **already ships** in develop (PR
     #960). Fix the text (the external Ceph cluster remains a legitimate
     requirement).
   - `net.ipv6` (line ~146): marked **supported** when the v6 dataplane is
     still scaffolding (`delonix-sdn/src/infra.rs:46` «the dual-family table
     never governs a v6 packet yet») — the «governed ON» lives on a branch
     not yet integrated. **Downgrade to experimental/partial** until v6
     governs a packet in develop.

---

## Part A — Declarative foundation

| Point | Verdict | Evidence | Missing |
|---|---|---|---|
| A1 diff from the schema + `x-delonix-update` + per-field gate | **OPEN** | 0 `x-*` annotations in `docs/schema`; live/reboot/cold classification is a hand-written table per Kind (`reconcile.rs:277-437`, `grow_only_fields:451`); `_ => &[]` arm (`:436`) silences the unclassified | per-field annotation in the schema + a diff that reads it + a gate that refuses a field with no class |
| A2 common metadata/status | **OPEN** | `Metadata{name,ns,labels,annotations}` ok (`manifest.rs:20-34`); **no** `generation/observedGeneration/conditions/outputs/storedVersion`; the current `Condition` (`condition.rs:5-11`) is a prerequisite, with no `lastTransitionTime` or Ready/Progressing/Degraded taxonomy | `status` block + `generation` written by the engine + uniform conditions + `stack wait` reading only `Ready` |
| A3 two scopes | **CLOSED** (model) | enum `Namespaced{Never,Always,PerDocument}` (`kinds.rs:236`); VM/Container/Pod=Always, App=Never, Volume=PerDocument | v1 changes the map (Application namespaced; see the VM-name contradiction) |
| A4 graph order | **OPEN** | FIXED order from the FACTS table (`kinds.rs:275-304`) via hardcoded `run_layers_inner` (`stack.rs:2442-2523`), 1 Kind at a time; no `dependsOn/outputRef/toposort`; `validate_graph` (`stack.rs:3496`) only validates existence; reverse destroy is CLOSED | a real reference graph, per-level parallelism, cycle detection with a path |
| A5 lifecycle controls | **PARTIAL** | 3-way with `delonix.io/last-applied` CLOSED (`reconcile.rs:63,605-642`); `plan -o json` CLOSED but `apply` consumes a manifest + `--plan-digest` (not a plan file); `planDigest` only provider-backed (`reconcile.rs:235`); `protect`/`ignore-fields`/`drift`/lock **OPEN** (0 occurrences) | `stack apply plan.json`; protect/ignore-fields; **drift** (contradicts a recorded decision); per-stack lock |

## Part B — Catalog (24 v1 Kinds)

Current catalog: **27 Kinds** in a single table
(`crates/contexts/delonix-stack/src/kinds.rs`), all `v1alpha1`, read by
`api-resources`/schema/parser/reconciler (no second list). **There is no
`delonix manifest convert`** nor any version-conversion / storage-version
machinery. Deprecation modeled as `enum Form` (`kinds.rs:154`):
`Sunset(successor)` announces, `Sugar`/`Compat` downgrade, merges warn, pure
renames (`KIND_ALIASES`) are a silent alias.

Of the 24 v1 Kinds: **14 already exist, 4 are partial, 6 do not exist.**

| v1 group | Kind | Verdict | Origin / note |
|---|---|---|---|
| compute | Application | **PARTIAL** | derives from `App` (today in **artifact**, only builds the image) + `Workload`/`Pod`/`Container` that run; no «Application» Kind |
| compute | Job | **DOES-NOT-EXIST** | no Job Kind; no `action` or `template` |
| compute | VirtualMachine | **EXISTS** | `kinds.rs:479` |
| networking | Network | **EXISTS** | `kinds.rs:345` |
| networking | NetworkRoute | **EXISTS** | `kinds.rs:359` |
| networking | Router | **DOES-NOT-EXIST** | closest is `NetworkGateway` (`kinds.rs:603`) |
| networking | IPPool | **EXISTS** | `kinds.rs:624` |
| networking | IPAddress | **DOES-NOT-EXIST** | only `IPPool` (reservation ledger) |
| networking | NetworkPolicy | **EXISTS** | `kinds.rs:572` |
| networking | Service | **EXISTS** | `kinds.rs:544` (DNS round-robin, ADR-0032) |
| networking | Gateway | **PARTIAL** | **name collision**: the current `Gateway` is the former `Tunnel` edge resource (`kinds.rs:652`), semantics ≠ Gateway API |
| networking | HTTPRoute | **EXISTS** | `kinds.rs:638` (today in the **gateway** group) |
| storage | StoragePool | **EXISTS** | `kinds.rs:397` |
| storage | Volume | **EXISTS** | `kinds.rs:413` |
| storage | Bucket | **PARTIAL** | derives from `ObjectStorage` (`kinds.rs:429`) |
| storage | Snapshot | **DOES-NOT-EXIST** | VM snapshot is imperative-only |
| core | Secret | **EXISTS** | `kinds.rs:327` |
| core | Config | **DOES-NOT-EXIST** | no ConfigMap-like resource; non-secret data only via `Secret` |
| core | Image | **EXISTS** | today in **artifact** (`kinds.rs:447`) |
| core | Stack | **EXISTS** | `kinds.rs:721` |
| core | GitOpsSource | **EXISTS** | today in **gitops** (`kinds.rs:671`) |
| core | Node | **DOES-NOT-EXIST** | only the `NodeService.GetCapacity` RPC, no Kind |
| policy | RuntimePolicy | **EXISTS** | today in **security** (`kinds.rs:310`) |
| cluster | KubernetesCluster | **EXISTS** | today in **infrastructure** (`kinds.rs:736`), `in_stack:false` |

Group relocation even among the «exists» ones: Image/App→core, RuntimePolicy
security→policy, KubernetesCluster infrastructure→cluster, GitOpsSource
gitops→core, HTTPRoute/Gateway gateway→networking.

Conversions of current Kinds (origin → v1 destination): `Container`→
`Application` (Sunset already announced), `Pod` (instance)→an `Application`
instance, `App`→`Application.containers[].build` + `Image`, `Workload`→
`Application`/`VirtualMachine`, `NetworkAccessRule`→`NetworkPolicy`,
`Dependency`→`dependsOn`/`NetworkPolicy`, `NetworkZone`→`Network`+provider
(Sunset already), `NetworkGateway`→`Router`, `ObjectStorage`→`Bucket`,
`Ingress`→`HTTPRoute` (Compat already), `Gateway` (tunnel)→`Gateway` tunnel
class, `SystemContainer`→`proxmox.delonix.io/SystemContainer`.

## Part B — Convergent KubernetesCluster (2 CLOSED / 7 PARTIAL / 6 OPEN)

| # | Capability | Verdict | Evidence |
|---|---|---|---|
| 1 | 3 modes kind/vm/ssh | **CLOSED** | `cluster.rs:138,630,2454` |
| 2 | declarative/idempotent convergence | **PARTIAL** | idempotent per step (`cluster.rs:3125-3128`); no diff/removal/state; no `plan` |
| 3 | HA CP + one-by-one join with healthy etcd | **PARTIAL** | `cluster.rs:2891-2897` + ADR-0085; no etcd quorum gate between joins |
| 4 | stacked vs external etcd | **PARTIAL** | `cluster.rs:92,663`; external only `ssh` (or `vm` via flag), never `kind` |
| 5 | kube-vip/HAProxy/external endpoint | **PARTIAL** | `lb.rs` + ADR-0085; HAProxy+external yes, **kube-vip NO** (ADR-0085 rejected VIP) |
| 6 | scale CP/workers + remove an etcd member | **OPEN** | no `scale`, no `nodePools[]` (only a single `workers`), etcd removal «to be done» |
| 7 | minor-by-minor upgrade, snapshot, one-by-one | **PARTIAL** | `cluster.rs:2367,2380`; only `ssh`, no prior snapshot, no version-skip gate |
| 8 | backup/restore (etcd+PKI+config), CA for a new cluster | **PARTIAL** | `etcd_backup.rs:9-26`; only stacked etcd, in-place single CP; no PKI/config/manifest; no restore to a new cluster |
| 9 | worker self-healing | **OPEN** | no NotReady→replace loop |
| 10 | resume after `kill -9` (operations log) | **PARTIAL** | via idempotent stateless re-execution; no op-ledger |
| 11 | pinned CNI profiles + offline | **CLOSED** | `net_profiles.rs:13,32` + `--cni-offline`; CNI images not bundled |
| 12 | `x-delonix-update` per field | **OPEN** | 0 occurrences |
| 13 | CNCF/Sonobuoy conformance | **OPEN** | only CRI's `critest` |
| 14 | secure by default (PSA/encryption/audit/…) | **OPEN** | `kubeadm_config.rs` only writes etcd |
| 15 | subcommands | **PARTIAL** | missing `plan`, `scale`, `certs`; the rest exists (some in only one mode) |

## Part C — Node contract

| Point | Verdict | Evidence |
|---|---|---|
| 10 services served (not just proto) | **CLOSED** | `service.rs:948-993` + impls `:135-937` (only `VM.Console` and `Image.Push/Build/Scan` UNIMPLEMENTED) |
| generic `ResourceService` (Apply/Plan/Get/List/Watch/Delete over any Kind) | **OPEN (by design)** | absent from the proto; the contract gate (`AGENTS.md:148,159-166`) **enforces** «one request per RPC» + explicit identity, **never** a generic `ResourceMeta` → C1 contradicts this |
| Exec / Logs | **CLOSED** | `service.rs:910-929` → `container_exec`/`container_logs` |
| VM console / Attach / port-forward | **OPEN** | `Console` UNIMPLEMENTED (`service.rs:712`); Attach/port-forward not even in the contract |
| NodeService / OperationService (Get/Watch/Cancel) / WatchEvents | **CLOSED** | `service.rs:135-196,726-781,174-187` |
| Watch with offset resume | **PARTIAL** | only temporal `since` (`node.proto:114`); no generic offset |
| Socket activation + `.socket` + idle-exit | **CLOSED** | `dist/delonix-node-api.socket`; `lib.rs:169-183,264-279`; tests `idle_exit.rs`/`socket_activation.rs` |
| typed `ErrorDetail` on ALL paths | **PARTIAL** | REST gives full problem+json (`service.rs:1385`); gRPC (`network_ops.rs:73-74`) only a DX trailer — the proto (`common.proto:113`) promises more than gRPC delivers |
| in-process vs CLI (ADR-0040 D5) | **PARTIAL (by design)** | reads/mutations in-process; only launch/exec/pod-create re-exec `delonix` (`container_exec.rs:6-11`) because `setns`/`clone` must be single-thread |
| bug N3 (`set_network` on VM creation) | **CLOSED** | `lib.rs:128-143` + `tests/vm_network_composition.rs` |
| **C2** authz by namespace/verb/Kind (ADR-0003) | **OPEN** | only `SO_PEERCRED` uid-equality (`lib.rs:295`) + 0600 socket; ADR-0003 **accepted but the capability gate is NOT built** (the existing `enum Capability` is the provider catalog, ADR-0050) |

## Part D — Capabilities

| # | Item | Verdict | Evidence |
|---|---|---|---|
| D1a | `RuntimePolicy` strict/baseline/privileged profile per `namespaceSelector` | **PARTIAL** | Kind exists (`policy.rs:425`); no `profile` or `namespaceSelector`; node-level ceiling; only `baseline` (`policy.rs:488`) |
| D1b | CRI capabilities ceiling, refuse without a ceiling | **CLOSED** | `cap_ceiling.rs:84` (default = safe set; malformed refuses startup) |
| D1c | Secret only on tmpfs, no persistent disk | **CLOSED** | `delonix-linux/src/lib.rs:3468` (0600 tmpfs in the ns) |
| D1d | root fallback (ADR-0062) visible | **CLOSED** | `lifecycle.rs:998,2503` (`userFallbackToRoot` in status) |
| D2 | per-instance container in a microVM with the container API + RuntimeClass + measurements | **OPEN** | `type: microvm` downgrades to a full VM (`workload.rs:15`); `runtime_handlers=vec![]` (`runtime_svc.rs:208`); only container density is measured |
| D3 | `storedVersion` + state migration tested in CI | **OPEN** | 0 `storedVersion`; only the `56_P4_D6_...` spike |
| D4a | VXLAN overlay + WireGuard between zones (ADR-0084) | **CLOSED** (ADR Proposed) | `delonix-sdn/src/infra.rs:8289`, `wg.rs`; live `g4-overlay-forwarding.md` 0% loss, 2 nodes |
| D4b | multi-node CNI / explicit refusal (N15) | **CLOSED** | `cni:default`→Cilium (`cluster.rs:305`); `none`→named NotReady (`:159`) |
| D4c | movable 1:1 `IPAddress` NAT between nodes | **OPEN** | Kind does not exist |
| D4d | Service type:LoadBalancer (ADR-0066) + IPv6 (0065/0082) | **PARTIAL** | LB = **single-node** L4 VIP (`lb.rs:1`), not cross-node; IPv6 scaffolding, v6 refused (matrix ahead of reality) |
| D5a | ADR-0081 ceph-native implemented | **CLOSED** (ADR Proposed; matrix stale) | `delonix-provider-ceph` crate in develop; `storage.rs:13`; MicroCeph 19.2.3 live |
| D5b | StoragePool ceph-rbd/cephfs + ADR-0067 pools (N20) | **CLOSED** | `storage.rs:215-221`; dir/btrfs/zfs/lvm-thin + ceph |
| D5c | `Volume.size` grows hot (N16) | **CLOSED** | `volume.rs:336` |
| D5d | scheduled/retained `Snapshot`→Bucket via `Job action:CreateSnapshot` | **OPEN** | ad-hoc pieces (`vm.snapshot`, etcd backup); no Bucket target, no Job Kind, no scheduling |
| D6 | CPU/mem/disk/NIC hotplug ≤ max (imperative) | **CLOSED** | `vm.rs:1388`; matrix 49-56; DX-8504 (`codes.rs:1532`) |
| D6 | …via manifest (reconciler, ADR-0068 D9) | **OPEN** | `reconcile.rs:323,451` still plans a Replace |
| D6 | guest agent on the 3 backends | **PARTIAL 2/3** | libvirt/proxmox live; CH has no virtio-serial channel (`:538`, structural) |
| D6 | cold migration (N12) | **CLOSED** | matrix L110; `e2e.sh:6497/6594`; proxmox live |
| D6 | boot from an existing shared disk | **OPEN** | root is always a fresh overlay (`local_ports.rs:258-310`); only extra disks reference a volume |
| D6 | RBD lock fencing + blocklist + split-brain test | **OPEN** | no code; delegated to an external manager |
| D6 | live migration out of scope (ADR-0031) | **CONFIRMED out** | CH `:509`; only Proxmox delegates to PVE |
| D7 | CRI re-measured with critest (N6) | **CLOSED** 84/104 ≈ 80.8% | `docs/cri-conformance.md` (v1.36.0, 2026-10-24) |
| D7 | CRI ≥95% (N26) | **OPEN** (~14 pp) | 5 real gaps (2 already fixed on an unintegrated branch) |
| D8 | `Node` Kind capacity/allocatable/allocated/overcommit/events | **OPEN** | only the `GetCapacity` RPC (`node.proto:80-102`); no allocated/IPs/GPU, no events |
| D8 | `unschedulable` drains with disruption | **OPEN** | 0 in the contract; only a `kubectl` wrapper (`cluster.rs:2141`) |
| D9 | daemonless SLIs + observe/diagnose | **OPEN** | `engine-slis-2026-10-12.md` «nothing wired»; no `/metrics` on node-api |
| D10 | init generates Stack App+Svc+Gateway+HTTPRoute+Secret | **OPEN** | generates compose + `Container`/`Stack` |
| D10 | rust + DORM (migrations+outbox) | **OPEN** | `rust/src/memstore.rs:12` (in-memory) |
| D10 | gRPC+REST buf / LRO / Idempotency-Key / JetStream | **OPEN** | ADR-0061 D6 «gRPC not generated»; RabbitMQ, not NATS |
| D10 | signed OCI templates (`-t oci://`) | **OPEN / out of contract** | `scaffold.rs:25-27` embedded; ADR-0061 D2 «no network at generation time» |
| D11 | manifest/converter/diff fuzzing | **PARTIAL** | `fuzz/` 6 targets (input parsers) + CI; does not cover manifest→diff |
| D11 | scanner CVE feed | **PARTIAL** | OSV parser exists (`scanner lib.rs:525,632`); embedded DB = 5 placeholders, no automatic sync |
| D11 | CLI coverage (N11) | **95.6%** (258/270) but ratchet X at the tip | `RELEASES.md:318`; `cli-execution-ratchet` red at 64a5953a |
| D11 | clean battery at the tip (N30) | **OPEN at the tip** | green at `b5e153b3`; `64a5953a` CI FAILURE |

October gaps confirmed **closed**: etcd backup (`etcd_backup.rs`),
`UpdateContainerResources` (`runtime_svc.rs:333`), add+remove hotplug on
CH/libvirt, cold migration on all 3 backends — several well beyond
ADR-0068's phase plan.

---

## Contradictions with recorded decisions (the prompt is the owner's new will; each one needs an ADR)

By rule #3 of the prompt, each contradiction below has to be written into an
ADR that states what it replaces:

1. **Drift detection.** `AGENTS.md:1246,1691` + ADR-0069 decide **not** to do
   drift («an orchestrator's job, with controllers, deliberately»). A5 asks
   for `stack drift` with a report|correct policy. → the foundation ADR
   replaces that decision.
2. **VM/instance name unique within the namespace.** ADR-0069 Pending #2
   decided the name is unique **on the node** (conflict across namespaces).
   A3 asks for uniqueness **within the namespace** (two namespaces can each
   have their own VM named `X`). → the catalog ADR replaces ADR-0069 #2.
3. **kube-vip.** ADR-0085 **rejected** keepalived/VIP. The v1
   KubernetesCluster wants kube-vip (ARP/BGP) as the default endpoint. → the
   cluster ADR replaces ADR-0085's endpoint decision.
4. **Convergent KubernetesCluster.** ADR-0069 D7 put `KubernetesCluster`
   **outside** `stack apply` (`in_stack:false`, «remote SSH procedure»). The
   prompt wants a convergent resource. → the cluster ADR replaces that
   classification.
5. **Generic `ResourceService` vs «one request per RPC».** The contract gate
   (`AGENTS.md:148,159-166`) enforces explicit service/identity and
   prohibits a generic `ResourceMeta`. C1 proposes exactly a generic service
   over any Kind. → the resource contract ADR replaces the rule and **names
   the exception to `buf breaking`** (the per-Kind services are withdrawn;
   since they are served, there is a client warning, but on the node they
   are consumed by the CLI/agent itself — the ADR documents the migration
   window).

---

## Phase 1 ADR roadmap

| ADR | Title | Replaces / builds on | State in this delivery |
|---|---|---|---|
| **0086** | Kind catalog v1 — six groups, `v1beta1`, conversion machinery | ADR-0069 D7, ADR-0020 (names/groups), ADR-0069 #2 (VM name) | **drafted in this phase** |
| **0087** | Generic resource contract on the node socket | contract gate (one-request-per-RPC), per-Kind services; names the `buf breaking` exception; implements ADR-0003 (namespace authz) | **drafted in this phase** |
| 0088 | Declarative foundation — schema-driven diff, status/conditions, dependency graph, lifecycle controls (drift) | AGENTS.md «no drift», extends ADR-0007/0019/0069 D1 | decision outline ready; draft at the start of Phase 2 |
| 0089 | KubernetesCluster as a convergent resource | ADR-0069 D7 (`in_stack:false`), ADR-0085 (endpoint/kube-vip) | decision outline ready; draft at the start of Phase 10 |
| 0090 | Capability amendments — microVM containers, state versioning, inter-node IPAddress/LB, shared-storage fencing, Node Kind, SLIs | extends ADR-0006, closes ADR-0081; new for Node/fencing/SLIs | decision outline ready; draft per chapter across Phases 7–11 |

0088–0090 keep the decision outlined here and get drafted at the start of
the phase that builds them (the prompt delivers by phases), so as not to fix
detail before each one's implementation is measured. 0086 and 0087 — the ones
Deliverable #2 names explicitly and that all following phases depend on —
are drafted already.
