# SKILL.md — Delonix Architecture Review

## Skill Name

`delonix-architecture-review`

## Purpose

Review a change — or a proposed design — against the Delonix engine's layered
architecture (ADR-0040) and its invariants, so structure does not erode one
"small exception" at a time. The engine is a large layered Rust workspace
(`foundation → contexts → adapters/providers → interfaces → assembly → bins`)
with an automated fitness gate; this skill is the human-judgement layer on top
of that gate: it decides whether a change BELONGS where it is, not just whether
it compiles.

## Domain

Layer direction and dependency rules; crate placement and responsibility;
the "engine knows no consumer" boundary (it consumes the Kubernetes CRI, it is
not a PaaS and does not run on top of Kubernetes); ports/adapters; public
contracts (CLI/JSON/node.v1 gRPC/schemas/`DELONIX_*`); ADR coverage of
structural decisions.

## Trigger conditions

Invoke when a change: adds or moves a crate; introduces a dependency across
layers; adds a public contract or a `DELONIX_*` var; makes a structural
decision that GOVERNANCE.md says needs an ADR; or when `arch_fitness.py` /
`folhas_gate.py` go red and someone needs to decide whether to fix the code or
(deliberately, with a reason) lower a ratchet baseline.

## Permission posture

Read-only review (Tier A, see `.ai/agents/permission-tiers.md`): it reads code,
ADRs and the fitness data and renders a judgement. It does not refactor; it says
what should change and why.

## References (the real assets)

- `ARCHITECTURE.md` (repo root) — the canonical C4 architecture, maintained
  against the code. **Not** `docs/ARCHITECTURE-SCOPE.md`, which is a superseded
  pre-ADR-0040 spike (it carries a banner saying so).
- ADR-0040 `docs/adr/0040-engine-restructuring-layers-ports-node-contract.md`
  — the layering and node contract. The ADR index is `docs/adr/README.md`
  (gated by `scripts/adr_status_gate.py`).
- `docs/dev/architecture.md` — the contributor map (defers to `ARCHITECTURE.md`;
  its tables are generated).
- `scripts/arch_fitness.py` + `scripts/arch_baseline.json` — the layer-direction
  rules and the two-way debt ratchets (`interface_mechanism_deps`,
  `library_prints`, `provider_namers`, `raw_error_variant_matches`,
  `self_exec_sites`, `shared_error_imports`, declared layer exceptions).
- `scripts/folhas_gate.py` + `scripts/folhas_baseline.json` — the fail-closed
  inventory of the seven public surfaces.
- `scripts/arch_report.py` — renders `reports/architecture.md` from the fitness
  data (CI checks it is current).

## Method

1. **Place it** — which layer/crate does the change touch, and does it respect
   the direction (`foundation` depends on nothing; `interfaces`/`bins` compose,
   they don't hold mechanism; contexts are domain logic; adapters/providers are
   the mechanism)?
2. **Run the gate as evidence** — `arch_fitness.py` (+ `folhas_gate.py` if a
   public surface moved). A RED gate is a finding, not a nuisance. If a ratchet
   must move, say why the debt legitimately changed; never lower a baseline to
   silence a real regression.
3. **Boundary check** — does the change keep the engine consumer-agnostic
   (no PaaS/tenant notion leaking in)? Does it add a public contract that needs
   an alias/migration note?
4. **ADR coverage** — is this a structural decision GOVERNANCE.md says needs an
   ADR? Is there one, and does its status (index + body) agree?

## Expected output

Findings as: what, where (`crate`/`path:line`), which rule/ADR it bears on,
impact, and the recommended placement/fix. A clear VERDICT: the change fits the
architecture as-is / fits with a named adjustment / needs an ADR first. Separate
VERIFIED (gate ran, rule cited) from JUDGEMENT.

## Validation strategy

Anchor every structural claim to `ARCHITECTURE.md`/ADR/fitness output, not to
memory. When the recommendation is "lower a ratchet", the justification must
name the legitimate reason the debt moved; otherwise the finding is "fix the
code", not "move the baseline".
