# B1c — OCI runtime-spec validation suite at the **root** tier (VM), measured

**Date:** 2026-11-20 · **Binary:** `delonix 0.1.2` (`oci-runtime`, ADR-0080; build
`1bf325bce` +81) · **Suite:** `opencontainers/runtime-tools` validation suite
`v0.9.1-0.20261017090856-bd3b0bd00f44` (commit `bd3b0bd`) · **Reference runtime:**
`runc 1.5.3` · **Go:** 1.25.3, `GOFLAGS=-mod=mod`.

**Environment (the privileged tier).** A rootful qemu/KVM microVM
(Ubuntu 26.04, kernel `7.0.0-14-generic`, **cgroup v2 unified**, glibc 2.43), run
as **`root` (uid 0)**. No `sudo` was used on the host (owner directive): the VM was
booted by a host user in the `kvm` group via SLIRP user-net, and every privileged
case ran as root *inside* the guest. Both runtimes were driven by the SAME harness
([`scripts/runtime-tools.sh`](../../scripts/runtime-tools.sh)) in the SAME mode
(root), so the figures are directly comparable — the condition the harness header
requires.

This is the companion of the rootless result
([B1b](B1b-oci-runtime-delonix-measured.md)), which explicitly deferred "a full-green
run of the privileged cases (cgroups/namespaces/mounts/seccomp)" to "a separate
root/VM measurement". That measurement is this report.

## What it took to make the privileged cases *runnable* at all

`scripts/runtime-tools.sh` compiles the suite but does **not** build the rootfs the
suite needs. `validation/util/test.go::PrepareBundle()` untars `rootfs-<arch>.tar.gz`
from the working directory, and `RuntimeInsideValidate()` copies a `runtimetest`
binary from that directory into each bundle. Without them every container-launching
case errors out — which is why the rootless B1b run only ever exercised the 15
config-only assertions. For this root run we staged, in the harness working dir:

- a static `runtimetest` (built from the pinned suite commit with
  `make runtimetest`, `-extldflags -static`), and
- `rootfs-amd64.tar.gz` — a busybox-static rootfs with `/runtimetest` at its root.

With those present the privileged cases genuinely create containers (cgroups,
namespaces, mounts, seccomp, caps, devices…) for both runtimes.

## Result — delonix is **NOT** at root parity with runc

| Runtime (root) | Assertions `ok` | Assertions `not ok` | Files clean-pass | Files with failures | Files skip/err |
|---|---|---|---|---|---|
| `runc 1.5.3` (reference) | **53** | 53 | 24 / 58 | 15 | 18 |
| `delonix 0.1.2` (RT4 wrapper) | **19** | 31 | 7 / 58 | 19 | 32 |
| *(rootless baseline, B1b — both)* | *15* | *26* | — | — | — |

The root tier does lift the number above the rootless 15 (both runtimes now execute
real privileged containers), but **delonix does not reach runc**: 19 vs 53 assertions,
7 vs 24 clean-pass files. The target "full-green root run at parity with runc" is
therefore **not met**, and — as measured here — **cannot be met**, for two independent
reasons, one about delonix and one about the suite/environment.

### Reason 1 (delonix) — `oci-runtime` is a deliberate, fail-closed *subset*

`delonix oci-runtime` validates the bundle up front and **rejects** (`DX-1000`,
*"… or run it under runc"*) every OCI feature outside its supported subset. The
suite's **default** generated config (`generate.New`) carries several of these at
once, so delonix refuses to create the container before any behaviour is observed.
The distinct rejections seen across the run:

- `linux.seccomp` with argument-filtered rules (per-arg) — unsupported subset;
- `process.capabilities.ambient` — ambient caps not raised;
- `process.terminal: true` — no interactive terminal;
- `hooks` (prestart/poststart/poststop) — no OCI-hook stdin-state protocol;
- network namespace **with a `path`** (join an existing netns) — not implemented;
- `linux.devices` **block** devices — only char devices (host-bound) supported;
- `linux.resources.cpu.cpus`/`mems` (cpuset) — controller not delegated;
- `linux.resources.pids` per-container limit — engine writes a fixed leaf `pids.max`;
- `process.rlimits` (`RLIMIT_DATA`) — engine does not `setrlimit`.

A **diagnostic** run with a wrapper that strips the default seccomp profile
([`…-noseccomp-diagnostic.prove.log`](evidence/oci-runtime-tools-delonix-root-noseccomp-diagnostic.prove.log))
returned the **same** 19/31 — confirming seccomp is only the *first* wall: with it
gone the config still trips ambient caps / terminal / hooks / etc. This is a
genuine, documented product boundary (the engine delegates full OCI fidelity to
runc), not a harness or wrapper artefact — the wrapper maps the CLI 1:1 and is the
same one B1b used.

### Reason 2 (suite/environment) — the suite does not support cgroup v2

Every `linux_cgroups_*` case fails for **runc too**, emitting
`# cgroupv2 is not supported yet` (the suite hard-codes v1 paths such as
`/sys/fs/cgroup/pids/cgrouptest/tasks`). On this cgroup-v2-unified guest that caps
the reference runtime at 24 clean-pass files; no runtime — runc included — can go
58/58 here. (A seccomp `geteuid` *"cannot allocate memory"* quirk also fails one
runc case.) So "full-green" is not a property any runtime can exhibit in this
environment; the honest comparison is **per-case parity against runc in the same
mode**, and delonix is behind it.

## Per-case verdicts (root, both runtimes)

`pass` = all subtests ok · `fail` = all failed · `mix a/b` = a of b ok ·
`skip/err` = the `.t` produced no subtests (feature rejected before any assertion,
or an upstream skip). Raw TAP (temp paths normalised to `<WORK>`/`<BUNDLE>`):
[`runc`](evidence/oci-runtime-tools-runc-root.prove.log) ·
[`delonix`](evidence/oci-runtime-tools-delonix-root.prove.log).

| Case | runc (root) | delonix (root) |
|---|---|---|
| `config_updates_without_affect` | pass | skip/err |
| `create` | pass | pass |
| `default` | pass | skip/err |
| `delete` | pass | pass |
| `delete_only_create_resources` | skip/err | skip/err |
| `delete_resources` | mix 1/2 | skip/err |
| `hooks` | skip/err | skip/err |
| `hooks_stdin` | fail | fail |
| `hostname` | skip/err | fail |
| `kill` | pass | pass |
| `kill_no_effect` | pass | fail |
| `killsig` | pass | fail |
| `linux_cgroups_blkio` | skip/err | skip/err |
| `linux_cgroups_cpus` | fail | skip/err |
| `linux_cgroups_devices` | fail | fail |
| `linux_cgroups_hugetlb` | skip/err | skip/err |
| `linux_cgroups_memory` | fail | fail |
| `linux_cgroups_network` | fail | skip/err |
| `linux_cgroups_pids` | fail | fail |
| `linux_cgroups_relative_blkio` | fail | fail |
| `linux_cgroups_relative_cpus` | fail | fail |
| `linux_cgroups_relative_devices` | fail | fail |
| `linux_cgroups_relative_hugetlb` | fail | fail |
| `linux_cgroups_relative_memory` | fail | fail |
| `linux_cgroups_relative_network` | fail | fail |
| `linux_cgroups_relative_pids` | fail | fail |
| `linux_devices` | skip/err | skip/err |
| `linux_masked_paths` | skip/err | skip/err |
| `linux_mount_label` | skip/err | skip/err |
| `linux_ns_itype` | pass | skip/err |
| `linux_ns_nopath` | pass | skip/err |
| `linux_ns_path` | pass | fail |
| `linux_ns_path_type` | pass | pass |
| `linux_process_apparmor_profile` | skip/err | skip/err |
| `linux_readonly_paths` | skip/err | skip/err |
| `linux_rootfs_propagation` | skip/err | fail |
| `linux_seccomp` | fail | fail |
| `linux_sysctl` | pass | skip/err |
| `linux_uid_mappings` | pass | skip/err |
| `misc_props` | pass | fail |
| `mounts` | skip/err | skip/err |
| `pidfile` | fail | fail |
| `poststart` | skip/err | skip/err |
| `poststart_fail` | skip/err | skip/err |
| `poststop` | pass | skip/err |
| `poststop_fail` | skip/err | skip/err |
| `prestart` | skip/err | skip/err |
| `prestart_fail` | pass | pass |
| `process` | pass | skip/err |
| `process_capabilities` | pass | skip/err |
| `process_capabilities_fail` | pass | skip/err |
| `process_oom_score_adj` | pass | skip/err |
| `process_rlimits` | skip/err | skip/err |
| `process_rlimits_fail` | skip/err | skip/err |
| `process_user` | pass | skip/err |
| `root_readonly_true` | pass | skip/err |
| `start` | pass | pass |
| `state` | pass | pass |

## Conclusion

- At the **root** tier delonix's `oci-runtime` runs the real privileged cases but
  reaches **19/58 assertions vs runc's 53/58** — it is **not at parity** with the
  reference runtime, because the engine deliberately **rejects** (fail-closed,
  `DX-1000`) a broad set of OCI features (seccomp-args, ambient caps, terminal,
  hooks, netns-by-path, block devices, cpuset, per-container pids, `RLIMIT_DATA`)
  that the suite's default config uses.
- Literal 58/58 is unreachable in this environment even for **runc**, because the
  upstream suite does not support cgroup v2.
- Net: the H3 claim at root is **PARTIAL**, not PASS. The B1b rootless "parity"
  (15/15 of the measurable subset) stands; it just never touched the privileged
  features this run exposes. Closing the gap is engine work (grow the `oci-runtime`
  supported subset), and a true full-green reference figure would additionally need
  a cgroup-v1 (or cgroup-v2-aware) revision of the upstream suite.

## Reproduce

In a rootful Linux box/VM with cgroup v2, as root, with Go 1.25 + runc + perl `prove`:

```sh
export PATH="$PATH:/usr/local/go/bin" GOFLAGS=-mod=mod
# stage the suite's rootfs so the privileged cases are runnable:
make -C <runtime-tools> runtimetest                     # static /runtimetest
#  …pack a busybox rootfs with /runtimetest -> rootfs-amd64.tar.gz
cp runtimetest rootfs-amd64.tar.gz .                    # into the prove CWD
# reference, then delonix via the RT4 wrapper — SAME mode:
RUNTIME=runc ./scripts/runtime-tools.sh
DELONIX_BIN="$PWD/target/debug/delonix" \
  RUNTIME="$PWD/scripts/oci-runtime-rt4-wrapper.sh" ./scripts/runtime-tools.sh
```

The summary line `== runtime-tools: N ok / M not ok ==` is the figure.
