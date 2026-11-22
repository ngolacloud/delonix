# 54 — P2: `container run` as a Compute use case

**Date:** 2026-09-16 · **Base:** `origin/main` `db9efa3b` (v3.1.0 +78) · **ADR:** 0040 (P2, D3)

## The question

ADR-0040 puts in `delonix-compute` "the single execution specification" and the use
cases that run it, and sets as P2's exit criterion that every migrated use case have
**zero subprocess paths** in CRI, mgmt and MCP. Today the CRI starts a container by
assembling `delonix container run`'s argv (`start_argv`) and executing the binary.

`cmd_run` (`bins/delonix-runtime-bin/src/cmd/container.rs`) is **1074 lines** and depends
on the whole binary. It does not move all at once: a context cannot depend on adapters
(`delonix-image`, `delonix-net`, `delonix-runtime`, `delonix-volume`). This document fixes
the phases, the ports and the order.

## What is already done

| PR | What |
|---|---|
| #340 | `RunOpts` (the specification) in `delonix-compute` |
| #342 | Pod shape types and their translators; warnings as `Notice` |
| #344 | `preflight::check_run_opts` — the combinations that only read the specification, before any effect |

## The phases of `cmd_run`, measured

| # | Phase | Nature | Depends on |
|---|---|---|---|
| 1 | node policy (`policy::enforce`) | pure, over a read file | node state |
| 2 | validate the spec, expand ports, refuse taken ports | pure + host probes | `delonix-net` (parse), store (port owner) |
| 3 | re-exec into the netns of a custom network/pod | effect | `infra`, process |
| 4 | resolve volumes and CDI | I/O | `VolumeStore`, `/etc/cdi` |
| 5 | resolve/pull the image, id, prepare the rootfs | I/O | `ImageStore` |
| 6 | **build the `Container` record** | **pure given what 4 and 5 resolved** | free name (store), `--env-file`, `--user` against the rootfs, seccomp profile, secrets |
| 7 | attach to network/pod, publish ports | effect | `infra`, slirp |
| 8 | build the `RunSpec` | pure, but the type belongs to the adapter | `delonix-runtime` |
| 9 | create or supervise | effect | `runtime::create_with`, fork |
| 10 | post-start (`--rm`, id, `--wait`) | effect + terminal | store, output |

Phase 6 is the largest portion of business rule (default name and its validation,
house limits and the kubelet hierarchy, image env + `--env-file` + `-e`, labels, userns,
`--security-opt`, logs, DNS, knobs) and is today entangled with ad-hoc I/O.

## The ports

The target contexts (`networking`, `artifact`, `storage`) do not exist yet. The ports are
born in `delonix-compute`, under the name the ADR gives them, and move house when the
context exists — a `pub use` in Compute keeps the callers working.

| Port | Final home (ADR) | Implementation today | Phases |
|---|---|---|---|
| `ImageStore` (resolve config + prepare rootfs) | `delonix-artifact` | `delonix-image::ImageStore` | 5 |
| `StorageProvider` (resolve `-v`) | `delonix-storage` | `delonix-volume::HostVolumes` | 4 |
| `NetworkProvider` (attach, publish, shaping, firewall) | `delonix-networking` | `delonix-net::infra` | 3, 7 |
| `WorkloadRuntime` (create, supervise, remove) | `delonix-compute` | `delonix-runtime::workload::HostWorkload` | 9 |
| `ContainerRecords` (list, write) | `delonix-compute` | `delonix-runtime-core::Store` | 2, 6, 9 |

In P3 the implementations leave the binary for each port's adapter, and the binary is
left only with composition and terminal text: `DeviceResolver` → `delonix-runtime::cdi::HostDevices`,
`NetworkProvider` → `delonix-net::run_network::HostNetwork`, `StorageProvider` →
`delonix-volume::HostVolumes`, `WorkloadRuntime` → `delonix-runtime::workload::HostWorkload` (P3h:
`run` and `start` are driven only by it; network and terminal come in as hooks, because an
adapter does not depend on another). `--user` resolution moved to
`delonix-image::rootfs_user`, which returns the error as data (`UserLookupError`) for the
binary to translate.

`--user <name>` had been broken in every rootless `run` since the rootfs became overlay
(v0.59.0): `merged/` is only mounted by init, inside the userns, and the resolution was
reading an empty directory. Measured on `main` (`-u nobody` on `alpine:3.20` → «does not
exist in the image»). Resolution now reads the layers (upper, then the lowers, with
whiteouts), and never follows a symlink from `/etc/passwd`, which would be a host file.

`RunSpec` (phase 8) is `WorkloadRuntime`'s contract and stays on the adapter side: the
use case hands over a complete `Container` plus what only exists at start time (rootfs,
netns to join, hooks), and the adapter builds the `RunSpec`.

## Migration order

Each step is a PR, with no behaviour change except a measured defect, and with the
`container run` E2E battery passing.

1. **Pure record builder (phase 6)** — `compute::run::build_record(opts, ResolvedRun)`
   where `ResolvedRun` carries what phases 4–5 and the ad-hoc reads already resolved
   (image config, mounts, `--env-file` content, `--user`'s uid/gid, existing names in the
   namespace, the id, the read seccomp profile, the memory/CPU defaults, `rootless`). `cmd_run`
   becomes: resolve → `build_record` → effects. Testable without a host.
2. **Read ports** (`ImageStore`, `StorageProvider`, `ContainerRecords`) and `ResolvedRun`
   resolution as a Compute function over them.
3. **`NetworkProvider` and `WorkloadRuntime`**, and the whole `run` use case in Compute;
   `cmd_run` is left with the terminal (id, warnings, `--wait`) and the composition of
   adapters.
4. **CRI in-process**: `start_container` builds `RunOpts` and calls the use case instead
   of `start_argv` + subprocess. P6's criterion, which this step anticipates only for
   `run`.

## Risks

- **The re-exec on a custom network/pod** (phase 3) runs `cmd_run` twice in different
  processes; the use case has to be idempotent between the two passes (it already is: the
  id travels in `DELONIX_REEXEC_ID`). None of the steps above changes where the re-exec
  happens.
- **The supervisor does a raw `fork`** and assumes a single-threaded caller; the CRI is
  multi-thread. Step 4 inherits the restriction the Docker API has already resolved by
  re-exec (`__apirun`) — not removed without measuring.
- **The same defect class #344 fixed** ("applied only after the supervisor's return") may
  exist in other fields; step 3 gathers the effect phases into one place, and that is
  where it gets reviewed.
