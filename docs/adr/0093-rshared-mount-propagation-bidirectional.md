# ADR-0093 — `rshared` (propagação de mount bidireccional) exige um grupo de peers com o host, atrás de privilégio

**Estado:** Proposto (2026-11-08) — identificado e medido na campanha de
conformância CRI (`critest v1.36.0`, 84 → 99/103 rootful-VM+CNI,
`reports/cri-conformance-0.1.1/00-baseline-develop.md`). O spec **mount with
'rshared' should support propagation from host to container and vice versa** falha,
enquanto `rprivate` e `rslave` passam. **NÃO implementado** — a propagação
container→host é um privilégio real (abre a árvore de mount do host a um container);
a decisão de a suportar e sob que porta é do dono. **Relaciona:** ADR-0083 (rootful
suportado), `reports/cri-conformance-0.1.1/`.

## Contexto

O `critest` verifica três modos de propagação de mount:

- **`rprivate`** — sem propagação (nenhum dos lados vê os mounts do outro). Passa.
- **`rslave`** — unidireccional **host→container** (um mount criado no host aparece
  no container, mas não o inverso). Passa.
- **`rshared`** — **bidireccional** (host→container **e** container→host). Falha: o
  teste cria um mount DENTRO do container e verifica que aparece no HOST
  (`len(fileInfo) should not be zero`, `container_linux.go`) — e não aparece.

A causa está em `container_init` (`delonix-linux/src/lib.rs` ~2504-2519): a raiz de
mount do container é posta a **`MS_REC | MS_SLAVE`** quando algum mount pede
propagação (e `MS_PRIVATE` caso contrário). O comentário no código é explícito e a
escolha é deliberada:

```
MS_PRIVATE … makes rslave/rshared on any individual mount a no-op … MS_SLAVE keeps
the one-way link the propagating mounts need (and is what Docker and runc use for
ALL containers …).
```

`MS_SLAVE` dá a direcção **host→container** (o slave recebe do master), por isso
`rslave` funciona. Mas um slave **não envia de volta** ao master — logo a metade
**container→host** do `rshared` não acontece, e o per-mount `MS_SHARED` aplicado ao
alvo (linhas ~1508/1718) não basta enquanto a raiz continuar slave: não há grupo de
peers partilhado com o host para receber o evento.

## Porque não é só «trocar SLAVE por SHARED»

Pôr a raiz do container em `MS_SHARED` com o host faria **todos** os mounts do
container propagarem para o host (e para outros containers no mesmo grupo de peers)
— uma abertura muito maior do que o pedido e um vector de escalada (um container a
criar mounts visíveis no host). Por isso `MS_SLAVE` é o default seguro que o
Docker/runc usam. O `rshared` verdadeiro é, por natureza, um **privilégio**: em
Kubernetes, `mountPropagation: Bidirectional` **exige um container privilegiado**.

## Decisão

Honrar `rshared` **por-mount** (não na raiz) e **só** quando pedido explicitamente,
montando esse alvo específico num **grupo de peers partilhado com o mount de origem
do host**:

1. a raiz do container permanece `MS_REC | MS_SLAVE` (o default seguro inalterado);
2. para um mount com `propagation: rshared`, garantir que a **origem no host** é
   `shared` (num grupo de peers), fazer o bind para dentro do container e marcar o
   alvo `MS_SHARED` — assim origem e destino ficam no MESMO grupo de peers e a
   propagação é bidireccional **apenas nesse sub-caminho**, não em toda a árvore;
3. **gate de privilégio:** `rshared` só é aceite quando o container é privilegiado
   (o mesmo requisito do k8s para `Bidirectional`). Um `rshared` pedido por um
   container não-privilegiado é **recusado com razão** (não degradado em silêncio
   para slave — a classe de falha-calado que o repo persegue), com a mensagem a
   nomear o requisito de privilégio e a alternativa (`rslave`).

Isto alinha com o runc (propagação por-mount; `rshared` exige a origem partilhada) e
mantém o isolamento por omissão.

## Alternativas consideradas

1. **Raiz `MS_SHARED` (trivial).** Faz o spec passar mas vaza TODOS os mounts do
   container para o host — escalada. Rejeitada.
2. **Degradar `rshared`→`rslave` em silêncio.** Seguro mas falha-calado (o container
   pede bidireccional e recebe unidireccional sem aviso). Rejeitada — contra a
   doutrina do repo.
3. **Peer group por-mount atrás de privilégio (esta decisão).** Conforma o spec sem
   abrir a árvore do host nem enganar o chamador.

## Consequências

- **+** Conforma o `rshared`; mantém `rprivate`/`rslave` e o isolamento por omissão;
  a abertura é limitada ao sub-caminho pedido e só a containers privilegiados.
- **−** Exige tornar a **origem no host** `shared` (uma operação de mount no host,
  logo root — coerente com ADR-0083, rootful suportado; em rootless `rshared`
  permanece recusado por falta de privilégio sobre a árvore do host). Novo caminho
  de recusa a testar (não-privilegiado → erro claro).
- **Compat:** nenhum contrato público muda de forma; é comportamento do caminho de
  mount. `rshared` passa de «degrada a slave» para «suportado-se-privilegiado /
  recusado-caso-contrário».

## Validação exigida antes de Aceite

- **Comportamento:** `critest` `rshared` passa (mount criado no container aparece no
  host **e** vice-versa); `rslave`/`rprivate` continuam a passar.
- **Segurança:** um `rshared` de container não-privilegiado é recusado com a razão;
  um container `rprivate`/`rslave` não ganha nenhuma propagação para o host.
- **Unit:** a selecção de propagação por-mount e o gate de privilégio, puros.

### Investigação (2026-11-08) — a falha é provavelmente específica do delonix, não do kernel

O `delonix` **já aplica** a abordagem do runc: em `bind_volume` (`lib.rs` ~1505)
um mount com `propagation: rshared` recebe `mount(dst, MS_SHARED | MS_REC)` como
`mount()` próprio após o bind. O runc faz exactamente isto e o `Bidirectional`
mountPropagation do Kubernetes **funciona** sobre runc — com a raiz do rootfs
`rslave`, igual à nossa (`mount("/", MS_REC|MS_SLAVE)`, `lib.rs` ~2519). Logo a
abordagem não é o problema; a falha é **específica do delonix**.

Suspeitos (por confirmar com depuração de propagação na VM — ler
`/proc/self/mountinfo` do holder e do host, comparar `shared:`/`master:` peer-group
IDs antes/depois):
1. **Ordem vs `pivot_root`**: os volumes são montados ANTES do `pivot_root`
   (`lib.rs` ~2541); o `pivot_root` + `MNT_DETACH` do root antigo pode quebrar a
   pertença ao peer-group do host do mount `rshared`.
2. **A origem, dentro do mount-ns novo, já é slave** (a raiz foi `MS_REC|MS_SLAVE`
   antes do bind), por isso o `MS_SHARED` no dst cria um peer-group LOCAL
   (shared-and-slave), não reentra no grupo do host. runc evita isto por desenho —
   confirmar a diferença exacta.
3. O teste `critest` monta a origem rshared NO HOST; confirmar que, da vista do
   container, essa origem ainda está no peer-group do host no instante do bind.

É **depuração iterativa de propagação de mount** (segurança-sensível: um erro
propaga mounts do container para o host). Contida aos mounts `rshared` (blast
radius baixo — não afecta o isolamento geral), mas exige uma sessão dedicada com
repro na VM. A decisão do ADR (peer-group por-mount + gate de privilégio) mantém-se.
