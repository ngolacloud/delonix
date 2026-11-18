# report.md — ronda de teste CLI 0.1.1 (@596663c6)

Campanha de teste ao vivo do binário `delonix 0.1.1` (+307 commits desde v0.1.0),
host **rootless** (cgroup2 delegado, slirp4netns/nft/ip presentes), 2026-11-17.
Exercitados os 7 grupos funcionais com recursos reais (prefixo `clitest-`, todos
removidos no fim). Lentes: Docker 29.x, Podman, Kubernetes/CRI.

> Snapshot do host no momento: `images 12.2 GiB / layers ≥20.3 GiB`, build-cache
> **17 GiB**, **3 GiB livres** sob o state root (`DLX-RES-007`). Vários achados
> ganham peso por causa da pressão de disco.

## Veredicto

O núcleo (ciclo de vida, rede SDN, camada declarativa, interop Docker) está
**funcional e muito rápido** — qualidade de produto. Os dois maiores riscos para
"pronto para produção/densidade" são **arquitecturais** (imagens achatadas sem
partilha de layers; camada declarativa cega ao estado de runtime). Há 3 bugs
cirúrgicos e uma lista de gaps de paridade Docker fáceis de fechar. O eixo de
maior retorno estratégico é a **nativização ao kernel** (ver `PERF_KERNEL_BACKLOG.md`).

## Latência medida (nenhum problema; daemonless sem overhead)

| Operação | Tempo | Operação | Tempo |
|---|---|---|---|
| `run -d` | 120 ms | `apply` (Network+Pod) | 1.12 s |
| `pause`/`unpause` | 13/10 ms | `plan` / `drift` | 109/86 ms |
| `kill` | 18 ms | `commit` | 72 ms |
| `exec` | 313 ms | `save`/`load` | 31/40 ms |
| `cp` | 19 ms | `build` (FROM+RUN) | 1.6 s |
| `logs` | 10 ms | nginx boot (rede+vol+porta) | 1.2 s |
| `network create` | 18 ms | **curl host→porta publicada** | **1.2 ms** |
| `backup create` | 40 ms | docker-api `/_ping` | 1.5 ms |

`stop`/`restart` ~10 s = grace period SIGTERM→SIGKILL (correcto, não é latência do motor).

## O que está sólido (confirmado ao vivo)
- **Rede SDN real**: publicação de porta, DNS interno container↔container, L3 — tudo OK.
- **Secrets encriptados em repouso** (valor não aparece em claro no disco); `--secret-files` monta `0600` em tmpfs.
- **Declarativo rico e idempotente**: `plan`/`apply`/`diff` (3 colunas)/`drift`/`delete`; 33 Kinds com apiVersion/FORM.
- **Interop Docker provada ponta-a-ponta**: o cliente `docker` oficial falou com `serve docker-api` via `DOCKER_HOST`.
- **Transparência**: `compatibility docker` (21/28, 7 recusadas com razão), `provider ls` (149 capacidades), `serve --matrix`, `system features` (níveis de maturidade honestos).
- **self-healing de runtime**: `--restart always/on-failure/unless-stopped` + supervisor por-container (verificado: mata→reinicia).

## Índice do backlog desta ronda
- `BUG_BACKLOG.md` — BUG-0007 (ImageID docker-api), BUG-0008 (grupos nobody no exec), BUG-0009 (drift casing).
- `GAP_BACKLOG.md` — GAP-0018..0026: paridade Docker/CLI + arquitectura (layer-sharing, liveness declarativa, build-cache GC).
- `PERF_KERNEL_BACKLOG.md` — KRN-0001..0006: netlink nativo, pidfd, io_uring, landlock, KVM/virtio nativo, higiene de crates. **Prioridade de topo (dono, 2026-11-17).**
