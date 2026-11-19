# Prioritised backlog — from the `fa8c6297` audit

Each item points to the line in `gap-matrix.csv`. Nothing here has been implemented.

## P0
None confirmed. **Reproduce G-007** (force the tmpfs mount failure in an integration test) — if it
confirms secrets written to the upper layer, it rises to P0.

## P1 — production blockers
1. **G-008** fail-closed anti-spoof: `run_ok` → `run` with the error propagated in all three sites; attach fails
   if the rule does not land. Test: this audit's nft shim (`raw/g008-nft-shim.sh`) as an integration
   test. Requires respawning the holder on rollout.
2. **G-007 / G-026** fail-closed `write_secret_files`: secrets requested and not delivered → `run` fails.
3. **G-029** Docker API: reconcile state before responding to `ps`/`inspect`.
4. **G-027** `docker run -d` hangs — investigate the create+start+attach flow of the Go CLI.
5. **G-003** chaos job that fails instead of `skipped` when it cannot run.
6. **G-036** (delonix-paas) use `delonix_net::parse_publish` in the reaper.
7. **G-005** CRI: pull by digest and `Username` in image status (bugs); RunAsUserName; OOMKilled
   (verify `4a4d029f` against the candidate release).
8. **G-024** rootful battery on a throwaway VM.

**Left this list:** **G-009** (masked/readonly paths and rlimits → fail-closed) is done and
measured — see the `G-009` line in `gap-matrix.csv`, now `PASS`.

## P2
- G-011 unknown nested Compose sub-keys → refuse.
- G-033 tag+digest in conflict → refuse.
- G-037 (paas) forwards without container registration outside the reaper.
- G-010 rro via `mount_setattr(AT_RECURSIVE)`.
- G-012 Docker API: fields ignored without warning.
- G-020 pin actions by SHA.
- G-021 aarch64 artefacts; verify signature/provenance of the assets.
- G-025 conformance docs regenerated (one number, version, date, command).
- G-028 `docker run --rm` with no orphans.
- G-030 `critest.sh` with both roots isolated and correct cleanup.
- G-031 gRPC error logging in `delonix-cri`.
- G-041 pause→Stopped (already in origin/main — confirm in the release).
- G-002 live tests with an explicit skip instead of a silent pass.

## P3
G-014, G-015, G-016, G-017, G-018, G-034, G-035, G-038, G-039, G-042, G-004 (check threshold), G-006, G-022.
