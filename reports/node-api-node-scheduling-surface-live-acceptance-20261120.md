# A6 live acceptance — node scheduling surface (labels / taints / cordon / capacity), ADR-0101

**Date:** 2026-11-20 · **Branch:** `feat/node-api-mtls-transport` · **Sub-step:** A6 (last of fleet-operability Phase A)

Proves ADR-0101: the node's scheduling write surface on `node/v1` (`SetNodeLabels`/`SetNodeTaints`/
`SetSchedulability`) **and** the declarative `kind: Node` apply — both over the one shared
`delonix-model::node_meta` record — with cordon **enforced** on workload create (DX-5003) and
`Capacity` enriched with GPU / IPs / overcommit by the `unmeasured` doctrine.

## Setup

- Node API served over **TCP + mutual TLS** on the host at `192.168.122.1:29443` (ADR-0096 transport,
  ADR-0097 CA), with the A3 authorization policy (`control-plane` CN granted `*`/`*`) and audit log.
  The `serve` execs the sibling `delonix-node-api` binary — both rebuilt (`cargo build -p
  delonix-node-api-bin -p delonix-bin`).
- PKI: one CA (`CN=delonix-node-ca`), a server leaf (`SAN IP:192.168.122.1`), a client leaf
  (`CN=control-plane`), via `openssl`.
- Driver: `/home/walter/.cache/delonix-a6/{proof.sh,vmproof.sh}` (a persistent dir — the session
  scratchpad under `/tmp` is wiped between turns).

## Proof 1 — `node/v1` imperative write surface + cordon enforcement (host → bridge IP, mTLS)

Every call is `curl` over mutual TLS with the `control-plane` client cert. HTTP status and the
decisive fields:

| # | call | result |
|---|------|--------|
| 1 | `GET /v1/node` | **200** — fresh node: `generation 0`, `unschedulable false`, `labels {}`, `taints []` |
| 2 | `POST /v1/node:setLabels {zone,tier}` | **200** — `generation 1`, `labels {tier:edge, zone:lis1}` |
| 3 | `POST /v1/node:setTaints [dedicated=ml:NoSchedule]` | **200** — `generation 2`, taint present |
| 4 | `POST /v1/node:setSchedulability {unschedulable:true, reason:maintenance}` | **200** — `generation 3`, `unschedulable true` |
| 5 | `POST /v1/namespaces/default/containers` (CORDONED) | **409** — `"conflict: the node is cordoned (unschedulable): maintenance"` (DX-5003) |
| 6 | `POST /v1/node:setSchedulability {unschedulable:false}` | **200** — `generation 4`, `unschedulable false` |
| 7 | `POST /v1/namespaces/default/containers` (SCHEDULABLE) | **200** — the cordon gate PASSES |

The cordon is **enforced at the node**: a workload create is refused with a distinct code while the
node is cordoned, and accepted once uncordoned — not an advisory flag. Each write bumps `generation`
and `observed_generation` moves with it (single-phase), confirming the record round-trips.

## Proof 2 — `Capacity` enrichment, `unmeasured` doctrine

`GET /v1/node/capacity` **before** any declarative config:

```
devices: []           ip_pools: []
unmeasured: [ cpu_millis_allocatable, memory_bytes_allocatable, pids_allocatable,
              devices.gpu, overcommit.cpu, overcommit.memory ]
```

GPU and overcommit are **named in `unmeasured`**, never a misleading zero: this host has no NVIDIA
driver (`/proc/driver/nvidia` absent) and no overcommit is declared yet — exactly the partial
measurement the owner accepted.

## Proof 3 — declarative `kind: Node` + `kind: IPPool` write the SAME record

```
$ delonix stack apply -f ippool.yaml   → ippool/edge: 254 address(es), announce local, 0 leased
$ delonix stack apply -f node.yaml     → node/a6node: scheduling config applied (schedulable=true)
```

- `GET /v1/node` after the apply: **`generation 5`** — it continued from the imperative `generation 4`,
  proving the declarative apply and the `node/v1` RPCs write the one shared record (not two).
- `GET /v1/node/capacity` after the apply:
  - `ip_pools: [ { name: edge, addresses_total: 254, addresses_allocated: 0 } ]` — IPs **measured**
    from the declarative IPPool ledger.
  - `overcommit: { cpu: 2.0, memory: 1.5 }` — now **measured** (set by `kind: Node`), and
    `overcommit.cpu`/`overcommit.memory` are **gone from `unmeasured`**, while `devices.gpu` remains
    (still no GPU). The doctrine holds both ways: declared ⇒ measured, absent ⇒ named.

## Proof 4 — authorization + audit (A3 integrates automatically)

`audit.jsonl` — every mutating call (and the create) audited, by cert identity, over the TLS transport:

```
NodeService.SetNodeLabels        | allow | control-plane | rest | ns=-
NodeService.SetNodeTaints        | allow | control-plane | rest | ns=-
NodeService.SetSchedulability    | allow | control-plane | rest | ns=-
ContainerService.CreateContainer | allow | control-plane | rest | ns=default
NodeService.SetSchedulability    | allow | control-plane | rest | ns=-
ContainerService.CreateContainer | allow | control-plane | rest | ns=default
```

The three new `SetNode*` RPCs are authorized by CN and audited with **no extra wiring** beyond listing
them in `is_mutating` (ADR-0098 A3) — the REST transcode routes and the authz operation names generate
from the proto.

## Proof 5 — cross-machine (client is a delonix VM)

Client = a freshly-created delonix VM `a6node` (`192.168.122.158`, local golden image
`delonix-vm-k8s:1.36`), running a small **python3 mTLS client** against the host node API at
`192.168.122.1:29443` with the `control-plane` client cert. The sequence, FROM the VM:

| step | result (from the VM) |
|------|----------------------|
| A `GET /v1/node` | **200** — `generation 0`, `unschedulable false`, `labels {}` |
| B `SetSchedulability cordon` | **200** — `generation 1`, `unschedulable true` |
| C `CreateContainer` (cordoned) | **409** — `"conflict: the node is cordoned (unschedulable): vm-drain"` |
| D `SetSchedulability uncordon` | **200** — `generation 2` |
| E `SetNodeLabels {rack:r1}` | **200** — `generation 3`, `labels {rack:r1}` |
| F `GET /v1/node` | **200** — `generation 3`, `labels {rack:r1}` |

The host `audit.jsonl` records every mutating call with **`remote=192.168.122.158`** (the VM's
address) — proving the writes and the cordon gate were driven **across a real network hop from a
separate machine**, authorized by cert identity and audited:

```
NodeService.SetSchedulability    | allow | remote 192.168.122.158:57442
ContainerService.CreateContainer | allow | remote 192.168.122.158:57452   (the 409 — cordoned)
NodeService.SetSchedulability    | allow | remote 192.168.122.158:57462
NodeService.SetNodeLabels        | allow | remote 192.168.122.158:57474
```

(The delonix VM golden image was reused from the local store — the base VM image *registry*
`ghcr.io/ngolacloud/delonix-vm-base` was unreachable, an infra condition unrelated to A6. `--wait`
returns when the VM answers on the network; sshd comes up a few seconds later via cloud-init, so the
client is driven after a short readiness retry.)

## Gates

- Builds green: `delonix-node-api-bin` + `delonix-bin`.
- Tests: `delonix-node-api` lib **174 pass**; `delonix-bin` **1736 pass, 3 failed** — the 3 are the
  pre-existing base reds (`provider`×2 + `schema`×1 — the latter is the `loja.exemplo.ao`→
  `shop.example.com` drift from the English-source campaign; A6 regenerated `docs/schema/v1/delonix.json`
  to add the `Node`/`NodeSpec`/`OvercommitSpec`/`TaintSpec` definitions).
- `arch_fitness` vs the A5 reference (`126957/25/119/1`): `env_writes 25`, `library_prints 119`,
  `raw_error 1` **byte-identical**; `cli_context_lines` **127000 (+43)** — the irreducible dispatch
  for wiring a new declarative Kind into `cmd/stack.rs`/`cmd/schema.rs` (both context-mapped). The new
  `cmd/node.rs` is classified `delonix-cli` (the node-API command family, beside `node_enroll`/`serve`),
  so its ~290 lines do not count. No new `cli_destination_map`/rule failures beyond the base
  `cmd/distroless`+`cmd/help`.
- `lang_ratchet`: docs **382** = A5 reference (the ADR is English; `pt.po` translations for DX-5003 do
  not count as docs).
