# ADR 0003 — A two-stage image on a distroless/cc runtime

## Status
Accepted.

## Context
A Rust toolchain image is ~1.5 GB; shipping it to production is wasteful and
widens the attack surface. The runtime needs only the compiled binary, glibc
(the binary is dynamically linked), CA roots for outbound HTTPS, and a way to
answer the healthcheck. It does not need a package manager, a shell, or any
other userland — each of those is only attack surface at runtime.

## Decision
Build in `rust:<toolchain>-slim`, copying `Cargo.toml` first so the dependency
build layer is cached until it changes, then the source. Run on
`<distroless-registry>/cc-debian12:nonroot` (the registry is a scaffold token,
default `gcr.io/distroless`). The `cc` variant ships glibc, libgcc and
libstdc++, so the dynamically linked, glibc-based binary from the `-slim` build
stage runs unchanged; it also ships the CA roots and a pre-created `nonroot`
user (uid 65532). The container runs as `65532:65532` (numeric, so it is valid
on distroless and on a conventional base alike), read-only root with a `/tmp`
tmpfs.

Because distroless has no shell, there is no entrypoint script and no `curl`:
the binary is `ENTRYPOINT ["/usr/local/bin/<name>"]` directly (PID 1, so SIGTERM
reaches it for the ordered shutdown), and the healthcheck is an exec-form
subcommand of the app itself — `HEALTHCHECK CMD ["/usr/local/bin/<name>",
"healthcheck"]`. That subcommand opens a `TcpStream` to the local listener, GETs
`/api/v1/health/live`, and exits 0 on a 2xx status, 1 otherwise — no extra
dependency, nothing that needs a shell. The same two stages back the
`Delonixfile` (for `delonix build`) and the `Dockerfile` (for `docker build` /
compose) — keep them in step.

## Consequences
The runtime image is tens of MB, not gigabytes, and contains no compiler,
source, package manager or shell — a much smaller attack surface than a
`debian:*-slim` runtime, and no `apt` layer to keep patched. The build stays
glibc-based (slim build → `cc` distroless, both glibc) so the dynamically
linked binary runs without surprises; a fully static musl build onto
`distroless/static` is a later option if even glibc is unwanted.

The trade-offs are deliberate: there is no shell for `docker exec`-style
debugging (use a debug sidecar or the `:debug` distroless tag when needed), and
the healthcheck lives in the application binary rather than an external tool, so
the probe and the server share one build. The orchestrator-level probe is
unaffected — the e2e checks liveness over HTTP from the host, not through the
image `HEALTHCHECK`.
