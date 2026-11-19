# report.md — CLI test round 0.1.1 (@596663c6)

Live test campaign of the `delonix 0.1.1` binary (+307 commits since v0.1.0),
**rootless** host (delegated cgroup2, slirp4netns/nft/ip present), 2026-11-17.
The 7 functional groups were exercised with real resources (prefix `clitest-`, all
removed at the end). Lenses: Docker 29.x, Podman, Kubernetes/CRI.

> Host snapshot at the time: `images 12.2 GiB / layers ≥20.3 GiB`, build-cache
> **17 GiB**, **3 GiB free** under the state root (`DLX-RES-007`). Several findings
> gain weight because of the disk pressure.

## Verdict

The core (lifecycle, SDN networking, declarative layer, Docker interop) is
**functional and very fast** — product quality. The two biggest risks for
"production/density ready" are **architectural** (flattened images without
layer sharing; declarative layer blind to runtime state). There are 3 surgical
bugs and a list of Docker parity gaps that are easy to close. The axis of
greatest strategic return is **kernel nativisation** (see `PERF_KERNEL_BACKLOG.md`).

## Measured latency (no problem; daemonless with no overhead)

| Operation | Time | Operation | Time |
|---|---|---|---|
| `run -d` | 120 ms | `apply` (Network+Pod) | 1.12 s |
| `pause`/`unpause` | 13/10 ms | `plan` / `drift` | 109/86 ms |
| `kill` | 18 ms | `commit` | 72 ms |
| `exec` | 313 ms | `save`/`load` | 31/40 ms |
| `cp` | 19 ms | `build` (FROM+RUN) | 1.6 s |
| `logs` | 10 ms | nginx boot (net+vol+port) | 1.2 s |
| `network create` | 18 ms | **curl host→published port** | **1.2 ms** |
| `backup create` | 40 ms | docker-api `/_ping` | 1.5 ms |

`stop`/`restart` ~10 s = SIGTERM→SIGKILL grace period (correct, not engine latency).

## What is solid (confirmed live)
- **Real SDN networking**: port publishing, internal container↔container DNS, L3 — all OK.
- **Secrets encrypted at rest** (value does not appear in cleartext on disk); `--secret-files` mounts `0600` on tmpfs.
- **Rich and idempotent declarative layer**: `plan`/`apply`/`diff` (3 columns)/`drift`/`delete`; 33 Kinds with apiVersion/FORM.
- **Docker interop proven end-to-end**: the official `docker` client talked to `serve docker-api` via `DOCKER_HOST`.
- **Transparency**: `compatibility docker` (21/28, 7 refused with a reason), `provider ls` (149 capabilities), `serve --matrix`, `system features` (honest maturity levels).
- **Runtime self-healing**: `--restart always/on-failure/unless-stopped` + per-container supervisor (verified: kill→restart).

## Index of this round's backlog
- `BUG_BACKLOG.md` — BUG-0007 (ImageID docker-api), BUG-0008 (nobody groups on exec), BUG-0009 (drift casing).
- `GAP_BACKLOG.md` — GAP-0018..0026: Docker/CLI parity + architecture (layer-sharing, declarative liveness, build-cache GC).
- `PERF_KERNEL_BACKLOG.md` — KRN-0001..0006: native netlink, pidfd, io_uring, landlock, native KVM/virtio, crate hygiene. **Top priority (owner, 2026-11-17).**
