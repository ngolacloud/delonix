# 0003 — Two-stage image, distroless Python 3.11 runtime, numeric nonroot user

Status: accepted (template default)

## Context
The image should carry the application and its dependencies, not the package
manager, its cache, the tests, the source checkout — or a shell and an OS
userland the service never uses. Delonix is distroless-by-default: the runtime
stage carries only the interpreter and the application, so the attack surface
is the code and nothing else.
Delonix runs containers rootless: uid 0 inside maps to the unprivileged user
that runs Delonix. A distroless `:nonroot` image removes the need to create a
second user (and the subordinate-uid dance that implied): it ships a built-in
unprivileged user, uid 65532.

## Decision
Build in `python:3.11-slim` with a pinned uv: `uv sync --locked --no-dev
--no-editable` when the context has `uv.lock`, a plain resolve (with a
message) when it does not, so a freshly generated project still builds. The
project is installed into `/app/.venv` as a wheel, so no source tree is copied.
The runtime stage is `gcr.io/distroless/python3-debian12:nonroot` (registry
configurable with `DELONIX_DISTROLESS_REGISTRY`), with only `/app/.venv`
copied in. It runs as `USER 65532:65532` (numeric — distroless has no named
`app` entry to resolve), with a read-only root filesystem and a `/tmp` tmpfs
in the manifest.

The build base is pinned to **Python 3.11 on purpose**: distroless
`python3-debian12` ships `/usr/bin/python3.11`, and the venv must be built with
the same minor or the runtime interpreter cannot run it. The venv's `python`
and the image's `/usr/bin/python3.11` are the same minor, so the copied venv
works; the service is always invoked through the venv binary
(`/app/.venv/bin/python -m <module>`) rather than a bare `python` on PATH.

## Alternatives
- A conventional `python:3.x-slim` runtime: carries a shell and apt, which make
  debugging easier but enlarge the attack surface. If the distroless image
  cannot be pulled (air-gapped, blocked registry), Delonix already falls back
  to `python:3.11-slim` automatically, so this remains reachable without being
  the default.
- uid 0 inside the container: needs no subordinate uid range, but the process
  could rewrite its own binaries and `/etc`.
- Copying uv from its official image (`COPY --from=ghcr.io/astral-sh/uv`):
  the documented pattern; `pip install uv==<version>` gives the same pinned
  binary without depending on a second registry.

## Trade-off
No shell in the runtime image: `delonix container exec … sh` does not work, and
the HEALTHCHECK must be exec-form JSON run by the venv interpreter (not a
`python -c … || exit 1` shell line). Debugging is done from the build stage or
a one-off conventional image. The build and runtime Python minors must stay in
lockstep (3.11 ↔ 3.11); bumping one without the other breaks the venv.
The engine applies an image's `USER` only when the manifest names the user, so
`USER 65532:65532` in the `Delonixfile` and `user: "65532:65532"` in
`delonix-manifest.yaml` must agree. The user cannot bind a port below 1024 and
writes only to the `/tmp` tmpfs. An image built without `uv.lock` is not
reproducible; CI refuses to run without the lock for that reason.
