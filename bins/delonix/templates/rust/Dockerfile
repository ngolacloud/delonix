# The image compose.yaml and `docker build` use. It is the same two-stage cargo
# build as the Delonixfile (which `delonix build` uses and may extend with
# Delonix-only directives): keep the two in step when you change either.
#
#   docker build -t __NAME__:dev .          # or: docker compose build
#
# Stage 1 compiles a release binary against glibc; only the binary reaches the
# runtime image — no toolchain, no source tree, no build cache. Manifests are
# copied first so the dependency build is cached until they change.
# .dockerignore keeps .env, target/ and the git tree out of the context.
#
# The runtime is distroless/cc: a minimal image with no shell, no package
# manager and no extra userland — just glibc, libgcc and libstdc++ (so the
# dynamically linked, glibc-based binary runs), the CA roots for outbound HTTPS,
# and a pre-created `nonroot` user (uid 65532). No apt, no entrypoint script,
# no curl: the healthcheck is an exec-form subcommand of the app itself.
# Build on bookworm so the glibc the binary links against matches the runtime
# image (distroless/cc is debian12/bookworm, glibc 2.36). A plain `rust:*-slim`
# now tracks trixie, whose newer glibc symbols would fail to resolve at runtime.
FROM rust:__TEMPLATE_VERSION__-slim-bookworm AS build
WORKDIR /src
# Cache dependencies: a manifest-only layer that only rebuilds when they change.
# No `|| true`: a real dependency error must fail HERE, not hide until the next
# stage (Phase-0 hygiene). Reproducibility wants `--locked`, which needs a
# COMMITTED Cargo.lock the scaffold does not yet ship — see docs/services.md;
# once the project commits its lock, add it to the COPY and pass `--locked`.
COPY Cargo.toml ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && echo "" > src/lib.rs \
    && cargo build --release --quiet
COPY . /src
# Prefer the lock when present; the error is VISIBLE (no `2>/dev/null`) so a lock
# mismatch is not silently dropped, and the fallback keeps a lock-less scaffold
# building until it commits one.
RUN cargo build --release --locked --quiet || cargo build --release --quiet
RUN cp "target/release/__NAME__" /usr/local/bin/__NAME__

# distroless/cc already ships glibc + libgcc + libstdc++ (so the dynamically
# linked binary runs), the CA roots for the outbound webhook over https, and the
# `nonroot` user (uid 65532). No apt, no shell, nothing to install: the service
# owns nothing in the image and writes only to the manifest's tmpfs.
FROM __DISTROLESS_REGISTRY__/cc-debian12:nonroot
COPY --from=build /usr/local/bin/__NAME__ /usr/local/bin/__NAME__
ENV APP_ENV=production LOG_LEVEL=info
EXPOSE __PORT__
# The runtime probes liveness; readiness is for the load balancer. distroless
# has no shell and no curl, so the app answers its own probe via the
# `healthcheck` subcommand (exec-form), which GETs /api/v1/health/live.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["/usr/local/bin/__NAME__", "healthcheck"]
STOPSIGNAL SIGTERM
# Numeric uid:gid — valid in distroless and in a conventional base alike.
USER 65532:65532
# No entrypoint script (distroless runs no shell): exec the binary directly, so
# it is PID 1 and SIGTERM reaches it for the ordered shutdown in main.rs.
ENTRYPOINT ["/usr/local/bin/__NAME__"]
