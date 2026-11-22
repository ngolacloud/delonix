# Lab 2 — Resource limits that actually stick

> **Identifier:** `lab-2` · **Level:** beginner · **Journey:** first contact

## Concrete objective

Put a memory ceiling on a container (`-m 128M`) and **prove it stuck by reading
the REAL `memory.max` of the kernel's cgroup v2** — not the command's output, which lies
by default when cgroup delegation has not been done. It is the #1 mistake of anyone
starting out in rootless: the container looks limited and runs with no limit at all.

## Prerequisite knowledge

Linux terminal and a notion of cgroup v2. Lab 1 helps but is not required.

## Tested version and environment

> **Historical baseline (5.1.x line):** the values below were measured on the old line and **have not yet been re-measured on the current 0.1.0 line**.

`delonix 5.1.1`, Linux with cgroup v2 and rootless delegation of `cpu memory pids`
(`cpuset`/`io` absent, and that is correct). Rootless execution. Validated on 2026-10-24.

## Minimum resources

~8 MiB of disk for the `alpine` image; network for the first `pull`.

## Required privileges and access

No `sudo` on the happy path. Needs the `memory` controller delegated
to your `user@uid.service` — which on a stock Ubuntu already comes wired up. If
it does not, run the lab inside a delegated scope (see "How to run").

## Estimated duration

~1 minute (dominated by the first `pull` of the image).

## Preparation files

No external files. The harness lives in this folder:

| File | Phase |
|---|---|
| `prepare.sh` | ensures prerequisites and the local image |
| `run.sh`     | runs the journey (system setup → run -m → locate cgroup) |
| `verify.sh`  | asserts the cgroup's REAL `memory.max` |
| `cleanup.sh` | removes only what the lab created |
| `lab.sh`     | orchestrator `prepare → run → verify → cleanup` |

## How to run

```bash
# with delonix on the PATH (normal install):
bash docs/labs/lab-2/lab.sh

# pointing at a specific binary (e.g. a local build):
DELONIX=/path/to/delonix bash docs/labs/lab-2/lab.sh

# if your host does not delegate `memory`, without root, inside a delegated scope:
systemd-run --user --scope -p Delegate=yes -- bash docs/labs/lab-2/lab.sh
```

## Steps (what `run.sh` does) and why

1. `system setup` — asks BEFORE assuming whether the host enforces limits. If
   it says INERT/PARTIAL for `memory`, the fix does not need root (delegated
   scope above).
2. `container run -d --name limited -m 128M alpine sleep 300` — declared memory
   ceiling.
3. Locates the container in your user's cgroup v2: reads the PID from
   `container inspect` (which returns an **array** — ADR-0005) and the `0::` line of
   `/proc/<pid>/cgroup`.
4. `cat /sys/fs/cgroup<rel>/memory.max` — the kernel's truth, not the
   `run` command's output.

## Checkpoints / success assertions (what `verify.sh` requires)

- **A** — the container is `Up` (read from `container ps -o json`).
- **B** — the cgroup's REAL `memory.max` is **`134217728`** (128 MiB). The harness
  distinguishes three outcomes and **fails loudly** on any that is not
  this one:
  - `memory.max=max` → delegation missing, the container runs **without** a
    ceiling, silently (which is what the lab exists to catch);
  - file absent → the controller never made it down to you (the `cpuset`/`io` case);
  - another value → the limit did not stick.

## Cleanup

`cleanup.sh` removes the `lab2-limited` container and the lab's state. It does **not** remove
the `alpine` image (other labs reuse it).

## Lesson learned

A command's output can say `128M` while the container runs with no limit: the only
proof is the kernel cgroup's `memory.max`. In rootless, the limit only sticks if the
`memory` controller is delegated — and that is something you read, not something you trust.

## Next lab

**Lab 3 — Two applications that talk to each other by name** (user network and internal
DNS).
