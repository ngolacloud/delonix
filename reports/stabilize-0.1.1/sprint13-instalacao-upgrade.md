# §13 — Installation, upgrade, removal and data preservation

**Candidate:** `delonix 0.1.0` @ `8ab783dd` (line `stabilize/0.1.1`).
**Date:** 2026-10-29. **Method:** inspection of `install.sh`/`MIGRATION.md`/
`Makefile` + deterministic validation of the logic; the clean-environment E2E is
marked as pending on a disposable VM/container (not validated on the dev host).

## Summary by §13 item

| Item | State | Evidence / note |
|---|---|---|
| Requirements | ✅ covered | `install.sh` detects the distro via `/etc/os-release` (apt/dnf/zypper/pacman) and installs runtime deps only if they are missing |
| Installation | ✅ covered | `install.sh` (binary+deps+host config); `--user` = rootless binary in `~/.local/bin` without sudo |
| Configuration | ✅ covered | `providers.yaml` via `--vm-provider` (ADR-0054); an existing file is **never** overwritten |
| Permissions | ✅ (root) | subuid/subgid, AppArmor, systemd — root operations |
| Initialization | ✅ covered | rootless-first, daemonless; on-demand networking |
| **Diagnostics** | ✅ covered | `delonix system doctor` + `delonix system info` (observed reporting state root, mode, cgroup2) |
| **Upgrade** | ✅ + fix | `install.sh` re-runs; `check_version` enforces the 5 sibling binaries on the SAME version; **FIX** `dab971fe`: 5.x→0.1.x warning surfaced up front |
| **Migration 5.x→0.1.x** | ✅ documented + surfaced | `MIGRATION.md` (latest-by-date=0.1.0; SemVer auto-update does NOT trigger→reinstall; rollback to immutable v5.* tags); now **warned during upgrade** |
| **Data preservation** | ✅ by construction | `install.sh` has NO `rm` of `~/.local/share/delonix` nor `/var/lib/delonix` — it only deletes temporaries (l.144/1256). Upgrade only replaces binaries. P3 guards against silent state downgrade |
| **Removal (uninstall)** | ✅ RESOLVED | `install.sh --uninstall` (preserves data by default; `--purge` removes state/config; honours `--user`; disables boot first; names the shared host config). `bash -n` + logic tested in both modes. Before: only `make uninstall` (from-source) |
| Clean-environment E2E | ⚠️ PENDING on environment | A full install (deps via package manager + systemd + subuid) requires a supported distro + root → **validate on a disposable VM/container**, not on the dev host (§18: absence of environment ≠ approval) |

## Correction applied (`dab971fe`)

`install.sh` only warned on the removal of Kinds `0.64→0.65`. A
`1.*|2.*|3.*|4.*|5.*)` branch was added to the version check BEFORE replacing the
binary: it names the re-base as deliberate (not a downgrade), asserts the
preservation of state/images/volumes/config/sockets/services, reminds of the
5 sibling binaries' alignment, and points to `MIGRATION.md`. `bash -n` clean;
the `case` logic exercised for `5.1.1` (warns re-base), `0.64.0` (warns Kinds),
`0.1.0`/empty (no warning — fresh install).

## Link to F-1 (healing on upgrade)

The upgrade path has a state effect already proven in this campaign: image
layers unpacked by a pre-fix binary are **healed on first use** after the upgrade
(`LAYER_EPOCH` format epoch, live A/B proof with rabbitmq). That is, the binary
upgrade + first use reconciles the on-disk state that the upgrade itself does not
touch.

## Gaps / recommended actions for GO

1. **Operator uninstall:** give `install.sh` a `--uninstall` (removes the 5
   binaries from `BIN_DIR`, preserves the state dir by default, with an opt-in
   `--purge` to delete it), OR document the explicit manual procedure. P3/low.
2. **Clean-environment E2E:** run `install.sh` on a VM/container of each
   supported distro (apt/dnf/zypper/pacman): install → `system doctor` green →
   re-run (upgrade, data preserved) → remove. Blocks the
   "production-supported installation" claim until done.
3. **Branding nit:** the `--help` header still says «Delonix **Runtime**» —
   align with the `Delonix` identity.
