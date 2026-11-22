# B1e — `delonix oci-runtime`: stdio inherited from `create` (runc's fd model)

Measured 2026-11-21 on `h3-root-vm` (qemu/KVM, Ubuntu 26.04, cgroup v2 unified,
uid 0), with a **release** `delonix` built from the working tree (B1d's
5-set capability model + ns validation + the owner's uncommitted seccomp-args
work + **this stdio lever**). Suite:
`runtime-tools@v0.9.1-...bd3b0bd`, through `scripts/oci-runtime-rt4-wrapper.sh`.

## Result

| binary | ok | not ok | fully green .t files |
|---|---|---|---|
| delonix 0.1.2 (B1d, BEFORE — no stdio) | **21** | 58 | 7 |
| delonix 0.1.2 (THIS run — inherited stdio) | **32** | 58 | **17** |
| runc 1.5.3 (reference, B1c) | 53 | 53 | 24 |

`linux_ns_path_type` still holds **14/14** (validated on its own first, before
the full suite). `cargo test -p delonix-linux` 263 ok; `-p delonix-bin` scoped to
`oci_runtime` 29 ok (28 plus the new `oci_stdio_target_*`).

## What changed and why (+11, no regression)

The suite reads the container's stdout from the **fds inherited from `create`**
(runc's model): `validation/util/container.go` opens `<bundle>/stdout-<uuid>` and
`stderr-<uuid>`, points the `create` process's `cmd.Stdout`/`cmd.Stderr` at them,
and once the container has stopped reads those files
(`ReadStandardStreams`) — `runtimetest` writes its TAP to stdout. Before this,
`delonix oci-runtime`'s `start` ran `detach:true` WITHOUT a `log_path`, so
`detach_stdio` sent stdout to `/dev/null`: the TAP was lost and `prove` saw
"No subtests run / No plan found in TAP output" on every `RuntimeInsideValidate`
case (`default.t`, `process.t`, `process_capabilities.t`, `misc_props.t`, …).

Now `create` resolves `/proc/self/fd/{1,2}` to the PATHs of those files (only
when they are regular files — tty/pipe/socket/`/dev/null` fall through to the
historical path) and persists them in `Container.oci_stdout`/`oci_stderr`;
`start` reopens them (`O_WRONLY|O_APPEND`) and the init `dup2`s them straight
over 1/2 in `detach_stdio` — **no log shim, no rotation, no CRI framing**: the
TAP reaches the file the suite reads byte for byte.

Cases that went from "No subtests run" to green (measured against the B1d
baseline): `default.t`, `process.t`, `process_capabilities.t`, `misc_props.t`,
`mounts.t`, `poststart.t`, `poststop.t`, `prestart*.t`,
`linux_cgroups_relative_*`, `delete_*`, among others — 7 → 17 fully green `.t`
files.

## What is still missing for runc's 53

- `process_capabilities.t` creates the container (green TAP on the 1st subtest),
  but one of its bundles still falls into `error[DX-1000] oci-runtime:
  config.json: serde failed` upstream (a gap in the `oci-spec` crate on some
  shapes of `process.capabilities`) — not pursued in this lever.
- `misc_props.t`/`mounts.t`: `start` refuses `rshared` (`DX-1117`) because the
  container is not privileged — the correct refusal, but it counts as `not ok`.
- `killsig.t`: timeout waiting for the container's state.
- `linux_cgroups_*` (the non-relative ones): the upstream suite does not support
  cgroup v2 (it fails the same way for runc).

The number is not monotonic: the `not ok` total stays at 58 because the cases
that now DO create the container emit lifecycle assertions that previously did
not even run.
