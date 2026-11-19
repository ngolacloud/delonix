# Delonix 0.1.x — Production support contract (§3)

**Candidate:** `delonix 0.1.0`, capability catalog `1.4.0`, commit
`8ab783dd` (= `origin/develop`). Fix line: `v0.1.1`.
**Measurement date:** 2026-10-29. **Living source of truth:** `delonix provider ls`
(measured on this host), `delonix provider describe <id> -o json` and
`delonix provider matrix` (declared view) — this document is the **support
classification layer** on top of that matrix, not a parallel copy.

> **0.x — contract NOT stable.** This is a pre-1.0 line. The surfaces
> (CLI, JSON, gRPC/node.v1, `*.delonix.io` schemas, persisted state) **may
> change between minor versions** with a migration note; there is no stability
> guarantee of a 1.0. See `docs/cli-stability.md` and `VERSIONING.md`. A
> capability marked SUPPORTED guarantees the *behaviour within the validated scope*, not
> the immutability of the contract.

## 1. Validated scope (where these classifications hold)

| Axis | Validated |
|---|---|
| OS / kernel | Linux `7.0.0-38-generic` |
| Architecture | `x86_64` (`aarch64` and `x86_64-v3` binaries published, **not** re-validated here) |
| Mode | **rootless-first** (daemonless), delegated `cgroup2` = yes; rootful supported but less exercised in this measurement |
| Engine | Delonix 0.1.0, state root `~/.local/share/delonix` |
| cloud-hypervisor | **v52.0** — available, health Ok |
| libvirt / QEMU | **libvirt 10.0.0 / QEMU 8.2.2** — available, health Ok |
| proxmox | **NotConfigured** on this host (no live PVE target) |
| opnsense (gateway) | **NotConfigured** on this host (no appliance) |
| linux/storage | Ok for local volumes; **network shares (nfs/cifs/webdav) need root** |

Outside this scope (another OS/arch, other provider versions, another
operator PVE/OPNsense target) support is **not demonstrated** until re-validation.

## 2. Support classes and criterion

| Class | Criterion (rule applied) |
|---|---|
| **SUPPORTED FOR PRODUCTION WITHIN THE VALIDATED SCOPE** | `supported` state with evidence of **exercised behaviour** — `live:` / `e2e:` / `chaos:` / `check:` (battery) — that the `every_supported_capability_cites_evidence_that_exists` gate confirms exists |
| **IMPLEMENTED, NOT VALIDATED** | `supported` state whose only evidence is `test:` (unit test / mock), without end-to-end behaviour proof |
| **EXPERIMENTAL** | `partial` state — implemented with a written limit, or without live proof |
| **NOT SUPPORTED** | `not-implemented` state (absent) |
| **BLOCKED BY PREREQUISITE** | `requires-external-component`, `unavailable-on-host`, or provider `NotConfigured`/no infra in this scope |
| *(not-applicable)* | `unsupported-by-provider` — the capability does not belong to that provider (e.g. VMs on the `linux` provider). **This is not a gap**; it is the denominator that the N2 metric (270 cells) excludes on purpose. |

**Honest methodological note:** of the 170 cells in the `supported` state across the four
compute/network/storage providers, the evidence distribution is **live 63 ·
check 48 · e2e 35 · chaos 18 · test 6**. Only **6** are unit-only (→ IMPLEMENTED,
NOT VALIDATED). The published N2 metric (**180/270**) also includes 10 cells
declared-supported by `opnsense`, which **on this host are BLOCKED** (see §4).

## 3. Classification by domain (4 describable providers — 467 cells)

| Domain | SUPPORTED | EXPERIMENTAL | IMPL-NOT-VALID. | NOT-SUPP. | BLOCK-PREREQ | (not-applic.) |
|---|---:|---:|---:|---:|---:|---:|
| vm-compute | 61 | 8 | 3 | 9 | 1 | 46 |
| network | 25 | 9 | 0 | 9 | 1 | 50 |
| protection | 21 | 2 | 0 | 0 | 0 | 13 |
| containers | 11 | 1 | 0 | 0 | 0 | 36 |
| console | 9 | 1 | 0 | 0 | 0 | 6 |
| metrics | 8 | 2 | 2 | 4 | 0 | 0 |
| system-containers | 7 | 4 | 0 | 0 | 0 | 45 |
| firewall | 6 | 5 | 0 | 2 | 0 | 5 |
| storage | 6 | 1 | 0 | 2 | 5 | 1 |
| inventory | 4 | 5 | 1 | 6 | 0 | 0 |
| mobility | 4 | 0 | 0 | 1 | 6 | 5 |
| access | 2 | 4 | 0 | 0 | 0 | 2 |
| **TOTAL** | **164** | **42** | **6** | **33** | **13** | **209** |

SUPPORTED per provider: proxmox 47 · linux 44 · libvirt 37 · cloud-hypervisor 36.
(Cell-by-cell TSV: `scratchpad/support-contract-cells.tsv`.)

## 4. Critical overlay: host availability (what changes the GO)

The SUPPORTED class above rests on evidence that **exists** in the repository. For
**production within the operator's scope**, real availability overlays it:

| Provider / kind | Host of this measurement | Effect on production support |
|---|---|---|
| cloud-hypervisor compute | available, Ok | SUPPORTED **demonstrable here** |
| libvirt compute | available, Ok | SUPPORTED **demonstrable here** |
| linux compute / network | available, Ok | SUPPORTED **demonstrable here** |
| linux storage (local) | Ok | SUPPORTED **demonstrable here** |
| linux storage (nfs/cifs/webdav) | needs root | **BLOCKED BY PREREQUISITE** (3 cells) |
| **proxmox compute+network** (47 SUPPORTED) | **NotConfigured** | `live:` evidence exists (proven against a reference PVE, `pve92`), but **requires configuring + re-validating against the operator's PVE target** before claiming production |
| **opnsense gateway** (10 declared) | **NotConfigured** | **BLOCKED BY PREREQUISITE** (needs the live OPNsense appliance) |

## 5. Experimental features policy

The 42 EXPERIMENTAL cells and the 6 IMPLEMENTED-NOT-VALIDATED **must not be
operated in production** without explicit risk acceptance. When an experimental one
may affect **security, integrity or availability** (e.g. cpu/mem hotplug
in proxmox «mock-tested, never run against a guest»; `async-operations`
without a job handle; firewall `read back live, not measured on a packet`), its
activation must be **explicit** and recorded. The honest ceiling of the matrix is
**247/270** — 23 cells (bucket D) that **must never** become SUPPORTED by design/
security/provenance (see `reports/capability-ceiling-20261027.md`).

## 6. Reflection in the interfaces

The classification is derived from the matrix in CODE, so it is already reflected in the CLI
(`provider ls`/`describe`/`matrix`) and in the generated docs. **Pending alignment
on the v0.1.1 line:** the matrix-generating line still says «engine 5.1.1» —
cosmetic, but it should move to 0.1.x. Desktop/console/REST/gRPC consume the same
catalog; their conformance with this classification is a Sprint 3 item (§11).

## 7. What is left to close the decision (not a GO yet)

1. Re-validate the remaining blocks on HEAD: DEF-003 (compose up does not reconcile),
   CRI OOM, CRI entrypoint/flannel branches (§10).
2. Re-validation per unavailable provider: proxmox against the operator's target;
   opnsense with the appliance (§8).
3. Recovery/continuity (§12), install/upgrade in a clean environment (§13),
   sustained tests (§15), artefact security (§16).
4. Integrate the F-1 fix (layer cache) into v0.1.1 — the only integrity P1
   closed in this campaign (code+unit+live A/B).
