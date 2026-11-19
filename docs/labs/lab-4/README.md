# Lab 4 — From a Dockerfile to an image, without a daemon

> **Identifier:** `lab-4` · **Level:** intermediate · **Track:** first contact

## Concrete objective

Build an image from a **`Delonixfile`** (no `-f`, no daemon) and run what you
built with `--wait`, proving **functionally** that the `HEALTHCHECK` passed: the
STATUS column of `container ps` shows `(healthy)`, not just `Up`. The engine
probes the container **on its own** — no systemd, unlike Podman rootless.

## Prerequisite knowledge

A Linux terminal and a notion of container/image. Lab 1 helps but is not
required.

## Version and environment tested

> **Historical baseline (line 5.1.x):** the values below were measured on the old line and **have not yet been re-measured on the current 0.1.0 line**.

`delonix 5.1.1`, Linux with cgroup v2, rootless execution. Validated on
2026-10-24 (building from the `Delonixfile` with precedence over the
`Dockerfile`, and `(healthy)` read from `container ps -o json` — positive
`EXIT=0`; both failure modes — «build failed» and «healthcheck never passed» —
confirmed to give `EXIT=1` with distinct messages).

## Minimal resources

~15 MiB of disk for `alpine:latest` + `curl` installed in the layer; network
for the first base `pull` and for `apk add`.

## Privileges and access required

No `sudo`. Build and run both run rootless.

## Estimated duration

~10 seconds once the base is cached (dominated by `apk add --no-cache curl` and
by one or two healthcheck probes).

## Preparation files

No external file. The `Delonixfile`, the `ola.txt` it `COPY`s, and a
**decoy** `Dockerfile` (nonexistent base, to prove precedence) are **generated
by the lab itself** in a build directory **under the lab's state** (never in
the repo) and removed on cleanup. The harness lives in this folder:

| File | Phase |
|---|---|
| `prepare.sh` | ensures prerequisites and the base image; cleans up leftovers |
| `run.sh`     | writes the `Delonixfile` (+ decoy `Dockerfile`), `build` without `-f`, `run --wait` |
| `verify.sh`  | asserts the build, the precedence and the `(healthy)` |
| `cleanup.sh` | removes only what the lab created (container, image, build directory) |
| `lab.sh`     | `prepare → run → verify → cleanup` orchestrator |

## How to run

```bash
# with delonix on PATH (normal install):
bash docs/labs/lab-4/lab.sh

# pointing at a specific binary (e.g. a local build):
DELONIX=/path/to/delonix bash docs/labs/lab-4/lab.sh

# leave the container and image standing to explore:
bash docs/labs/lab-4/lab.sh --no-clean
```

## Steps (what `run.sh` does) and why

1. Writes a `Delonixfile` (`FROM alpine` → `RUN apk add curl` → `COPY ola.txt`
   → `HEALTHCHECK test -f /ola.txt` → `CMD`) and a **decoy** `Dockerfile` whose
   base does not exist, both in the build directory.
2. `build -t minha-app:1.0 <dir>` **without `-f`**: the `Delonixfile` is looked
   for **before** any `Dockerfile`. Since the decoy would fail the build, a
   passing build is the proof of precedence.
3. `container run -d --name a1 --wait --health-interval 2`: `--wait` blocks
   until the `HEALTHCHECK` passes (or `--wait-timeout` runs out), sparing the
   `until ...; do sleep 1; done` every script ends up writing — and badly.
4. `container ps` to see the state via the CLI.

## Checkpoints / success assertions (what `verify.sh` requires)

- **A** — the **build produced the image** (`minha-app:1.0` exists in the
  store). This is the first fork: without an image, we say **«BUILD
  FAILED»** — not the healthcheck.
- **B** — **precedence**: the decoy `Dockerfile` was present and the build
  still passed → `delonix build` without `-f` chose the `Delonixfile` first.
- **C** — the container is **Up** (did not exit). A process that dies is
  reported as such, never confused with a healthcheck that does not pass.
- **D** — the STATUS column of `container ps -o json` shows **`(healthy)`**,
  with **retry** (`(healthy)` only appears after one or two probes, even after
  `--wait`). The harness **fails loudly** if it never becomes healthy — and,
  since **A** already proved the build, that failure is **unambiguously** the
  healthcheck, stated as **«HEALTHCHECK NEVER PASSED»**, never a silently
  broken build.

## Cleanup

`cleanup.sh` removes the `a1` container, the `minha-app:1.0` image and the
build directory. It does **not** remove the `alpine` base (other labs reuse
it).

## Lesson learned

An image is born from a `Delonixfile` with no daemon and no `-f`, and the
engine goes on to probe the container's health **itself**. The proof the
image is good is never «the build did not error»: it is the container
reaching `(healthy)` on its own — and the harness separates, with different
words, an image that did not build from a health probe that never turned
green.

## Next lab

**Lab 5 — A real microVM** (a full VM with the same CLI as containers).
