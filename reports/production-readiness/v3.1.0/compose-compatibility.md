# Compose compatibility — `delonix compose` (native) against `fa8c6297`

**Static** measurement (`delonix compose config`, parse and translation only, nothing created).
Live `compose up` of real applications: **NOT TESTED** at this stage — the
E2E battery covers `compose up/down/ps/logs` and passed (see `raw/e2e-results.jsonl`).

## Allowlist (read in `cmd/compose.rs`)

- Top level (7): `version name services networks volumes secrets configs`.
- Service (28): `image build extends environment env_file ports volumes command entrypoint depends_on
  healthcheck restart networks labels working_dir user cap_add cap_drop privileged tmpfs extra_hosts
  deploy container_name hostname read_only profiles secrets configs`.
- Refused with a reason: `include`, `extends.file`, `devices dns security_opt group_add logging network_mode pid ipc`,
  secret/config `external: true`, `target ≠ source`.

## Unknown-keys test

| Case | rc | Result |
|---|---|---|
| Control: `services.web.bogus_key` | 1 | **REJECTED** with a clear message |
| `deploy.placement.constraints` | 0 | **IGNORED — BUG** (disappears from the render) |
| `healthcheck.disable: true` | 0 | **IGNORED — BUG** (the healthcheck block disappears from the render) |
| `volumes[].bind.propagation: rshared` | 0 | **IGNORED — BUG** (render: `/tmp:/data`, propagation lost) |
| `depends_on.<svc>.required: false` | 0 | **IGNORED — BUG** |
| `deploy.resources.reservations` | 0 | **IGNORED — BUG** |

Proof render: `raw/compose-nested-config.txt`.

Root cause (G-011): `check_unsupported_fields` checks only the first level; the nested structs
are deserialised by serde without `deny_unknown_fields`, so unknown sub-keys are
silently discarded — the same class of defect the top-level allowlist was written to close.

## Not validated

Side-by-side comparison with `docker compose`/`podman-compose`; NGINX, PostgreSQL,
Redis, Odoo+PostgreSQL applications, multi-service with healthchecks, persistent volumes and secrets.
