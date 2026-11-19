# Dockerfile — build with:
#   docker build -t __NAME__:dev .
#
# Same grammar as Delonixfile — this file mirrors it for Docker/Compose users.
# See Delonixfile for build details and rationale.
#
# Two stages: uv resolves and installs into /app/.venv in the first, and the
# runtime image gets the project and its virtualenv without uv or its cache.
#
# The build base is pinned to Python 3.11 ON PURPOSE: the runtime image is
# distroless `python3-debian12`, which ships `/usr/bin/python3.11`. The venv
# must be built with the SAME minor (3.11) or the distroless interpreter cannot
# run it (a 3.13 venv has 3.13-only stdlib and a pyvenv.cfg pointing at a 3.13
# that the runtime image does not have).
FROM python:3.11-slim AS build
ENV UV_COMPILE_BYTECODE=1 UV_LINK_MODE=copy UV_PYTHON_DOWNLOADS=never UV_NO_DEV=1
RUN pip install --no-cache-dir uv==0.12.8
WORKDIR /app
COPY . /app
# From the lock when the project has one (reproducible); a fresh project that
# has not run `uv lock` yet resolves here instead, so `delonix init --up` works.
RUN --mount=type=cache,target=/root/.cache/uv if [ -f uv.lock ]; then uv sync --locked; else echo "uv.lock not found: resolving now — run 'uv lock' and commit uv.lock for reproducible images" >&2; uv sync; fi
# The runtime is distroless (no shell), so /data cannot be created with a RUN
# there. Create it in this stage and carry it over with the right owner below;
# an empty named volume takes that owner on its first mount.
RUN mkdir -p /data

# Distroless Python 3.11 runtime: no shell, no package manager, no OS userland
# — only the interpreter (/usr/bin/python3.11), the CA roots and the files
# copied below. The `:nonroot` tag ships a built-in unprivileged user (uid
# 65532), so there is nothing to create here. Registry configurable with
# DELONIX_DISTROLESS_REGISTRY (default gcr.io/distroless); if the pull fails
# not-found, Delonix falls back to python:3.11-slim automatically.
FROM __DISTROLESS_REGISTRY__/python3-debian12:nonroot
ARG VERSION=dev
# PATH puts the venv first; the venv's 3.11 interpreter matches the image's own.
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 APP_VERSION=${VERSION} PYTHONPATH=/app/.venv/lib/python3.11/site-packages
WORKDIR /app
COPY --from=build /app /app
# /data is the only thing the service writes to (SQLite + generated secret key);
# give it to uid 65532 so the volume inherits that owner on first mount.
COPY --from=build --chown=65532:65532 /data /data
EXPOSE __PORT__
# Liveness probe without a shell: exec-form JSON, run by the venv interpreter.
HEALTHCHECK --interval=30s --timeout=5s --start-period=40s --retries=3 CMD ["/usr/bin/python3.11", "-c", "import urllib.request as u; u.urlopen('http://127.0.0.1:__PORT__/api/v1/health/live', timeout=2)"]
STOPSIGNAL SIGTERM
# Numeric uid:gid — the distroless `nonroot` user; no name to resolve.
USER 65532:65532
# No shell for an entrypoint script: a Python launcher applies the migrations
# first when MIGRATE_ON_START=true, then execs the command below (gunicorn,
# which reads /app/gunicorn.conf.py). See docker/launch.py.
ENTRYPOINT ["/usr/bin/python3.11", "/app/docker/launch.py"]
CMD ["/usr/bin/python3.11", "-m", "gunicorn.app.wsgiapp", "config.wsgi:application"]
