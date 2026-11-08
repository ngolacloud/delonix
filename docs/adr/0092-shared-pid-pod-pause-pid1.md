# ADR-0092 — Um pod `shareProcessNamespace` precisa de um processo pause como PID 1

**Estado:** Proposto (2026-11-08) — identificado e medido na campanha de
conformância CRI que levou o `critest v1.36.0` de **84 → 99/103** em rootful-VM+CNI
(`reports/cri-conformance-0.1.1/00-baseline-develop.md`). O caminho de partilha de
PID no root-CNI já foi ligado (ADR-sem-número nesta campanha: `apply_shared_pid_rootful`
em `lifecycle.rs`, que fechou HostNetwork×2/HostIpc×2/portforward-host), mas o spec
**PodPID** continua a falhar por uma razão arquitectural distinta descrita aqui.
**NÃO implementado** — adiciona um processo por-pod (decisão de recurso/ciclo de
vida) que exige GO do dono. **Relaciona:** ADR-0020 (envelope/semântica de pod),
`reports/cri-conformance-0.1.1/00-baseline-develop.md`.

## Contexto

O `critest` **PodPID** (`cri-tools` v1.36, `pkg/validate/security_context_linux.go`)
cria um pod sandbox com `namespace_options.pid = POD` (k8s `shareProcessNamespace`),
arranca um container nginx, cria um segundo container (que herda o modo POD do pod),
e dentro dele executa:

```
cat /proc/1/cmdline
```

A asserção é:

```go
Expect(o).ToNot(ContainSubstring("master process"))
```

Ou seja: **num pod de PID partilhado, o PID 1 do namespace NÃO pode ser o processo
de um container** (o nginx seria `nginx: master process …`). É o contrato que o
próprio campo k8s documenta: *«the first process in each container will not be
assigned PID 1»*. Em `containerd`/CRI-O esse PID 1 é o **processo pause** (o
«sandbox container») que cria e detém o namespace de PID partilhado; os containers
reais juntam-se a ele e recebem PIDs mais altos.

O Delonix **não tem** um processo pause por-pod. Por desenho rootless, o *ingress
holder* é por-UID e serve MUITOS netns, por isso o código escolhe como «anchor» o
**primeiro container vivo** do pod (`sandbox_shared_pid_anchor` em `lifecycle.rs`,
comentário em ~1880: *«There is no per-pod "pause" process here … so the anchor is
the first started container»*). Consequência: no pod de PID partilhado, o PID 1 é o
init do **primeiro container** (o nginx) → `cat /proc/1/cmdline` devolve
`nginx: master process …` → o spec falha.

A partilha em si **funciona** (o segundo membro junta-se ao namespace do anchor via
`enter_pod_pidns`, provado: 0 recusas de `shareProcessNamespace` na VM, eram 6). O
que falha é **quem é o PID 1**.

## Decisão

Para pods com `shareProcessNamespace` (`namespace_options.pid == POD`), introduzir
um **processo pause mínimo por-pod** que:

1. cria o namespace de PID partilhado (`CLONE_NEWPID`) e fica como **PID 1** nele,
   sem fazer mais nada (um `pause`: bloqueia num sinal, reaped os órfãos);
2. é o **anchor** que os membros do pod passam a juntar — `pod_infra_pid` deixa de
   ser «o primeiro container vivo» e passa a ser **o PID do pause**;
3. tem ciclo de vida ligado ao **sandbox** (nasce no `RunPodSandbox` do pod
   partilhado, morre no `RemovePodSandbox`), sobrevivendo a qualquer membro — o
   kernel mantém o namespace vivo enquanto o pause lá estiver, por isso a vinda e ida
   de containers não o colapsa (resolve também a fragilidade do anchor-é-container:
   se o primeiro container morre, o namespace sobrevivia só por acaso).

Os membros continuam a juntar-se com a plumbing EXISTENTE (`pod_shared_pid` +
`pod_infra_pid` → `enter_pod_pidns`, `setns(CLONE_NEWPID)`); muda apenas **a
identidade do anchor** (pause em vez de primeiro container) e **quem o cria**
(o `RunPodSandbox`, não o primeiro `StartContainer`). A correcção já feita para o
root-CNI (`apply_shared_pid_rootful`, root junta-se sem holder userns) mantém-se e
compõe: o pause é o alvo do `setns` em ambos os modos.

### Âmbito e modos

- **Rootful (nó Kubernetes):** o pause corre como root; `setns(CLONE_NEWPID)` é
  privilegiado sem truque de userns. É o caso que o `critest`/kubelet exercitam.
- **Rootless:** o pause corre no userns do pod (como o membro re-exec'd faz hoje),
  por isso o `setns` mantém o privilégio no namespace que o pause possui.

### O que NÃO muda

Pods **sem** `shareProcessNamespace` (o default, PID por-container) não ganham pause
— cada container tem o seu `CLONE_NEWPID`, como hoje. O custo do pause é pago só
pelos pods que pedem partilha de PID.

## Alternativas consideradas

1. **Anchor = primeiro container (actual).** Zero processos extra, mas o PID 1 é o
   container → viola o contrato CRI e falha o PodPID. Rejeitada para conformância.
2. **Não suportar `shareProcessNamespace` (recusar).** Honesto mas regride
   funcionalidade real do k8s (sidecars que sinalizam o peer). Rejeitada.
3. **Pause por-pod (esta decisão).** Alinha com `containerd`/CRI-O; custo de um
   processo trivial por pod partilhado; reutiliza toda a plumbing de `setns`.

## Consequências

- **+** Conforma o PodPID e dá a semântica correcta de PID 1 (nenhum processo de
  container é PID 1); torna o namespace partilhado robusto à ida/vinda de membros.
- **−** Um processo extra por pod com PID partilhado (memória/efémero mínimos; é um
  `pause`). Ciclo de vida novo a gerir no sandbox (criar/derrubar, reconciliação
  após crash — o pause tem de ser reaped e recriado com o sandbox).
- **Compat:** nenhum contrato público (CLI/JSON/gRPC/estado) muda de forma; é
  comportamento interno do caminho de pod partilhado. O `pod_infra_pid` persistido
  passa a apontar para o pause — nota de migração: pods partilhados criados pela
  versão anterior (anchor=container) não existem entre reinícios (o estado de pod é
  efémero), por isso não há estado incompatível a migrar.

## Validação exigida antes de Aceite

- **Comportamento:** `critest` PodPID passa (PID 1 = pause, não nginx), **e** o
  caminho positivo continua verde (o segundo container VÊ o nginx num PID > 1).
- **Unit:** o anchor resolve para o pause; `enter_pod_pidns` junta ao PID do pause;
  pods não-partilhados não ganham pause.
- **Recuperação:** derrube/recriação do pause com o sandbox; nenhum membro fica
  órfão num namespace morto.
