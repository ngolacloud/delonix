# §15 — Estabilidade e desempenho sustentado

**Candidato medido:** `delonix 0.1.0` release (`8ab783dd`, o candidato shipped) para
os números de recurso/latência; footprint do P2-1 medido com o binário da linha
`stabilize/0.1.1` (debug). **Data:** 2026-10-29. **Host:** Linux 7.0.0, rootless,
root de estado isolado em scratch (não o do dono).

## Workload e critérios (definidos ANTES de medir)

- **Churn:** 300× `run --rm alpine:3.20 -- true` (ciclo create→start→exit→rm) +
  30× lifecycle explícito `create→start→stop→rm`, com 1 container de longa
  duração (`sleep 1000`) a servir de supervisor-sob-observação.
- **Aprovação:** sem crescimento monótono de fds/RSS do supervisor; records de
  estado não acumulam (`--rm`/`rm` limpam); leaves de cgroup `dlx-*` não
  acumulam; 0 órfãos pós-churn (volta à baseline = só o holder); latência por
  ciclo estável (sem subida monótona).

## Resultados

| Métrica | Baseline | @300 churn | @+30 lifecycle | Veredicto |
|---|---|---|---|---|
| supervisor fds | 3 | 3 | 3 | ✅ sem fuga de fd |
| supervisor RSS | 832 kB | 832 kB | 832 kB | ✅ sem crescimento |
| records `.json` | 1 | 1 | 1 | ✅ `--rm`/`rm` limpam |
| cgroup `dlx-*` leaves | 11 | 11 | 11 | ✅ **sem leaf-leak** (a preocupação histórica NÃO reproduz aqui) |
| containers restantes | 1 (holder) | 1 | 1 | ✅ 0 órfãos |
| latência/ciclo | ~0.07 s regime (2 primeiros ~0.4 s = warmup frio) | estável | — | ✅ sem degradação |

**Idle:** o supervisor de um container parado consome **832 kB RSS / 3 fds**
(perfil daemonless muito leve). Rede «down» até ser precisa.

## Achado S15-1 — acumulação ILIMITADA de ficheiros de lock (P3)

Os ficheiros de lock no `containers/` **nunca são apagados** (por desenho:
apagá-los abriria uma janela em que dois processos trancam inodes diferentes).
Medido:

- **Per-id `.{id}.lock`** (PRÉ-EXISTENTE, antes desta campanha): 330 containers →
  **330** ficheiros de lock; crescem 1:1 com containers já criados.
- **`.name-{ns}.{name}.lock`** (ADICIONADO pelo meu fix **P2-1**): 30 containers →
  **30** name-locks. Como os nomes auto-gerados do `run --rm` são únicos, o P2-1
  **~duplica** a taxa de acumulação (um id-lock + um name-lock por container).

**Impacto:** num nó de alta rotatividade e vida longa (p.ex. PaaS a criar
milhões de containers), o directório `containers/` acumula milhões de ficheiros
de 0 bytes. Cada `Store::list()` (usado por `ps`, pela verificação de nome, etc.)
faz `read_dir` de TODAS as entradas → o scan degrada linearmente com o histórico
total de containers, não com os vivos. É consumo ilimitado de recurso (§6#9) e
degradação de latência sustentada (§15). Severidade **P3** (0 bytes cada, sem
perda de dados; só morde à escala e no longo prazo).

**Mitigações propostas (decisão de desenho do dono):**
1. **Lock partilhado único** para o P2-1: em vez de um `.name-*.lock` por nome,
   serializar o check-de-nome+escrita-do-record num único `.names.lock`
   (1 ficheiro, permanente) mantido só à volta da secção crítica *curta* (não do
   trabalho pesado de imagem/rootfs). Elimina os name-locks per-nome; contenção
   mínima. Requer mover o `save` para dentro desse lock após o trabalho pesado.
2. **GC seguro de lock-files mortos:** uma passagem (estilo `startup-cgroup-gc`)
   que, sob um lock global, poda `.{id}.lock`/`.name-*.lock` sem record vivo nem
   processo a segurá-los. Fecha também o legado per-id.

## Cobertura e limites honestos

- **Coberto:** o caminho daemonless CLI (create/run/stop/rm) — leaks de
  fd/memória/cgroup/estado/processo e latência, em 330 ciclos.
- **NÃO coberto:** (a) os **servidores de longa duração** (`delonix-cri`,
  `delonix-node-api`, `delonix-mgmt`) que correm continuamente num nó —
  precisam da sua própria bateria sustentada (arranque do servidor + churn via
  CRI/gRPC + RSS/fd ao longo de horas); (b) duração longa real (horas / 10⁴–10⁶
  ciclos) — este teste é 330 ciclos (minutos). **Não se afirma "sem fugas"**, mas
  sim "sem fuga observada em 330 ciclos, com fds/RSS/leaves/records planos".
- Comparação com baseline v5.1.1 em condições equivalentes: não feita (fica para
  o E2E sustentado em nó dedicado).
