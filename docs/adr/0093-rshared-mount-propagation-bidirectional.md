# ADR-0093 — `rshared` (propagação de mount bidireccional) exige um grupo de peers com o host, atrás de privilégio

**Estado:** Aceite (2026-11-08) — identificado e medido na campanha de
conformância CRI (`critest v1.36.0`), e **implementado e validado numa VM golden
rootful+CNI**: o spec **mount with 'rshared' should support propagation from host
to container and vice versa** passa, fechando a suite a **103/103**. **Relaciona:**
ADR-0083 (rootful suportado), `reports/cri-conformance-0.1.1/`.

> **O que foi preciso (medido numa VM, não assumido — ver «Investigação»):** o
> `MS_SHARED` por-mount sozinho NÃO basta, e pôr a raiz `MS_REC|MS_SHARED` conforma
> o spec mas **vaza** os mounts do container para o host (parte o cgroup do
> servidor CRI). O volume só aterra como **peer** do grupo do host (`shared:N`, o
> que leva um mount feito no container até ao host) se a origem no host **mantiver**
> o grupo de peers com que foi clonada E o pai do bind (o rootfs) estiver `shared`
> no instante do bind. Qualquer mudança RECURSIVA de propagação em `/` destrói a
> primeira condição (arranca a origem do grupo, e um mount nunca reentra num grupo
> que deixou). Daí a forma final, toda atrás do gate de privilégio:
>
> 1. **`/` NÃO é tocado recursivamente** — a origem mantém o `shared:N` do clone;
> 2. o mount que **ENCLAUSURA o rootfs** (ex.: `/run` no CRI) é posto `MS_PRIVATE`,
>    confinando a partilha ao sub-caminho do rootfs (nada que o container monte por
>    baixo escapa ao host);
> 3. o **rootfs** é tornado `MS_SHARED` antes dos volumes → o bind da origem
>    partilhada aterra como peer do grupo do host;
> 4. mesmo antes do `pivot_root` (que recusa uma raiz-actual partilhada), `/` passa
>    a `MS_PRIVATE` **não-recursivo** (os volumes, mounts próprios, mantêm `shared:N`);
> 5. a raiz antiga (`/.delonix_old`) é posta `MS_REC|MS_PRIVATE` **antes** do
>    `MNT_DETACH` — senão o detach, via a partilha, desmontaria o `/sys/fs/cgroup`
>    do host onde o motor corre (medido: `cgroup: could not create delonix.slice`).
>
> Um `rshared` de container não-privilegiado ou motor rootless é **recusado com
> razão** (`Error::BidirectionalNeedsPrivileged`, DX-1117), nunca degradado em
> silêncio para slave.

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

> **Nota (as-built):** esta é a *proposta* original. A medição na VM mostrou que o
> ponto 1 abaixo (raiz permanece `rec-slave`) **não** consegue tornar o volume peer
> do host — a forma efectivamente implementada está na caixa-resumo no topo e na
> «Investigação». O objectivo (bidireccional só no sub-caminho `rshared`, atrás de
> privilégio) e o ponto 3 (gate) mantêm-se; a mecânica da raiz é que mudou.

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

## Validação (feita — 2026-11-08, VM golden `delonix-vm-k8s:1.36` rootful+CNI)

- **Comportamento:** `critest` `mount with 'rshared'` **passa** (o `mount --bind
  /etc` feito DENTRO do container aparece no host), `rslave`/`rprivate` continuam a
  passar; suite completa a **103/103**.
- **Segurança:** `rshared` de container não-privilegiado / motor rootless é
  recusado com `Error::BidirectionalNeedsPrivileged` (DX-1117); `rprivate`/`rslave`
  não ganham propagação para o host (cada mount não-rshared é fixado `rprivate`).
- **Unit:** `per_mount_propagation`, `root_propagation`, `rshared_allowed` e
  `enclosing_mount_point` — puras, em `mount_propagation_tests`.

### Investigação (2026-11-08/09) — resolvida na VM, peer-groups medidos

A hipótese inicial («o `MS_SHARED` por-mount já basta, igual ao runc») estava
**errada para o delonix**. Medido em `/proc/self/mountinfo` do container vs host,
iterando numa VM golden:

- Com a raiz `MS_REC|MS_SLAVE` (default), o volume saía `shared:X master:1` —
  **slave** do grupo do host (recebe host→container, nunca devolve). Suspeito #2
  CONFIRMADO.
- Pôr `/` em `MS_REC|MS_SHARED` fez o spec passar com `delonix run` (o host passou a
  ver o submount), MAS no caminho do servidor CRI partiu o cgroup: o `MNT_DETACH`
  da raiz antiga, com `/` partilhado, PROPAGAVA o desmonte de `/sys/fs/cgroup` de
  volta à ns do host onde o motor corre (`cgroup: could not create
  delonix.slice/...`). Confirmado por comparação directa: o binário `develop`
  (raiz rec-slave) arrancava o mesmo pod rshared; o meu (rec-shared) não. É o
  «vaza TODOS os mounts» da Alternativa 1, observado.
- A forma final (ver a caixa no topo) evita o vazamento: `/` **não** é tocado
  recursivamente (a origem mantém `shared:N`), o mount que enclausura o rootfs fica
  `MS_PRIVATE`, o rootfs fica `MS_SHARED` só para o bind dos volumes aterrar peer, e
  a raiz antiga é `MS_REC|MS_PRIVATE` antes do `MNT_DETACH`. Receita medida a
  funcionar ponta-a-ponta: um `mount --bind /etc` feito DENTRO do container aparece
  em `/tmp/.../src/sub` no host, e o cgroup do container cria-se sem erro.

Implementação em `setup_rootfs`/`detach_old_root`/`enclosing_mount_point`
(`delonix-linux/src/lib.rs`) e no gate em `spawn` via
`rshared_allowed(privileged, rootless)`. Blast radius contido ao sub-caminho
`rshared`: a raiz antiga é isolada antes do detach, o enclausurante do rootfs é
privado, e cada volume não-rshared é fixado `rprivate` por `per_mount_propagation`.
