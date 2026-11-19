# Migrating to Delonix (the 0.x line)

The `delonix-runtime` product has been renamed to **Delonix** and re-based its version from
`5.1.1` to `0.1.0` (a brand-new line, on purpose — see [VERSIONING.md](VERSIONING.md)).
This guide has two parts: for those who **use** the engine, and for those who **maintain** the repository.

## For users — `5.1.1` → `0.1.0`

**What changes:** the name and the addresses (repo, Pages, GHCR image), and the
version number. The CLI binary is still called `delonix`.

**What does NOT change** (operational contracts kept stable across the re-base):
- Persisted state: `/var/lib/delonix/`, rootless `~/.local/share/delonix`.
- Config: `/etc/delonix/`. Sockets: `/run/delonix-{cri,mgmt,node}.sock`.
- systemd services, `DELONIX_*` variables, `*.delonix.io` schema groups.
- Your installation keeps its data, images, volumes and configuration.

**How to update:**
```
curl -fsSL https://github.com/ngolacloud/delonix/releases/latest/download/install.sh | bash
```
GitHub's "latest" is by **date**, so it resolves to `0.1.0` (not by SemVer
ordering). The **five binaries** (`delonix`, `-cri`, `-mcp`, `-mgmt`, `-node-api`)
must all stay on the same version — the engine rejects misaligned sibling binaries
(`check_version` by exact equality) and `install.sh` warns if it finds one on the
PATH from another version.

**Auto-update:** a tool that compares versions by SemVer sees `0.1.0 < 5.1.1` and
will **not** update on its own — reinstall via the `install.sh` above. This is a
line transition, announced, not an incremental update.

**Rollback:** the `v1.*`…`v5.1.1` releases remain published (immutable tags,
never overwritten); you can roll back to one of them.

## For maintainers — GitHub transfer runbook

Transfer the `angolardevops/delonix-runtime` → `ngolacloud/delonix` repository.
**Prefer TRANSFERRING** (preserves history, issues, PRs, releases and redirects)
over recreating.

**Pre-flight (check BEFORE):**
- [ ] correct source; admin on the source and on the `ngolacloud` org; `delonix` name free at the destination.
- [ ] branch protections / rulesets (main, develop) to recreate at the destination.
- [ ] Actions, environments, self-hosted runners.
- [ ] secrets / OIDC / publishing credentials (minisign `MINISIGN_SECRET`, GHCR token).
- [ ] GHCR packages and images; apps, webhooks, deploy keys.
- [ ] GitHub Pages and custom domain (there is a `.nojekyll`, no CNAME today).

**Transfer:** Settings → *Transfer ownership* → `ngolacloud`, and rename
`delonix-runtime` → `delonix` (or transfer already under the new name).

**Post-transfer (VALIDATE — do not assume the redirects are enough):**
- [ ] `git remote set-url` + `clone` from the new address.
- [ ] green CI on the new org (minimal permissions reconfigured).
- [ ] release downloads (assets by `v{version}`) and `SHA256SUMS(.minisig)`.
- [ ] Pages at the new URL (`ngolacloud.github.io/delonix` or domain) and the
      `schema/v1/*.json` resolving.
- [ ] GHCR `ghcr.io/ngolacloud/delonix`.
- [ ] `vmimage.rs` downloads `delonix-cri` from the new release.
- [ ] external integrations re-pointed.

**Only then** complete the identity and cut the release:
1. `scripts/rebrand.sh build-coupled` (Rust constants, `gen.py`, `docs/dev`, proto `go_package`).
2. rebuild the 5 binaries at `0.1.0` + `python3 docs/gen.py` + `scripts/dev_docs_site.py` + `buf generate`.
3. `docs/releases/v0.1.0.md` + `scripts/version_gate.py` green (opens the delonix line).
4. `scripts/release_verify.py` + pipeline (`release.yml`): SHA256SUMS, SBOM, minisign, SLSA.
5. tag `v0.1.0` and publish. See the checklist in [VERSIONING.md](VERSIONING.md#checklist-before-tagging-v010).

## For contributors — internal crate renames (ADR-0040 R1)

The engine restructuring (ADR-0040) standardized **internal crate** names
(`publish = false`), without touching anything exposed to the user — binaries, sockets,
systemd services, `DELONIX_*`, schema groups and CLI/API contracts stay the same.
Anyone with a branch to integrate re-points their imports:

| Before | After | Rust import |
| --- | --- | --- |
| `delonix-proxmox` | `delonix-provider-proxmox` | `delonix_provider_proxmox` |
| `delonix-truenas` | `delonix-provider-truenas` | `delonix_provider_truenas` |
| `delonix-opnsense` | `delonix-provider-opnsense` | `delonix_provider_opnsense` |
| `delonix-powerdns` | `delonix-provider-powerdns` | `delonix_provider_powerdns` |
| `delonix-security-runtime` | `delonix-security` | `delonix_security` |
| `delonix-runtime-bin` (in `bins/delonix-runtime-bin`) | `delonix-bin` (in `bins/delonix`) | — (it is a bin, no lib) |

Moved with `git mv` (so `git log --follow`/`blame` keep tracking). `scripts/arch_fitness.py`
now rejects a crate in `providers/` without the `delonix-provider-` prefix. The
binary **is still `delonix`** — only the package name and the directory changed
(`delonix-bin` is the only one in `bins/` whose directory diverges from its name, A6a); anyone
invoking `cargo … -p delonix-runtime-bin` switches to `-p delonix-bin`.

## Status of this transition (2026-10-28)

- **Done and integrated into `develop`:** safe identity in config/docs
  (`Cargo.toml`/`Makefile`/`README`/units/`sbom.py`), line-aware `version_gate`
  + tests, compatibility policy (VERSIONING.md), consolidated CHANGELOG,
  addendum to ADR-0061, `scripts/rebrand.sh`.
- **Pending (depends on the transfer and the release build):** `rebrand.sh
  build-coupled`, regenerating the docs with the new identity, and the `0.1.0`
  release itself. The transfer is an irreversible external operation on a live
  public repo (while cutting 5.x releases) — run it with the pre-flights satisfied.
