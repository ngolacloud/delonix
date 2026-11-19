# 0003 — Distroless static runtime, an unprivileged user inside the container

Status: accepted (template default)

## Context
The image should carry the binary and CA roots, not the toolchain, and the
smallest possible everything-else. Distroless is the third Delonix pillar,
alongside rootless-first and daemonless: a runtime image with no shell, no
package manager and no coreutils means a far smaller attack surface and nothing
to exec if the process is ever subverted.

Delonix runs containers rootless: uid 0 inside maps to the unprivileged user
that runs Delonix. That bounds a compromise on the host, but a process running
as uid 0 can still rewrite anything in its own container, so the image still
drops to a non-root user. On a distroless image that user needs no subordinate
uid range and no `adduser`: the base already defines one.

## Decision
Build in `golang:<version>-alpine` with `CGO_ENABLED=0` (a static binary, with
no libc dependency), and run on `gcr.io/distroless/static-debian12:nonroot`
(the registry is the `__DISTROLESS_REGISTRY__` scaffold token). The image:

- ships the CA roots for the outbound https webhook at
  `/etc/ssl/certs/ca-certificates.crt` — no `apk add ca-certificates`;
- already defines the `nonroot` user (uid 65532) via the `:nonroot` tag — no
  `addgroup`/`adduser`. The `Delonixfile` sets `USER 65532:65532` numerically so
  the id resolves without an `/etc/passwd` lookup, and `delonix-manifest.yaml`
  names the same id (`user: "65532"`); the two must agree, because the engine
  applies an image's `USER` only when the manifest asks for it;
- carries no shell, so there is no `entrypoint.sh`: `ENTRYPOINT` is the server
  binary directly, which is PID 1 and owns readiness, drain and the exit code;
- runs with a read-only root filesystem and a `/tmp` tmpfs from the manifest.

### Health check without a shell
Distroless has no `wget`/`curl` and no `/bin/sh`, so the previous
`HEALTHCHECK CMD wget …` cannot work. The check is now the binary's own
subcommand, in exec-form (a JSON array, run WITHOUT a shell):

```
HEALTHCHECK CMD ["/usr/local/bin/server", "healthcheck"]
```

`server healthcheck` GETs `http://127.0.0.1:<PORT>/api/v1/health/live` over
loopback (standard library, 3s timeout), reading `PORT` exactly as the server
does, and exits 0 on a 2xx. Orchestrators do not depend on the image
`HEALTHCHECK`: they probe `/api/v1/health/live` (liveness) and the readiness
endpoint directly over the network.

## Alternatives
- `alpine` + `apk add ca-certificates` + `adduser`: a shell and a package
  manager in the runtime image, a larger surface, and a healthcheck that relies
  on `wget`. Kept only as the conventional fallback base when the distroless
  image cannot be pulled (air-gapped or mirror-only networks); the static binary
  runs unchanged on it.
- `scratch`: smaller still, but carries no CA roots and no `nonroot` user, so
  the outbound https webhook and the non-root default would need rebuilding by
  hand. Distroless/static gives both for free.
- uid 0 inside the container: needs no user at all, but the process could
  rewrite its own binary. Numeric `USER 65532:65532` avoids that at no cost.

## Trade-off
`USER` in the `Delonixfile` and `user:` in `delonix-manifest.yaml` must stay in
agreement (both `65532`). A host without a subordinate uid range cannot run a
second user inside the container; there, remove `user:` from the manifest and
the process runs as the rootless-mapped uid. The user cannot bind a port below
1024 and writes only to the `/tmp` tmpfs.
