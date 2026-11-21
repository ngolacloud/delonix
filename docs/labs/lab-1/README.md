# Lab 1 — First service, in 60 seconds

> **Identifier:** `lab-1` · **Level:** beginner · **Journey:** first contact

## Concrete objective

Run a real web service with Delonix and rigorously observe the
`run → ps → observe → logs → exec → rm` cycle — and what "daemonless" means in this
engine (there is no central daemon; there are lightweight per-resource holders).

## Prerequisite knowledge

Linux terminal and a notion of TCP ports. No prior container knowledge is assumed.

## Tested version and environment

> **Historical baseline (5.1.x line):** the values below were measured on the old line and **have not yet been re-measured on the current 0.1.0 line**.

`delonix 5.1.1`, Linux with cgroup v2 and unprivileged user namespaces, rootless
execution. Validated on 2026-10-24.

## Minimum resources

~150 MiB of disk for the `nginx:alpine` image; network for the first `pull`.

## Required privileges and access

No `sudo`. Runs as your user (rootless). Needs a free
local TCP port (the harness picks the first one starting from 18080).

## Estimated duration

~2 minutes (dominated by the image's first `pull`).

## Preparation files

No external files. The harness lives in this folder:

| File | Phase |
|---|---|
| `prepare.sh` | ensures prerequisites and the local image |
| `run.sh`     | runs the journey |
| `verify.sh`  | functional-outcome assertions |
| `cleanup.sh` | removes only what the lab created |
| `lab.sh`     | orchestrator `prepare → run → verify → cleanup` |

## How to run

```bash
# with delonix on the PATH (normal install):
bash docs/labs/lab-1/lab.sh

# pointing at a specific binary (e.g. a local build):
DELONIX=/path/to/delonix bash docs/labs/lab-1/lab.sh

# leave the service up to explore (no cleanup):
bash docs/labs/lab-1/lab.sh --no-clean
```

## Steps (what `run.sh` does) and why

1. `container run -d --name web -p <port>:80 nginx:alpine` — background
   service, port published on the host.
2. `container ps` — `STATUS` confirms it is `Up`.
3. `ps -o pid,ppid,comm,args -C delonix` — shows the resource **holders**
   (`netns pin`, `netns control`, the `run` holder); the PPID column proves they are
   children of your `systemd --user`, not of a system daemon.
4. `curl localhost:<port>` — talks to the service.
5. `container exec web …` — goes inside the container.
6. `container logs web` — reads what nginx wrote.

## Checkpoints / success assertions (what `verify.sh` requires)

- **A** — the container is `Up` (read from `container ps -o json`).
- **B** — `GET localhost:<port>` returns **HTTP 200** (functional outcome, with
  retry up to 20 s; not an isolated exit code).
- **C** — the nginx logs are not empty.
- **D** — there are resident `delonix` holders, children of the user's init —
  and **not** a central daemon.

## Cleanup

`cleanup.sh` removes the `lab1-web` container and the lab's state. It does **not** remove the
`nginx:alpine` image (other labs reuse it).

## Lesson learned

A service runs and responds with no central daemon at all; the engine keeps it alive
through a minimal per-resource holder, which disappears with `rm`.

## Next lab

**Lab 2 — Resource limits that actually stick** (rootless cgroups).
