# SKILL.md — Delonix Release

## Skill Name

`delonix-release`

## Purpose

Take a Delonix version from "the code is ready" to "a human can safely publish
it" — and NOT one step further. This skill gathers and verifies release
readiness; the act of publishing (tag push, `gh release create`, `gh workflow
run release.yml`) stays with a human. It exists because release-cut doctrine was
scattered — some in the e2e skill's §47 gate, some in `make gates`, some only in
`release.yml` — with no single owning competency, and because a release was once
cut red (v5.1.1, suites "run on develop weeks ago", not on the tag's commit).

## Domain

Version alignment, release gating, supply-chain assurance (minisign signature,
SBOM, SLSA provenance), release notes / changelog / migration docs, and the
post-publish end-to-end asset verification.

## Trigger conditions

Invoke when preparing to cut or publish a release: a version bump is proposed, a
tag is about to be created, or someone asks "is `<tag>` ready to publish?". Also
invoke to audit an already-published release end to end.

## Permission posture

Used by the `delonix-release-manager` agent at **Tier C, PROPOSE-ONLY** (see
`.ai/agents/permission-tiers.md`). This skill NEVER publishes: no `git tag`, no
`git push`, no `gh release create`, no `gh workflow run release.yml`, no setting
of signing secrets. It produces a GO/NO-GO and the exact commands for a human.

## References (the real assets — read them, do not reinvent)

- `scripts/release_gate.py` — a tag may not publish unless cargo-test + e2e +
  chaos + critest are recorded PASS AT the tag's commit
  (`reports/release-validation/<tag>.json`) and CI is green. Run its `check`
  subcommand (read-only): `release_gate.py check --tag <t> --commit <sha>
  --require-ci --repo <owner/name>`.
- `scripts/release_verify.py` — refuses a release whose maturity-table evidence
  (`delonix system features -o json`) has rotted.
- `scripts/verify_release_assets.py` — the post-publish end-to-end proof
  (signature verifies against the install.sh pubkey, every asset matches
  `SHA256SUMS`, the SBOM rides INSIDE the signed manifest, SLSA provenance
  verifies). Now wired into `release.yml` as a post-publish step.
- `.github/workflows/release.yml` — the pipeline: `guard` (tag shape/exists +
  release gate), build x86/v3 + native arm64, SBOM (`sbom.py`), minisign
  signing, `attest-build-provenance`, publish, post-publish verify.
- `VERSIONING.md` (the 5.1.1→0.1.0 re-base, pre-1.0 policy, `version_gate.py`),
  `MIGRATION.md`, `CHANGELOG.md`, `docs/releases/<tag>.md` (release notes).

## Method

1. **Version** — `scripts/version_gate.py`: workspace version aligns with the
   tag; release notes present. Confirm `CHANGELOG.md` and (if a migration)
   `MIGRATION.md`/`VERSIONING.md` are consistent.
2. **Gate at the commit** — `release_gate.py check --tag <t> --commit <sha>
   --require-ci`. A tag whose suites weren't recorded green ON ITS OWN COMMIT is
   NO-GO, full stop (the v5.1.1 lesson).
3. **Evidence not rotten** — `release_verify.py`.
4. **Supply chain** — confirm `release.yml` will sign (minisign secret present
   when the install.sh pubkey is real), build the SBOM into the signed manifest,
   and attest provenance; and that the post-publish `verify_release_assets.py
   --tag` step is present.
5. **Verdict** — GO only if every step passed. Otherwise NO-GO with the failing
   reason named.

## Expected output

A GO / NO-GO with evidence per step, and the exact commands the HUMAN runs to
publish (e.g. `git tag`/`git push` of the tag, then `gh workflow run
release.yml -f tag=<tag>`), for them to execute. For an already-published
release, the `verify_release_assets.py --tag` result and whether it holds end to
end.

## Validation strategy

No GO on "by design": a check that could not be run (no network for provenance,
signing secret unknown) is reported as NOT verified, never assumed. A release
whose pipeline, signature, SBOM or attestation was not verified is NO-GO.
