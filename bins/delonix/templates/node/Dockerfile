# The image compose.yaml and `docker build` use. It is the same two-stage pnpm
# build as the Delonixfile (which `delonix build` uses and may extend with
# Delonix-only directives): keep the two in step when you change either.
#
#   docker build -t __NAME__:dev .          # or: docker compose build
#
# Stage 1 installs every dependency, compiles TypeScript and prunes to the
# production dependencies; only package.json, node_modules (prod) and dist
# reach the runtime image — no source tree, no dev tooling, no tests.
# .dockerignore keeps .env and node_modules out of the context.
# The runtime is distroless/nodejs22, so the build stage MUST be Node 22: the
# node_modules (and any native addon) are pinned to the Node 22 ABI.
FROM node:22-slim AS build
WORKDIR /app
RUN corepack enable
COPY . .
# From the lock when the context has one (reproducible); a fresh project that
# has not run `pnpm install` yet resolves here instead, and says so.
RUN if [ -f pnpm-lock.yaml ]; then pnpm install --frozen-lockfile; \
    else echo "pnpm-lock.yaml not found: resolving now; run 'pnpm install' and commit it for reproducible images" && pnpm install; fi
RUN pnpm build && pnpm prune --prod

# Distroless/nodejs22: just the Node 22 runtime, the CA roots and a pre-created
# nonroot user (uid 65532) — no shell, no npm/pnpm, no apt. The token
# __DISTROLESS_REGISTRY__ is substituted by the scaffold (default
# gcr.io/distroless), with a conventional fallback (node:22-slim) if it cannot
# be pulled.
FROM __DISTROLESS_REGISTRY__/nodejs22-debian12:nonroot
ENV NODE_ENV=production
WORKDIR /app
COPY --from=build /app/package.json ./package.json
COPY --from=build /app/node_modules ./node_modules
COPY --from=build /app/dist ./dist
ARG VERSION=dev
ENV SERVICE_VERSION=${VERSION}
EXPOSE __PORT__
# No shell in distroless: the healthcheck is exec-form (a JSON array, run WITHOUT
# a shell) and calls node by absolute path. The runtime probes liveness;
# readiness is for the load balancer.
HEALTHCHECK --interval=30s --timeout=5s --start-period=15s --retries=3 CMD ["/nodejs/bin/node", "-e", "fetch('http://127.0.0.1:__PORT__/api/v1/health/live').then(r=>process.exit(r.ok?0:1),()=>process.exit(1))"]
STOPSIGNAL SIGTERM
# distroless/nodejs ships the nonroot user (uid 65532); numeric uid:gid needs no
# /etc/passwd entry, so it also holds on the conventional fallback base. The
# process owns nothing under /app and writes only to the manifest's tmpfs.
USER 65532:65532
# No shell in distroless: no entrypoint.sh wrapper. distroless/nodejs sets
# ENTRYPOINT=["/nodejs/bin/node"]; declared explicitly so the node path is
# unambiguous and the CMD carries only the arguments. node is PID 1 directly, so
# SIGTERM reaches the ordered shutdown.
ENTRYPOINT ["/nodejs/bin/node"]
CMD ["--import", "./dist/instrumentation.js", "dist/index.js"]
