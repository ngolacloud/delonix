# CNCF Sandbox — Phase A readiness (build the community before applying)

> **What this sheet is.** The actionable checklist for taking Delonix from a
> "one-person open-source project" to a "credible CNCF Sandbox candidate",
> WITHOUT applying prematurely (a solo application with no traction gets *deferred*
> and burns the first impression with the TOC).
>
> **What it is NOT.** The decision to hand over the "Delonix" trademark/domain to
> the Linux Foundation (Phase B) — that is a business decision for the owner, not
> a checklist item. See [GOVERNANCE.md](GOVERNANCE.md) and the "The decision"
> section at the end.

## Where we already are (do not redo)

| Sandbox requirement | Status | Proof |
|---|---|---|
| Public repository | ✅ | `github.com/ngolacloud/delonix` (PUBLIC) |
| OSI license | ✅ | Apache-2.0 ([LICENSE](LICENSE)) |
| Code of Conduct | ◑ | Contributor Covenant 2.1 — the CNCF CoC link is missing (item A4) |
| Documented governance | ✅ | [GOVERNANCE.md](GOVERNANCE.md) (honest BDFL + multi-maintainer path + escape valve) |
| Release supply-chain | ◑ | the engine ALREADY emits an SPDX SBOM + SLSA provenance + OCI attestations (2026-11 campaign) — what's missing is *applying it to releases* (A1) |

The administrative baseline is almost entirely done. Phase A is about **community
signals and vendor-neutrality**, which is what the TOC actually weighs.

---

## Phase A checklist

Each item: **what · why (CNCF criterion) · how · done when**.

### A1 — Public, regular, signed releases with SBOM + provenance
- **Why:** the TOC looks at "does the project make releases?" and at
  supply-chain maturity. It's the cheapest differentiator we have — the engine
  already produces everything.
- **How:**
  1. Cadence: a minor release every ~4–6 weeks, tag `vX.Y.Z` on `main`.
  2. Each release signed (`cosign` or the native `image sign`) + **SBOM and
     provenance attached** using what the campaign delivered:
     `delonix build --provenance=max`, `delonix image sbom <ref> --attach`,
     `delonix image provenance <ref> --attach`.
  3. `scripts/release_verify.py` in the pipeline (signature + SBOM + attestations).
  4. Generated release notes (CHANGELOG kept up to date).
- **Done when:** ≥2 public signed releases with attestations verifiable by a
  third party (`cosign verify-attestation` / `oras discover`).

### A2 — DCO (Developer Certificate of Origin) on all PRs
- **Why:** the CNCF requires contribution provenance (DCO or CLA); DCO is the
  lightest and the preferred one.
- **How:** add the "DCO" GitHub App to the repo; require `Signed-off-by:` on each
  commit; document it in [CONTRIBUTING.md](CONTRIBUTING.md); `git commit -s`.
- **Done when:** the DCO check is required and passes on every new PR.

### A3 — `ADOPTERS.md` (real-usage signal)
- **Why:** the TOC asks "who uses this?". Zero adopters is the weakest point of a
  Sandbox application.
- **How:** create `ADOPTERS.md` listing real usages — NgolaCloud itself
  (control-plane, workspace, mail) **counts** as an adopter; ideally ≥1 external.
  Format: organization · use case · since when · public contact.
- **Done when:** ≥2 adopters listed, ≥1 outside NgolaCloud.

### A4 — Explicitly adopt the CNCF Code of Conduct
- **Why:** the CNCF requires its CoC (or a compatible one that links to it).
- **How:** in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), keep the Contributor
  Covenant but **add** the reference to the CNCF CoC
  (`https://github.com/cncf/foundation/blob/main/code-of-conduct.md`) and the
  conduct-reporting email.
- **Done when:** the CoC links to the CNCF's and names a reporting channel.

### A5 — Public roadmap
- **Why:** the TOC wants to see direction and that the project is not a code dump.
- **How:** `ROADMAP.md` (or a public GitHub Projects) with the milestones — reuse
  the already-measured map of the **8 CNCF value areas**
  (`reports/cncf-acceptance-2026-11-12/report.md`): supply-chain ✅, k8s interop
  ✅, cloud-native build still missing (area 5), runtime lineage (area 1).
- **Done when:** `ROADMAP.md` public, dated, linked from the README.

### A6 — A second maintainer from another organization ⭐ (the strongest signal)
- **Why:** it's the #1 proof that the project is **not captive to a single
  vendor**. A 1-maintainer/1-org project is the most common reason for a *defer*
  in Sandbox.
- **How:** [GOVERNANCE.md](GOVERNANCE.md) already describes how to add a
  maintainer (proposed in the open, objection window). Recruit someone with a real
  track record (reviews/PRs), ideally from outside NgolaCloud; add them to
  `MAINTAINERS.md` + `.github/CODEOWNERS` for the area they take ownership of.
- **Done when:** ≥2 maintainers in `MAINTAINERS.md`, ≥1 from another org, with
  real ownership (not cosmetic).

### A7 — Open contributors and community channels
- **Why:** a signal of activity and that there is *somewhere* the community
  happens.
- **How:** GitHub Issues + Discussions enabled; `good-first-issue`/`help-wanted`
  labeled; a public channel (CNCF Slack `#delonix` once accepted, or a
  Matrix/Discord in the meantime); an informal response SLA documented in
  CONTRIBUTING.
- **Done when:** ≥3 PRs/issues from external contributors closed; public channel
  linked in the README.

### A8 — Mature security disclosure
- **Why:** a maturity criterion; we already have [SECURITY.md](SECURITY.md).
- **How:** confirm the private disclosure email/process, response window, and
  (once there is a 2nd maintainer) a *security list* with ≥2 people.
- **Done when:** SECURITY.md names a process + ≥2 contacts.

---

## Readiness scorecard

| Item | TOC weight | Status |
|---|---|---|
| A1 signed releases + SBOM/provenance | high | ◑ engine ready; pipeline missing |
| A2 DCO | medium | ☐ |
| A3 ADOPTERS.md | **high** | ☐ |
| A4 CNCF CoC | low | ◑ |
| A5 public ROADMAP | medium | ☐ (measurement already exists, publishing missing) |
| A6 2nd maintainer from another org | **critical** | ☐ |
| A7 contributors/channels | high | ☐ |
| A8 security disclosure | low | ◑ |

**Ready to apply when:** A1, A3, A5, A6, A7 closed (the high/critical-weight
ones). A4/A8 are quick; A2 is mechanical.

---

## The decision (Phase B — not a checklist item)

Even with all of Phase A green, **admission requires transferring the "Delonix"
trademark, the name and the project's domains to the Linux Foundation** (CNCF rule
since 2021). That means giving up exclusive control:

- **You want Delonix as a NgolaCloud product** (control, direct monetization under
  the brand) → it stays your open-source project, excellent, **non-CNCF**. It's a
  legitimate choice; Phase A strengthens it all the same.
- **You want the CNCF seal** (adoption, neutrality, ecosystem) → you donate the IP
  and open up governance. Your GOVERNANCE.md already anticipates this.

Phase A **does not compromise** either of the two — it makes the project better on
both paths. The Phase B decision can wait until Phase A is done.

## Differentiation (the "why CNCF", which the application will require)

Kept here because the TOC always asks "why a new runtime?":
container **and** microVM in a single rootless/daemonless binary (unlike Kata =
containerd+QEMU), kernel-native in Rust outside the containerd chain, and the
**sovereign cloud** angle for low-connectivity/data-sovereignty contexts. It's a
*mission* differentiator beyond the technical one — something the CNCF values.
