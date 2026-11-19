# Guide — using the skills to take Delonix to the CNCF

How each skill/agent does its work, in what order they chain together, and the
path to **submitting Delonix to the CNCF (Sandbox)**. Audience: whoever drives
engineering sessions in `delonix-runtime`.

> Uncomfortable truth, stated up front: **100% across the 8 technical areas is
> necessary but not sufficient** for the CNCF. The CNCF Sandbox also weighs
> **open governance, vendor neutrality and adoption**. That is why the plan has
> two tracks: *technical* (BUGs/GAPs) and *project* (GAP-0014).

## 1. Catalogue — each skill/agent and its role

### Driving skills (user-level, `~/.claude/skills/`)
| Skill | Role | When to invoke |
|---|---|---|
| `delonix-runtime-engineer` | **Entry contract** — product boundaries, verified commands, delegation rules, references per domain | **Always first**, in any work on the engine |
| `delonix-backlog-driver` | **Driver** — picks the next item (dependencies→tier→CNCF weight→WIP), fires the right prompt, reviews alignment | "what's next", "implement the backlog", "validate prompts" |
| `delonix-rust-reviewers` | Elite Rust panel (unsafe/ABI/perf/linking/libc) | reviewing Rust PRs/crates/diffs |
| `escopo` | Opens the session contract (1 objective, ≤3 criteria, out-of-scope) | when starting each item |
| `fila` | A queue of ready prompts, with a WIP cap | "what's open", "next" |
| `fechar` | Closes the session/consolidates with a record | criteria met; "close this" |

### Harness skills (in the repo, `.ai/skills/`)
| Skill | Role |
|---|---|
| `delonix-runtime-e2e-quality` | **Produces** the backlogs (`reports/e2e/<version>/`): only the verified *effect* counts |
| `delonix-bug-triage` | **Grooms** findings → a deduplicated, ordered queue (P0–P4) |
| `delonix-architecture-review` | Reviews a change/design against the engine's layers (ADRs) |
| `delonix-security-review` | Reviews isolation/privilege/secrets/supply-chain/MCP risk |
| `delonix-release` | Takes «code ready» → «human publishes safely» |

### Harness agents (in the repo, `.ai/agents/`, by tier)
| Agent | Tier | Role |
|---|---|---|
| `delonix-auditor` | A (read-only) | «how X works / where Y lives», with file:line evidence |
| `delonix-builder-tester` | B (rootless) | build/test/gates + small edits; **never root** |
| `delonix-gatekeeper` | B | runs and interprets the gates (arch_fitness, leaves, contract, version, lang…) + baselines |
| `delonix-security-reviewer` | A | classifies risk in namespaces/cgroups/caps/seccomp/mounts/KVM diffs |
| `delonix-release-manager` | C (proposes) | validates release readiness; **human publishes** |

Tiers (from `.ai/agents/permission-tiers.md`): **A** read-only · **B** rootless ·
**C** human approval · **D** privileged/root (rootful-VM; never a root shell).

## 2. The end-to-end flow (who calls whom)

```
  measure          triage          enqueue         drive               execute          verify             close
┌───────────┐   ┌───────────┐   ┌─────────┐   ┌───────────────┐   ┌───────────┐   ┌─────────────┐   ┌─────────┐
│ e2e-      │ → │ bug-      │ → │  fila   │ → │ backlog-driver│ → │ escopo +  │ → │ regression  │ → │ fechar  │
│ quality   │   │ triage    │   │         │   │ (order+review)│   │ agent     │   │ test        │   │         │
└───────────┘   └───────────┘   └─────────┘   └───────────────┘   └───────────┘   └─────────────┘   └─────────┘
 reports/e2e     BACKLOG.md       prompts        Prompt N +          worktree         CI green,         8-area
                                                 checklist           + right tier      no re-appearance  matrix++
```

Cross-cutting **non-negotiable** rule: an item only closes with an **automated
regression test** — that is what stops it coming back in the next `report.md`.

## 3. Playbook — "I want X, I use Y"

| I want… | I do |
|---|---|
| Start any work | Load `delonix-runtime-engineer` |
| Know the next item / advance the backlog | `delonix-backlog-driver` → gives Prompt N and the slot |
| Execute an item | Paste Prompt N (opens with `/escopo`); 1 item = 1 worktree = 1 branch |
| Run build/test/gates without root | the `delonix-builder-tester` agent (Tier B) |
| Touch AppArmor/host-ns/OOM/mount (root) | **rootful-VM** (Tier D), coordinate with the owner |
| Understand code before touching it | the `delonix-auditor` agent (Tier A) |
| Understand why a gate is red | the `delonix-gatekeeper` agent |
| Review the security of a diff | `delonix-security-review` / the security-reviewer agent |
| Review Rust quality | `delonix-rust-reviewers` |
| Measure conformance/parity again | `delonix-runtime-e2e-quality` |
| Prepare (not publish) a release | `delonix-release` / the release-manager agent (Tier C) |
| Consolidate and close | `fechar` |

## 4. Path to the CNCF — two tracks

### Technical track: 100% across the 8 areas (see `BACKLOG.md`)
Order recommended by `delonix-backlog-driver`:
1. **Slot 1** — BUG-0001 + M-TEST-02 (Tier B): clean install + honest harness.
2. **Slot 2** — BUG-0004→GAP-0001/0003, BUG-0003/05/06 (Tier D, rootful-VM):
   CRI conformance ≥95%.
3. **Slot 3** — GAP-0009→0008→0017, GAP-0013 (Tier B): supply-chain
   (SBOM SPDX in-toto, SLSA, policy-based trust, scanner CI) — **highest CNCF weight**.
4. **Slot 4** — GAP-0015 (k8s interop), GAP-0011 (Compose Spec), GAP-0016 (systemd).
5. **Roadmap** — GAP-0012 (published conformance), GAP-0010 (multi-arch).

Technical milestone: the whole 8-area matrix ✔.

### Project track: CNCF Sandbox readiness (GAP-0014)
Beyond the code, the CNCF Sandbox requires:
- **Open-source with an approved license** — ✔ the repo has `LICENSE` (Apache-2.0).
- **Public governance** — ✔ `GOVERNANCE.md`, `MAINTAINERS.md`,
  `CODE_OF_CONDUCT.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODEOWNERS` exist.
- **Public repo** — ✗ today it is private → **make it public**.
- **Signed releases + SBOM/provenance** — ties into the technical track (GAP-0008/09).
- **Neutrality and adoption** — show users/adopters and a public roadmap.
- **CNCF Code of Conduct** adopted; **DCO** on the commits.

Project milestone: public repo + governance + signed releases + 1st submission
to the CNCF TOC (Sandbox).

## 5. Submission checklist (close before proposing to the CNCF)
- [ ] 8-area matrix 100% ✔ (`BACKLOG.md`)
- [ ] CRI ≥95% published + OCI image/distribution/runtime suites (GAP-0012)
- [ ] Images with verifiable SBOM SPDX in-toto + SLSA provenance (GAP-0008/09)
- [ ] `delonix kube play/generate` (k8s interop) working (GAP-0015)
- [ ] **Public** repo, Apache-2.0, DCO, CNCF CoC, clear governance and maintainers
- [ ] Signed releases (PGP/sigstore), CHANGELOG, versioning policy
- [ ] User documentation + public adoption/roadmap page
- [ ] ADR decision on own runtime vs containerd (GAP-0012)

## 6. Golden rules (never break)
- **Do not touch the owner's dirty tree** (commits in parallel) — own worktree;
  `reports/` by append only.
- **Tiers:** never give root to a Tier B agent; Tier D items only via rootful-VM.
- **Distinguish** implemented / tested / proposed / blocked — without a regression
  test, it is not closed.
- **Interoperable standards** in supply-chain (SPDX, in-toto, SLSA, OSV, sigstore)
  — never a proprietary format.
- **Re-confirm `git status`** before any branch operation.

---
Sources: `report.md`, `BUG_BACKLOG.md`, `GAP_BACKLOG.md`, `BACKLOG.md`,
`PROMPTS.md`, `CNCF_GAP_DOCKER.md`, `CNCF_GAP_PODMAN.md`; skills in
`~/.claude/skills/` and `.ai/{skills,agents}/`.
