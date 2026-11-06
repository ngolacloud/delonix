# SKILL.md — Delonix Security Review

## Skill Name

`delonix-security-review`

## Purpose

Review a Delonix change (a diff, a branch, or a module) for the security risks
that matter in a **rootless-first, daemonless container/microVM engine running
on production hosts with real namespace / cgroup / nftables / KVM privileges**.
A vulnerability here is not a bug — it is a namespace escape, a rootless→root
escalation, a tenant-isolation break, or a host-hardware compromise (SECURITY.md).

This skill makes the review **reproducible and anchored to code**, and it is the
**single owning source of SEC doctrine for the engine**: it does not re-invent a
checklist, it points at the canonical one and adds the engine-specific judgement.

## Domain

Container/microVM isolation; Linux namespaces, cgroups, capabilities, seccomp;
mounts and overlay; netfilter/netlink/CNI; KVM / Cloud-Hypervisor / libvirt and
VFIO passthrough; OCI image + supply chain; node admission policy; the local
control surfaces (CRI, node-api, mgmt, MCP) and their SO_PEERCRED auth; secrets
vault and the audit trail.

## Trigger conditions

Invoke this skill when a change:
- touches `crates/adapters/delonix-linux` (mount/unshare/pivot_root, seccomp,
  capabilities, CDI), `delonix-sdn` (nft/ip/netns/wireguard/CNI/bpf),
  `delonix-vm` or the VM providers (KVM, VFIO), `delonix-oci`/`delonix-volume`
  (mounts, overlay), or `bins/delonix-storage-helper` (the root helper);
- edits `crates/contexts/delonix-security` (admission, policy, score, redact);
- adds or changes any `DELONIX_*` flag, a default that could flip a guard open,
  a control socket, or the MCP tool surface;
- is being prepared for release and needs a SEC sign-off.

Also invoke for a periodic offensive review of the isolation surface.

## Required knowledge

Rust; Linux isolation primitives (namespaces, cgroup v2, caps, seccomp-bpf);
OCI/CRI; netfilter/nftables; KVM/QEMU/Cloud-Hypervisor and IOMMU/VFIO; supply
chain (signing, SBOM, provenance); the Delonix invariants (rootless-first,
daemonless, fail-closed admission) from `AGENTS.md` and `SECURITY.md`.

## References

- **Canonical checklist (do not fork):**
  `.ai/skills/delonix-runtime-e2e-quality/references/security-checks.md` —
  privilege/isolation, input handling, secrets, supply chain, AI/MCP surface,
  severity mapping, the `dlx-e2e-*` scoping discipline, and the regression rule.
  This skill DEFERS to it; when they appear to disagree, that file wins and this
  skill is updated to match.
- `references/engine-surfaces.md` (this skill) — the code anchors: which crate
  owns which privileged mechanism, and the exact symbols to read.
- `crates/contexts/delonix-security/src/domain/{admission,policy,score,redact}.rs`
  — the node's security decisions.
- `SECURITY.md`, `docs/guia-politica-de-seguranca.md`, and the relevant ADRs
  (e.g. capability model ADR-0003).

## Permission posture

This skill is used by the `delonix-security-reviewer` agent at **Tier A
(read-only)** — see `.ai/agents/permission-tiers.md`. Review FINDS and
CLASSIFIES; it never runs an exploit against a live host, never mutates state,
and never performs a Tier-D operation to "prove" a finding. A proof of concept
is described, not executed against production kernel interfaces.

## Method

1. **Scope** — list the privileged surfaces the change touches (from the trigger
   list). If none, say so and stop; not every diff needs this skill.
2. **Walk the canonical checklist** (`security-checks.md`) against each touched
   surface. For isolation/privilege, read the actual anchors in
   `references/engine-surfaces.md`.
3. **Hunt fail-open regressions** — any new `DELONIX_ALLOW_UNENFORCED_*`,
   `*_INSECURE_TLS`, `INSECURE_REGISTRIES`, `POLICY_LINT=0`, a default that
   weakens admission, or a capability/seccomp relaxation. Default a change you
   cannot prove safe to the HIGHER severity (fail closed, like `risk_of()`).
4. **Check the audit trail & secrets** — does the change keep mutations going
   through the engine (so admission + `events.jsonl` see them), and secrets in
   the vault, never in logs (`domain/redact.rs`)?
5. **Classify and report.**

## Expected output

Findings, each with: title; severity P0–P4 (per `security-checks.md`);
`path:line` evidence; the concrete failure scenario (inputs/state → escape/
escalation/leak); CONFIRMED vs PLAUSIBLE; and the fix direction. End with a SEC
GO / NO-GO for the reviewed change and, for a release, whether the isolation
surface regressed versus the last reviewed baseline.

## Validation strategy

A finding is only CONFIRMED when the code path is read end to end and the
failure scenario is concrete (not "looks risky"). Severity follows the mapping
in `security-checks.md`, not gut feel. A review that changed no verdict still
records what surfaces it walked, so the next reviewer knows what was covered.
