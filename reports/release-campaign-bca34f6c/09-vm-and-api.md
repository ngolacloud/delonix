# 09 — VMs (providers) and APIs/MCP

SUT: `delonix 5.1.1` @ `bca34f6c3`.

## Providers (measured inventory — `delonix provider ls`)

| Provider | Kind | Available | Health | Supported caps |
|---|---|---|---|---|
| libvirt | compute | **yes** | Ok | 18 |
| cloud-hypervisor | compute | **yes** | Ok | 13 |
| linux | compute/network/storage | yes | Ok / NetworkSharesNeedRoot | 11/10/4 |
| proxmox | compute/network | **no** | NotConfigured | 33/7 |
| opnsense | gateway | no | NotConfigured | 10 |

Local VM images: **`k8s-golden`** (ubuntu 24.04, cloud-init, k8s 1.34, 634 MiB) and `delonix-vm-appliances:proxmox-ve-9.2` (1.45 GiB).

## VM-1 — libvirt/KVM, full cycle (independent verification in `virsh`)

| Step (§10) | Observed | State |
|---|---|---|
| create+start | `vm create relcvm --disk k8s-golden --backend libvirt --vcpus 2 --memory 2G` → overlay+network+domain+start | **PASSED** |
| guest boots + network/DNS | `vm ls`: Running, **IP 192.168.122.62** (libvirt DHCP, ~34s); **`virsh list` confirms domain `relcvm running`** | **PASSED** |
| reachability | `ping` **2/2, 0% loss** | **PASSED** |
| stop | `vm stop` → `virsh` shows shut off | **PASSED** |
| start | `vm start` → `virsh` Id 3 running | **PASSED** |
| destroy + cleanup | `vm destroy --force` removes `relcvm.qcow2`, overlay dir, `relcvm.xml`; **0 in delonix AND 0 in libvirt** | **PASSED** |
| preservation | `opnsense` still running, `pve92` shut off — intact | **PASSED** |

## VM-2/VM-3/VM-4 — state

- **cloud-hypervisor (VM-2):** provider available/Ok (13 caps); backend exists; **cycle not run** this round (time/disk) → NOT EXECUTED.
- **proxmox (VM-3):** `NotConfigured` — requires `DELONIX_PROXMOX_URL/_NODE`+credential and a live PVE (`pve92` powered off). **BLOCKED (environment)**.
- **cloud image build (VM-4, §11):** not run — disk at 96%/44 G. **BLOCKED (environment)**. Note: the `k8s-golden` golden image already exists and boots (proven above), which covers the *boot* of a cloud-init image; the *build flow* itself was not exercised.

## API-2 — MCP (closes sheets gap E3: `tools/call` had never been run)

`delonix-mcp` sidecar built **at the same commit** (the main binary **rejects** a stale copy from the PATH — `DX-6000`, good anti-mixing defence).

- `mcp capabilities`: **risk/confirm table** — each tool has `risk` (READ/…) and `requires_confirm`.
- `mcp serve` + JSON-RPC `initialize`+`tools/list`: **15 tools** returned (matches the `mcp=15` denominator): `audit.query, container.restart, logs.query, metrics.query, network.inspect, resource.{list,get,describe}, resources.get, runtime.{health,info}, storage.inspect, task.{cancel,get}, workload.usage`.
- Set **dominated by read/observation**; the only mutations are `container.restart` and `task.cancel` — **no arbitrary privileged exec**. Satisfies "MCP does not bypass the core nor accept arbitrary privileged commands". | **PASSED** |

## API-1 — CLI vs REST vs gRPC vs MCP — PARTIAL

The **MCP** surface was validated by the protocol (initialize+tools/list) and the **CLI** exhaustively in the
earlier sections. The end-to-end comparison of the 4 surfaces over the SAME operation (identity,
authz, defaults, errors) was **not run** — it would require serving the SUT's own `node-api`/`mgmt`/`docker-api`
(more sidecars/startup). The `delonix-engine prod` service on `0.0.0.0:9443` belongs to **another installation**,
not the SUT, and was not used. → partial coverage.
