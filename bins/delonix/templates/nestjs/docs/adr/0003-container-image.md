# 0003 — Two-stage image, production dependencies only, an unprivileged user

Status: accepted (template default)

## Context
The image should carry the compiled service and its production dependencies,
not TypeScript, the Nest CLI, the test tooling or a package manager.
Delonix runs containers rootless: uid 0 inside maps to the unprivileged user
that runs Delonix. That bounds a compromise on the host, but a process running
as uid 0 can still rewrite anything in its own container. A second user inside
needs a subordinate uid range on the host (`/etc/subuid`; the Delonix
installer sets one up).

## Decision
Build in `node:22-slim`: install every dependency, `nest build`, then
`pnpm prune --prod`. The runtime stage is distroless
(`__DISTROLESS_REGISTRY__/nodejs22-debian12:nonroot`) and takes `package.json`,
the pruned `node_modules` and `dist/` — nothing is installed there. Distroless
is the third Delonix pillar beside rootless-first and daemonless: the image
carries only the Node 22 runtime, the CA roots and a pre-created `nonroot` user
(uid 65532) — no shell, no npm/pnpm, no apt. The build installs from
`pnpm-lock.yaml` when the context has one, and resolves otherwise so a freshly
generated project still builds.

The runtime pins Node 22 (`nodejs22-debian12`), so the build stage is
`node:22-slim`: the `node_modules` — including any native addon — match the
Node 22 ABI the image ships. The image runs as `65532:65532` (numeric, no
`/etc/passwd` lookup), with a read-only root filesystem and a `/tmp` tmpfs in
the manifest. Distroless/nodejs sets `ENTRYPOINT=["/nodejs/bin/node"]`; the
Delonixfile declares it explicitly and the `CMD` carries only the arguments
(`--require ./dist/instrumentation.js dist/main.js`) — the instrumentation
loads before the application, and node is PID 1 so SIGTERM reaches the ordered
shutdown in `dist/main.js`. There is no shell, so the previous
`docker/entrypoint.sh` wrapper is gone and the `HEALTHCHECK` is exec-form
calling node by absolute path.

## Alternatives
- `pnpm install --prod` in the runtime stage: installs the dependencies a
  second time and leaves corepack and pnpm in the image.
- A `node:24-slim` runtime: a shell for debugging, but a much larger attack
  surface and a Node 24 ABI mismatch with the distroless pillar. Rejected.
- uid 0 inside the container: needs no subordinate uid range, but the process
  could rewrite its own binaries and `/etc`. A host without a range cannot run
  a second user; there, remove `user:` from the manifest.

## Trade-off
pnpm's `node_modules` is a tree of symlinks into `.pnpm`; the copy between
stages relies on the builder keeping them as links (`delonix build` and Docker
both do). No shell in the runtime image: `delonix exec` cannot drop into `sh`,
and the healthcheck and entrypoint are exec-form with an absolute node path. If
the distroless image cannot be pulled, the engine falls back to `node:22-slim`,
where the node binary is at `/usr/local/bin/node` (not `/nodejs/bin/node`), so
the explicit entrypoint is tuned for the distroless primary path. The engine
applies an image's `USER` only when the manifest names the user, so `USER` in
the `Delonixfile` and `user:` in `delonix-manifest.yaml` must agree (both
`65532`). The user cannot bind a port below 1024 and writes only to the `/tmp`
tmpfs.

The runtime is Node 22, while the dev/test toolchain still targets Node 24.9
(the NestJS 12 Jest `require(esm)` path — see
[docs/adr/0004](docs/adr/0004-commonjs-jest.md)). This is deliberate and safe:
the image ships the compiled CommonJS build, which loads the NestJS ESM
packages through `require(esm)` — stable and default-on from Node 22.12 — so
the service runs on Node 22. Only Jest (not the image) needs the newer Node, so
`engines.node` and CI stay at Node 24.9 for the test path; the `node:22-slim`
build stage runs `tsc`/`nest build` only, which needs no such version.
