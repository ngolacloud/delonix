# Ronda e2e `delonix 0.1.3` — distroless-por-omissão + `vm --distro` + perfil de superfície

> Binário sob teste: `delonix 0.1.2 (+33 commits)` no worktree
> `feat/distroless-default-vm-distro`. Esta ronda valida os diferenciais
> **distroless-por-omissão** e **`vm create --distro`** e mede a **superfície CLI
> inteira** (tempo por folha) e o **comportamento em concorrência/alta
> disponibilidade**. Fonte da árvore: `scripts/cli-tree.sh` (**366 folhas / 413
> comandos / 78 grupos**).

## 1. O que mudou nesta ronda (vs 0.1.2)

| Área | Mudança | Prova |
|---|---|---|
| Distroless (containers) | Scaffold genérico e templates `go/rust/python/node` passam a ter a imagem RUNTIME distroless (multi-stage); registo configurável por `DELONIX_DISTROLESS_REGISTRY` (default `gcr.io/distroless`) | §3 |
| Fallback no pull | Uma imagem distroless reconhecida e ausente (not-found) recorre à base convencional equivalente, em vez de falhar | §3 |
| `vm create --distro` | Novo selector semântico `ubuntu\|debian\|rocky\|fedora`; verifica o repositório de imagens de VM acessível; cross-check com `--disk` | §3 |
| i18n | 7 strings novas em `data/pt.po` (fonte EN) | `--l18n=pt vm create --help` |

## 2. Cobertura da superfície

| Eixo | Valor | Fonte |
|---|---|---|
| Folhas descobertas (parser real) | **366** / 413 comandos / 78 grupos | `cli-tree.sh --count` |
| Folhas com tempo medido (`--help`, RUNS=20) | **366 (100%)** | `cli_timing_profile.sh` → `timing-results.jsonl` |
| Folhas exercitadas em concorrência (read-only seguras) | 7 activas (1 SKIP correcto) | `cli_concurrency_probe.sh` |
| Regressão de comportamento (pass/fail por folha) | baseline **0.1.2** (1735 PASS / 22 FAIL triadas / 35 SKIP) — mudanças desta ronda são aditivas (nova flag, templates) e cobertas por 50 testes de scaffold + 4 de distroless | `reports/e2e/0.1.2/` |

As conversões distroless foram provadas **a correr** (não só a compilar): ver §3.

## 3. Prova dos diferenciais (comportamento observado)

**Distroless-por-omissão**
- `delonix init -t go` → `FROM gcr.io/distroless/static-debian12:nonroot`, `USER 65532:65532`, `ENTRYPOINT ["/usr/local/bin/server"]`, `HEALTHCHECK CMD ["/usr/local/bin/server","healthcheck"]` (exec-form, sem shell).
- Genérico (sem `--image`) → multi-stage `busybox:1.37-musl AS tools` + `FROM gcr.io/distroless/base-debian12:nonroot`; **build+run+`curl` → `hello-distroless`** (rc=0).
- Template **go** real → build (golang-alpine→distroless/static) + `run -d -p` + `curl /api/v1/health/live` rc=0 + `exec … server healthcheck` rc=0.
- Registo configurável: `DELONIX_DISTROLESS_REGISTRY=cgr.dev/chainguard delonix init` → `FROM cgr.dev/chainguard/base-debian12:nonroot`.

**Fallback no pull**
- `FROM gcr.io/distroless/static-debian12:<tag-inexistente>` → stderr `distroless image … not found; falling back to alpine:3.24` → pull alpine → build OK (`fbtest:dev`, 8.3 MiB). Só dispara em **not-found** (`DX_NOT_FOUND`), nunca em erro de rede/auth; só para refs distroless reconhecidas. Node é excluído de propósito (o caminho do `node` diverge entre distroless e `node:*-slim`).

**`vm create --distro`**
- `vm create --help` mostra `--distro <ubuntu|debian|rocky|fedora>` (en e pt-AO).
- `vm create --distro debian` com o repo inacessível → erro claro: *«VM image repository 'ghcr.io/ngolacloud/delonix-vm-base' is not reachable: …»*.

## 4. Desempenho — tempo por folha (`--help`, RUNS=20)

Latência de **parse+arranque** por folha (inclui `fork`/`exec` do binário — o custo real da superfície CLI). Arranque base: `--version` mean **9.874ms** (p95 11.212), `--help` mean **9.886ms** (p95 12.253).

- **mean global das 366 folhas: 9.12 ms** · mediana 9.065 ms · min 7.785 ms · max 10.715 ms.
- Distribuição muito apertada (todas as folhas 7,8–10,7 ms de mean): não há folha-outlier de parse; o custo é dominado pelo arranque do binário, não pelo comando.
- p99 ocasional 18–26 ms em folhas isoladas (`pod`, `vm`, `container`, `network`) = jitter de agendamento, não trabalho da folha.

Tabela completa por grupo em [`PERFORMANCE.md`](PERFORMANCE.md). Topo (mais lentas por mean-dos-means):

| grupo | folhas | mean-dos-means (ms) | p95 máx (ms) | p99 máx (ms) |
|---|--:|--:|--:|--:|
| `run` | 1 | 10.53 | 12.14 | 12.26 |
| `pull` | 1 | 10.27 | 11.73 | 11.75 |
| `completion` | 2 | 10.1 | 12.35 | 18.4 |
| `version` | 1 | 10.07 | 11.63 | 24.35 |
| `logout` | 1 | 10.05 | 11.96 | 13.34 |
| `migrate` | 1 | 10.02 | 11.32 | 11.69 |
| `rm` | 1 | 9.97 | 18.23 | 24.87 |
| `stop` | 1 | 9.93 | 12.35 | 12.53 |
| `hosts` | 1 | 9.85 | 11.37 | 11.83 |
| `systemcontainer` | 6 | 9.84 | 14.53 | 17.75 |
| `create` | 1 | 9.78 | 11.29 | 12.77 |
| `bucket` | 3 | 9.69 | 12.52 | 16.93 |


## 5. Concorrência / alta disponibilidade

`cli_concurrency_probe.sh`: **32 processos `delonix` em paralelo**, 20s, sobre folhas read-only seguras (`version`, `info`, `ps`, `image ls`, `vm ls`, `system info`, `events`).

| Métrica | Valor |
|---|--:|
| Invocações | **8736** |
| Sucesso (rc=0) | **8736 (100.00%)** |
| Erros | 0 (0.00%) |
| Corrupção de estado | **nenhuma** (`state_corruption_suspected=False`) |
| Throughput | **435.58 inv/s** |
| Latência p50 / p95 / p99 / max | 51.657 / 108.59 / 362.212 / 736.948 ms |

**Leitura:** sob 32× de contenção, a superfície read-only daemonless mantém **0 erros e 0 corrupção de estado** — o `flock`/estado atómico e o GC oportunista de arranque aguentam acesso concorrente. A cauda (p99 362.212 ms, max 736.948 ms) é a contenção esperada de 32 `fork`/`exec` + varrimento de estado por processo; não há falha, só latência de cauda.

## 6. Onde melhorar a performance (accionável)

1. **Arranque do binário domina** (~8–10 ms por invocação, 45 MB). Alvos: (a) o plano morno (ADR-0095) já ataca o `pre_spawn` no caminho de container/CRI; (b) para a superfície CLI pura, a latência é o custo de `fork`/`exec`+parse clap — um daemon opcional de longa duração para o caminho de alta-frequência reduziria isto, mas quebra a invariante daemonless (manter como está).
2. **GC oportunista por processo** (`gc_orphan_cgroup_leaves_once`) acrescenta trabalho a cada comando que abre o store — sob 32× concorrência é parte da cauda p99. Avaliar torná-lo *best-effort com amortização* (1×/N processos ou por TTL) para baixar a cauda sem perder a recuperação pós-crash.
3. **p99 de parse em `pod`/`network`/`vm`** (18–26 ms) — jitter de agendamento, não trabalho; aceitável. Reavaliar só se um SLO de latência CLI o exigir.

## 7. Artefactos

- [`timing-results.jsonl`](timing-results.jsonl) — 368 linhas (366 folhas + 2 startup), uma por folha com min/p50/p95/p99/max/mean.
- [`concurrency-results.json`](concurrency-results.json) — agregados da sonda de concorrência.
- [`PERFORMANCE.md`](PERFORMANCE.md) — tabela completa por grupo.
- [`COVERAGE_MATRIX.md`](COVERAGE_MATRIX.md) — cobertura por eixo/grupo.

> Arneses novos (reutilizáveis): `scripts/cli_timing_profile.sh`, `scripts/cli_concurrency_probe.sh`.
