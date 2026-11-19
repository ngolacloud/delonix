# Delonix versioning and compatibility

## Two version lines

The product was renamed from `delonix-runtime` to **Delonix** and **deliberately
re-based its version number**:

| Line | Versions | State |
|---|---|---|
| `delonix-runtime` | `v1.0.0` … `v5.1.1` (2026-08-29 … 2026-10-15) | **frozen** — historical, does not continue |
| **Delonix** | `0.1.0` onward | active |

`0.1.0 < 5.1.1` in pure SemVer. This is **intentional** — a new line, not a
regression. Tools that compare versions must know this; see below.

## Compatibility policy (pre-1.0)

While the Delonix line is in `0.x` (before `1.0.0`):

- **Patch** (`0.1.0 → 0.1.1`): no incompatible contract changes.
- **Minor** (`0.1.0 → 0.2.0`): **may** break CLI/API/JSON/schema contracts;
  each break is documented in the [CHANGELOG.md](CHANGELOG.md) as `BREAKING`.
- API stabilization for strict SemVer arrives with `1.0.0`
  (`docs/cli-stability.md` is the contract that applies from then on).

**Operational contracts that do NOT change with the re-base** (they were stable
on the 5.x line and continue): state directories (`/var/lib/delonix`,
`/etc/delonix`, `~/.local/share/delonix`), sockets (`/run/delonix-*.sock`),
systemd services, `DELONIX_*` environment variables, and the `*.delonix.io`
schema groups (`config.delonix.io/v1`, `compute.delonix.io/v1alpha1`, …). A
5.1.1 installation keeps its state when moving to 0.1.0.

## What the 5.1.1 → 0.1.0 downgrade touches (measured)

| Mechanism | Downgrade effect | Handling |
|---|---|---|
| `scripts/version_gate.py` | Broke: `0.1.0 < 5.1.1` (tag contained) | **Resolved** — line-aware gate; it only compares within the line and requires containing the most recent `v5.*`. Tests in `scripts/test_version_gate.py`. |
| `check_version` (`delonix-node/src/dispatch.rs`) | Exact equality between `delonix` and the co-installed server binaries | Rebuild **all 5 binaries** (`delonix`, `-cri`, `-mcp`, `-mgmt`, `-node-api`) to `0.1.0` **together**. `install.sh` already warns about sibling binaries of a different version in the PATH. |
| `vmimage.rs` (`:4799`, `:4925`) | Downloads `releases/download/v{version}/delonix-cri-*` | The `v0.1.0` release must exist with the 5 binaries + `SHA256SUMS(.minisig)`. |
| GitHub "latest release" | Is by **date**, not SemVer | A fresh installation via `install.sh` (`VERSION=latest`) obtains `0.1.0` without issue. |
| `install.sh` migration warning | **Prefix** match (`0.64.*`), not ordering | Harmless on the downgrade. |
| Third-party SemVer comparators | Would see `0.1.0 < 5.1.1` → do not update | **Explicit transition**: announce the new line/channel; do not rely on auto-update by ordering. Document the migration step. |

## Tagging rules

- **Never** overwrite the `v1.*`…`v5.*` tags or republish immutable releases.
- The Delonix line tags remain in the `v<ver>` format (`v0.1.0`), because
  `release.yml` and `vmimage.rs` compose `v{version}`.
- The 5.x line stays as historical releases; `v0.1.0` becomes the "latest" by
  date.

## Checklist before tagging `v0.1.0`

1. Identity renamed and repository at `github.com/ngolacloud/delonix` (URLs,
   `repository`, GHCR image, Pages, workflows).
2. `Cargo.toml` `workspace.package.version = "0.1.0"`.
3. `docs/releases/v0.1.0.md` written (`version_gate.py` requires it to open the
   line; `release.yml` publishes it).
4. The 5 binaries rebuilt to `0.1.0` (dispatch by exact equality).
5. `python3 scripts/version_gate.py` green (opens the delonix line at 0.1.0).
6. `python3 scripts/release_verify.py` and the pipeline (`release.yml`) validated:
   SHA256SUMS, SBOM SPDX, minisign signature, SLSA provenance.
7. Test a clean installation and an upgrade from an existing 5.1.1 (state
   preserved; co-installed binaries coherent).

## Implementation note

`version_gate.line_of()` separates the lines by major (0 = Delonix; 1..5 =
`delonix-runtime`). When the Delonix line approaches `1.0.0`, this cutoff must
be revisited (the legacy `v1..v5` tags can no longer be distinguished by major) —
see the warning at the top of `scripts/version_gate.py`.
