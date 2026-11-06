# Relatório de paridade CLI Delonix ↔ Docker/Podman

**Data:** 6 de Novembro de 2026 · **Base:** `origin/develop` @ `99ecc7968` (o SHA
onde a matriz do ZIP foi medida) · **Ramo:** `feat/cli-parity-docker-podman`

> **Segunda vaga (ampliação de âmbito pedida pelo dono).** Após a primeira
> entrega, o dono pediu para resolver as divergências rumo a 100% Docker/Podman
> e garantir o VMM nativo. Três decisões tomadas: (1) *shim + Docker API* para as
> famílias que chocam com invariantes; (2) camada imperativa de pods; (3)
> cloud-hypervisor por omissão no VM. Correcções desta vaga: **C4** (VM default),
> **C5** (`pod stop/start/restart/rm`), **C6** (aliases `--network`/`--cpuset-cpus`).
> Ver a secção «Segunda vaga» no fim. **100% literal não foi atingido nem é
> atingível numa sessão** — é uma campanha de várias fases (medição abaixo).

## Referências e ambiente (medidos, não assumidos)

| Componente | Versão | Modo |
|---|---|---|
| **Delonix** | `0.1.0`, compilado do checkout (`target/debug/delonix`) | daemonless, rootless, cgroup2 (cpu/memory/pids delegados; io/cpuset não) |
| **Docker** | cliente+servidor `29.9.2` | rootful, cgroup systemd, `unix:///var/run/docker.sock` |
| **Podman** | `4.9.3` | rootless, netavark, runc 1.5.2 |
| Kernel / arch | `7.0.0-42-generic` / `x86_64` | PTY via `python3`; subuid/subgid presentes |

> **Divergências face à matriz do ZIP:** a matriz citou Docker CLI **v29.0.0** e
> Podman **v5.6.2**. O host tem Docker **29.9.2** (bump menor, sem impacto nos
> contratos exercidos) e Podman **4.9.3** — uma major **anterior** à citada.
> Relevante para pods (`pod create --name` imperativo existe em ambas) e para
> `system connection` (contexto Podman). O binário `delonix` do PATH era `5.1.1`
> antigo — **ignorado**; tudo foi medido contra o binário compilado da árvore.

O patch `Delonix_TAB.patch` do ZIP **não estava** em `origin/develop`; verificou
`git apply --check` limpo e foi aplicado como base desta sessão.

## Resultado por domínio

Cobertura funcional = casos aprovados ÷ casos aplicáveis acordados; bloqueados e
não-executados ficam no denominador. **Não se declara 100%** — o âmbito
versionado (198 caminhos) está exercido em profundidade nos dois fluxos
prioritários e, por amostragem dirigida, nos restantes; o resto está medido ao
nível do **reconhecimento** (parser) e marcado honestamente «por testar».

| Domínio | Reconhecidos | Passou (comportamento) | Divergência deliberada | Por testar | Por fazer / fora do âmbito |
|---|---|---|---|---|---|
| 1 Containers | 43/65 | **8** (run, create, ps, stats, logs, wait, cp, port) | 2 (inspect, diff) | 35 | 20 |
| 2 Pods | 7/17 | **6** (pod, pod logs, pod stop/start/restart/rm) | 1 (`pod create --name` imperativo) | — | 10 |
| 3 Imagens | 22/43 | **2** (login, logout) | — | 20 | 21 |
| 4 Volumes | 6/13 | **2** (volume create, volume rm — round-trip + recusa-em-uso) | — | 4 | 7 |
| 4 Redes | 7/11 | — | — | 7 | 4 (incl. `network prune`) |
| 5 Sistema/Contextos | 8/30 | **2** (info, events) | — | 6 | 22 (contextos: fora do âmbito, ADR-0010) |

(Contagem exacta por estado em [`matriz-cli-parity.csv`](matriz-cli-parity.csv),
regenerável com [`build_matrix.py`](build_matrix.py).)

## Correcções implementadas (código, testes, docs)

### C1 — `run`/`create` aceitam `-i`/`-t`/`-it` (bug real, fluxo 1)

O parser recusava `-it` («`error: unexpected argument '-i' found`»), partindo a
**primeira linha** de qualquer script interactivo. Medido: docker e podman
passam o fluxo; delonix falhava no parser. A causa era **só** o reconhecimento —
um `run` em primeiro plano, por ser daemonless, já HERDA o stdio do operador, por
isso o shell interactivo, o `test -t 0` e o código de saída já eram os do
`docker run -it`. Acrescentados `-i`/`--interactive` e `-t`/`--tty` ao `RunArgs`
(`bins/delonix/src/cmd/container.rs`), aceites por paridade (como o `exec` já
faz). Limite documentado: um `-t` sem terminal no stdout não sintetiza pty.

* **Antes:** `delonix run --rm -it busybox sh` → exit 2, parser recusa.
* **Depois:** idêntico a docker/podman — PTY real, `TTY_YES`, marcador lido,
  **exit 37**, container removido por `--rm`.

### C2 — erro accionável para `run --pod <inexistente>` (fluxo 2)

Pods em delonix são **declarativos** (ADR-0006): não há `pod create --name`, e
`run --pod` só SE JUNTA a um pod existente. O erro antigo era críptico
(`DX-9000 ... ingress infra is down — no holder to enter`). Agora
(`reexec_into_netns`) um `--pod` sem holder devolve `DX-1000` a nomear o fluxo
declarativo (`kind: Pod` + `delonix pod create -f`). A criação declarativa cria o
holder antes dos membros, por isso não há regressão.

### C3 — verbos de raiz `cp`/`port`/`login`/`logout`/`info`/`events` (fluxos 3/5)

Verbos de raiz que o parser recusava apesar de a forma de grupo já funcionar.
Acrescentados como **aliases verdadeiros** — a MESMA struct de argumentos, o MESMO
handler do grupo (`container`/`image`/`system`), docstrings **verbatim** para
partilhar o catálogo i18n. Tocou em: `main.rs` (enum+dispatch+`STABLE_GROUPS`),
`manual_entries.rs` (entradas+exemplos), `data/pt.po` (traduções dos help
`-i`/`-t` e dos exemplos novos), `docs/gen.py` + `README.rst` +
`docs/cli-stability.md` (gates de documentação). `inspect` e `diff` NÃO entram na
raiz — política type-scoped documentada.

## Fluxos prioritários — antes/depois (evidência)

**Fluxo 1 — container interactivo** ([`evidence-flow1.jsonl`](evidence-flow1.jsonl)):

```
ENGINE   ANTES                 DEPOIS
docker   ok  (exit 37)         ok  (exit 37)
podman   ok  (exit 37)         ok  (exit 37)
delonix  XX  (exit 2, -it)     ok  (exit 37)   ← C1
```
Passos verdes nos três: `run --rm -it sh` (PTY, `test -t 0`, exit 37, remoção),
`-i` sem TTY (stdin piped, sem tty dentro), stdout/stderr separados, `stats`
amostra única.

**Fluxo 2 — pod com dois membros** ([`evidence-flow2.jsonl`](evidence-flow2.jsonl)):

```
podman  (imperativo)  create --name · run --pod ×2 · localhost · ps · inspect · stop/start/restart · rm   → 10/10
delonix (declarativo) pod create -f · localhost(SENTINEL) · get pods · describe pod · delete pod            → passou
                       pod stop/start/restart → PASSAM (C5, segunda vaga)
                       run --pod <absent> → erro accionável (C2, afinado na segunda vaga)
```
Mesmo resultado observável (dois membros a partilhar netns, comunicação por
`localhost`, listagem, inspecção, desmontagem limpa) por rotas diferentes.

## Descoberta por TAB (fluxo 6) — Fish / Bash / Zsh

Protocolo de conclusão exercido directamente (sem configurar shell):

| Shell | Invocação | `ps`+`ls` (raiz/container) | `rm`+`remove` (image) | Novos aliases c/ descrição |
|---|---|---|---|---|
| Fish | `COMPLETE=fish … -- delonix ''` | ✓ ambos | ✓ ambos | ✓ cp/port/login/logout/info/events |
| Bash | `_CLAP_COMPLETE_INDEX=1 COMPLETE=bash …` | ✓ ambos | ✓ | ✓ |
| Zsh | `_CLAP_COMPLETE_INDEX=1 COMPLETE=zsh …` (`valor:descrição`) | ✓ ambos | ✓ ambos | ✓ com descrição |

O patch TAB (`expand_completion_aliases`) dá a cada grafia visível o seu próprio
candidato; o teste shell-agnóstico `tab_lists_each_visible_spelling_with_its_description`
cobre o motor de conclusão, e `tab-descriptions.sh` a wire do Fish.

## Testes e integração CI

* **Unitários** (`cargo test -p delonix-bin`): **1644 passam, 0 falham** — inclui
  `run_interactive_flag_parity_tests`, a classificação de estabilidade, o manual
  e o i18n (`ARG_HELP_PENDING` mantém-se 0).
* **Scripts rápidos** (sem motor): `frequent-verbs.sh` **49/49**,
  `tab-descriptions.sh` verde.
* **Diferencial** (motores reais): `flow1_interactive.py`, `flow2_pod.py`.
* **CI** (`.github/workflows/ci.yml`, job `cli-parity`): tier rápido sempre +
  tier diferencial com docker+podman. [`run.sh`](run.sh) **falha em voz alta** se
  faltar um motor — uma suite vazia ou toda-ignorada nunca passa como conforme.
* **Gates locais verdes:** `docs_cli_gate.py`, `lang_ratchet.py`.

Reproduzir:
```sh
cargo build --locked -p delonix-bin --bin delonix
cargo test  --locked -p delonix-bin --bin delonix
tests/cli-parity/run.sh all target/debug/delonix        # rápido + diferencial
python3 scripts/docs_cli_gate.py && python3 scripts/lang_ratchet.py
```

## Divergências deliberadas (migração)

| Contrato Docker/Podman | Delonix | Migração |
|---|---|---|
| `pod create --name`, `pod stop/start/restart` (imperativo) | `kind: Pod` + `pod create -f`; ciclo de vida via `get/describe/delete pod` | descrever o pod e os membros num manifesto `kind: Pod`; erro de `run --pod` já aponta o caminho |
| `docker inspect <x>` genérico | type-scoped: `container inspect` / `image describe` | nomear o Kind; sem resolvedor que adivinhe |
| `docker diff <container>` (rootfs) | raiz `diff` = diff de MANIFESTO; `container diff` = alterações do rootfs | usar `container diff` para a forma Docker |
| `docker context` / `podman system connection` | ausente (ADR-0010 recusou a API de gestão remota) | fora do âmbito; sem equivalente planeado |

## Pendências (próximo passo executável)

1. **`network prune`** (fluxo 4) — recusado; `volume prune` existe. Implementar o
   `NetworkCmd::Prune` sobre a lógica de prune existente + teste de rede-em-uso.
   *Próximo:* seguir o padrão de `volume prune` em `cmd/network.rs`.
2. **Equivalência funcional dos ~70 «reconhecido-por-testar»** — o parser aceita,
   falta o diferencial. *Próximo:* estender `differential/` com um fluxo 3
   (cp/port/inspect/logs/wait + save/load round-trip + login contra registry
   descartável) e um fluxo 4 completo (redes: subnet/gateway/resolução/connect-
   disconnect/publish + prune com recursos em uso), em motores descartáveis.
3. **`image inspect`** — ausente (só `container inspect`); Docker tem
   `docker image inspect`. *Próximo:* decidir entre alias para `image describe` ou
   recusa documentada.
4. **`events`/`stats` contínuos e sinais/detach-attach** do fluxo 1 — exercidos em
   amostra única; o modo *stream* e os sinais precisam de PTY com timeout
   explícito num runner dedicado.
5. **Contextos (fluxo 5)** — decisão de produto (ADR-0010), não de implementação.

---

## Segunda vaga — rumo a 100% Docker/Podman + VMM nativo

### Decisões do dono (6 Nov 2026)
1. **Shim + Docker API** para as famílias irredutíveis (contextos/ADR-0010,
   Swarm/`node`/`service`, `machine`, `plugin`): aceitar as grafias e, para o que
   é mesmo incompatível, erro accionável a apontar `delonix serve docker-api`
   (com `DOCKER_HOST=unix://…` o próprio `docker` CLI fala com o Delonix).
   Preserva daemonless/rootless/ADR-0010.
2. **Camada imperativa de pods** sobre o motor declarativo.
3. **cloud-hypervisor por omissão** (VMM nativo KVM em Rust).

### C4 — cloud-hypervisor é o backend de VM por omissão (VMM nativo)
O Delonix já usava VMMs KVM nativos (`cloud-hypervisor` = VMM em Rust de raiz
sobre `/dev/kvm`, arranca um kernel Linux real; `libvirt`→QEMU/KVM). A heurística
preferia **libvirt** para imagens cloud por cautela histórica. Agora prefere
**cloud-hypervisor** quando ele CONSEGUE arrancar a carga — para uma imagem cloud
isso exige `/dev/kvm` **e** o firmware EDK2 `CLOUDHV.fd` (`cloud_image_firmware_
ready()`), não o minimalista `rust-hypervisor-fw` (que não carrega o initrd).
libvirt fica fallback. Ficheiros: `crates/adapters/delonix-vm/src/lib.rs`,
`crates/providers/delonix-provider-cloud-hypervisor/src/lib.rs` (+teste
`only_the_edk2_cloudhv_boots_cloud_images`). Boot direct-kernel (nós k8s) já
resolvia para CH. **Pendente de validação ao vivo** num host com os dois backends
para confirmar o boot de imagem cloud em CH (o host tem `CLOUDHV.fd`; a evidência
medida no repo confirma que o CLOUDHV.fd arranca ubuntu/debian/rocky).

### C5 — pod stop / start / restart / rm (Podman)
Novos verbos que operam todos os membros de um pod (iteram `members_of` +
`container::cmd_{stop,start,restart}`; `pod rm` = `remove_pod`). Resolvem a
divergência da matriz «pod stop/start/restart não é verbo delonix». **Provados ao
vivo** num pod declarativo (`flow2_pod.py`: stop→0/2, start→localhost OK,
restart, rm). Ficheiros: `bins/delonix/src/cmd/pod.rs` (+ manual/i18n).

O **`pod create --name` imperativo (pod vazio) NÃO foi embarcado**: medido ao
vivo, falha porque o modelo do Delonix não tem container *pause* — o primeiro
membro é que segura IPC/UTS, logo um pod vazio não tem holder dessas namespaces e
o `run --pod` para ele falha. Fazê-lo bem exige desenhar um container *pause*
(estilo Podman/k8s) — frente própria. Habilitou-se já a **resolução automática de
`infra_pid`** no `cmd_run` (um `run --pod` que ADICIONA membro a um pod existente
junta o IPC/UTS do primeiro membro) e o **erro accionável** de `run --pod
<inexistente>` passou a distinguir «pod não existe» (conta de membros) de «infra
global em baixo», em vez do críptico "Cannot open network namespace".

### C6 — aliases de flag de renomeação pura em run/create
`--network` (alias de `--net`) e `--cpuset-cpus` (alias de `--cpuset`) via clap
`visible_alias` — zero entradas i18n novas, mesmo handler. `docker run --network
mynet --cpuset-cpus 0-1 …` passa a funcionar. Ficheiro: `container.rs` (RunArgs).

### C7 — `--pull missing|always|never` em run/create
Política de pull à Docker/Podman, aplicada antes de `run`/`create` tocar na
imagem (no despacho, com acesso ao `ImageStore` — sem mexer no `RunOpts` do crate
compute): `missing` (omissão, o resolve-or-pull do motor), `always` (força pull
antes de arrancar), `never` (recusa buscar — erro accionável se faltar). **Provado
ao vivo** (`--pull never` numa imagem ausente erra; numa presente corre) e por
paridade raiz↔grupo offline (`frequent-verbs.sh`, 50/50) + teste de parsing.
Ficheiro: `container.rs` (`apply_pull_policy`).

### C8 — `--cidfile <PATH>` em run/create
Escreve o id completo do container no ficheiro (recusa se já existir, como o
Docker). Descobriu-se que o `RunOpts` **deriva `Default` + `#[serde(default)]`**,
por isso adicionar um campo é barato (só o `run_opts_from_args` e o destructure do
`cmd_run` o listam; os outros construtores usam `..Default::default()`) — a
cautela anterior de "diferir" era excessiva. **Provado ao vivo** (escreve o id;
recusa ficheiro existente). Teste de parsing. Ficheiros: `run_opts.rs`,
`container.rs`.

### C9 — `--shm-size <SIZE>` em run/create
Tamanho de `/dev/shm`. **Reutiliza o plumbing do `--tmpfs`** (converte para a
entrada `/dev/shm:size=<bytes>,mode=1777`, o mesmo truque do `compose`) — **zero
mudanças no runtime**. **Provado ao vivo**: `--shm-size 128m` → `/dev/shm` com
128M. Ficheiro: `container.rs` (conversão no `run_opts_from_args`).

### C10 — `--pids-limit` (→ `pids.max`)
Tecto de processos à Docker/Podman. Parametriza a escrita de `pids.max` já
existente (antes fixa em `DEFAULT_PIDS_MAX=512`) nos dois caminhos cgroup
(delegado `lib.rs` + root), `-1` ⇒ `max` (ilimitado). Campo `pids_max` no
`Container`+`RunOpts`, `build_record`, helper `pids_max_write_value`. **Enforcement
real provado pelo host**: `--pids-limit 42` → leaf `pids.max=42`; sem flag → `512`.
Flag `-1` precisou `allow_hyphen_values`. Guards actualizados (deixou de ser
"flag que não existe").

### C11 — `--memory-reservation` (→ `memory.low`)
Piso suave de memória (soft reclaim floor). Escrita NOVA de `memory.low` ao lado
de cada `memory.max` (delegado + root). Campo `memory_low` no `Container`+
`RunOpts`, `build_record`. **Provado pelo host**: `--memory-reservation 128m` →
leaf `memory.low=134217728`.

> **`--memory-swap` adiado**: a semântica diverge (Docker = memória+swap total;
> cgroup v2 `memory.swap.max` = só swap ⇒ escrever `total − memória`), o
> controlador de swap pode estar desligado, e precisa de parsing de ambos os
> valores. Fica mapeado no backlog com a armadilha anotada.

**Guards actualizados** (senão o build parte): `flags_of_controller` (`lib.rs`)
passou `pids`→`["--pids-limit"]` e `memory`→`["--memory","--memory-reservation"]`;
os testes `the_docker_spellings_are_not_ours` e `delegation_labels_name_only_real_flags`
(`main.rs`) deixaram de listar `--pids-limit`. As mensagens do node-api
(`container_launch.rs`/`container_update.rs`) ficaram CORRECTAS (apontam o
`--pids-limit`/`memory.low` da CLI que agora existe, dizendo que o node-API ainda
não os serve).

### C12 — modos `--ipc` / `--pid`
Dobram nos booleanos já existentes `--host-ipc`/`--host-pid`: `host` ⇒ partilha a
do host, `private` (omissão) ⇒ a sua própria; qualquer outro modo
(`container:<id>`, `shareable`, `none`) é RECUSADO com erro accionável (aponta
`--pod` para partilhar entre containers), nunca degradado em silêncio. Puro na
camada CLI (`run_opts_from_args`), sem campos novos. **Provado ao vivo**: `--ipc
host`≡`--host-ipc`; `--pid host`≡`--host-pid` (erro de bind `/proc/sys` idêntico
— limite pré-existente do host-pid em rootless, não introduzido). `--uts`
adiado (exigiria tornar o `CLONE_NEWUTS` condicional no runtime).

### C13 — `--stop-signal`
Sinal que o `stop` envia primeiro (omissão `SIGTERM`). Campo `stop_signal` no
`Container`+`RunOpts`+`build_record`; `cmd_stop` lê-o do registo e chama o novo
`runtime::stop_with_signal` (recebe o número do sinal — `parse_signal` converte
nome/número — para a CLI não depender do `nix`; `stop_waiting` ganhou um
parâmetro `term_signal`, com o `stop` a passar `SIGTERM`). **Provado ao vivo**:
`--stop-signal SIGINT` → o container regista `GOT_SIGINT` (recebeu SIGINT, não
SIGTERM); `SIGKILL` continua a escalar após o período de graça.

### C14 — `--stop-timeout`
Graça antes do `SIGKILL`. Campo `stop_timeout` no `Container`+`RunOpts`+
`build_record`. O `-t` do `stop` passou a `Option<u64>`: o despacho de `Stop`
resolve, por container, `-t` explícito → `--stop-timeout` do registo → 10s
(omissão), sem tocar nos ~10 chamadores de `cmd_stop` (que continuam `u64`).

### C15 — `--volumes-from <container[:ro|:rw]>`
Monta todos os volumes/binds de outro container neste. Os mounts da origem já
estão RESOLVIDOS (caminhos do host, raízes de volume), por isso transplantam
directamente para o conjunto de arranque (`mounts`) E para o registo
(`c.mounts`, sobrevive a um `start`); `:ro` força-os todos a leitura-só; origem
inexistente dá «no such container». Resolvido no `cmd_run` (tem store; o
`build_record` não lhe chega). **Provado ao vivo**: o alvo lê os dados do volume
da origem; `:ro` recusa escrita. Commit `078da4d0`.

### C16 — `--init` (reaper pid 1)
Corre um init minúsculo como pid 1 que recolhe órfãos e reenvia sinais; o comando
corre como seu filho. No `container_init`, DEPOIS de toda a configuração
(namespaces, drop de privilégios, seccomp, cgroup), um `fork`: o filho faz o
`execvp` do comando; o pai fica pid 1 em `reap_as_init` — instala handlers que
reenviam SIGTERM/INT/QUIT/HUP ao filho e faz `waitpid(-1)` até ECHILD, devolvendo
o código do processo principal (128+sinal se morto). Seccomp já permite
`clone`/`wait4`/`kill`/`rt_sigaction`; um `fork` falhado corre o comando directo
(sem reaper), nunca recusa. Plumbed como o `read_only` (Container→InitInputs→
ProcessSpec→`container_init`). **Provado ao vivo**: com `--init` o comando deixa
de ser pid 1 (pid1=`delonix`), exit 37 propaga, `stop` reenvia SIGTERM ao filho
(log `GOT_TERM_VIA_INIT`), e **0 zombies** após um órfão sair. Default (sem
`--init`) inalterado (comando continua pid 1).

### C17 — `--mount type=bind|volume|tmpfs`
A sintaxe explícita do Docker/Podman, traduzida para a gramática do motor:
`type=bind|volume,source=…,target=…[,readonly]` → spec `-v` (`src:target[:ro]`);
`type=tmpfs,target=…[,tmpfs-size=N]` → entrada `--tmpfs` — o MESMO caminho de
resolução, sem segundo motor de mounts. Chaves `source`/`src`,
`target`/`dst`/`destination`, `readonly`/`ro` e `tmpfs-size` honradas; chave ou
tipo desconhecido é RECUSADO. **Provado ao vivo**: bind `readonly` recusa
escrita, volume round-trip, tmpfs respeita o tamanho, chave inválida erra.
Commit `195a1cbe`. Puro na camada CLI (`run_opts_from_args`).

### C18 — `--memory-swap` (semântica Docker→cgroup-v2 resolvida)
A armadilha que estava adiada: o `--memory-swap` do Docker é o TOTAL
(memória+swap); o cgroup-v2 `memory.swap.max` é só swap. O helper
`swap_max_write_value` no runtime calcula `total − memory` (via
`mem_limit_write_value`), com `-1`/`max` → swap ilimitado e `None` → default do
motor. Escrito nos dois caminhos per-container (leaf delegado + cg root); o
agregado da slice fica no default. Campo `memory_swap` (raw) no
`Container`+`RunOpts`+`build_record`. Guards actualizados (`flags_of_controller`
memory += `--memory-swap`; removido das listas "não existem"). **Provado pelo
host**: `-m 256m --memory-swap 768m` → leaf `memory.swap.max=536870912` (512 MiB
= 768−256); `--memory-swap -1` → `max`.

### C19 — `--uts host` (runtime `CLONE_NEWUTS` condicional)
O item que estava adiado por tocar no runtime. Agora o `--uts` dobra no padrão
`host_ipc`/`host_pid`: `host` → o container partilha o UTS do host (vê o seu
hostname), `private` (omissão) → o seu próprio. No runtime: `clone_flags` só
adiciona `CLONE_NEWUTS` quando `!uts_host`; e o `sethostname` em `setup_rootfs`
passa a ser condicionado por `has_own_utsns` (= `!uts_host`), para **nunca
renomear o host** (que em rootless falharia EPERM na mesma). `--hostname` com
`--uts host` é RECUSADO. Plumbing completa espelhando `host_ipc` (Container,
RunOpts, RunArgs, build_record, launch_spec, cmd_run RunSpec, clone_flags,
InitInputs/NamespaceSpec, container_init). **Provado ao vivo**: `--uts host` →
hostname `delonix` (o do host); default → `dlxpar-uts0` (próprio); `--hostname
--uts host` recusado; host NÃO renomeado; pods continuam a receber hostname.

### C20 — shim accionável para famílias irredutíveis (`serve docker-api`)
Seis verbos de cluster/daemon do Docker/Podman — `swarm`, `node`, `service`,
`machine`, `context`, `plugin` — passam a ser aceites pelo parser como subcomandos
ESCONDIDOS (`#[command(hide = true)]` + `trailing_var_arg`), devolvendo um erro
accionável: estas famílias são de orquestração/daemon que um motor *daemonless*
não corre, e quem precisa mesmo da superfície Docker usa `delonix serve docker-api`
(com `DOCKER_HOST=unix://…` o próprio `docker` CLI fala com o Delonix). Não
«fingem» funcionar: dizem porquê e para onde ir. Preserva daemonless/rootless/
ADR-0010. A mensagem foi CORRIGIDA para não prometer que o shimm serve swarm/node
(serve só a superfície container/image). Ficheiros: `main.rs` (`docker_api_shim`,
`NOT_A_GROUP`, teste por-verbo), i18n. Commit `94696da3`.

### C21 — `--platform <os/arch>` em run/create
Busca a variante OS/arquitectura do manifesto no pull (`--platform linux/amd64|
linux/arm64`), à Docker/Podman. Liga `RunArgs.platform` pelo `run_opts_from_args`
ao `apply_pull_policy`, que extrai a arquitectura e chama
`pull_from_registry_with_creds_platform`. **Provado ao vivo**: `linux/amd64` puxa e
corre; `linux/arm64` é seleccionada (registo «platform selected: linux/arm64») num
host amd64. Teste de parsing + flag prometida + `cli-stability`. Commit `62f94f25`.

### C22 — `--umask <MASK>` (Podman)
Máscara de criação de ficheiros do processo do contentor, em octal (`0077`). Campo
`umask: Option<u32>` no `Container`+`RunOpts`, `build_record`, `ProcessSpec`;
aplicada com `libc::umask` em `container_init` como ÚLTIMO passo antes do `execve`
(nada acima — mounts, tmpfs — é afectado). Parsing octal recusa valores inválidos
em vez de os ignorar. **Provado ao vivo**: `--umask 0077` → `umask` dentro do
contentor = `0077`; sem flag → `0022`.

### C23 — `--cgroupns private|host` (Docker/Podman)
`private` dá ao contentor o seu próprio espaço de nomes de cgroup (a raiz de
`/proc/self/cgroup` passa a `/`); `host` (omissão) mantém a vista do host. No
runtime, `unshare(CLONE_NEWCGROUP)` em `container_init` DEPOIS de o pai mover o
processo para a folha `dlx-<id>` mas ANTES de `drop_capabilities` (o `unshare`
precisa de `CAP_SYS_ADMIN`; largado antes dava EPERM silencioso — apanhado ao
vivo). *Opt-in* e *best-effort*: `host` nunca chega ao `unshare`,
e uma recusa degrada para a mesma vista do host (sem regressão). Ignorado no
caminho Kind node (já corre o seu próprio `setup_node_cgroup_ns`). Campo
`cgroupns_private` plumbed como o `node_cgroup` (Container, RunOpts, build_record,
NamespaceSpec, InitInputs). **Provado ao vivo**: `--cgroupns private` →
`/proc/self/cgroup` = `0::/`; `host`/omissão → caminho completo do host.

### Flags de nicho DIFERIDAS (divergência declarada, ligadas a backend)
Encerrada a vaga de nicho, estas ficam como divergência deliberada porque
dependem de um backend que o caminho rootless/cgroup-v2 actual não serve — não são
«esquecidas», são recusadas com motivo:

| Flag Docker/Podman | Porquê diferida | Caminho futuro |
|---|---|---|
| `--mac-address` | MAC fixo só faz sentido em *bridge* rootful/macvlan; o default rootless é slirp/ingress | ligar quando a paridade de rede rootful (ramo `feat/network-parity`) entrar |
| `--ip6` / `--link-local-ip` | exigem backend IPv6 no SDN (fase própria noutro ramo) | ligar sobre a fase IPv6 (`feat/ipv6-phase1`) |
| `--device-cgroup-rule` | cgroup v2 controla dispositivos por eBPF, não pelo `devices.allow` v1; inviável em rootless hoje | precisa do programa eBPF `device` anexado ao cgroup |
| `--detach-keys` | UX da sequência de *detach* no `attach`; baixo valor, interactivo | implementar no laço de TTY do `attach` quando houver procura |

### Próximo lote — totalmente mapeado (investigação paralela por agentes)
As flags abaixo tocam no runtime (`delonix-linux`) e em *guards* que hoje afirmam
que não existem — ficam prontas a implementar com os pontos exactos:

- **`--pids-limit` → `pids.max`** (delegado, aplicável): parametrizar as escritas
  já existentes em `delonix-linux/src/lib.rs:6111` (delegado) e `:6862` (root),
  hoje com `DEFAULT_PIDS_MAX` fixo. Campo novo no `Container` (`record.rs:349`) +
  `RunOpts` + `build_record`.
- **`--memory-swap` → `memory.swap.max`** (delegado): escritas em `:6103`/`:6860`
  (hoje `"0"`). ARMADILHA semântica: Docker conta memória+swap, cgroup v2 é só
  swap → escrever `swap_total − memory`.
- **`--memory-reservation` → `memory.low`** (soft floor): NÃO é escrito hoje —
  acrescentar uma escrita ao lado de cada `memory.max`.
- **GUARDS a actualizar** (senão o build parte): `lib.rs:5761` (`flags_of_
  controller`), `main.rs:1898/1923` e `system.rs:2190` (listas `named_flags_all_
  exist` que afirmam que `--memory-swap`/`--pids-limit` NÃO existem), e as
  mensagens do node-api (`container_launch.rs:209`, `container_update.rs:131`).
- **`--stop-signal` / `--stop-timeout`**: campos no `Container` (`record.rs`, +
  init em `Container::new`) + `RunOpts` + `build_record`; `cmd_stop`
  (`container.rs`) lê-os; `-t` do `stop` passa a `Option<u64>` (distinguir
  "não passado"); encaminhar o sinal inicial por `runtime::stop`→`stop_waiting`
  (`lib.rs:10141`/`:10180`, hoje SIGTERM fixo), convertendo `parse_signal`
  (i32)→`nix::Signal`.
- **`--init`** (reaper pid1): localizado em `container_init` (`lib.rs:4040`/exec em
  `:4426`) — fork: pai fica pid1 a fazer `waitpid(-1)`+reenvio de sinais, filho
  faz o drop de privilégios + exec actual. Seccomp já permite `wait4`/`waitid`/
  `kill`. Bool plumbed como o `read_only`.
- **modos `--ipc`/`--pid`/`--uts`** → mapear `host` para os `--host-ipc`/
  `--host-pid` já existentes (no `run_opts_from_args`), recusando modos não
  suportados em vez de os ignorar.

### Medição do caminho para 100% (fundamentada, não estimada)
Só o verbo `run` tem **51 flags Docker / 91 Podman** ainda em falta; muitas são
features reais (`--init`, `--volumes-from`, `--mount`, `--pull`, `--platform`,
`--ipc`/`--uts`/`--pid` por valor, `--device-cgroup-rule`, `--blkio-*`…),
algumas são renomeações (feitas em C6) e algumas são inertes em rootless. Por ~40
verbos, **100% de flags é uma campanha de dezenas de PRs**. Fica como backlog
estruturado (próximas vagas):

- **Vaga 4** — flags-feature de alto valor em `run`/`create` (`--init`,
  `--volumes-from`, `--mount`, `--pull`, `--platform`, modos `--ipc/--uts/--pid`),
  cada uma com comportamento + i18n + manual + teste diferencial.
- **Vaga 5** — shim de erro accionável para `swarm`/`node`/`service`/`machine`/
  `plugin`/`context` a apontar `serve docker-api`, e verificação de que o `docker`
  CLI real conduz o Delonix via `DOCKER_HOST`.
- **Pod pause/infra container** — para `pod create --name` imperativo completo.
- **Gerador de gap flag-level por verbo** (a medição acima, automatizada) para
  versionar o progresso rumo a 100%.

### Estado dos testes (segunda vaga)
`cargo test -p delonix-bin` **1644/1644**; `frequent-verbs.sh` **49/49**;
`tab-descriptions.sh` verde; `flow1`/`flow2` diferenciais **ALL OK** (delonix pod
stop/start/restart verdes); `docs_cli_gate`, `lang_ratchet` verdes; clippy limpo.

---

## Terceira vaga — fecho das divergências resolvíveis (6 Nov 2026)

O dono pediu «resolver as divergências para a paridade». Partindo da matriz
autoritativa (as linhas `por-fazer` = comando que o parser RECUSA = ausência
real), implementámos **23 comandos** numa vaga de agentes em paralelo (uma
categoria por agente, worktree isolado, validada ao vivo) + os verbos-raiz feitos
em separado. A matriz passou de **20→43 `passou`** e **85→62 `por-fazer`**.

### C24 — verbos-raiz Docker: `pause`/`unpause`/`commit`/`top`/`rename`
Mesma mecânica dos alias de raiz existentes (`cp`/`port`/…): variante `clap` que
reutiliza o handler de `container <verb>` e as docs verbatim, logo herda a
promessa de estabilidade do `container` sem segundo contrato. `docker pause`,
`docker commit`, `docker top`, `docker rename` trocam só o executável. `update`
fica FORA da raiz de propósito: `docker update` é de LIMITES de recursos, o
`container update` do Delonix é reconfiguração a quente (portas/volumes/banda) —
semânticas distintas. **Provado ao vivo** (pause→Paused, unpause, top, rename,
commit→nova imagem). Registado em STABLE_GROUPS, `_ROOT_VERB_ALIASES`, README,
`cli-stability.md`, manual.

### C25 — `container exists` + `container list`
`container exists <id|name>` à Podman: silencioso, saída 0 se existe / 1 se não
(via o mesmo `find(store,id)` do `inspect`/`rm`; `std::process::exit(1)` para o
contrato 0/1 limpo — devolver `Err` dava código 4 com mensagem). `container list`
= segundo `visible_alias` do `ps` (zero handler, zero i18n). **Provado ao vivo.**

### C26 — `image inspect`/`list`/`exists`/`untag`
`image inspect` serializa os MESMOS registos `Image` que o `describe` lê, em array
JSON à Docker. `image list` = `visible_alias` do `ls`. `image exists` à Podman
(0/1 silencioso). `image untag` larga UM nome (via `resolve`+`save`, nunca os
blobs; o último nome deixa a imagem *dangling* `<none>`, conteúdo intacto).
**Provado ao vivo.** (Removidas 2 excepções obsoletas no `docs_cli_allow.tsv`.)

### C27 — `network prune`/`exists` + `volume exists`
`network prune` espelha o `volume prune` (classifica→avisa os retidos→confirma→
varre); uma rede em uso (container OU VM anexo) é RETIDA, nunca forçada, pelo
mesmo `network_dependents` que o `network rm` recusa (fail-closed: store ilegível
aborta em vez de varrer). A bridge default (`delonix0`) nunca está em `list()`,
logo nunca é podada (como no Docker). `network exists`/`volume exists` à Podman
(0/1 silencioso). **Provado ao vivo**, incluindo a prova de que uma rede com
container anexo é retida.

### C28 — pod `ps`/`exists`/`prune`/`kill`/`pause`/`unpause`/`stats`/`top`/`inspect`
Nove verbos, todos no padrão `members_of(...)` + handler-por-container já usado no
`pod stop/start`. `pod prune` remove só pods sem membro Running/Paused; `pod kill`
sinaliza em ordem inversa; `pause`/`unpause` congelam/descongelam (holder por
último/primeiro); `stats`/`top` delegam no container por membro; `inspect` é um
array JSON (pod + membros). Promoveu `cmd_freeze`/`cmd_top`/`cmd_stats` a
`pub(crate)`. **Todos provados ao vivo.**

### As 62 divergências restantes — classificação honesta
Não são «esquecidas»: a grande maioria exige um backend ou uma decisão de produto
que não é trabalho de CLI.

| Grupo | Nº | Natureza |
|---|---|---|
| `system *` + `context *` (daemon/remote-mgmt) | 22 | ADR-0010 recusa a API de gestão remota; `context` já cai no shim `serve docker-api`. Fora-de-âmbito, não ausência acidental |
| `mount`/`unmount` de rootfs (container/image/volume) | 10 | precisa de montar o overlay no host — em rootless, `fuse-overlayfs`; frente própria |
| subsistema de build (`buildx`/`bake`/`builder`/`image build`/`diff`/`tree`) | 6 | motor de build; campanha à parte |
| CRIU (`checkpoint`/`restore`) | 3 | *checkpoint/restore* de processos via CRIU; backend dedicado |
| vaga futura barata (root `history`/`untag`, `search`, `import`, `export`) | ~8 | alias de raiz + `docker search`/`import`/`export` — implementáveis sem novo backend, próxima vaga |
| misc Podman (`clone`/`scp`/`trust`/`runlabel`/`cleanup`/`reload`/`update`) | ~13 | caso a caso; vários dependem de subsistemas (overlay/assinatura/rede) |

### Estado dos testes (terceira vaga)
`cargo test -p delonix-bin` **1661/1661** (+5 +2 binários auxiliares); `lang_ratchet`,
`docs_cli_gate`, `docs_command_gate` verdes; clippy limpo. Smoke vivo combinado
(image/container/network/volume/pod) num só `DELONIX_ROOT` isolado: todos os 23
comandos respondem como o Docker/Podman. Matriz: **43 passou / 62 por-fazer /
76 reconhecido-por-testar / 14 fora-do-âmbito / 3 divergência-deliberada**.
