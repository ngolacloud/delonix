# 06 — init / templates (generation)

SUT: `delonix 5.1.1` @ `bca34f6c3`. Available templates (13): django, fastapi, go, haproxy, httpd, laravel, nestjs, nextjs, nginx, node, odoo, rust, wordpress.

| # | Case (§15) | Observed | State |
|---|---|---|---|
| INIT-1 | non-interactive generation, name, template (`fastapi`, `nginx`) | both generated, exit 0; complete structure (compose/Dockerfile/Delonixfile/manifest/Makefile/tests/CI/tunnel/TLS) | **PASSED** |
| INIT-2 | no unresolved placeholders | `grep` of `{{`/`__X__`/`CHANGEME`/`<REPLACE` in py/yaml/Dockerfile/Delonixfile → **0 occurrences** | **PASSED** |
| INIT-3 | no hardcoded secrets | `POSTGRES_PASSWORD`/`RABBITMQ_DEFAULT_PASS` (32 chars) **differ between two generations** (distinct hash) → generated per project | **PASSED** |
| INIT-4 | `.env` protected | `.env` in `.gitignore` (line 12); `.env` (1396 B) distinct from `.env.example` (2763 B) | **PASSED** |
| INIT-5 | generated manifest valid | `compose config` in the generated project → exit 0, 3 services resolved (offline) | **PASSED** |
| INIT-6 | mkcert TLS in the scaffold | `tls/tls.crt|key|pem` issued by the local mkcert CA for localhost/127.0.0.1/::1 | **PASSED** (generation; client trust not tested) |

## Observations

- `init` detects/dispatches to `stack init`/`vm init` and **adopts** existing projects
  (only writes the Delonix/CI glue, does not touch the code) — documented and coherent behaviour.
- The `nginx` project ran `git init` (has `.git/`); `fastapi` did not — a difference between templates,
  not a defect.
- `WEBHOOK_INBOUND_SECRET` stays **empty** in `.env` (to be filled by the user) — it is a blank field,
  not a hardcoded secret; acceptable.

## Coverage and limits (honesty)

- Tested the **generation** and **static validity** of 2 of the 13 templates (one web+DB+queue monolith,
  one static). The declared combinations (stack×version, monolith×microservices, existing destination,
  name with spaces, cancellation, failure midway) **were not all exercised** — a risk-based
  strategy, as the scope itself admits. → partial coverage, not "catalogue validated".
- **TEMPLATE-1 (running the generated project: `make doctor`/`dev`/build/test/lint, functional request,
  Redis, RabbitMQ)**: **NOT EXECUTED** this round (heavy: toolchains + supporting containers;
  disk at 96%). Medium risk: generation is proven; end-to-end execution of the generated stack is not.
