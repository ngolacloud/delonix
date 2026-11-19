# 0003 — Two-stage image, distroless Python 3.11, migrations at start via a Python launcher

Status: accepted (template default)

## Context
The image should carry the project and its virtualenv, not uv or its cache —
nor a shell and an OS userland the service never uses. Delonix is
distroless-by-default: the runtime stage carries only the interpreter and the
application.
Delonix runs containers rootless: uid 0 inside maps to the unprivileged user
that runs Delonix. A distroless `:nonroot` image removes the need to create a
second user (and the subordinate-uid dance that implied): it ships a built-in
unprivileged user, uid 65532.
A fresh deployment has no schema, and `delonix init --up` should end with a
service that answers ready.

## Decision
- Build in `python:3.11-slim` with uv (`uv sync --locked` when `uv.lock` is
  in the context, a plain `uv sync` otherwise, with a message), run on
  `gcr.io/distroless/python3-debian12:nonroot` (registry configurable with
  `DELONIX_DISTROLESS_REGISTRY`), as `USER 65532:65532` (numeric — distroless
  has no named `app` entry), with a read-only root filesystem, a `/tmp` tmpfs
  and a volume for SQLite and the generated secret key.
- The build base is pinned to **Python 3.11 on purpose**: distroless
  `python3-debian12` ships `/usr/bin/python3.11`, and the venv must be built
  with the same minor or the runtime interpreter cannot run it. The service is
  always invoked through the venv binary (`/app/.venv/bin/python`,
  `/app/.venv/bin/gunicorn`).
- `/data` is created in the build stage (`RUN mkdir -p /data`) and carried into
  the runtime stage with `COPY --from=build --chown=65532:65532 /data /data`,
  because distroless has no shell for a runtime `RUN mkdir && chown`. The empty
  named volume takes that owner on its first mount.
- `docker/launch.py` (a Python launcher, since there is no shell for an
  entrypoint script) runs `manage.py migrate` before gunicorn only when
  `MIGRATE_ON_START=true`, then `os.execv`s the command. The manifest sets it,
  because its default is one replica on SQLite. gunicorn itself never migrates.

## Alternatives
- A conventional `python:3.x-slim` runtime: carries a shell and apt (easier
  debugging, larger attack surface). If the distroless image cannot be pulled,
  Delonix already falls back to `python:3.11-slim` automatically, so this stays
  reachable without being the default.
- uid 0 inside the container: needs no subordinate uid range, but the process
  could rewrite its own binaries and `/etc`.
- A shell entrypoint (`docker/entrypoint.sh`): impossible on distroless (no
  `/bin/sh`); the Python launcher does the same migrate-then-exec and keeps the
  container PID via `os.execv`.
- Always migrating at start: with several replicas each would race to
  migrate, and a schema change would arrive with whichever replica restarts
  first.
- Never migrating at start: a fresh deployment answers 503 on readiness
  (`"dependency": "migrations"`) until someone runs the command — what
  happens here once `MIGRATE_ON_START` is removed.

## Trade-off
No shell in the runtime image: `delonix container exec … sh` does not work, and
the HEALTHCHECK must be exec-form JSON run by the venv interpreter. Migrations
run from the launcher at start, or as a release step with
`delonix container exec __NAME__ /app/.venv/bin/python manage.py migrate` —
which still works, because it execs the venv binary directly, not through a
shell. The build and runtime Python minors must stay in lockstep (3.11 ↔ 3.11).
The engine applies an image's `USER` only when the manifest names the user, so
`USER 65532:65532` in the `Delonixfile` and `user: "65532:65532"` in
`delonix-manifest.yaml` must agree. The user cannot bind a port below 1024 and
writes only to the `/tmp` tmpfs and the `/data` volume. With `MIGRATE_ON_START`
a failed migration stops the container before it listens; with several replicas
the variable must be removed and the migration run as a release step.
