# 55 — A single engine API: the measured map (ADR-0042)

Measured on `origin/main` on 2026-09-17. It is the decision basis for
[ADR-0042](../adr/0042-one-engine-api-maturity-and-docs.md) and the work list for the
coverage waves (step E). Each `delonix-mgmt` operation has to end up in one of three
columns — served by the contract, a gap the contract closes, or refused with a reason —, and
none leaves silently.

## What exists today

| Surface | Version | Form | Status |
|---|---|---|---|
| `delonix-mgmt` (`delonix serve api` → `delonix-mgmt`) | `/v1` | 32 paths, 40 hand-written operations, RPC-shaped | served; no OpenAPI, pagination, idempotency, or ETag; mutations return the CLI's stdout |
| `delonix.node.v1` contract (`proto/`) | `/v1` | ~45 resource-oriented operations (AIP), async `Operation` | **not served**; OpenAPI generated and kept honest by a CI gate (P1) |
| `http_post_json`/`http_get_auth`/`http_post_stream` (`delonix-image`) | `/v2/cli`, `/v2/studio/designs` | HTTP client for a platform's API | **zero callers in this repository**; it is a consumer's API inside the engine → leaves (step B) |
| OCI registry (`/v2/<repo>/…`), TrueNAS (`/api/v2.0`) | third-party | other parties' protocols | stay — they are not our versions |

## Maturity level (Richardson) of each surface, today

| Level | `delonix-mgmt` | `node.v1` contract |
|---|---|---|
| 0 — HTTP as transport | yes | yes |
| 1 — resources | partial: `/v1/containers/:id`, but `/v1/net/publish`, `/v1/net/dhcp/:net/:mac` are procedures | yes, `/v1/namespaces/{ns}/{kind}/{name}` |
| 2 — verbs and codes | partial: `POST …/action` with the verb in the body; errors with no single shape | yes by design (`201`/`202`/`204`, `PATCH`+`update_mask`, `DX_*`); to be proven on a server |
| 3 — hypermedia | no | no — ADR-0042 adds `links` to the contract |

## `delonix-mgmt`, operation by operation

| Operation | Contract destination | Status |
|---|---|---|
| `GET /_ping` | `GET /v1/node/health` | served in the contract |
| `GET /metrics` | `/metrics` (unversioned, Prometheus convention) | stays on the API server |
| `GET /v1/dash` | `GET /v1/node/capacity` + counts via `List` | **gap**: the count summary has no RPC; decide whether it is `GetNodeInfo` or a `Summary` |
| `GET/POST /v1/volumes`, `GET/DELETE /v1/volumes/:name` | `VolumeService` List/Create/Get/Delete | served in the contract |
| `GET/POST /v1/containers`, `GET/DELETE /v1/containers/:id` | `ContainerService` List/Create/Get/Delete | served in the contract |
| `POST /v1/containers/:id/action` (start/stop/kill/restart/pause/unpause) | `:start`, `:stop`, `:kill`, `:wait` | **gap**: `restart`, `pause`, `unpause` |
| `GET /v1/containers/:id/logs` | `ContainerService.Logs` | served in the contract |
| `POST /v1/containers/:id/exec` | `Exec` (bidirectional stream) | **gap in REST**: no HTTP mapping; WebSocket (ADR-0040 D4) |
| `POST /v1/containers/:id/reconfig` | `PATCH …/containers/{name}` with `update_mask` | served in the contract |
| `PUT /v1/containers/:id/rate` | `PATCH …/containers/{name}` (`update_mask: network.rate`) | **gap**: confirm the field on `Container` |
| `GET/DELETE /v1/images`, `POST /v1/images/pull`, `POST /v1/images/build` | `ImageService` List/Delete/Pull/Build | served in the contract |
| `GET /v1/images/scan` | `POST /v1/images:scan` | served in the contract (changes from `GET` to `POST`: a scan is work, it returns an `Operation`) |
| `GET /v1/images/sbom` | — | **gap**: SBOM as a scan resource or its own RPC |
| `GET/POST /v1/networks`, `GET/DELETE /v1/networks/:name` | `NetworkService` List/Create/Get/Delete | served in the contract |
| `POST /v1/net/attach-extra`, `DELETE /v1/net/attach-extra/:id/:idx/:ip` | `…/networks/{network}:connect` / `:disconnect` | served in the contract |
| `DELETE /v1/net/attach/:id/:ip` | `:disconnect` | served in the contract |
| `GET /v1/net/status` | `network` condition of `GET /v1/node/health` | served in the contract |
| `POST /v1/net/publish`, `DELETE /v1/net/publish/:host_port` | `Container` ports via `PATCH` (`update_mask: ports`) | served in the contract; publishing by raw IP is **refused** — it is plumbing, not a resource |
| `PUT/DELETE /v1/net/firewall/:ip` | `NetworkPolicy`/`NetworkAccessRule` Kinds | **gap**: the contract has no policy service; the raw-IP rule is **refused** |
| `PUT /v1/net/egress`, `PUT /v1/net/egress/:bridge` | egress `NetworkPolicy` | **gap** (same as the firewall one) |
| `GET /v1/net/dhcp/:net/:mac`, `GET /v1/net/dhcp6/:net/:mac`, `GET /v1/net/container-ip/:id` | addresses in `Container`/`VirtualMachine` status | **refused**: internal plumbing; the address is a resource field |
| `POST /v1/vms/:name/action` | `VirtualMachineService` `:start`/`:stop`/`:pause`/`:resume` | served in the contract (and the contract adds List/Get/Create/Delete and snapshots, which `mgmt` did not have) |

**Summary**: of the 40 operations, 28 have a served destination in the contract, 7 are gaps that the
contract closes (count summary, restart/pause/unpause, exec over REST, SBOM, bandwidth
limit, firewall and egress policy), and 5 are refused with a reason (DHCP addresses and
container IP, raw-IP rules and publishes).

## What this did not measure

- Whether `prost`/`tonic` compiles the contract as it stands (risk logged in ADR-0040, closed
  in step C).
- The completeness of the generated OpenAPI (descriptions, examples, errors per operation): the
  completeness gate (step G) measures it before it enters, so it starts from a true baseline.
