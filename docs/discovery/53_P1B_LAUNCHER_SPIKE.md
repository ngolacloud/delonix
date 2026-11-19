# 53 — P1b: spike of the `delonix-launcher` and the network holder (ADR-0040)

**Date:** 2026-09-16 · **Base:** `fix/p0-findings` (`636ccaf1`, v3.1.0 +64 commits) ·
**Verdict: GO with one condition** — the network holder must create its namespaces
inside its own process.

## The question

ADR-0040 (D2.4) splits the `delonix` executable into binaries with one role each, and
wants **only** `delonix-launcher` and `delonix-netns-holder` to need to create user
namespaces. On an Ubuntu with `kernel.apparmor_restrict_unprivileged_userns=1`, that means
an AppArmor profile with `userns` on those two paths and **none** on the CLI. The P1b
gate question: does this work, or does the restriction break the engine from within?

## How it was measured

- VM `p1b-launcher` created by the engine itself from `delonix-vm-base:ubuntu-24.04`
  (libvirt, 2 vCPU, 2 GiB), state root isolated on the host.
- Inside the guest: Ubuntu 24.04.4 LTS, kernel 6.8.0-136-generic,
  `kernel.apparmor_restrict_unprivileged_userns = 1`, AppArmor parser 4.0.1,
  util-linux 2.39.3, uidmap 4.13. Unprivileged user (`delonix`, subuid
  `100000:65536`).
- **The same binary** (built from this tree, no symbols) installed on three paths:
  `/opt/p1b/delonix` (no profile, acts as CLI), `/opt/p1b/delonix-launcher` and
  `/opt/p1b/delonix-netns-holder`. AppArmor authorises by path, so three copies of the
  same file are the honest way to isolate the variable.
- The matrix (`53_P1B_LAUNCHER_SPIKE/matrix.sh`) runs the namespace-creating operations:
  `container run`, `run -d`, `exec`, `cp` on a stopped container (re-exec `__ovlhold`),
  `rm` (re-exec `__rmtree`) and the network pin (`net netns up`). Each path has its own
  root.
- Two runs, with the raw output stored alongside this file: without profiles
  (`matrix-base.txt`), and with `unconfined + userns` profiles only for the launcher and
  the holder (`matrix-profiles.txt`). Control: the `/usr/local/bin/delonix` the image
  ships, with the profile `install.sh` writes.

## What was measured

| Operation | CLI without `userns` | Launcher with `userns` |
|---|---|---|
| `image load` | rc=0 | rc=0 |
| `container run --net none` | **rc=126** `failed to prepare the rootfs: EPERM` | rc=0 |
| `container run -d` + `exec` | run **rc=0**, exec rc=3 `container is not running` | rc=0 / rc=0 |
| `container cp` on a stopped one (`__ovlhold`) | **rc=1** `__ovlhold unshare: EPERM` | rc=0 |
| `container rm` (`__rmtree`) | rc=0 | rc=0 |
| `net netns up` (pin) | **rc=1** `timeout waiting for the netns holder` | rc=1 `control socket: No such file` — **not AppArmor**, see «Correction» |

1. **Launcher: GO.** With the profile only on the launcher's path, everything that
   creates namespaces for containers passes — including the `__ovlhold`/`__rmtree`
   re-execs, which re-exec `current_exe()` and so inherit the path, and with it the
   profile. The same binary on another path, without a profile, is refused. D2.4's split
   is viable.
2. **Holder: NOT per the D2.4 split, and the proof here was the wrong one** (see
   «Correction», below). The pin starts with `Command::new("unshare") --user
   --map-root-user --net --mount -- <exe> netns pin` (`delonix-net/src/infra.rs`,
   `start_pin`), and it is **`/usr/bin/unshare`** that creates the user namespace. This
   only passes while the CALLER has a profile with `userns`, because an `unconfined`
   profile is inherited on `exec`; a caller without a profile makes the `unshare(1)` hit
   `unprivileged_userns` and the `mount` is denied. Giving `userns` to `/usr/bin/unshare`
   would open user namespaces to any user and void the restriction — not an option.
3. **The path that works is already proven by the launcher:** a binary with a profile
   that calls `unshare(2)` and `newuidmap` INSIDE its own process (`reexec_mapped` does
   exactly this) passes. The condition for GO is the holder doing the same.
4. **In-place upgrade (read in the code, no VM):** the pin is recognised by the `netns
   pin` pair in the argv and by the environment variables the engine sets
   (`argv_matches`, `env_names_this_root`), never by the binary's name — and there is
   already precedent for accepting the old spelling (`is_pre_split_holder`, `netns
   holder`, prior to v0.42.0). A `delonix-netns-holder` recognises a pin started by
   `delonix netns pin` as long as it keeps that rule; it is a pure test once the binary
   exists.

## Correction (2026-09-16, after the spike)

The pin line in the launcher column **was misread**. The raw output
(`matrix-profiles.txt`) says `control socket: No such file or directory`, not a holder
timeout: the matrix set `DELONIX_NET_RUNTIME_DIR` to `/run/user/<uid>/…`, and the control
process mounts a tmpfs on `/run` inside the pin's mount namespace, which hides the socket.
It is its own defect, independent of AppArmor.

Remeasured on the same image, with the restriction set to 1 (`pin-matrix.sh`, output in
`pin-matrix-result.txt`): the base binary (`unshare(1)`) and the binary with the pin
creating namespaces in its own process, each on one path with a profile and another
without, and the runtime folder in `/tmp` and in `/run`.

| Binary | Profile on path | `/tmp` | `/run` |
|---|---|---|---|
| base, `unshare(1)` | yes | **UP** (456 ms) | `control socket: No such file` |
| pin in its own process | yes | **UP** (393 ms) | `control socket: No such file` |
| base, `unshare(1)` | no | 5 s timeout | 5 s timeout |
| pin in its own process | no | refused in ~170 ms with the reason (`mount … EACCES`) | same |

The audit explains the first line: in the runs with a profile there is no
`userns_create` landing on `unprivileged_userns` — `unshare(1)` inherited the caller's
profile. In the runs without a profile there is, with `comm="unshare"`. Therefore:

- **With the CLI and the holder on the same profiled path (today's `install.sh`),
  `unshare(1)` works.** The spike did not show the contrary.
- **D2.4's condition still holds, for a different reason:** with the profile only on the
  holder, the caller has no profile, and the `unshare(1)` it launches hits
  `unprivileged_userns` — measured in the «no profile» line. It has to be the holder's
  own executable calling `unshare(2)`.
- **A runtime folder under `/run` breaks control** on any binary. Recorded as a separate
  defect.

## Side finding

**`container run -d` reports rc=0 with a dead container.** With the restriction active
and no profile, the container's init dies preparing the rootfs (`EPERM`), `run -d`
returns the id and **exits 0**, and only the following `exec` says `container is not
running`. `AGENTS.md` already recorded this as a decision pending («a `run -d` whose init
dies while mounting keeps reporting what it reported»); here it is measured on a real host
with a common cause. Deserves its own fix: a script or CI reading this `0` as success.

## Not validated

- In-process creation was written afterwards (branch `fix/holder-in-process-userns`) and
  measured in the «Correction» table, but **with the CLI and the pin on the same path**:
  the split into two executables does not exist yet.
- `--net <custom-network>` and pods (which need the holder) were not measured.
- Rootless only, libvirt only, Ubuntu 24.04 only. The golden image used is from
  2026-08-12.
- Debug build (no symbols), not a release.
