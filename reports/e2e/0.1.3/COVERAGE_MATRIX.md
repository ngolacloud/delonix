# COVERAGE_MATRIX.md — cobertura `delonix 0.1.3`

## Eixo 1 — superfície (folhas invocáveis)

| Métrica | Valor | Nota |
|---|--:|---|
| Descobertas (parser real) | **366** folhas / 413 comandos / 78 grupos | `cli-tree.sh --count` |
| Tempo medido (`--help`, RUNS=20) | **366 (100%)** | `cli_timing_profile.sh` |
| Exercitadas em concorrência | 7 activas + 1 SKIP correcto | `cli_concurrency_probe.sh` |

## Eixo 2 — diferenciais desta ronda (comportamento observado)

| Diferencial | Veredicto | Prova |
|---|---|---|
| Distroless genérico (build+run+curl) | ✅ PASS | `hello-distroless` rc=0 |
| Distroless go (build+run+health+healthcheck) | ✅ PASS | `/health/live` rc=0, `server healthcheck` rc=0 |
| Registo configurável (env) | ✅ PASS | `cgr.dev/chainguard` no FROM |
| Fallback no pull (not-found→convencional) | ✅ PASS | «falling back to alpine:3.24» |
| 7 templates geram distroless válido | ✅ PASS | 50 testes de scaffold verdes |
| `vm create --distro` (flag+acessibilidade) | ✅ PASS | `--help` + erro claro de repo inacessível |
| **7/7 templates a CORRER em distroless** | ✅ PASS | build+run+`curl /api/v1/health/live` HTTP 200: go, rust, fastapi, django, node, nestjs, nextjs (+ genérico `hello-distroless`) |

### Matriz runtime por template (distroless)

| Template | Build stage | Runtime distroless | Veredicto |
|---|---|---|---|
| genérico | `busybox:1.37-musl` | `base-debian12:nonroot` | ✅ curl → hello-distroless |
| go | `golang:1.26-alpine` (static) | `static-debian12:nonroot` | ✅ HTTP 200 + `server healthcheck` rc=0 |
| rust | `rust:1.90-slim-bookworm` | `cc-debian12:nonroot` | ✅ HTTP 200 |
| fastapi | `python:3.11.2-slim` | `python3-debian12:nonroot` | ✅ HTTP 200 |
| django | `python:3.11-slim` | `python3-debian12:nonroot` | ✅ HTTP 200 (launch.py migra+gunicorn) |
| node | `node:22-slim` | `nodejs22-debian12:nonroot` | ✅ HTTP 200 |
| nestjs | `node:22-slim` | `nodejs22-debian12:nonroot` | ✅ HTTP 200 |
| nextjs | `node:22-slim` | `nodejs22-debian12:nonroot` | ✅ HTTP 200 (standalone) |

## Eixo 3 — concorrência/HA

| Cenário | Resultado |
|---|---|
| 32× paralelo, 20s, read-only | **8736 inv, 0 erros, 0 corrupção**, 435 inv/s |

## Eixo 4 — regressão de comportamento

Baseline **0.1.2** (1735 PASS / 22 FAIL triadas / 35 SKIP) mantém-se; as mudanças
de 0.1.3 são aditivas (nova flag `--distro`, imagens base dos templates) e cobertas
por testes unitários (`distroless` 4/4, `scaffold` 50/50) e pelas provas runtime.
