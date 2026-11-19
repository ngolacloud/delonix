# 60 — P4/D5: the launcher/holder binary pair, for real (ADR-0044)

**Date:** 2026-09-19 · **Base:** `origin/main` (`fd11501a`, v4.0.0 +10 commits, the binary
from this SAME spike series) · **Target:** `p4d-launcher`, new VM (libvirt, Ubuntu 24.04.4
LTS, `delonix-vm-base:ubuntu-24.04`) — the same environment as P1b, rebuilt from scratch.
**Verdict: GO, with a finding that simplifies D2.4** — `--net <custom-network>` does not need
the launcher's profile, because it never creates its own userns.

## The question

The last of the five spikes of ADR-0044: "Repeat P1b's exact matrix with `delonix`,
`delonix-launcher` and `delonix-netns-holder` as three genuinely separate installed
binaries — the gap the original spike's own «not validated» section names — and add
`--net <custom>` and a pod to the matrix, neither of which P1b touched."

P1b (`53_P1B_LAUNCHER_SPIKE.md`) had measured this: (1) with the CALLER and the pin on the
SAME installed path, toggling only the presence of the AppArmor profile per operation; (2)
without `--net <custom>` or a pod, the two cases that genuinely need the holder.

## How it was measured

- New VM (P1b's own no longer existed — cleaned up by another session), created by
  `delonix vm create --backend libvirt --disk delonix-vm-base:ubuntu-24.04`. Environment
  confirmed identical to P1b: Ubuntu 24.04.4 LTS, kernel 6.8.0-136-generic,
  `kernel.apparmor_restrict_unprivileged_userns=1`, `apparmor_parser` 4.x.
- **Side finding, on entry**: `--ssh-key` did not inject the key (the `authorized_keys`
  file stayed empty, despite the CLI printing "log in with the key you injected" —
  `AGENTS.md` already has an analogous finding catalogued, "a capability that only
  exists...", but this one is new and is recorded here, not investigated in depth — out of
  scope for this spike). Access obtained via the serial console (autologin, as P1b also
  used), injecting the key by hand.
- The binary from this SAME series (`fd11501a`, with the replacement spike already inside)
  copied to the guest and installed in **three hardlinked paths** (`ln`, not `cp` — the
  golden disk at 2.4 GiB had no room for three copies of 282 MB; AppArmor confines by
  PATH, not by inode, so the hardlink preserves exactly the variable the spike tests):
  `/opt/p4d/delonix` (no profile), `/opt/p4d/delonix-launcher`,
  `/opt/p4d/delonix-netns-holder`.
- Two AppArmor profiles, the same mould as `install.sh` (`profile <name> <path>
  flags=(unconfined) { userns, }`), one per privileged path — none on bare `delonix`.
  Confirmed that the golden image's pre-existing profile (`/etc/apparmor.d/delonix`,
  pointing at `/usr/local/bin/delonix`, from an earlier install) does not interfere —
  different path.
- P1b's matrix repeated (now with three GENUINELY distinct paths, not one profile
  toggled on/off at the SAME path) + the two operations that were missing.

## What was measured

| Operation | `delonix` (no profile) | `delonix-launcher`/`delonix-netns-holder` (with profile) |
|---|---|---|
| `container run --net none` | `EPERM: failed to prepare the rootfs`, rc=126 | rc=0 |
| `container run -d` + `exec` | *(not repeated — already covered by P1b)* | rc=0 / rc=0, same wording |
| `container cp` on a stopped one (`__ovlhold`) | *(ditto)* | rc=0 |
| `container rm` (`__rmtree`) | *(ditto)* | rc=0 |
| `net netns up` (pin) | **self-explanatory** error (new since P1b — names both the cause AND the remedy), rc=1 | rc=0 — `ps` confirms `/opt/p4d/delonix-netns-holder netns pin`/`netns control`, BOTH processes running from the right path |
| **`container run --net <custom-network>`** (NEW) | **rc=0** — works WITHOUT a profile (see "Finding", below) | rc=0, real IP on the custom network (`10.253.0.2/16`) |
| **`pod create`** (NEW) | `EPERM: failed to create the pod netns`, same self-explanatory text as the pin, rc=1 | rc=0 — `10.200.0.2`, shared netns, `exec` inside the pod works |

## Finding — `--net <custom-network>` does not need the launcher's profile, and the reason
is already written into the mechanism itself

Measured, not hypothetical: `delonix container run --net p4dnet alpine:3.20 echo hi`
**succeeded without any AppArmor profile at all**, assigning a real IP (`10.253.0.2/16`) —
unlike `--net none`, which fails without the profile.

The reason is already written in this repository's `AGENTS.md`, "CLI" section (`container
run`): the re-exec for `--net <custom-network>` runs `nsenter -t <holder_pid> -U -m -n …
ip netns exec <netns>`, and this SECOND pass runs with `RunSpec.inherit_userns` — it
suppresses `CLONE_NEWUSER`/`CLONE_NEWNET` and **inherits** the holder's namespaces instead
of creating its own. `nsenter -U` to ENTER an already-existing userns is not the same call
as `unshare(CLONE_NEWUSER)` to CREATE one — and it is only the second that AppArmor's
`userns` rule mediates. Since the userns already exists (created ONCE, when the holder
started — that creation DID need the profile), joining it needs no privilege that AppArmor
would deny.

**This simplifies the real D2.4**: not every operation that today "creates namespaces"
needs the `delonix-launcher` profile — only those that create a userns from SCRATCH
(`--net none`/default network, `pod create` — which creates ITS OWN netns from zero,
confirmed by the "failed to create the pod netns" error, different from the pin's error).
A container that attaches to an already-existing custom network, with the holder already
up, lives entirely off the holder's PRIOR creation of the userns. **Not fixed nor built
here** — it is a finding for whoever designs D2.4/D3's capability matrix for real: the
boundary is "who CREATES a namespace", not "who touches namespaces".

## Confirmations that match P1b

- The launcher remains GO: all namespace creation FOR CONTAINERS passes with the profile
  only on its own path.
- **The holder's condition is CONFIRMED, now with real separation**: the pin/control
  processes run from the actual `delonix-netns-holder` binary, not from a copy of
  `delonix` pretending to be the holder — `ps aux` proves it via `/proc/<pid>/comm`/the
  executable's path, not by reading the code.
- The pin error without a profile IMPROVED since P1b: it is no longer a generic timeout
  ("waiting for the netns holder") nor the `/run` vs `/tmp` defect P1b had to fix by
  reading — it is now a message that names the cause (`kernel.
  apparmor_restrict_unprivileged_userns=1`) and the remedy (`install.sh` writes the
  profile) — the same class of fix this repository had already applied to other privilege
  errors.

## Proven vs not validated

**Proven**: the three binaries, on genuinely distinct paths, with independent profiles,
converge correctly — including the two cases P1b never touched; that `net netns up`/
`pod create` need the profile and `container run --net <custom>` does not, with the
mechanical reason already documented elsewhere in this repository, now confirmed live;
that the error without a profile is today self-explanatory on both paths that require it.

**Not validated**: in-place upgrade with the THREE real binaries (P1b only read the code
for this part, see its item 4; this spike also did not run it — it would need an EARLIER
version of the binary installed first); a `delonix-launcher`/`delonix-netns-holder` as
DISTINCT crate/`[[bin]]` targets with their own dispatch (this spike, like P1b, uses three
hardlinked copies of the SAME binary — the real code split is P4d, not this spike);
libvirt CH was not re-tested here (already covered by spike #1); Debian/Rocky/other
distros; root (rootless only).

## Cleanup

VM `p4d-launcher` destroyed (`delonix delete vm`) — nothing was left on the host beyond
what was already there. State inside the guest (network `p4dnet`, pod, containers) died
with the VM.
