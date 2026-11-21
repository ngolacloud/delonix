# G1f — rootful + SELinux-enforcing: assessment and scope decision

**Date:** 2026-11-01. **Host:** Zorin OS 18.1 (Ubuntu base, LSM=AppArmor;
`/sys/fs/selinux` absent). **Binary:** `target/release/delonix` @ feat/ipv6-phase1.
**Decision:** ADR-0083.

Closes item (f) of the G1 gate in the NO-GO→GO plan: *"rootful and
SELinux-enforcing assessed (or declared out of scope in writing)"*. Rootful was
**measured** on this host (passwordless sudo available); SELinux-enforcing was
**assessed from the code** — this host has no SELinux, so a real measurement would
require a Fedora/RHEL VM (not done; see ADR-0083 for the scope decision).

## Rootful battery (measured, `sudo -E DELONIX_ROOT=<scratch>`)

| # | What | Result |
|---|---|---|
| 1 | effective uid inside the container | ✅ `uid=0(root) gid=0(root)` — real root, not userns |
| 2 | `/dev` via **mknod** (not bind), real nodes | ✅ `/dev/null` `crw-rw-rw- 1,3`, `/dev/zero` `1,5` |
| 3 | `--memory 64m` visible from inside | ⚠️ `cat /sys/fs/cgroup/memory.max` → empty from inside the container; the container's cgroup sits in `system.slice` (root), not the exposed view. **Limit enforcement not confirmed conclusively** — needs dedicated verification (read `memory.max` of the host-side PID's cgroup) |
| 4 | **`FROM scratch` build** rootful | ❌ COPY+RUN run, but **packaging fails**: `DX-1410 image has no layers`. Rootless packages the same Dockerfile; it is **specific to rootful+scratch** (the rootful OCI commit path does not handle a zero-layer base even after COPY/RUN add content) |
| 4b | rootful build `FROM alpine:3.12` (normal base) | ✅ COPY+RUN+packaging ok (image created) — isolates #4 as scratch-only |
| 5 | network + `run -d`/`exec`/`rm -f` | ✅ detached container, `exec` with a network interface, clean removal |
| 6 | `image ls` rootful | ✅ the rootful store lists the images |

**Rootful verdict:** the core (real root, `/dev` via mknod, network, lifecycle,
exec, store) **works**. Two edges recorded, neither blocking normal use: **(#4)**
`build FROM scratch` fails in rootful packaging — a bug distinct from the commit
path (the three scratch bugs fixed on this line were in startup/rootless; this one
is in the rootful commit); **(#3)** rootful memory-limit enforcement to be
confirmed with a dedicated test.

## SELinux-enforcing (assessed from the code)

This host has no SELinux; the assessment is of what the engine does, not measured
on an enforcing target.

- **PROCESS label:** ✅ supported. `apply_selinux` (delonix-linux `lib.rs:3388`)
  writes the context to `/proc/thread-self/attr/exec` when
  `selinux_active()` (`/sys/fs/selinux/enforce` exists) — the container process's
  domain transition (equivalent to `setexeccon`) is applied if the caller passes a
  context.
- **VOLUME RELABEL (`:z`/`:Z`):** ❌ **not implemented**, and refused fail-closed
  (`delonix-volume/src/lib.rs:1077-1081`, error in `error.rs:95`). A bind mount on
  an enforcing host keeps the host label; with no relabel to the container domain,
  access is **denied** — the typical failure mode on enforcing RHEL/Fedora.
- **MOUNT labeling (`context=`/`fscontext=`) and file labeling:** ❌ absent.

**SELinux verdict:** partial. Process labeling exists, but without volume relabel
or mount labeling a workload with binds does not run on an enforcing host. The
missing piece (recursive relabel to the container domain, via
`libselinux`/`setfilecon` + per-mount `context=`) is work in its own right, not a
tweak — so **out of scope for the 0.x line** (ADR-0083), with the support path
described there.

## Reproduction

```
BIN=target/release/delonix
R=$(mktemp -d); sudo mkdir -p "$R"
# rootful core
sudo -E env DELONIX_ROOT="$R" $BIN container run --rm --net none alpine:3.12 id
sudo -E env DELONIX_ROOT="$R" $BIN container run --rm --net none alpine:3.12 \
  sh -c 'ls -l /dev/null /dev/zero'
# rootful FROM scratch (reproduces DX-1410 in packaging)
d=$(mktemp -d); sudo cp "$(command -v busybox)" "$d/busybox"
printf 'FROM scratch\nCOPY busybox /bin/sh\nRUN echo ok\n' > "$d/Dockerfile"
( cd "$d" && sudo -E env DELONIX_ROOT="$R" $BIN build -t rootscratch:1 . )
```
