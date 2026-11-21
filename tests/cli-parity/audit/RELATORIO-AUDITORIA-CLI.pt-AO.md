# Auditoria da CLI Delonix — relatório de engenharia de testes

**Data:** 21 de Novembro de 2026 · **Objecto:** `delonix 0.1.2`, commit `78c4488bf88d7ca89396d3cb46c944c59a9f5cbb` (`develop`)
**Suite:** [`tests/cli-parity/audit/`](.) · **Matriz:** `matriz.csv` + `inventario.csv` (geradas)

> **Âmbito da aprovação.** Tudo o que este relatório afirma vale para **esta
> versão, neste ambiente**, e só para o que foi efectivamente executado. Cada
> número traz o seu denominador. O que não foi medido está nomeado como tal.

---

## 1. Objecto do ensaio

### 1.1 O que foi medido

| | |
|---|---|
| Repositório | `ngolacloud/delonix`, ramo `develop` |
| Commit | `78c4488bf88d7ca89396d3cb46c944c59a9f5cbb` |
| Último tag contido | `v0.1.2` (o binário diz `+87 commits since v0.1.2`) |
| Binário | `target-audit/release/delonix`, sha256 `81c768a843f63a0e949b628b7a0ffa03b1a6c372814279f7ebf5984f044ed70e` |
| Perfil | release (`opt-level=3`, `lto=thin`, `codegen-units=1`, `strip=debuginfo`) |
| Toolchain | `rustc 1.96.0` |
| Features | default (`make build` não passa nenhuma) |
| Piso de glibc | `GLIBC_2.39` (build local; a release oficial fixa 2.34 noutros runners) |

Os cinco binãrios irmãos, construídos no mesmo `CARGO_TARGET_DIR`:

| binário | sha256 (16) |
|---|---|
| `delonix` | `81c768a843f63a0e` |
| `delonix-cri` | `8d43ca2fe9b05323` |
| `delonix-mgmt` | `22a14bb353daa002` |
| `delonix-mcp` | `85fc58e760d62e24` |
| `delonix-node-api` | `e857a675e5be0588` |

Instalados em `/usr/local/bin` (`make install-system`) e conferidos **byte a byte**
iguais aos da árvore.

### 1.2 Um problema de integridade da medição, e como foi resolvido

A primeira build correu no checkout partilhado `/home/walter/workspaces/ngolacloud/delonix`.
Arrancou às 11:27 e ligou o `delonix` às 11:35:36. Entre as **11:28 e as 11:30**,
**31 ficheiros em `crates/` e `bins/` mudaram** — outra sessão a trabalhar no mesmo
clone. O `cargo` lê cada ficheiro quando compila a sua crate, e
`bins/delonix/src/main.rs` mudou às 11:29:59, antes da ligação.

A auditoria passou a medir um **worktree fixado no commit, com zero alterações
locais** e `CARGO_TARGET_DIR` próprio. A comparação dos dois builds é a prova do
alcance da contaminação:

| binário | build suja | build limpa | veredicto |
|---|---|---|---|
| `delonix` | `81c768a843f63a0e` | `81c768a843f63a0e` | **igual** |
| `delonix-mgmt` | `22a14bb353daa002` | `22a14bb353daa002` | igual |
| `delonix-mcp` | `85fc58e760d62e24` | `85fc58e760d62e24` | igual |
| `delonix-cri` | `7b4d7dc07b576775` | `8d43ca2fe9b05323` | **difere** |
| `delonix-node-api` | `bb33d2e80a8abec1` | `e857a675e5be0588` | **difere** |

Os dois que diferem são exactamente aqueles cujos ficheiros mudaram às 11:29:59
(`bins/delonix-cri-bin/src/warm.rs`, `bins/delonix-node-api-bin/src/main.rs`). O
`delonix` saiu idêntico, o que mostra que as edições não chegaram à CLI — mas isso
só se soube **depois** de comparar. Durante a auditoria a `develop` avançou um
commit (`3246cde5`, `fix(cdi): o glob do --gpus dri só entrega nós de dispositivo`);
este relatório mede `78c4488bf`.

### 1.3 Ambiente

| | |
|---|---|
| Distribuição | Zorin OS 18.1 (base Ubuntu) |
| Kernel / arquitectura | `Linux 7.0.0-43-generic` / `x86_64` |
| Modo | **rootless**, uid 1000 |
| cgroup | `cgroup2fs`. Scope da sessão: `memory pids`. **Base que o motor usa** (`user@1000.service/dlx-containers`): `cpu memory pids` |
| Limites | `--memory` e `--cpus` **aplicam-se** (medido: `memory.max` = 134217728 no cgroup real). `--cpuset`, `--io-weight`, `--device-*` são recusados com **exit 69** e mensagem que nomeia a causa |
| subuid/subgid | `walter:100000:65536` |
| userns não privilegiado | permitido (`apparmor_restrict_unprivileged_userns=0`) |
| `ip_unprivileged_port_start` | **80** (baixado neste host) |
| `br_netfilter` | carregado, `bridge-nf-call-iptables=1` |
| `/dev/kvm` | presente, legível/escrevível por `walter` (ACL) |
| Estado do motor no host | nenhum `pin`/`control`/`slirp4netns` vivo; root real **vazio** |

**Comparadores** — e a assimetria registada por caso:

| motor | versão | modo | endpoint |
|---|---|---|---|
| Docker | cliente e servidor `29.9.3`, Compose `v5.7.0` | **rootful** (`dockerd` como root) | `unix:///var/run/docker.sock` |
| Podman | `4.9.3`, netavark | **rootless** | CLI directa (socket inexistente) |
| Delonix | `0.1.2` | **rootless** | daemonless |

> Docker é rootful e o Delonix foi medido rootless. Uma diferença pode vir do
> **modo**, não do motor; os casos afectados trazem a marca.

**Fixture partilhada:** `busybox@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e`,
fixada por digest para que os três motores vejam o mesmo conteúdo.

### 1.4 Armadilhas de ambiente registadas

1. **`~/.local/bin/delonix-cri` de 2026-09-07 está ANTES de `/usr/local/bin` no
   `PATH`.** Não existe `~/.local/bin/delonix`, por isso o `exec_server` pega o irmão
   ao lado do executável; mas qualquer resolução de `delonix-cri` por `PATH` serve o
   de Setembro. A suite mede sempre pelo caminho do `target`, onde os cinco estão juntos.
2. **Isolar só o `DELONIX_ROOT` não isola.** Dois roots disputam
   `/tmp/delonix-net-1000/*.sock` — incidente já medido neste repo. A suite isola os
   **dois**, e o directório de runtime vive num caminho **curto** porque o `sun_path`
   de um socket unix tem 108 bytes.
3. **Um `image pull` meu correu sem `DELONIX_ROOT`** e introduziu uma imagem de
   2,1 MiB no root real. Foi removida (`image rm`, rc=0); o root real voltou a zero
   imagens, zero containers, zero volumes, zero redes.

---

## 2. Superfície descoberta

Percurso recursivo da ajuda, da raiz até às folhas, com o resultado cruzado contra
as definições do parser Rust.

| | |
|---|---|
| Caminhos | **420** (419 + a raiz) |
| **Folhas invocáveis** | **371** |
| Grupos | 49 |
| Opções longas | **1 099** |
| Argumentos posicionais | **302** |
| Grupos de 1.º nível | 80 |

**Cruzamento independente:** o `scripts/cli-tree.sh` do próprio repo dá **371
folhas**, e os dois conjuntos são **idênticos** (`comm -3` vazio). Dois caminhadores
a convergir é a verificação que o inventário precisava.

Folhas por grupo (os dez maiores): `vm` 40 · `net` 36 · `container` 35 · `image` 28 ·
`cluster` 22 · `network` 21 · `system` 20 · `pod` 19 · `volume` 16 · `stack` 11.

### 2.1 Entradas ocultas

Onze subcomandos da raiz são `#[command(hide = true)]`. Dez são verbos
Docker/Podman fora de âmbito e respondem **rc=1** com `DX-1000` e uma explicação do
porquê — não com «unrecognized subcommand», que é a diferença entre informar e
despachar: `swarm`, `node`, `service`, `machine`, `context`, `plugin`, `buildx`,
`builder`, `bake`, `checkpoint`. O décimo primeiro, `ingress-proxy`, é interno
(rc=2, argumentos obrigatórios em falta).

### 2.2 Dois factos medidos sobre este parser, que desenharam a matriz

1. **O conflito é avaliado ANTES do posicional obrigatório.**
   `volume create --parent p --driver nfs` (sem o `<NAME>`) dá rc=2 com
   `cannot be used with`; `volume create` só com o nome dá rc=2 com
   `required arguments were not provided`. Logo uma folha com posicional
   obrigatório pode ser sondada com um par de opções **sem operandos**: o parser
   decide, o handler nunca corre, nada na máquina é tocado. É o que permite cobrir
   pares em largura numa CLI cujas folhas mutam o host.
2. **`--help` curto-circuita a validação de conflitos.**
   `volume create --parent p --driver nfs --help` dá **rc=0** e imprime a ajuda. Uma
   camada de pares construída sobre `--help` prova **tokenização**, nunca aceitação
   ou recusa. A primeira versão desta matriz fazia isso; foi substituída.
   (A validação de **valor** enumerado, em contrapartida, dispara mesmo com `--help`:
   `container run --restart cascalho --help` dá rc=2.)

---

## 3. Resumo da execução

**1 753 casos, todos com resultado.** Os totais reconciliam com os dados
(`soma_dos_estados == casos_na_matriz` = 1 753, verificado no `resumo.json`).

| estado | n |
|---|---|
| **PASS** | **1 720** |
| **FAIL** | **6** |
| BLOQUEADO | 0 |
| NÃO EXECUTADO | 27 |

Por camada:

| camada | PASS | FAIL | NÃO EXEC. | o que prova |
|---|---|---|---|---|
| `help` | 420 | — | — | toda a folha responde a `--help` com rc=0 e stderr vazio |
| `parser` | 932 | **6** | — | flag inexistente, opção sem valor, valor fora da enumeração, grupo sem subcomando |
| `pares` | 239 | — | — | pares de opções avaliados pelo parser, sem executar o handler |
| `negativo` | 63 | — | 27 | as 22 regras `conflicts_with`/`requires` declaradas no parser |
| `1-local` … `6-limpeza` | 66 | — | — | efeito observável: ciclos de vida, convergência, limpeza |

### 3.1 Cobertura, com denominadores

| medida | valor | denominador |
|---|---|---|
| Folhas com pelo menos um caso | **371** | 371 (**100 %**) |
| Folhas exercitadas **funcionalmente** (efeito julgado) | **32** | 371 (8,6 %) |
| Folhas só com `--help`/parser | 339 | 371 |
| Opções `(folha, flag)` com caso próprio | **974** | 1 099 (**88 %**) |
| **Pares** gerados | **239** | **1 114 possíveis** |
| Folhas elegíveis para pares | 227 | 371 |
| Folhas excluídas por não terem posicional obrigatório | 142 | — |
| Folhas excluídas por **executarem** sem operandos | 2 (`build`, `image build`) | — |
| Regras negativas declaradas | 22 | 22 resolvidas, 0 não resolvidas |

> **A distinção que importa.** «371 de 371 folhas cobertas» é cobertura de
> **contrato do parser**. **32** folhas tiveram o efeito verificado. Um caminho
> reconhecido pelo parser não é um contrato cumprido — é a mesma distinção que o
> relatório de paridade anterior deste repo já fazia, e mantém-se.

### 3.2 Os 27 não executados

Todos da camada negativa, todos pela mesma razão, e dita em cada caso: a folha
**não tem posicional obrigatório**, logo sem operandos o handler corria e o caso
deixaria de ser uma sonda de parser. Folhas afectadas: `hosts sync`, `man`,
`volume ls`, `volume prune`, `system prune`, `vm ls`, `container ps` e as suas
combinações de regra. Para os fechar é preciso dar a cada um contexto válido
próprio — trabalho identificado, não feito.

---

## 4. Falhas reproduzíveis

### 4.1 A flag desconhecida torna-se operando, e `run` vai ao registo remoto

**Severidade: alta.** Justificação pelo impacto: um erro de escrita numa flag faz
o motor enviar essa cadeia a um registo público e devolver um erro que aponta para
o lado errado do problema.

| | |
|---|---|
| **IDs** | `PARSE-0103`, `PARSE-0106`, `PARSE-0123`, `PARSE-0133`, `PARSE-0140`, `PARSE-0297` |
| **Comando mínimo** | `delonix run --flag-que-nao-existe` |
| **Esperado** | a flag é recusada **como flag**, nomeada, sem resolver nem puxar nada |
| **Observado** | `pulling --flag-que-nao-existe…` → pull de `library/--flag-que-nao-existe:latest` em `registry-1.docker.io`, **rc=4** |
| **Em `exec`** | `error[DX-4101] no such container: --flag-que-nao-existe`, rc=4 |
| **Folhas** | `container run`, `container create`, `container exec` e os três aliases da raiz (`run`, `create`, `exec`) |
| **Evidência** | `results/evidence/PARSE-0123.txt` |

**Comparadores:**

| motor | saída | rc |
|---|---|---|
| Docker 29.9.3 | `unknown flag: --flag-que-nao-existe` | 125 |
| Podman 4.9.3 | `Error: unknown flag: --flag-que-nao-existe` | 125 |
| **Delonix 0.1.2** | **vai ao registo com a flag como nome de imagem** | **4** |

**Causa, nomeada.** O posicional de `RunArgs`/`ExecArgs` tem
`trailing_var_arg = true, allow_hyphen_values = true`
(`bins/delonix/src/cmd/container.rs:1561-1567` e `:1714-1717`). O comentário que o
justifica está escrito e o desenho é **correcto depois da imagem**: o Docker também
pára de interpretar flags no primeiro posicional, e é isso que permite
`run <img> echo --config`. O que divergiu é o **primeiro** token: no Docker um
token inicial com `-` é lido como flag (desconhecida → erro); aqui o
`allow_hyphen_values` torna-o elegível como posicional.

**Efeito no operador:** o diagnóstico aponta para a imagem ou para o container,
nunca para a flag; e a cadeia escrita por engano sai da máquina para um registo
público.

> **O caso positivo deste desenho passa e está medido** (`FUNC-101`, `DIF-03`):
> `run --rm --net none <img> echo --config --rm -p 1:1` entrega os quatro tokens
> intactos nos **três** motores. O desenho não está em causa; o primeiro token está.

---

## 5. Combinações divergentes

Catorze intenções, cada uma com as três invocações completas sobre fixtures
equivalentes. Duas avaliações por caso: Delonix–Docker e Delonix–Podman.

| ID | intenção | delonix | docker | podman | sintaxe | resultado | classificação |
|---|---|---|---|---|---|---|---|
| DIF-01 | flag inexistente em `run` | 4 (vai ao registo) | 125 | 125 | = | **≠** | **defeito** (§4.1) |
| DIF-02 | flag inexistente em `exec` | 4 (`no such container`) | 125 | 125 | = | **≠** | **defeito** (§4.1) |
| DIF-03 | argv do processo colide com flag do motor | 0 · `--config --rm -p 1:1` | 0 · idem | 0 · idem | = | = | paridade |
| DIF-04 | código de saída do processo atravessa | **37** | 37 | 37 | = | = | paridade |
| DIF-05 | parar container inexistente | **4** | 1 | 125 | = | ≠ códigos | intencional (documentado) |
| DIF-06 | criar volume com nome tomado | **0** (idempotente) | 0 | **125** | = | = docker, ≠ podman | paridade com docker |
| DIF-07 | listar sem containers | 0 · cabeçalho | 0 | 0 | = | = (+3 colunas) | paridade |
| DIF-08 | `run -dit` | **0** | 0 | 0 | = | = | paridade |
| DIF-09 | ler a porta publicada | 0 · `80/tcp -> 127.0.0.1:18190` | 0 · `0.0.0.0:18191` | 0 · `0.0.0.0:18192` | = | ≠ **endereço** | intencional (default seguro) |
| DIF-10 | `image inspect` por digest → JSON | 0 · JSON, 1 entrada | 0 · idem | 0 · idem | = | = | paridade |
| DIF-11 | `--rm` remove de facto | 0 · ausente | 0 | 0 | = | = | paridade |
| DIF-12 | `logs` de um detached | 0 · `marca-<rid>` | 0 · idem | 0 · idem | = | = | paridade |
| **DIF-13** | **`exec -dit`** | **2 (recusado)** | **0** | **0** | **≠** | **≠** | **incompatibilidade** |
| DIF-14 | `image describe` (forma humana) | 0 | 1 | 125 | ≠ | — | sem equivalente |

### 5.1 DIF-13 — `exec -dit` é recusado onde Docker e Podman aceitam

**Severidade: média.** É a primeira linha de muitos scripts de automação.

| | |
|---|---|
| **Comando mínimo** | `delonix exec -dit <container> sh -c true` |
| **Esperado** | aceite, como nos dois comparadores |
| **Observado** | `error: the argument '--detach' cannot be used with:` — **rc=2** |
| **Causa** | `detach conflicts_with_all = ["interactive", "tty"]` em `ExecArgs` (`bins/delonix/src/cmd/container.rs:1678`) |
| **Comparadores** | `docker exec -dit` rc=0 · `podman exec -dit` rc=0 |

A semântica do Delonix é defensável (destacado e interactivo são contraditórios, e
o Docker simplesmente ignora `-i`/`-t` quando destacado). O problema é **não estar
documentada**: o `docs/cli-stability.md` lista `-i` e `-t` como estáveis no `exec` e
não menciona o conflito com `-d`. Classificação: **incompatibilidade com a paridade
prometida**, resolúvel por documentação ou por alinhamento.

> **Correcção a uma leitura minha.** A primeira versão deste caso atribuiu o
> conflito ao `run` e ia publicar que `run -dit` quebrava a paridade. A medição
> desmentiu: `run -dit` dá **rc=0 nos três**. O conflito está na `ExecArgs`, não na
> `RunArgs`. O caso foi reescrito para medir o verbo que a declaração afecta.

### 5.2 DIF-09 — a porta publicada liga em loopback

`delonix container port` devolve `80/tcp -> 127.0.0.1:18190`; Docker e Podman
devolvem `0.0.0.0:…`. O **formato é idêntico**; o endereço é o default seguro que o
motor documenta. Classificação: **diferença intencional documentada** — mas com
consequência prática que merece estar à frente do operador: **uma porta publicada
não é alcançável da LAN por omissão**, e um script migrado do Docker que publique e
teste de outra máquina falha sem que nada no comando o diga.

### 5.3 Grupos sem equivalente

Sonda definitiva (um verbo inexistente diz-no em ambos os comparadores; `--help`
**não serve** como sonda — `docker vm --help` dá **rc=0** e imprime o help da raiz):

**Só no Delonix:** `vm`, `workload`, `systemcontainer`, `provider`, `policy`,
`gitops`, `drift`, `node-ca`, `node-enroll`, `ippool`, `bucket`.
**Nos três:** `network`, `volume`, `image`, `system`, `compose`, `secret`.
**`stack` existe no Docker** — mas é o `stack` do Swarm, uma colisão de nome, não um
equivalente do `kind: Stack` declarativo.

### 5.4 Verificação independente das alegações de `delonix compatibility`

Não foi feita. O grupo `compatibility` tem três folhas e nenhuma foi confrontada
com observação independente nesta auditoria. Fica identificado como pendente (§8).

---

## 6. UI/UX e layout

64 amostras: 16 comandos × {80, 120} colunas × {en, pt}, cada um em **PTY real**
(para que o enquadramento seja o que o operador vê) e com **stdout redireccionado**.
**37 amostras com achado.**

### 6.1 A ajuda não se enquadra na largura do terminal

**Severidade: alta para a adopção.** É o primeiro contacto com a ferramenta.

Linhas acima da largura do terminal:

| comando | 80/en | 80/pt | 120/en | 120/pt |
|---|---|---|---|---|
| `delonix --help` / `-h` | 42 | **46** | 5 | 7 |
| `container --help` | 17 | 19 | — | 2 |
| **`container run --help`** | **65** | **71** | **49** | **53** |
| `api-resources` | 34 | 34 | 33 | 33 |
| `stack plan --fields` | 54 | 54 | 20 | 20 |

**A contagem de linhas não muda com a largura** — a prova de que não há
enquadramento nenhum:

```
COLUMNS=40  -> 359 linhas, 117 acima de 40
COLUMNS=80  -> 359 linhas,  65 acima de 80
COLUMNS=200 -> 359 linhas,  10 acima de 200
```

A maior linha de `container run --help` tem **619 caracteres**:

**Actual (80 colunas, truncado à largura do terminal):**
```
          Publish a port, `[hostIp:]hostPort:contPort[/tcp|udp]` or just `port`. Repeatable. SAFE BY DEFAULT: without `hostIp` the port binds to `127.…
```

**Proposta (depois):**
```
  -p, --publish <PUBLISH>
          Publish a port: `[hostIp:]hostPort:contPort[/tcp|udp]`, or just
          `port`. Repetível.

          SEGURO POR OMISSÃO: sem `hostIp` a porta liga a 127.0.0.1 — não
          é alcançável de outra máquina. Use `0.0.0.0:8080:80` para expor.
```

**Causa, numa linha.** `Cargo.toml:102` declara
`clap = { version = "4", features = ["derive"] }` — **sem a feature `wrap_help`**.
Sem ela o clap não reparte o texto longo pela largura do terminal.

**Critério de aceitação:** com `COLUMNS=80`, nenhuma linha de `delonix <qualquer>
--help` excede 80 caracteres; a contagem total de linhas **varia** com `COLUMNS`.
**Impacto em scripts:** nenhum — a ajuda não é formato de dados. O `pt` é
sistematicamente 4 a 6 linhas pior que o `en`, porque o catálogo é mais longo; o
enquadramento resolve ambos.

### 6.2 `-h` esconde uma flag global que funciona

| | |
|---|---|
| **Actual** | `delonix -h` lista só `-h, --help`. `delonix --help` lista também `-V, --version`. As duas flags funcionam (rc=0). |
| **Agravante** | no `-h` a dica diz `Print help (see a summary with '-h')` — auto-referencial: já se está no resumo. |
| **Proposta** | `-h` lista `-V, --version`; a dica do `-h` passa a `(veja mais com '--help')` e a do `--help` mantém-se `(veja um resumo com '-h')`. |
| **Critério** | `diff <(delonix -h) <(delonix --help)` não omite nenhuma opção global. |
| **Impacto** | nenhum em scripts. |

### 6.3 A sugestão de correcção oferece um comando oculto

O Delonix **sugere**, e melhor que o Docker:

```
$ delonix contaner ps
error: unrecognized subcommand 'contaner'
  tip: some similar subcommands exist: 'context', 'container'
```

(Docker: `unknown command`, sem sugestão. Podman: `Did you mean this?`.)

Mas `context` é um dos onze **ocultos** e recusa sempre com `DX-1000`. A sugestão
manda o operador para um comando que nunca pode funcionar, e oferece-o **primeiro**.
**Proposta:** excluir os `hide = true` do conjunto de sugestões.
**Critério:** nenhuma dica nomeia um subcomando oculto.

### 6.4 ANSI com stdout redireccionado

Quatro amostras emitem ANSI com stdout num pipe e sem `NO_COLOR` — todas a do
progresso de `pulling …`. Num log de CI isso são sequências de escape no meio do
texto. **Critério:** com stdout não-TTY, zero sequências `\x1b[`.

### 6.5 Duas contradições na mesma saída de `delonix system setup`

```
  limits:   APPLY — --memory/--cpus take effect
  ...
Container limits work, but `cpu` is not delegated here.
```

As duas frases contradizem-se, e a base que o motor usa (`user@1000.service`) tem
`cpu memory pids` — medido. A causa é consultar **dois cgroups diferentes**: os
limites contra a base de escape, o `cpu` contra o scope actual (que aqui é o scope
da aplicação e tem só `memory pids`). O operador que lê não consegue decidir.
**Proposta:** dizer qual cgroup responde a cada pergunta, ou reduzir a uma só
afirmação. **Critério:** nenhuma saída de `system setup` afirma e nega a delegação
do mesmo controlador.

### 6.6 Um valor inválido de `--l18n` é aceite e ignorado, sem aviso

| | |
|---|---|
| **Actual** | `delonix --l18n klingon container --help` → **rc=0**, e a saída é **byte a byte igual** à do `--l18n en` (`diff -q` sem diferenças). Nada diz que o valor foi ignorado. O mesmo com um valor vazio. |
| **Medido ao lado** | a precedência **está** definida e funciona: `--l18n` ganha ao `DELONIX_L18N` nos dois sentidos (comparação do **hash da saída inteira**, não da primeira linha). `--l18n` é insensível a maiúsculas: `PT` ≡ `pt` ≡ `pt_AO`. |
| **Porque é um achado** | é a classe «aceite e ignorado» que este motor recusa noutros sítios (`--security-opt seccomp=`, `-v …:z`, `--network-alias` foram todos corrigidos por isso). Quem escreve `--l18n pt-PT` fica com a CLI em inglês e sem explicação. |
| **Proposta** | `--l18n` passa a `clap::ValueEnum` (ganha `[possible values: en, pt]` na ajuda e recusa com rc=2), ou, se a degradação for desejada, emite um aviso que nomeia o valor ignorado. |
| **Critério** | `--l18n <valor-invalido>` ou recusa com rc=2 nomeando o valor, ou escreve um aviso em stderr. |
| **Impacto em scripts** | nenhum nos valores válidos. |

> **Armadilha de medição a registar.** A primeira versão deste caso comparava a
> **primeira linha** da ajuda e dava a precedência como provada. Essa linha
> (`Containers: run/ps/stop/rm/…`) é a excepção que o próprio repo documenta como
> «igual nas duas línguas» (`is_same_in_both`), por isso a comparação era trivial e
> passava sempre. O que prova é o marcador traduzido — `rm  Remove one or more
> containers` contra `rm  Remove um ou mais containers` — e é isso que os casos
> `FUNC-011`→`FUNC-015` assertam agora.

### 6.7 O que passou

- **Sem bloqueios em CI:** nenhuma das 64 amostras pediu entrada nem animou com
  stdin fechado. Zero spinners braille fora de terminal.
- **Separação stdout/stderr:** correcta em todas as amostras com rc≠0.
- **`ps` e `container ls` respondem** — quem vem do Docker acerta à primeira.
- **`net` vs `network`:** ambos recusam sem subcomando (rc=2) com uma linha que os
  distingue («Low-level network/infra plumbing» vs «User networks»).
- **Um grupo inexistente é recusado:** `delonix cascalho --help` dá **rc=2**, onde
  Docker e Podman dão **rc=0** e imprimem o help da raiz.
- **Tabelas alinhadas:** zero linhas desalinhadas do cabeçalho em todas as amostras.

---

## 7. O que a auditoria confirmou a funcionar

Os 66 casos funcionais passaram todos, num root limpo. Os que provam mais:

| caso | o que foi verificado |
|---|---|
| `FUNC-101` | o argv do processo recebe `--config --rm -p 1:1` intacto — idêntico aos três motores |
| `FUNC-102` | `rc=37`: o código do **processo** atravessa o motor (daemonless) |
| `FUNC-105` | `exec` corre **dentro** do container (o hostname lido não é o do host) |
| `FUNC-107`/`108` | `cp` provado pelo **conteúdo** relido dentro do container, não pelo rc do `cp` |
| `FUNC-110` | `wait` devolve **7**, o código real capturado pelo supervisor |
| `FUNC-111` | um `stop` **pedido** termina `Exited (0)`, nunca `Dead` |
| `FUNC-118`→`123` | ciclo `plan`(rc=2) → `apply` → recurso a correr → `plan`(rc=0) → `apply` → **um só** recurso |
| `FUNC-124` | o `drift` **detecta** um `--volume-add` feito à mão (rc=2) |
| `FUNC-128`/`129` | `destroy` provado pela **ausência** do recurso |
| `FUNC-017`/`018` | `manifest validate` **nomeia** o campo desconhecido (rc=0, lenient); `stack apply` **recusa-o** — a divisão do ADR-0069 a funcionar |
| `FUNC-016` | `manifest validate` recusa YAML com chave duplicada |
| `FUNC-034` | `image pull` com digest adulterado é recusado |
| `FUNC-029` | `secret inspect` **não revela** o valor sem `--reveal` |
| `FUNC-030` | `secret rm` de inexistente nomeia `secret`, não `container` |
| `FUNC-027` | `network create ../evil` é recusado — o nome não escapa o store |
| `FUNC-026` vs `FUNC-022` | `network create` repetido dá **5**; `volume create` repetido dá **0** (idempotente, como o docker) — dois irmãos, duas respostas ao mesmo gesto |
| `FUNC-125`/`127` | `get containers` recusa e **encaminha** para `container ps`; um Kind inexistente é recusado pelo nome |
| `FUNC-011`→`015` | I18N: a flag `--l18n` ganha ao `DELONIX_L18N` **nos dois sentidos**, provado por marcador traduzido; `PT` ≡ `pt` |
| `FUNC-002` | `version` e `--version` dizem o **mesmo texto** — a promessa escrita, agora verificada por `diff` |
| `FUNC-007` | `explain DX-1000` responde: o `explain` é polimórfico (campo de Kind **e** código DX) |
| `NEG-0076`→`0087` | as regras `conflicts_with` do `exec` (`-d` × `-i`/`-t`) são honradas |

**Qualidade das recusas.** Várias mensagens nomeiam a causa **e** o remédio. A
melhor encontrada, que bloqueou um caso meu e explicou-se sozinha:

```
error[DX-1000] invalid argument: 'x' was created without `-p` and without
`--net <network>`, so its slirp has no open api-socket — there's nowhere to
publish a hot port. Publish at least one port in `run`, or use `--net <network>`.
```

---

## 8. Divergências documento ↔ comportamento

| documento | afirma | medido | ID |
|---|---|---|---|
| `docs/cli-stability.md` | «`image list` … falha com `unrecognized subcommand`, nunca em silêncio» (corte limpo em v2.0.0, sem alias) | **rc=0**, imprime a tabela. `bins/delonix/src/cmd/image.rs:295` tem `#[command(visible_alias = "list")]` | `FUNC-033` |
| `docs/cli-stability.md` | lista `-i`/`-t` como estáveis no `exec` | não documenta que `-d` conflita com ambos | `DIF-13` |
| `docs/cli-stability.md` | «`image describe` só imagens» (sugere ser o inspector de imagem) | `image inspect` **também existe** e dá JSON | `DIF-10` |

---

## 9. Pendências

### 9.1 Dependências e ambiente necessário

| pendência | o que falta | IDs / âmbito |
|---|---|---|
| 27 casos negativos | contexto válido próprio para folhas sem posicional obrigatório (`hosts sync`, `man`, `volume ls`, `volume prune`, `system prune`, `vm ls`, `container ps`) | os 27 NÃO EXECUTADOS |
| 875 pares | 239 de 1 114 gerados (teto de 8 por folha) | subir o teto é só tempo de execução |
| 339 folhas | exercitadas só a nível de parser; falta efeito observável | `vm` (40), `net` (36), `cluster` (22) são os maiores blocos |
| `vm` / `cluster` | hipervisor e nós reais; `/dev/kvm` existe mas nenhuma VM foi criada | 40 + 22 folhas |
| `serve`, `mcp` | cliente local contra o socket; não exercitado | 5 + 3 folhas |
| `compose` | serviço verificável; não exercitado | 5 folhas |
| `backup` / `restore` | ciclo com comparação de dados; não exercitado | 6 folhas |
| `compatibility` | confronto independente das alegações (§5.4) | 3 folhas |
| `--cpuset`, `--io-weight`, `--device-*` | controladores não delegados; só a **recusa** foi medida | — |
| Docker rootless | o host só tem Docker rootful; a assimetria de modo fica | DIF-07, DIF-09 |

### 9.2 Falhas intermitentes

**Nenhuma.** As 6 falhas são determinísticas: reproduzem em todas as corridas, com
a mesma saída. Cada uma das corridas da matriz (1 800 → 1 769 → 1 751 → 1 753 casos, à
medida que os oráculos foram corrigidos) deu as mesmas 6, nas mesmas folhas.

### 9.3 Instrução de reexecução

```bash
tests/cli-parity/audit/run.sh matrix target-audit/release/delonix
```

```bash
tests/cli-parity/audit/run.sh case target-audit/release/delonix PARSE-0123
```

```bash
tests/cli-parity/audit/run.sh differential target-audit/release/delonix
```

```bash
tests/cli-parity/audit/run.sh ux target-audit/release/delonix
```

---

## 10. Notas de método — os erros desta auditoria

Registados porque custaram medições e porque a próxima pessoa paga o mesmo preço.

1. **`$?` depois de um pipe é o rc do `head`.** Aconteceu **três vezes**; a terceira
   deu «os onze comandos ocultos respondem rc=0», que é falso (respondem 1). Resolvido
   com um ajudante (`probe.sh`) que corre o argv como **lista** e imprime o rc certo.
2. **`--help` não serve como sonda de existência nos comparadores.** `docker vm
   --help` dá **rc=0** e imprime o help da raiz. A primeira tabela de «grupos sem
   equivalente» saiu toda a «sim».
3. **Valores sintéticos sem tipo não chegam à regra que testam.** `--max-for-day
   aud-synth` dá «invalid digit» e o conflito nunca é avaliado. Três casos falharam
   pela razão errada até os valores passarem a ter tipo (`synth.py`).
4. **Resolver uma regra por «a folha publica as duas flags» sobre-corresponde.**
   Deu `network create --driver/--parent` como conflito quando a regra vive em
   `volume.rs`. Resolvido restringindo pelo módulo de origem.
5. **Um ID estável tem de ser único.** Dois geradores partilhavam o intervalo
   `PAIR-####`: 1 833 casos corridos, **1 751** após deduplicação silenciosa, e os
   totais deixaram de reconciliar. Há agora um portão que recusa IDs duplicados.
6. **Um `setup` tolerado que falha em silêncio esconde a causa.** O caso do `drift`
   «falhou» e o porquê só apareceu quando o executor passou a registar **sempre** o
   resultado do setup. Um setup tolerado que falha passa a **BLOQUEADO**, nunca a FAIL
   nem a PASS.
7. **Uma sonda de «não mutou» sensível ao tempo acusa o inocente.** `container ps -a`
   tem coluna de idade relativa; mudou sozinha entre o antes e o depois. Passou a
   `ps -aq`.
8. **Um caso só prova o que consegue alcançar.** `PAIR-0028` arrancou uma **build
   real** do próprio repo (contexto por omissão = CWD) até ao timeout. `build` e
   `image build` saíram das camadas cegas e têm agora exclusão escrita.
9. **O meu amostrador de PTY dizia «BLOQUEOU» em 64 de 64** por não fazer `waitpid`
   depois do EOF. Um achado universal é quase sempre defeito do instrumento.
10. **Ler só a primeira linha esconde a resposta.** Concluí que o Delonix não sugere
    correcções; sugere, duas linhas abaixo — e melhor que o Docker.
11. **Uma hipótese sobre namespaces desmentida pela medição.** Ia publicar que
    `container ps` não consegue listar todos os namespaces (porque não tem
    `--all-namespaces`, que o `volume ls` tem). Medido: `container ps` **já lista
    todos** por omissão (2 de 2); `--namespace` é um filtro.

---

## 11. Conclusão

Dos **1 753** casos executados contra `delonix 0.1.2` (`78c4488bf`), **1 720
passaram**, **6 falharam** e **27 não foram executados** com a razão escrita em cada
um. Todas as **371** folhas descobertas têm destino rastreável na matriz; os totais
reconciliam com os dados.

As 6 falhas são **um único defeito** em 6 folhas: uma flag desconhecida em
`run`/`create`/`exec` é consumida como operando, e no `run` sai da máquina para um
registo público. É a divergência mais significativa face aos dois comparadores, que
respondem `unknown flag` com rc=125.

Fora disso, o que foi exercitado funcionalmente comporta-se como prometido: o
código de saída do processo atravessa, o argv chega intacto, o `cp` preserva o
conteúdo, o ciclo `plan`/`apply` converge e é idempotente, o `drift` detecta, o
`destroy` remove, e as 22 regras de conflito declaradas são honradas. A qualidade
das mensagens de recusa é acima da média — várias nomeiam a causa e o remédio.

O que mais limita a adopção não é um defeito de motor: é a **ajuda não se enquadrar
na largura do terminal** (`container run --help` com 361 linhas e uma de 619
caracteres, sem a feature `wrap_help` do clap), agravada em `pt`.

**Esta aprovação vale para `0.1.2` em `78c4488bf`, neste host, rootless, e só para
as 32 folhas com efeito verificado.** As restantes 339 estão medidas a nível de
contrato de parser, e o §9 diz o que falta a cada uma.

### 11.1 Nota posterior ao relatório

O defeito único do §4.1 — a flag desconhecida consumida como operando nas 6
folhas — foi corrigido no commit que precede esta suite no histórico
(`fix(cli): uma flag mal escrita deixa de ser consumida como imagem ou
container`), com 3 testes que falham sem a correcção. **O resto deste relatório
não foi reescrito**: é o registo medido de `0.1.2` em `78c4488bf`, e reescrevê-lo
para dizer «corrigido» apagaria a linha de base contra a qual a próxima corrida
se compara.

Medido, não deduzido: `run.sh matrix` sobre o binário com a correcção
(`md5 d7ccce8882a32caec807aa7528c7da4d`, os cinco irmãos construídos juntos)
deu **1 726 PASS / 0 FAIL / 27 NÃO EXECUTADOS**, total 1 753 reconciliado, `rc=0`
— contra os 1 720/6/27 do §3. Os 1 720 + 6 = 1 726 dizem também que **nenhum
outro caso mudou de estado**, que é o controlo que importa numa correcção no
parser.
