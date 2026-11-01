# ADR-0083 — Rootful é suportado; SELinux-enforcing (com relabel) fica fora de âmbito na linha 0.x

**Estado:** Proposto (2026-11-01) — avaliação do item (f) do portão G1 do plano
NO-GO→GO (`reports/go-nogo-alta-criticidade-20261101/`). Rootful **medido** neste
host; SELinux-enforcing **avaliado pelo código** e declarado fora de âmbito, com
o caminho de suporte descrito. Aguarda GO do dono.
**Relaciona:** `reports/go-nogo-alta-criticidade-20261101/g1f-rootful-selinux.md`
(a bateria), ADR-0079 (grupo honrado em modo root), ADR-0077 (sensor rootless).

## Contexto

O relatório GO/NO-GO (2026-11-01) marcou «rootful/SELinux nunca avaliados» como
um dos pontos que mantêm o veredicto NO-GO para alta criticidade. O portão G1
exige ambos **avaliados**, ou **declarados fora de âmbito por escrito**. Este ADR
é a decisão escrita, fundamentada na bateria do relatório acima.

A linha de produto é **rootless-first** por desenho (daemonless, userns, sem
privilégio de arranque). Rootful existe e tem caminhos próprios (ADR-0079: grupo
em modo root, leaf de cgroup para o VMM). SELinux nunca foi exercido porque
nenhum host de desenvolvimento o tem (todos AppArmor/Ubuntu).

## Decisão

### 1. Rootful — SUPORTADO (medido)

O núcleo rootful é suportado e foi medido neste host (uid 0 real, `/dev` por
`mknod` com nós reais, rede bridge, `run -d`/`exec`/`rm`, store próprio). Fica
documentado como um modo suportado a par do rootless.

Duas arestas ficam **registadas como dívida conhecida**, nenhuma bloqueadora do
uso normal rootful:

- **`build FROM scratch` rootful falha no packaging** (`DX-1410 image has no
  layers`). O COPY/RUN correm; o caminho de commit OCI rootful não empacota um
  base de zero layers. O rootless empacota o mesmo Dockerfile. É distinto dos
  três bugs de `FROM scratch` corrigidos nesta linha (esses eram de
  arranque/rootless — resolução de `scratch`, `/dev` do userns, scaffold no
  COPY); este é do commit rootful e fica por corrigir.
- **Limite de memória rootful** não foi confirmado de forma conclusiva
  (`memory.max` não visível de dentro do container na bateria) — carece de um
  teste dedicado host-side.

### 2. SELinux-enforcing (com relabel de volumes) — FORA DE ÂMBITO na linha 0.x

O que **já existe**: o label de **processo** do container é aplicado
(`apply_selinux` escreve o contexto em `/proc/thread-self/attr/exec` quando
`/sys/fs/selinux/enforce` existe).

O que **falta**, e é a razão da exclusão: o **relabel de volumes** (`:z`/`:Z`)
está deliberadamente não implementado e recusado fail-closed
(`delonix-volume`), e não há labeling de mount (`context=`/`fscontext=`) nem de
ficheiro. Num host enforcing, um bind mount fica com o label do host e o domínio
do container é **negado** — o modo de falha típico em RHEL/Fedora. Sem isto, um
workload com volumes não corre em enforcing, por muito que o label de processo
esteja certo.

Essa peça — relabel recursivo da árvore do volume para o domínio do container
(via `libselinux`/`setfilecon`), `context=` por mount, e a semântica partilhado
(`z`) vs privado (`Z`) — é **trabalho próprio**, não um ajuste, e precisa de um
host enforcing real para ser validada honestamente. Declará-la fora de âmbito
para a linha 0.x é a escolha honesta: o contrário seria fingir suporte que a
primeira bind mount em enforcing desmente.

## Consequências

- O portão G1(f) fica fechado: ambos **avaliados por escrito**, rootful medido,
  SELinux-enforcing fora de âmbito com justificação e caminho.
- `delonix` anuncia suporte **rootful** e **rootless**; **não** anuncia suporte
  SELinux-enforcing para workloads com volumes. A recusa fail-closed de `:z`/`:Z`
  mantém-se — é preferível a um relabel silenciosamente ausente.
- O caminho para levantar a exclusão (quando o dono o decidir): implementar o
  relabel de volumes + `context=` por mount, e validar numa VM Fedora/RHEL
  enforcing (a bateria rootful deste relatório, repetida em enforcing, mais um
  caso com volume `:Z`). Entra como um ADR sucessor.
- Dívida conhecida rootful (build FROM scratch, limite de memória) fica
  rastreada aqui e no relatório; não altera o veredicto de «rootful suportado»
  para o uso normal.

## Alternativas consideradas

- **Implementar o relabel de volumes agora.** Rejeitado para a linha 0.x: sem um
  host enforcing para validar, seria código não exercido na exacta condição que
  existe para servir — a mesma armadilha «PASS por desenho» que o G1(e) fechou.
- **Afirmar SELinux suportado porque o label de processo é aplicado.** Rejeitado:
  é meia-verdade; falha na primeira bind mount em enforcing.
