# COVERAGE_MATRIX.md — `delonix 0.1.3` coverage

## Axis 1 — surface (invocable leaves)

| Metric | Value | Note |
|---|--:|---|
| Discovered (real parser) | **366** leaves / 413 commands / 78 groups | `cli-tree.sh --count` |
| Measured time (`--help`, RUNS=20) | **366 (100%)** | `cli_timing_profile.sh` |
| Exercised under concurrency | 7 active + 1 correct SKIP | `cli_concurrency_probe.sh` |

## Axis 2 — differentiators of this round (observed behavior)

| Differentiator | Verdict | Evidence |
|---|---|---|
| Generic distroless (build+run+curl) | ✅ PASS | `hello-distroless` rc=0 |
| Distroless go (build+run+health+healthcheck) | ✅ PASS | `/health/live` rc=0, `server healthcheck` rc=0 |
| Configurable registry (env) | ✅ PASS | `cgr.dev/chainguard` in the FROM |
| Fallback on pull (not-found→conventional) | ✅ PASS | «falling back to alpine:3.24» |
| 7 templates generate valid distroless | ✅ PASS | 50 scaffold tests green |
| `vm create --distro` (flag+accessibility) | ✅ PASS | `--help` + clear error for inaccessible repo |
| **7/7 templates RUNNING in distroless** | ✅ PASS | build+run+`curl /api/v1/health/live` HTTP 200: go, rust, fastapi, django, node, nestjs, nextjs (+ generic `hello-distroless`) |

### Runtime matrix per template (distroless)

| Template | Build stage | Runtime distroless | Verdict |
|---|---|---|---|
| generic | `busybox:1.37-musl` | `base-debian12:nonroot` | ✅ curl → hello-distroless |
| go | `golang:1.26-alpine` (static) | `static-debian12:nonroot` | ✅ HTTP 200 + `server healthcheck` rc=0 |
| rust | `rust:1.90-slim-bookworm` | `cc-debian12:nonroot` | ✅ HTTP 200 |
| fastapi | `python:3.11.2-slim` | `python3-debian12:nonroot` | ✅ HTTP 200 |
| django | `python:3.11-slim` | `python3-debian12:nonroot` | ✅ HTTP 200 (launch.py migrates+gunicorn) |
| node | `node:22-slim` | `nodejs22-debian12:nonroot` | ✅ HTTP 200 |
| nestjs | `node:22-slim` | `nodejs22-debian12:nonroot` | ✅ HTTP 200 |
| nextjs | `node:22-slim` | `nodejs22-debian12:nonroot` | ✅ HTTP 200 (standalone) |

## Axis 3 — concurrency/HA

| Scenario | Result |
|---|---|
| 32× parallel, 20s, read-only | **8736 inv, 0 errors, 0 corruption**, 435 inv/s |

## Axis 4 — behavior regression

Baseline **0.1.2** (1735 PASS / 22 triaged FAIL / 35 SKIP) holds; the 0.1.3
changes are additive (new `--distro` flag, template base images) and covered
by unit tests (`distroless` 4/4, `scaffold` 50/50) and by the runtime proofs.
