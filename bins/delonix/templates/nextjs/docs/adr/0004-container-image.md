# 0004 — Two-stage image from the standalone output, an unprivileged user

Status: accepted (template default)

## Context
The image should carry what the server needs to run, not the package manager,
dev dependencies or sources.
Delonix runs containers rootless: uid 0 inside maps to the unprivileged user
that runs Delonix. That bounds a compromise on the host, but a process running
as uid 0 can still rewrite anything in its own container. A second user inside
needs a subordinate uid range on the host (`/etc/subuid`; the Delonix
installer sets one up).

## Decision
`next.config.ts` sets `output: "standalone"`. The build stage
(`node:22-slim`) installs from `pnpm-lock.yaml` when it exists and runs
`pnpm build`, which also copies `.next/static` into `.next/standalone`; the
distroless runtime stage
(`__DISTROLESS_REGISTRY__/nodejs22-debian12:nonroot`) copies that one directory
and runs `server.js`. Distroless is the third Delonix pillar beside
rootless-first and daemonless: the image carries only the Node 22 runtime, the
CA roots and a pre-created `nonroot` user (uid 65532) — no shell, no npm/pnpm,
no apt. It runs as `65532:65532` (numeric, no `/etc/passwd` lookup), with a
read-only root filesystem and a `/tmp` tmpfs in the manifest.

The runtime pins Node 22 (`nodejs22-debian12`), so the build stage is
`node:22-slim`: the traced standalone `node_modules` — including any native
addon — match the Node 22 ABI the image ships. Distroless/nodejs sets
`ENTRYPOINT=["/nodejs/bin/node"]`; the Delonixfile declares it explicitly and
the `CMD` carries only `server.js`, so node is PID 1 and the ordered shutdown
(`NEXT_MANUAL_SIG_HANDLE`, docs/adr/0003-shutdown.md) receives SIGTERM. There
is no shell, so the previous `docker/entrypoint.sh` wrapper is gone and the
`HEALTHCHECK` is exec-form calling node by absolute path.

## Alternatives
- `next start` with the full `node_modules`: no copy step, a much larger image.
- A `node:24-slim` runtime: a shell for debugging, but a much larger attack
  surface and a Node 24 ABI mismatch with the distroless pillar. Rejected.
- uid 0 inside the container: needs no subordinate uid range, but the process
  could rewrite its own binaries and `/etc`. A host without a range cannot run
  a second user; there, remove `user:` from the manifest.
- Serving `.next/static` from a CDN: what Next.js recommends at scale; one
  container serving its own assets is simpler to start with.

## Trade-off
No shell in the runtime image: `delonix exec` cannot drop into `sh`, and the
healthcheck and entrypoint are exec-form with an absolute node path. If the
distroless image cannot be pulled, the engine falls back to `node:22-slim`,
where the node binary is at `/usr/local/bin/node` (not `/nodejs/bin/node`), so
the explicit entrypoint is tuned for the distroless primary path. The engine
applies an image's `USER` only when the manifest names the user, so `USER` in
the `Delonixfile` and `user:` in `delonix-manifest.yaml` must agree (both
`65532`). The user cannot bind a port below 1024 and writes only to the `/tmp`
tmpfs. With a read-only filesystem Next.js cannot write its on-disk caches;
this application uses none (every route is dynamic).
