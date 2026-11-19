# Production readiness — executive summary (PHASES 1–2)

**Commit:** `fa8c6297` (v3.1.0 + 26). **Date:** 2026-09-15. No runtime behaviour was
changed. All live measurements used isolated `DELONIX_ROOT` and `DELONIX_NET_RUNTIME_DIR`.

## Verdict: **LAB READY** — PILOT READY candidate for the rootless CLI path

It does not rise to PILOT/PRODUCTION because there are **four P1s confirmed live** (G-008, G-027, G-029
and G-003 in CI) and because rootful, upgrade/rollback, OCI and artefact verification remain
`NOT TESTED`.

## VERIFIED FACT (measured on `fa8c6297`)

| Evidence | Result |
|---|---|
| Baseline quality (fmt, lang, check, clippy -D warnings, test, doctest, deny) | 7/7 exit 0; **1 626 pass / 0 fail / 1 ignored** |
| E2E battery (`scripts/e2e.sh`) | **729 PASS / 1 FAIL / 5 SKIP** — the failure is the check's own threshold (G-004), not a regression |
| Chaos (`scripts/chaos.sh`) | **29 PASS / 0 FAIL / 1 SKIP** |
| CRI (`critest` v1.36.0, rootless) | **82 / 21 / 19 of 103** — +3 vs v0.63.1 (79/24, same suite and total) |
| Real Docker CLI 29.8 against `serve docker-api` | manual create/start/stop/kill/wait/restart/rename/rm cycle works; **`docker run` fails in every form** |
| Compose (`compose config`) | top refuses unknown keys; **5/5 nested sub-keys swallowed** |
| CI on the commit itself | `chaos harness` **skipped** with workflow `success` |

## Confirmed live in this round

| ID | Sev | Finding |
|---|---|---|
| **G-008** | P1 | Anti-spoof fail-open: with the `nft` insert failing, `container run` returns rc=0 and the container is left without an anti-spoof rule (control: with the rule). |
| **G-027** | P1 | `docker run -d` hangs (rc=124 in 60 s). |
| **G-029** | P1 | The Docker API reports `Up About a minute` for an already-terminated container; the CLI says `Exited`/`Dead` for the same id. |
| **G-026** | P2 | `--secret-files` with `/run` not writable: container starts **without secrets**, rc=0, no warning. |
| **G-028** | P2 | `docker run --rm` fails and leaves orphans (`AutoRemove` ignored). |
| **G-011** | P2 | Compose ignores unknown sub-keys (`deploy.placement`, `healthcheck.disable`, `bind.propagation`, …). |
| **G-030** | P2 | `scripts/critest.sh`: half-isolation and cleanup with a non-existent command (harness). |

## INFERENCE (read in the code, no reproduction)

- **G-007 (P1)** — a tmpfs mount failure in `--secret-files` writes the secrets to the container rootfs.
- **G-010 (P2)** — recursive read-only not supported.
- CRI: 2 port-forward failures with **UNKNOWN** cause (the server logs no errors — G-031).

## Answers to the 10 questions

| # | Question | Answer |
|---|---|---|
| 1 | Replace Docker in local dev? | **With the `delonix` CLI, yes for the flow covered by the battery.** As a drop-in for the Docker socket, **no**: `docker run`, pull, logs, exec and Compose fail. |
| 2 | Replace Podman on rootless servers? | **Pilot possible** for CLI/stack workloads without `--secret-files` on read-only `/run` and without depending on anti-spoof under an nft failure. Rootful and reboot not validated. |
| 3 | Kubernetes CRI? | **Not for production.** 82/103 rootless; 3 image/user bugs, OOMKilled and port-forward missing; no real kubelet. |
| 4 | Workloads for production? | None without a formal exception. Pilot: stateless services in rootless containers/stack, default network. |
| 5 | Blocked? | Kubernetes via CRI; tooling via the Docker API; per-IP isolation as a multi-tenant security boundary; `--secret-files` without a `/run` test. |
| 6 | Security risks | G-008 (fail-open isolation), G-007 (fail-open hardening), G-020 (unpinned actions). |
| 7 | Five blockers | G-008, G-029, G-027, G-005 (CRI), G-024 (rootful not validated). |
| 8 | Next release | **3.1.1**: G-008 and G-026 fail-closed with a test; G-029; G-003 (chaos job fails instead of skipping); G-030. **3.2.0**: `docker run`/pull/logs in the API; G-011. |
| 9 | Rollback | No change applied. For 3.1.1: the signed 3.1.0 binary remains; the new holder requires a respawn — rollback = reinstall 3.1.0 and `net netns down/up` in a window. |
| 10 | Verdict | **LAB READY** |

## PHASE 3 (OCI, storage, providers, PaaS)

| Area | Result | Report |
|---|---|---|
| OCI pull/digest/load | digest honoured and a non-existent digest refused; layer traversal contained; **a conflicting tag+digest writes the wrong label** (G-033); orphan `.tmp` (G-035) | `oci-conformance.md` |
| Storage under failure | a volume in use is not removed; restore without `--force` refused; a restore of a truncated archive fails without touching data or restarting; a valid restore returns the data | `storage-reliability.md` |
| VM providers | libvirt 51 PASS / 1 FAIL (pause→Stopped, G-041); CH core PASS; Proxmox ENVIRONMENT BLOCKED; OpenStack ABSENT; live migration NO-GO | `vm-provider-matrix.md` |
| delonix-paas | historical reaper bug **FIXED**; two new gaps (G-036 P1, G-037 P2) from code reading | `cni-networking.md` |
| origin/main | `b19fddcb` (+9 commits): the open findings remain in the code | gap G-040 |

Consolidated verdict: see `release-verdict.md` (**LAB READY**, with the rootless container CLI
path at PILOT READY).

## NOT VALIDATED

Rootful; SELinux (environment); upstream OCI suites; real Compose applications compared with Docker
Compose/Podman; CNI ADD/DEL/CHECK with external plugins; backup interrupted mid-way; Proxmox and
OpenStack; in-place upgrade and rollback; host reboot; minisign signature and provenance of the
v3.1.0 assets (requires downloading ~100 MB, awaiting authorization); benchmarks (production host);
cargo-audit; coverage; observability (Prometheus/OTel/stable JSON).
