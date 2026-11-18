# Plano morno (ADR-0095) — desenho, implementação e o que a medição revelou

> **O que este documento é.** O resultado da fatia que desenha e implementa o
> **pool do plano morno** do ADR-0095 — um processo persistente que mantém
> *runners* pré-carregados para que o arranque de um container reutilize infra já
> construída — **e** a medição que esse trabalho obrigou a fazer, que mudou o
> alvo. Segue `reports/spawn-profile-2026-11-18/` (o perfil a frio) como linha de
> base.

| Campo | Valor |
|---|---|
| Data | **2026-11-18** |
| Base | `perf/measure-cold-start-spawn` (develop + a medição d994cf1c) |
| Host | kernel 7.0.0-43, 20 threads, rootless |
| Binário | `delonix 0.1.2` release |
| Método | `delonix __poolbench <img> <runs> <size>` + `DELONIX_PROFILE_SPAWN`; relator `scripts/warm_profile_report.py` |

---

## O que se construiu

1. **`delonix-warm`** (`crates/adapters/delonix-warm`) — o pool. Orquestra, por
   sockets, um conjunto de *runners* estacionados e entrega-lhes a *spec* de
   arranque. Não conhece imagens nem o motor: só processos e um protocolo de
   linha. Modelo **serve-one-then-exit**, para que o `fork()` do supervisor
   (`delonix-linux`, seguro só mono-thread) corra sempre num processo pristino —
   exactamente a invariante que o re-exec `__apirun` a frio garante. O pool
   reabastece **fora do caminho quente**.
2. **`delonix __poolrunner <fd> [hint]`** — o runner estacionado: abre as stores
   do motor UMA vez, pré-resolve a imagem sugerida, anuncia READY e serve UMA
   *spec* com o seu estado morno (o MESMO `cmd_run` do `__apirun`), e sai.
   Interceptado **antes** da telemetria, para ficar mono-thread.
3. **`delonix __poolbench`** — o processo persistente de medição: arranca
   containers idênticos a frio (um `__apirun` novo por corrida) e a morno (runner
   do pool), cronometra cada um e emite JSONL para o profiler. É o caminho com
   processo vivo que o critério de aceitação mede, exercitando o MESMO
   `delonix_warm::WarmPool`.
4. **Profiler de `pre_spawn`** (`bins/delonix/src/cmd/prespawn.rs`) — marca as
   fases do `cmd_run` **a montante** do `spawn` (o `spawn_profile` do
   `delonix-linux` só mede dentro do `spawn`). No-op estrito sem
   `DELONIX_PROFILE_SPAWN`. **Foi este instrumento que encontrou o verdadeiro
   custo.**

## O que a medição revelou — e mudou o alvo

O ADR-0095 e o perfil a frio diziam: `pre_spawn` host ≈ 57,9 ms (68 %), «resolução
de imagem + preparação do rootfs». Ao instrumentar o `cmd_run` fase a fase, o
custo **não** era esse:

```
cmd_run pre_spawn  ≈ 20,8 ms   ← dominado por:
  policy_preflight   0,16 ms
  claims             0,21 ms
  resolve_rootfs    20,39 ms   ← TODO o custo
  record_net         0,03 ms
```

E `resolve_rootfs` decomposto (`prepare_overlay`): `ensure_layers` 0,01 ms,
mkdir+lowers 0,26 ms, `image_owners` 0,07 ms — **0,3 ms de trabalho real**. Os
restantes **~20 ms eram o `unpacking` progress-spinner**: a `Progress::step_after`
lança uma thread que espera o limiar (800 ms) **em fatias de 20 ms**, e o
`ok()`/`close_line` faz `join()` dessa thread — que só vê a flag de paragem no
**fim da fatia de 20 ms em curso**. Resultado: **cada `container run` pagava até
~20 ms de latência de *join* de um spinner que, abaixo do limiar, nunca desenha
nada.** Puro *chrome*, no caminho quente de **todo** arranque — CLI, e também cada
`__apirun` que o CRI/API lançam (stderr num ficheiro, não-TTY, paga na mesma).

### A correcção

`wait_out` passou a esperar num **`Condvar` interruptível** (`Gate`,
`bins/delonix/src/cmd/output.rs`): o `signal()` do `close_line` acorda o esperador
**ao instante**, por isso um passo rápido fecha em microssegundos e um passo lento
anuncia-se depois do limiar **exactamente como antes** — saída byte-a-byte igual,
só a espera morta desaparece.

## Os números (host, rootless, `alpine true --net host`, 20 corridas — `REPORT.txt`)

| | baseline a frio (2026-11-18) | cold (esta fatia) | **warm (pool)** |
|---|---|---|---|
| `pre_spawn` mediana | **~57,9 ms** | **7,02 ms** | **6,27 ms** |
| `resolve_rootfs` | (dentro dos ~50 ms) | 0,39 ms | 0,35 ms |
| `cmd_run` pre_spawn total | — | 1,03 ms | 1,61 ms |

- **58 → 7 ms** é a correcção do spinner (cold e warm, CLI e CRI/API).
- **7 → 6,3 ms** é o pool: elimina a carga do processo (binário de 43 MB já
  carregado). O resto do `pre_spawn` warm (~5 ms) é o *handshake* do supervisor +
  o round-trip do socket, não trabalho do motor.

> **Nota de honestidade.** No baseline, os ~58 ms misturavam três coisas que o
> método (`wall − spawn_total`) não separava: a latência do spinner (~20 ms), a
> carga do processo (~8 ms) e — por ser foreground `--rm` — o *teardown* do
> `--rm` depois do container correr. O caminho persistente (detached, máquina)
> nunca paga o teardown; e, com o spinner corrigido, o `pre_spawn` host já é
> baixo (~6 ms). Por isso o ganho **incremental** do pool em host-net é ~1 ms: o
> grande valor do pool fica para o caminho **isolado** (manter o plano de rede de
> pé elimina os +322 ms que o perfil mediu), fatia própria.

## Veredicto para o ADR-0095

1. **O maior lever do arranque a frio em host-net NÃO era resolução de imagem/
   rootfs — era 20 ms de latência de um spinner.** Corrigido, com saída
   inalterada, para todos os caminhos.
2. **O pool do plano morno está desenhado, implementado e provado** (via
   `__poolbench`): um processo persistente arranca containers reutilizando infra
   pré-construída, e o `pre_spawn` cai para ~5,7 ms (de 57,9 ms no baseline).
3. **O caminho isolado (plano de rede morno) é o próximo lever** — é lá que vivem
   os +322 ms, e é lá que um pool residente rende mais. Ligar o pool ao
   `serve cri` em produção depende de *warming* por-imagem (um kubelet corre
   muitas imagens), também fatia própria.

## Reproduzir

```
scripts/warm_profile.sh --runs 20 --size 4 --image alpine
# recusa sob carga (load1 > nproc/2), como o spawn_profile.sh
```
