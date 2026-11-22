# Sprint 0 — Reconciling the 5.1.1 baseline and the capability matrix

Date: 2026-10-24 · Responsible engineer: capability conformance campaign
Checkpoint document. Docs-only. Does not change code, catalogue or counters.

## 1. Baseline 5.1.1 fixed

| item | value |
|---|---|
| Published tag | `v5.1.1` |
| Exact commit | `a0da90f50abccb8d80a14136263d549dce257837` |
| Tag date | 2026-10-15 09:48:18 +0100 |
| Subject | `version: bump to 5.1.1 and regenerate documentation` |
| Previous tag | `v5.1.0` @ `1843b3217f3054f4f5807f8a03f40f313a239929` (2026-10-15 08:44) |
| Engine version | 5.1.1 (`Cargo.toml:41`, read via `env!("CARGO_PKG_VERSION")`) |
| Catalog version | 1.4.0 (`crates/contexts/delonix-compute/src/capability.rs:39` — `CATALOG_VERSION`) |
| Binary in use | `target/release/delonix` (38 MB, compiled 2026-10-24) and `~/.local/bin/delonix`; both report `delonix 5.1.1` |

**Relationship develop ↔ baseline (owner's guidance: the working baseline branch is the most recent one):**

- `git rev-list --left-right --count v5.1.1...develop` → **0 behind, 258 ahead**.
- `git merge-base v5.1.1 develop` → `a0da90f5` (the tag itself).

In other words, `develop` is a **clean linear descendant** of v5.1.1: no rebase/reset rewrote the baseline history. The v5.1.1 tag remains an immutable historical anchor; the campaign's work sits on `develop` (HEAD `558e5f20` at the start) and each new front is born from `develop` via a worktree, preserving traceability back to v5.1.1 (common base = the tag). **The main branches are not reset and the tag is not replaced.**

## 2. Matrix regenerated from the correct binary

Generated with the 5.1.1-descendant binary, without editing version strings:

- DECLARED view (host assumed complete, this is what the Desktop publishes):
  `delonix provider matrix` → `matrix-declared-5.1.1-descendant.md`.
- MEASURED view on this host (rootless, delegated cgroup2):
  `delonix provider ls -o json` → imported into `matrix-structured.json`.

`matrix-structured.json` is the **lossless** import: 515 rows, keyed by `provider|capability`, preserving `state`, `justification` (the catalogue's `detail`/`evidence`), `test_ref` (prefix `check:`/`e2e:`/`live:`/`test:`/`chaos:` when present), `domain`, `kind` and `supported`. The cited ADR lives in the variant's docstring (e.g. ADR-0050) and the "source line" is the identity of the Rust variant in the per-provider `match` (not a serialized field — see §4).

## 3. Reconciliation against the received report

Report totals (Desktop export) vs. measured on develop HEAD:

| state | report | measured @ develop | Δ |
|---|---|---|---|
| supported | 103 | **112** | +9 |
| partial | 93 | **86** | −7 |
| not-implemented | 74 | **72** | −2 |
| unsupported-by-provider | 230 | 230 | 0 |
| requires-external-component | 12 | 12 | 0 |
| unavailable-on-host | 3 | 3 | 0 |
| **total** | **515** | **515** | **0** |

**Explainable and conservative diff.** The total (515) is identical and the three structural states (unsupported-by-provider, requires-external-component, unavailable-on-host) match exactly — confirming that catalogue 1.4.0 and the provider set are the same as in the report. The only difference is internal to the "actionable" states: **+9 supported, −7 partial, −2 not-implemented, sum = 0**. No entry appeared or disappeared.

Cause: the 258 commits that `develop` gained since the v5.1.1 tag **promoted 9 cells** (partial/not-implemented → supported) by adding implementation + real test evidence. The report was exported from a build prior to v5.1.1 or from the tag itself; `develop` is ahead. Identifying the 9 promotions cell by cell is left for the final Sprint (regression/inventory), cross-referencing this JSON with an export of the original report once the file `Pasted text(6).txt` is recovered (it was not on disk in this session — the matrix was regenerated from code/binary, as point 1.5 of the mandate requires).

Methodological note: the report carries `unavailable-on-host=3`, a state that **only exists in the measured view** (never in the declared one). So the report is a MEASURED export from a host of the same class as this one (rootless, with no mountable NFS/CIFS/WebDAV — the 3 `unavailable-on-host` are storage.linux of those three families).

## 4. Catalogue model (how "supported" is enforced)

Subsystem map (key files):

- `crates/contexts/delonix-compute/src/capability.rs` — `enum CapabilityState` (6 variants, `#[serde(tag="state", rename_all="kebab-case")]`, lines 988-1019), `enum Capability` + `Capability::ALL` (catalogue/denominator), `ProviderReport::build` (walks `ALL`, `match` without wildcard → forgetting an entry does not compile), cell metric `CellMetric`.
- `bins/delonix-runtime-bin/src/cmd/provider.rs` — CLI `provider ls|describe|matrix|config`; `declared_reports()` and `measured_reports()`; Markdown generator; no-drift test `the_published_matrix_is_the_generated_one`.
- Per-provider classification (one state `match` each): cloud-hypervisor `delonix-provider-cloud-hypervisor/src/lib.rs:265`, libvirt `delonix-provider-libvirt/src/lib.rs:301`, proxmox `delonix-proxmox/src/lib.rs:7359` (+ network `:7131`), linux `delonix-linux/src/provider_report.rs:47`, sdn `delonix-sdn/src/provider_report.rs:51`, volume `delonix-volume/src/provider_report.rs:38`, opnsense `delonix-opnsense/src/capabilities.rs:20`.
- Desktop endpoint: `GET /v1/providers` (gRPC `ListProviders`), `crates/interfaces/delonix-node-api/src/{service.rs,providers.rs}`; `catalog_version` in the contract.
- Gate: `scripts/capability_ratchet.py` + `scripts/capability_baseline.json` (`{"proved":112,"applicable":273}`) + `docs/providers/capability-matrix.md` (published artefact).

**Guard-rail that satisfies criterion #16 of the mandate by construction:** a cell only serializes `supported` if the `match` arm returns `Supported { evidence: "..." }` **and** the evidence string points to a battery/section/test/live/chaos that a test in the bin crate verifies exists. Therefore it is **impossible** to promote a capability to `supported` just by changing the label — it requires a real backend + traceable evidence. The campaign works in favour of this invariant, not against it.

## 5. Catalogue boundary: no OpenStack provider (point 13)

Confirmed: the providers in catalogue 1.4.0 are **5** — `cloud-hypervisor`, `libvirt`, `proxmox`, `linux`, `opnsense`. There is no `compute/openstack` provider. This is neither a Desktop filter nor an omission in the report: there is no OpenStack adapter registered in `declared_reports()`/`measured_reports()` nor a corresponding crate. Inventory classification: **absence of implementation in 5.1.1** (not a regression nor a feature-flag). Recorded as a scope decision; implementing an OpenStack provider is its own larger sprint, outside the gap-closing of the existing entries.

## 6. Artefacts of this checkpoint

- `matrix-structured.json` — lossless import of the 515 entries (campaign source of truth).
- `matrix-declared-5.1.1-descendant.md` — snapshot of the regenerated declared matrix.
- `00-baseline-reconciliation.md` — this document.
- `01-sprint-plan.md` — sprint plan per front, with specialists and verifiable criteria.
- `README.md` — index + resumable checkpoint.
