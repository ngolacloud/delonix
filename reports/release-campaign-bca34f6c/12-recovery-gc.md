# 12 — Recovery and GC under coexistence

SUT: `delonix 5.1.1` @ `bca34f6c3`, rootless.

| # | Case (§19) | Observed | State |
|---|---|---|---|
| RECOVERY-2 | **GC with active resources** (`system prune` with live containers) | `--dry-run` separates DECLARED (0 containers/images to destroy without `-a`) from DEBRIS; `prune -f` did not touch the live ones | **PASSED** |
| RECOVERY-2b | **opportunistic GC of orphan cgroup leaves** (feat `startup-cgroup-gc`, commit 9d0d2dc3) | orphan leaf `dlx-relc0orphan000` created by hand was **reclaimed** during `run` AND later on a plain `ps` (daemonless, on `open_stores`); the 3 live leaves (relc-keep + 2 kaeso) intact | **PASSED** |
| RECOVERY-1 | resilience to control-client death | `run -d` with SIGKILL to the client at 0.4s → the detached supervisor **completed** creation of the 3 containers; **no half-state, no orphans** (`prune` reclaimed 0); correct ports | **PASSED** (resilience property) |

## Reading

- **Opportunistic leaf GC is real and safe live @ HEAD:** it reclaims debris (orphan cgroup
  leaves) even with active resources, and does **not** remove what is in use. Validates the
  project-memory feature at the current commit.
- **`prune` is safe by default:** without `-a` it does not remove tagged images (the many host images
  marked `ORPHAN=yes` would **not** be deleted by a default prune) nor running containers; the
  `--dry-run` shows the two halves separately (declared vs debris) before acting. Honest about
  rootless limits ("part of the tree was unreadable from outside the user namespace … lower bound").
- **Resilient daemonless model:** killing the client that launched the `run -d` leaves no half-state — the
  detached supervisor completes the operation. It is a positive property, **but not** the same as
  interrupting midway through the critical section.

## Limits (honesty)

- **I did not run** the repo's rigorous chaos harness (`scripts/chaos.sh`, `e2e_teardown.sh`,
  `net-lifecycle-leaks.sh`) that does SIGINT/SIGKILL midway through each mutation and counts orphan
  veth/bridge/nft/mount systematically. My RECOVERY-1 proved resilience to client death, not
  mid-mutation interruption. → rigorous §19 coverage still rests on that harness (candidate to run in a
  next round).
- I did not force a write failure on a space-constrained volume, a reboot, nor remote unavailability (providers).

## Preservation

All `relc-*` resources removed; `kaeso-odoo`/`kaeso-db` intact throughout; no
pre-existing image/network deleted; disk not full (kept ~96%, never pushed to the limit).
