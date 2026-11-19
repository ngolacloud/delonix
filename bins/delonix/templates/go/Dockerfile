# The image compose.yaml and `docker build` use. It is the same two-stage Go
# build as the Delonixfile (which `delonix build` uses and may extend with
# Delonix-only directives): keep the two in step when you change either.
#
#   docker build -t __NAME__:dev .          # or: docker compose build
#
# Stage 1 compiles a STATIC binary (CGO_ENABLED=0, required for the distroless
# static runtime); only that binary reaches the runtime image. go.mod and go.sum
# are copied first so the module download is cached until they change.
# .dockerignore keeps .env and the local build out of the context.
FROM golang:__TEMPLATE_VERSION__-alpine AS build
ENV CGO_ENABLED=0 GOFLAGS=-trimpath
WORKDIR /src
COPY go.mod go.sum ./
RUN go mod download
COPY cmd ./cmd
COPY internal ./internal
ARG VERSION=dev
RUN go build -ldflags "-s -w -X main.version=${VERSION}" -o /out/server ./cmd/__NAME__

# Distroless static: no shell, no apk, no /bin/sh. It already ships the CA roots
# for the outbound https webhook at /etc/ssl/certs/ca-certificates.crt, and the
# :nonroot tag already defines the nonroot user (uid 65532), so there is no RUN
# to add packages or create a user. __DISTROLESS_REGISTRY__ is substituted by the
# scaffold (default gcr.io/distroless).
FROM __DISTROLESS_REGISTRY__/static-debian12:nonroot
COPY --from=build /out/server /usr/local/bin/server
EXPOSE __PORT__
# No shell, so no wget: the healthcheck runs the server's own `healthcheck`
# subcommand in exec-form (a JSON array, executed WITHOUT a shell), which GETs
# /api/v1/health/live over loopback.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 CMD ["/usr/local/bin/server", "healthcheck"]
STOPSIGNAL SIGTERM
# Numeric uid:gid: valid on the distroless image (nonroot) and on the conventional
# fallback alike, without an /etc/passwd lookup.
USER 65532:65532
# Distroless runs no scripts: the server is the entrypoint and PID 1 directly. It
# owns readiness, drain and the exit code.
ENTRYPOINT ["/usr/local/bin/server"]
CMD []
