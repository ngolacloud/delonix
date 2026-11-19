# 0003 — Two-stage image, an unprivileged user inside the container

Status: accepted (template default)

## Context
The image should carry production dependencies and compiled JavaScript, not
the compiler and dev dependencies.
Delonix runs containers rootless: uid 0 inside maps to the unprivileged user
that runs Delonix. That bounds a compromise on the host, but a process running
as uid 0 can still rewrite anything in its own container. A second user inside
needs a subordinate uid range on the host (`/etc/subuid`; the Delonix
installer sets one up).

## Decision
Build and prune in a `node:22-slim` stage, then copy `node_modules`, `dist` and
`package.json` into a distroless runtime stage
(`__DISTROLESS_REGISTRY__/nodejs22-debian12:nonroot`). Distroless is the third
Delonix pillar beside rootless-first and daemonless: the runtime image carries
only the Node 22 runtime, the CA roots and a pre-created `nonroot` user
(uid 65532) — no shell, no npm/pnpm, no apt, no OS userland. Dependencies
install from `pnpm-lock.yaml` when it exists.

The runtime stage pins Node 22 (`nodejs22-debian12`), so the build stage is
`node:22-slim`: the `node_modules` — including any native addon — are built
against the same Node 22 ABI the image ships. The image runs as `65532:65532`
(numeric, no `/etc/passwd` lookup needed), with a read-only root filesystem and
a `/tmp` tmpfs in the manifest. Distroless/nodejs sets
`ENTRYPOINT=["/nodejs/bin/node"]`; the Delonixfile declares it explicitly and
the `CMD` carries only the arguments (`--import ./dist/instrumentation.js
dist/index.js`), so node is PID 1 and SIGTERM reaches the ordered shutdown.
Because there is no shell, the `HEALTHCHECK` is exec-form (a JSON array run
without a shell) calling node by absolute path (`/nodejs/bin/node -e ...`).

## Alternatives
- A `node:24-slim` runtime: a shell for debugging, but a much larger attack
  surface (a full Debian userland, a shell, a package manager) and a Node 24
  ABI mismatch with the distroless pillar. Rejected for the default.
- uid 0 inside the container: needs no subordinate uid range, but the process
  could rewrite its own binaries and `/etc`. A host without a range cannot run
  a second user; there, remove `user:` from the manifest.

## Trade-off
No shell in the runtime image: `delonix exec` cannot drop into `sh`, and the
healthcheck and entrypoint must be exec-form with an absolute node path. If the
distroless image cannot be pulled, the engine falls back to a conventional base
carrying the same runtime (`node:22-slim`); on that base the node binary lives
at `/usr/local/bin/node`, not `/nodejs/bin/node`, so the explicit entrypoint is
tuned for the distroless primary path. The engine applies an image's `USER`
only when the manifest names the user, so `USER` in the `Delonixfile` and
`user:` in `delonix-manifest.yaml` must agree (both `65532`). The user cannot
bind a port below 1024 and writes only to the `/tmp` tmpfs.
