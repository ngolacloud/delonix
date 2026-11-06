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
