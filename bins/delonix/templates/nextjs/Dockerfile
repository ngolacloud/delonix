# The image compose.yaml and `docker build` use. It mirrors the two-stage
# Delonixfile (which `delonix build` uses and may extend with Delonix-only
# directives): keep the two in step when you change either.
#
#   docker build -t __NAME__:dev .          # or: docker compose build
#
# Stage 1 installs every dependency and runs `next build`; only the result —
# .next/standalone (server.js, the traced production node_modules, and the
# browser assets `pnpm build` copies into it) — reaches the runtime image. No
# package manager, no build cache, no source tree, no tests. .dockerignore
# keeps node_modules, .next, .env and .git out of the context.
# The runtime is distroless/nodejs22, so the build stage MUST be Node 22: the
# traced standalone node_modules (and any native addon) are pinned to the Node
# 22 ABI.
FROM node:22-slim AS build
ENV NEXT_TELEMETRY_DISABLED=1 COREPACK_ENABLE_DOWNLOAD_PROMPT=0
WORKDIR /app
RUN corepack enable
COPY . .
# From the lock when the context has one (reproducible); a fresh project that
# has not run `pnpm install` yet resolves here instead, and says so.
RUN if [ -f pnpm-lock.yaml ]; then pnpm install --frozen-lockfile; \
    else echo "pnpm-lock.yaml not found: resolving now; run 'pnpm install' and commit it for reproducible images" && pnpm install --no-frozen-lockfile; fi
RUN pnpm build

# Distroless/nodejs22: just the Node 22 runtime, the CA roots and a pre-created
# nonroot user (uid 65532) — no shell, no npm/pnpm, no apt. The token
# __DISTROLESS_REGISTRY__ is substituted by the scaffold (default
# gcr.io/distroless), with a conventional fallback (node:22-slim) if it cannot
# be pulled.
FROM __DISTROLESS_REGISTRY__/nodejs22-debian12:nonroot
# NEXT_MANUAL_SIG_HANDLE: the service's own ordered shutdown handles SIGTERM
# (docs/adr/0003-shutdown.md). HOSTNAME: server.js binds to it, and a container
# runtime otherwise sets it to the container's own name.
ENV NODE_ENV=production NEXT_TELEMETRY_DISABLED=1 NEXT_MANUAL_SIG_HANDLE=true HOSTNAME=0.0.0.0 PORT=__PORT__
WORKDIR /app
COPY --from=build /app/.next/standalone ./
ARG VERSION=dev
ENV APP_VERSION=${VERSION}
EXPOSE __PORT__
# No shell in distroless: the healthcheck is exec-form (a JSON array, run WITHOUT
# a shell) and calls node by absolute path. The runtime probes liveness;
# readiness is for the load balancer.
HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 CMD ["/nodejs/bin/node", "-e", "fetch('http://127.0.0.1:__PORT__/api/v1/health/live').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"]
STOPSIGNAL SIGTERM
# distroless/nodejs ships the nonroot user (uid 65532); numeric uid:gid needs no
# /etc/passwd entry, so it also holds on the conventional fallback base. The
# process owns nothing under /app and writes only to the manifest's tmpfs.
USER 65532:65532
# No shell in distroless: no entrypoint.sh wrapper. distroless/nodejs sets
# ENTRYPOINT=["/nodejs/bin/node"]; declared explicitly, running server.js as node
# PID 1 so SIGTERM reaches the ordered shutdown.
ENTRYPOINT ["/nodejs/bin/node"]
CMD ["server.js"]
