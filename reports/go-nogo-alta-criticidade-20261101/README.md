# GO/NO-GO — Delonix vs Docker/Podman in HIGH-CRITICALITY production

> **Date:** 2026-11-01. **Scope:** decide whether Delonix is GO or NO-GO as a
> replacement for Docker/Podman in **computationally high-criticality** environments
> where both are already proven options. Does **not** assess use in dev/CI/edge (see §5).
>
> **Method.** Synthesis of four sweeps of the repository's own evidence
> (security/audits, capability matrix, defects/stabilisation, operational
> maturity) cross-checked against spot verification in the code. **Method caveat,
> and it weighs on the verdict:** almost all of the repo's evidence is
> **self-produced** (adversarial agent review, never an external pentest) and
> **dated** (measured on v0.x–v3.1.0/5.1.1 lines). A high-criticality buyer would
> require independent measurement — which is precisely item 1 of the gate (§6).
>
> **Sources:** `docs/COMPARACAO-DOCKER-PODMAN.md`, `docs/measured-comparison.md`,
> `docs/AUDITORIA-E2E.md`, `docs/cri-conformance.md`,
> `docs/providers/capability-matrix.md`, `reports/capability-ceiling-20261027.md`,
> `reports/production-readiness/v3.1.0/*`, `reports/release-campaign-*`,
> `docs/RELATORIO-PRE-PRODUCAO.md`, `VERSIONING.md`, `MAINTAINERS.md`,
> `GOVERNANCE.md`, + direct verification in `crates/adapters/delonix-oci/src/overlay.rs`.

## Verdict

**NO-GO** as a Docker/Podman replacement in high criticality, **today**.
**Conditional** GO for single-node niches without failover (§5). The gate to
become GO is in §6 and in the [plan](plano-no-go-para-go.md).

The bar for "high criticality where Docker/Podman are already options" is: proven
reliability, HA/failover, **external** security assurance, supportability and
safe upgrade. Delonix lags the battle-tested incumbent on **all** of them.

## Decision matrix

| Dimension | Verdict | Key evidence |
|---|---|---|
| **Security** | ⚠️ Honest, **not externally assured** | Fail-closed engineering; fail-open leaks closed (asserted in the CHANGELOG, not re-audited). All audits are **internal AI-adversarial, never an external pentest**. Weak supply chain: **G-020** 0/20 Actions SHA-pinned; no `cargo-audit`/trivy in CI; CVE scanner **with no real feed** (`delonix-scanner/src/lib.rs:371`); signing/SLSA/SBOM "by design", not verified against artefacts (`supply-chain.md`). OCI whiteout (finding #1) **verified FIXED in the code** (`overlay.rs:185-221`, `safe_rel` + confined canonicalisation) — it is `AUDITORIA-E2E.md` that is out of date. |
| **Reliability** | ❌ **LAB/PILOT READY** | Formal self-assigned verdict LAB READY; single-node rootless CLI = PILOT (`release-verdict.md`). **9 open P1s**; **no published release** (`CHANGELOG` 0.x "[Unreleased]"); historical **fail-open rc=0** pattern (anti-spoof/secrets/reaper — several already fixed); `compose up` does not restart a stopped service (DEF-003); `stack apply` reports "unchanged" with a changed `command` (DEF-002); CRI **84/104** (≈13/20 failures are environment/rootless); live migration **NO-GO** (ADR-0031); density/scale **not measured**. |
| **Capabilities** | ⚠️ 181/271 (66.8%), ceiling 247 — re-measured at the current commit 2026-11-02 (was 180/270@5.1.1: `net.ipv6` moved to `supported`, ADR-0082) | ≥ Docker/Podman for single-node lifecycle/pods/CRI/rootless-networking; **unique** in combining container+microVM+CRI in one engine. Missing: **multi-node orchestration + HA** (VM HA/replication = `requires-external-component`; inter-node overlay not proven E2E), full compose (`profiles`/`extends`/multi-file), log drivers, BuildKit (`ssh`/`cache`/multi-arch push), **GPU/CDI unproven on real HW**, `--format` Go-template. |
| **Operational** | ❌ Immature | Single-node by design (fleet → `delonix-paas`, ADR-0010); **no HA, no Helm/Terraform/Ansible, no SLA**; **1 maintainer** (BDFL; ~2735 commits from one person, `MAINTAINERS.md`/`GOVERNANCE.md`); pre-1.0 (minors may break CLI/API/schema, `VERSIONING.md`); **upgrade/rollback NOT TESTED** (`release-verdict.md`), no hot-patch of live supervisors (`RELATORIO-PRE-PRODUCAO.md:116`); observability strong-in-design but limited (daemonless; engine tracing not wired; mgmt `/metrics` frozen by ADR-0041). |

## Disqualifiers (each one is sufficient on its own)

1. **Upgrade/rollback not tested** + impossible to patch running supervisors without recreating containers.
2. **No HA / multi-node not proven E2E** — if the environment requires failover, it does not exist.
3. **No independent security assurance** — no external pentest; unpinned/unverified supply chain (the pipeline that produces "trusted" releases is the weakest link).
4. **Pre-release** (0.1.0 not released), **1 maintainer**, **no SLA**, breaking changes allowed in 0.x.
5. **Scale/density not measured** — the attach path does `nft` dumps that grow with the number of containers; everything measured on an empty node.

## In favour — genuine and measured

It is not "Docker with fewer features". **Real and measured** differentiators:
hot reconfiguration without changing PID; daemonless ~2.3–3.5× lower latency on the
default (91 vs 208/268 ms); **governed rootless networking** (incl. IPv6 dual-stack
by default with per-workload v6 policy — the only one of the three, see
`docs/measured-comparison.md` §IPv6); container+microVM+CRI in a single engine (real
kubelet, `kindest/node` `Ready`); stricter security by default (NNP always +
fail-closed check); mandatory resource ceilings (Docker/Podman leave memory
uncapped).

## §5 — Where it WOULD BE GO (conditional)

Single-node, **no failover requirement**, trusted images, operator who accepts
pre-1.0 and self-support: **dev/CI, single-node edge, rootless-first labs,
k8s-node-without-Docker**. There it is a credible PILOT and sometimes superior to
the incumbent.

## §6 — Gate to become GO in high criticality

The 6 points whose closure turns the verdict. Detailed plan in
[plano-no-go-para-go.md](plano-no-go-para-go.md).

1. **External security** — external pentest of the current line + SHA-pin the Actions + real CVE feed + evaluate rootful/SELinux.
2. **Validated release** — tag with green CI + e2e/chaos/critest run ON the tag (not the v5.1.1 process).
3. **Upgrade/rollback** — tested E2E + hot-patch history of live supervisors.
4. **HA/multi-node** — proven E2E, OR failover excluded from scope and single-node ceiling documented.
5. **Scale/density** — measured up to a declared ceiling with SLOs.
6. **Current evidence** — re-measure the stale data on the version actually shipped.
