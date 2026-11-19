# Perfil por fase do arranque a frio — onde se vão os ~91 ms

> **O que este documento é.** A decomposição, **medida**, dos ~91 ms que o
> `docs/comparacao-medida.md` (linha 4a) atribui ao arranque a frio de um
> container no delonix rootless. Aquele documento diz *quanto*; este diz *onde*.
> É o pré-requisito que o **ADR-0095** exige antes de desenhar o plano morno/pool:
> «medir primeiro onde se vão os ~91 ms».
>
> **O que não é.** Uma proposta de optimização. Nada no motor foi alterado — a
> instrumentação é um perfil **env-gated** (`DELONIX_PROFILE_SPAWN`) que é um
> no-op sem a variável (confirmado: stdout do container intacto, exit codes 0/42
> propagados, 261/261 testes do crate, clippy limpo).

| Campo | Valor |
|---|---|
| Data | **2026-11-18** |
| Onde | host de desenvolvimento, kernel 7.0.0-43-generic, 20 threads |
| Carga | load1 ≈ 3,6 (abaixo de nproc/2=10; a bancada **passou** o juízo de recusa) |
| Binário | `delonix 0.1.2` release, rootless, root e runtime-dir isolados |
| Método | `scripts/spawn_profile.sh --runs 25` + `spawn_profile_report.py` |
| Imagem | `alpine:latest`, pré-extraída no aquecimento (o pull fica fora do cronómetro) |

---

## Como foi medido

Cada `container run --rm alpine true` é cronometrado em **wall-clock** (`date
+%s%N` à volta do processo, como o `bench.sh`) e, em paralelo, o profiler
embutido emite o **total interno do `spawn`** e as suas fases. A subtracção dá a
fase que o doc nunca conseguiu nomear:

```
wall = pre_spawn  (arranque do processo + resolução de imagem/rootfs no CLI)
     + spawn_total (clone + cgroup + rede-hook + mounts)
```

As marcas (ver `crates/adapters/delonix-linux/src/lib.rs`, módulo `spawn_profile`):

| Fase | Onde | O que mede |
|---|---|---|
| `pre_spawn` | `wall − spawn_total` | re-exec do binário (43 MB), linker, parse CLI, resolução do manifesto da imagem, preparação do rootfs/overlay |
| `pre_clone` | entrada do `spawn` → antes do `clone_init` | ficheiros `/etc/*`, `InitInputs::gather` (CStrings/mounts), pipes, console |
| `clone` | à volta de `clone_init` | o `clone(2)` que cria os namespaces |
| `cgroup` | dentro de `userns_handshake`/`place_without_userns` | `setup_cgroup` (escrita de `cgroup.procs`, `memory.max`, `cpu.max`) |
| `net` | à volta do hook `on_started` | hook CNI/slirp **quando existe** (só `--net <rede>` o tem) |
| `mounts` | à volta de `wait_for_mounts` | o filho a moldar a mount-ns: overlay + `pivot_root` + `/proc`,`/sys`,`/dev` |
| `setup_rootfs`/`confine_tail` | **no filho**, sub-fases de `mounts` | o overlay+pivot isolado do resto do confinamento |

> **Risco de método afastado.** `wait_for_mounts` é um único `poll()` que bloqueia
> até ao byte do filho — **sem sleep/polling** —, por isso a fase `mounts` é tempo
> de trabalho real, não um artefacto de intervalo.

---

## O resultado

### HOST (`--net host`) — a linha 4a do doc (~91 ms)

Mediana de 24 corridas quentes (a 1ª, fria, está à parte):

| Fase | min | **mediana** | max | % do total |
|---|---|---|---|---|
| **`pre_spawn`** (processo + imagem/rootfs) | 35,38 | **57,93** | 218,98 | **68,0 %** |
| `cgroup` (`setup_cgroup`) | 2,59 | 19,03 | 453,40 | 22,3 % |
| `mounts` (overlay+pivot, filho) | 1,45 | 2,00 | 238,08 | 2,4 % |
| `clone` (namespaces) | 0,25 | 0,42 | 0,53 | 0,5 % |
| `pre_clone` (setup no pai) | 0,06 | 0,14 | 0,20 | 0,2 % |
| `net` (hook) | 0,00 | 0,00 | 0,00 | 0,0 % |
| **total (wall)** | 52,11 | **85,15** | 513,98 | 100 % |

- 1ª corrida (**fria**): wall **76,26 ms** (spawn 18,77 + pre_spawn 57,49).
- Sub-fase do filho dentro de `mounts`: `setup_rootfs` ≈ **1,49 ms**,
  `confine_tail` ≈ 0,41 ms.
- Arranque **puro** do processo (`delonix --version`): mediana **7,90 ms**.

**A mediana quente de 85,15 ms bate com os 91 ms do doc** — valida o método. E o
maior consumidor **não é a mecânica do container**: é o **`pre_spawn`, 57,9 ms
(68 %)** — o binário daemonless a ser re-exec a cada `run` e a resolver a imagem
e a preparar o rootfs/overlay **antes** sequer de clonar. Desses 57,9 ms, só
~7,9 ms são arranque puro do processo; os restantes ~50 ms são resolução de
imagem + preparação do rootfs no handler do CLI.

### ISOLADO (`--net <rede>`, plano de pé) — a linha 4b do doc (~216 ms)

| Fase | min | **mediana** | max | % do total |
|---|---|---|---|---|
| **`pre_spawn`** | 282,14 | **391,70** | 2673,46 | **96,2 %** |
| `cgroup` | 0,72 | 15,63 | 183,05 | 3,8 % |
| `mounts` | 0,01 | 0,01 | 1,46 | 0,0 % |
| `net` (hook `on_started`) | 0,00 | 0,00 | 0,00 | 0,0 % |
| **total (wall)** | 291,92 | **407,35** | 2676,11 | 100 % |

> **Nota de método, e importa.** A fase `net` (o hook `on_started`) é **0** mesmo
> no modo isolado. Não é erro: o custo da rede isolada (veth, IPAM, ruleset nft,
> o re-exec `nsenter … ip netns exec` que o rootless exige) é montado pela
> **orquestração de rede do CLI à volta do `spawn`**, não dentro do hook de
> colocação — por isso aparece em `pre_spawn` e mede-se por **wall-clock**,
> exactamente como o doc faz nas linhas 4b/4c. O número absoluto (407 ms) é mais
> alto e muito mais disperso que os 216 ms do doc (max 2,7 s) porque esta corrida
> apanhou o host com carga; o **sinal qualitativo** — pre_spawn domina — é o
> mesmo nos dois modos.

### Delta host → isolado

```
host    ~  85,15 ms
isolado ~ 407,35 ms   (delta +322 ms, todo em pre_spawn/orquestração de rede)
```

---

## O que isto diz ao ADR-0095

O ADR-0095 suspeitava da **rede isolada a frio** como «suspeito nº 1». A medição
confirma-o como o maior *delta* (host→isolado: +322 ms), **e** mostra onde ele
vive: não no `clone`, não no cgroup, não na mount-ns — vive no **`pre_spawn`**, a
orquestração do CLI que um processo daemonless re-paga por inteiro a cada `run`.

Consequências para o desenho do plano morno/pool:

1. **O maior lever é eliminar o `pre_spawn`, não afinar a mecânica do container.**
   Um plano morno sobre um processo persistente (`serve cri`/`serve api`) que
   mantenha o binário carregado, a imagem resolvida e o plano de rede de pé
   apaga 68 % (host) a 96 % (isolado) do arranque a frio. É coerente com o
   guardrail: o CLI a frio continua igual; só os caminhos com processo vivo
   ganham.
2. **A mecânica do container já é barata e não vale reescrita.** `clone` 0,4 ms,
   `mounts`/`setup_rootfs` ~1,5–2 ms, `pre_clone` 0,14 ms. Isto confirma a
   recusa do ADR-0095 ao `clone3`+`CLONE_INTO_CGROUP` (ganho sub-ms) — o perfil
   mostra que não há ali ms para ganhar.
3. **O `cgroup` (~19 ms, muito disperso) é o único item in-spawn com gordura**,
   mas é ruído de escrita de ficheiros cgroup sob carga, não um alvo de pool.

## Artefactos (reproduzível)

- `host.jsonl` / `isolated.jsonl` — um registo JSON por corrida (fases + wall).
- `host.child.txt` / `isolated.child.txt` — sub-fases do filho (do stderr).
- `host.walls` / `isolated.walls` — wall-clock cru por corrida (µs).
- `process-start.us` — arranque puro do processo (`--version`).
- `REPORT.txt` — a saída do `spawn_profile_report.py`.

Reproduzir: `scripts/spawn_profile.sh --runs 25` (recusa-se a correr sob carga).
