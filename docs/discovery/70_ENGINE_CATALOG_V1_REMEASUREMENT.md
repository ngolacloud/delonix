# 70 — Catálogo v1 e lacunas do motor: re-medição com evidência

- **Medido em:** `develop == main == 64a5953a` (2026-11-03). O ponto `0f172f5` do prompt
  (#958, 2026-10-29) é ancestral deste — a medição reflecte o estado de HOJE, alguns commits
  à frente.
- **Método:** 6 frentes de leitura em paralelo sobre `crates/`, `bins/`, `proto/`,
  `docs/`, `scripts/e2e.sh`, `scripts/chaos.sh` e `docs/providers/capability-matrix.md`.
  Cada ponto tem veredicto **FECHADO / PARCIAL / ABERTO** com `ficheiro:linha` ou secção.
- **Regra:** itens FECHADOS saltam-se nas fases de construção. Um PARCIAL diz o que falta.
  Esta é a tabela que a Fase 1 do prompt pede como Entregável #1.

> **Aviso de baseline (fora do âmbito desta campanha, mas bloqueante).** A run de CI do tip
> `64a5953a` está **FAILURE**: `contract gate`, `generated-docs` (handbook), `test` (arm64,
> `cargo test --workspace`) e `cli-execution-ratchet`. A última bateria verde é `b5e153b3`
> (Out-11, 1345/0/13 + chaos 53/0/2). A regra inegociável «gates verdes» tem de ser
> restabelecida ANTES de qualquer fase desta campanha fechar — senão a campanha herda uma
> baseline vermelha e mascara regressões próprias.

---

## Correcção às premissas do prompt (medido, não assumido)

1. **Grupos: são 9, não 7.** Além de `core/compute/networking/gateway/storage/artifact/
   infrastructure`, existem `security.delonix.io` (`RuntimePolicy`) e `gitops.delonix.io`
   (`GitOpsSource`). Todos em `v1alpha1`; não existe nenhum `v1` apesar do nome «catálogo v1».
2. **Os serviços gRPC por Kind JÁ são servidos.** O prompt (C1) assume que
   `VirtualMachineService/ContainerService/PodService/ImageService/StackService` «estão no
   `.proto` mas não são servidos». Medido: os **10 serviços estão registados e implementados**
   (`delonix-node-api/src/service.rs:948-993`), desde o Sprint 13. A premissa está obsoleta —
   o que C1 propõe é *substituir* um contrato servido, não terminar um por servir.
3. **Não há «excepção Pod/VM sem namespace».** O prompt (A3) quer «acabar com a excepção que
   a ADR-0069 documentou». Medido: `VirtualMachine/Container/Pod` já são `Namespaced::Always`
   (`kinds.rs:489`). A ADR-0069 Pending #2 decidiu outra coisa — o **nome de uma VM é único
   no NÓ** (conflito entre namespaces), não «único dentro do namespace». Isso é a contradição
   real a resolver (ver lista abaixo), não uma excepção de campo namespace.
4. **O bug N3 (`set_network` ao criar VM) está FECHADO** (`delonix-node-api/src/lib.rs:128-143`
   + teste de regressão `tests/vm_network_composition.rs`). C1 não precisa de o corrigir.
5. **Duas linhas da `capability-matrix.md` estão adiantadas/falsas face a develop** — violam
   a regra «só suportado com teste ao vivo»:
   - `storage.ceph` (linha ~198): diz «driver the engine does not ship» quando o crate
     `delonix-provider-ceph` **já embarca** em develop (PR #960). Corrigir o texto (o cluster
     Ceph externo continua a ser requisito legítimo).
   - `net.ipv6` (linha ~146): marcado **supported** quando o dataplane v6 ainda é scaffolding
     (`delonix-sdn/src/infra.rs:46` «the dual-family table never governs a v6 packet yet») —
     o «governado ON» vive em ramo não-integrado. **Rebaixar para experimental/partial** até
     o v6 governar um pacote em develop.

---

## Parte A — Fundação declarativa

| Ponto | Veredicto | Evidência | Falta |
|---|---|---|---|
| A1 diff a partir do schema + `x-delonix-update` + gate por-campo | **ABERTO** | 0 anotações `x-*` em `docs/schema`; classificação live/reboot/cold é tabela à mão por Kind (`reconcile.rs:277-437`, `grow_only_fields:451`); braço `_ => &[]` (`:436`) silencia o não-classificado | anotação por campo no schema + diff que a leia + gate que recuse campo sem classe |
| A2 metadata/status comum | **ABERTO** | `Metadata{name,ns,labels,annotations}` ok (`manifest.rs:20-34`); **sem** `generation/observedGeneration/conditions/outputs/storedVersion`; `Condition` actual (`condition.rs:5-11`) é pré-requisito, sem `lastTransitionTime` nem taxonomia Ready/Progressing/Degraded | bloco `status` + `generation` escrito pelo motor + condições uniformes + `stack wait` a ler só `Ready` |
| A3 dois âmbitos | **FECHADO** (modelo) | enum `Namespaced{Never,Always,PerDocument}` (`kinds.rs:236`); VM/Container/Pod=Always, App=Never, Volume=PerDocument | v1 muda o mapa (Application namespaced; ver contradição VM-name) |
| A4 ordem por grafo | **ABERTO** | ordem FIXA da tabela FACTS (`kinds.rs:275-304`) via `run_layers_inner` hardcoded (`stack.rs:2442-2523`), 1 Kind de cada vez; sem `dependsOn/outputRef/toposort`; `validate_graph` (`stack.rs:3496`) só valida existência; destroy inverso FECHADO | grafo real de referências, paralelismo por nível, detecção de ciclo com caminho |
| A5 controlos de ciclo de vida | **PARCIAL** | 3-vias com `delonix.io/last-applied` FECHADO (`reconcile.rs:63,605-642`); `plan -o json` FECHADO mas `apply` come manifesto + `--plan-digest` (não um ficheiro de plano); `planDigest` só provider-backed (`reconcile.rs:235`); `protect`/`ignore-fields`/`drift`/lock **ABERTOS** (0 ocorrências) | `stack apply plano.json`; protect/ignore-fields; **drift** (contradiz decisão registada); lock por stack |

## Parte B — Catálogo (24 Kinds v1)

Catálogo actual: **27 Kinds** numa só tabela (`crates/contexts/delonix-stack/src/kinds.rs`),
todos `v1alpha1`, lida por `api-resources`/schema/parser/reconciliador (sem segunda lista).
**Não existe `delonix manifest convert`** nem maquinaria de conversão de versões / storage
version. Deprecação modelada em `enum Form` (`kinds.rs:154`): `Sunset(successor)` anuncia,
`Sugar`/`Compat` baixam, fusões avisam, renomeações puras (`KIND_ALIASES`) são alias silencioso.

Dos 24 Kinds v1: **14 já existem, 4 parciais, 6 não existem.**

| Grupo v1 | Kind | Veredicto | Origem / nota |
|---|---|---|---|
| compute | Application | **PARCIAL** | deriva de `App` (hoje em **artifact**, só constrói imagem) + `Workload`/`Pod`/`Container` que correm; nenhum Kind «Application» |
| compute | Job | **NÃO-EXISTE** | sem Kind Job; sem `action` nem `template` |
| compute | VirtualMachine | **EXISTE** | `kinds.rs:479` |
| networking | Network | **EXISTE** | `kinds.rs:345` |
| networking | NetworkRoute | **EXISTE** | `kinds.rs:359` |
| networking | Router | **NÃO-EXISTE** | mais próximo `NetworkGateway` (`kinds.rs:603`) |
| networking | IPPool | **EXISTE** | `kinds.rs:624` |
| networking | IPAddress | **NÃO-EXISTE** | só `IPPool` (ledger de reservas) |
| networking | NetworkPolicy | **EXISTE** | `kinds.rs:572` |
| networking | Service | **EXISTE** | `kinds.rs:544` (DNS round-robin, ADR-0032) |
| networking | Gateway | **PARCIAL** | **colisão de nome**: o `Gateway` actual é o ex-`Tunnel` edge (`kinds.rs:652`), semântica ≠ Gateway-API |
| networking | HTTPRoute | **EXISTE** | `kinds.rs:638` (hoje em grupo **gateway**) |
| storage | StoragePool | **EXISTE** | `kinds.rs:397` |
| storage | Volume | **EXISTE** | `kinds.rs:413` |
| storage | Bucket | **PARCIAL** | deriva de `ObjectStorage` (`kinds.rs:429`) |
| storage | Snapshot | **NÃO-EXISTE** | snapshot de VM só imperativo |
| core | Secret | **EXISTE** | `kinds.rs:327` |
| core | Config | **NÃO-EXISTE** | não há ConfigMap-like; dados não-secretos só via `Secret` |
| core | Image | **EXISTE** | hoje em **artifact** (`kinds.rs:447`) |
| core | Stack | **EXISTE** | `kinds.rs:721` |
| core | GitOpsSource | **EXISTE** | hoje em **gitops** (`kinds.rs:671`) |
| core | Node | **NÃO-EXISTE** | só RPC `NodeService.GetCapacity`, sem Kind |
| policy | RuntimePolicy | **EXISTE** | hoje em **security** (`kinds.rs:310`) |
| cluster | KubernetesCluster | **EXISTE** | hoje em **infrastructure** (`kinds.rs:736`), `in_stack:false` |

Realocação de grupo mesmo nos «existe»: Image/App→core, RuntimePolicy security→policy,
KubernetesCluster infrastructure→cluster, GitOpsSource gitops→core, HTTPRoute/Gateway
gateway→networking.

Conversões dos Kinds actuais (origem → destino v1): `Container`→`Application` (Sunset já
anunciado), `Pod`(escrito)→instância de `Application`, `App`→`Application.containers[].build`
+ `Image`, `Workload`→`Application`/`VirtualMachine`, `NetworkAccessRule`→`NetworkPolicy`,
`Dependency`→`dependsOn`/`NetworkPolicy`, `NetworkZone`→`Network`+provider (Sunset já),
`NetworkGateway`→`Router`, `ObjectStorage`→`Bucket`, `Ingress`→`HTTPRoute` (Compat já),
`Gateway`(túnel)→`Gateway` class de túnel, `SystemContainer`→`proxmox.delonix.io/SystemContainer`.

## Parte B — KubernetesCluster convergente (2 FECHADO / 7 PARCIAL / 6 ABERTO)

| # | Capacidade | Veredicto | Evidência |
|---|---|---|---|
| 1 | 3 modos kind/vm/ssh | **FECHADO** | `cluster.rs:138,630,2454` |
| 2 | convergência declarativa/idempotente | **PARCIAL** | idempotente por passo (`cluster.rs:3125-3128`); sem diff/remoção/estado; sem `plan` |
| 3 | HA CP + join um-a-um c/ etcd saudável | **PARCIAL** | `cluster.rs:2891-2897` + ADR-0085; sem gate de quórum etcd entre joins |
| 4 | etcd stacked vs external | **PARCIAL** | `cluster.rs:92,663`; external só `ssh` (ou `vm` via flag), nunca `kind` |
| 5 | endpoint kube-vip/HAProxy/external | **PARCIAL** | `lb.rs` + ADR-0085; HAProxy+externo sim, **kube-vip NÃO** (ADR-0085 rejeitou VIP) |
| 6 | scale CP/workers + remover membro etcd | **ABERTO** | sem `scale`, sem `nodePools[]` (só `workers` único), remoção etcd «por fazer» |
| 7 | upgrade menor-a-menor, snapshot, um-a-um | **PARCIAL** | `cluster.rs:2367,2380`; só `ssh`, sem snapshot prévio, sem gate de salto de versão |
| 8 | backup/restore (etcd+PKI+config), CA p/ cluster novo | **PARCIAL** | `etcd_backup.rs:9-26`; só etcd stacked, in-place 1 CP; sem PKI/config/manifesto; sem restauro p/ cluster novo |
| 9 | auto-reparação de workers | **ABERTO** | nenhum laço NotReady→substitui |
| 10 | retoma após `kill -9` (registo de operações) | **PARCIAL** | por re-execução idempotente stateless; sem op-ledger |
| 11 | perfis CNI fixados + offline | **FECHADO** | `net_profiles.rs:13,32` + `--cni-offline`; imagens do CNI não empacotadas |
| 12 | `x-delonix-update` por campo | **ABERTO** | 0 ocorrências |
| 13 | conformidade CNCF/Sonobuoy | **ABERTO** | só `critest` de CRI |
| 14 | segurança por omissão (PSA/encryption/audit/…) | **ABERTO** | `kubeadm_config.rs` só escreve etcd |
| 15 | subcomandos | **PARCIAL** | faltam `plan`,`scale`,`certs`; resto existe (alguns só num modo) |

## Parte C — Contrato do nó

| Ponto | Veredicto | Evidência |
|---|---|---|
| 10 serviços servidos (não só proto) | **FECHADO** | `service.rs:948-993` + impls `:135-937` (só `VM.Console` e `Image.Push/Build/Scan` UNIMPLEMENTED) |
| `ResourceService` genérico (Apply/Plan/Get/List/Watch/Delete sobre qualquer Kind) | **ABERTO (por desenho)** | ausente no proto; portão de contrato (`AGENTS.md:148,159-166`) **impõe** «um pedido por RPC» + identidade explícita, **nunca** `ResourceMeta` genérico → C1 contradiz isto |
| Exec / Logs | **FECHADO** | `service.rs:910-929` → `container_exec`/`container_logs` |
| Console de VM / Attach / port-forward | **ABERTO** | `Console` UNIMPLEMENTED (`service.rs:712`); Attach/port-forward nem no contrato |
| NodeService / OperationService (Get/Watch/Cancel) / WatchEvents | **FECHADO** | `service.rs:135-196,726-781,174-187` |
| Watch com retoma por offset | **PARCIAL** | só `since` temporal (`node.proto:114`); sem offset genérico |
| Socket activation + `.socket` + idle-exit | **FECHADO** | `dist/delonix-node-api.socket`; `lib.rs:169-183,264-279`; testes `idle_exit.rs`/`socket_activation.rs` |
| `ErrorDetail` tipado em TODOS os caminhos | **PARCIAL** | REST dá problem+json completo (`service.rs:1385`); gRPC (`network_ops.rs:73-74`) só trailer DX — o proto (`common.proto:113`) promete mais do que o gRPC entrega |
| em-processo vs CLI (ADR-0040 D5) | **PARCIAL (por desenho)** | leituras/mutações em processo; só launch/exec/pod-create re-executam `delonix` (`container_exec.rs:6-11`) por `setns`/`clone` serem single-thread |
| bug N3 (`set_network` ao criar VM) | **FECHADO** | `lib.rs:128-143` + `tests/vm_network_composition.rs` |
| **C2** autz por namespace/verbo/Kind (ADR-0003) | **ABERTO** | só `SO_PEERCRED` uid-equality (`lib.rs:295`) + socket 0600; ADR-0003 **aceite mas o portão de capacidade NÃO está construído** (o `enum Capability` existente é o catálogo de providers, ADR-0050) |

## Parte D — Capacidades

| # | Item | Veredicto | Evidência |
|---|---|---|---|
| D1a | `RuntimePolicy` profile strict/baseline/privileged por `namespaceSelector` | **PARCIAL** | Kind existe (`policy.rs:425`); sem `profile` nem `namespaceSelector`; tecto de nó; só `baseline` (`policy.rs:488`) |
| D1b | tecto de capabilities CRI, recusa sem tecto | **FECHADO** | `cap_ceiling.rs:84` (default = conjunto seguro; malformado recusa arranque) |
| D1c | Secret só em tmpfs, sem disco persistente | **FECHADO** | `delonix-linux/src/lib.rs:3468` (tmpfs 0600 no ns) |
| D1d | fallback root (ADR-0062) visível | **FECHADO** | `lifecycle.rs:998,2503` (`userFallbackToRoot` no status) |
| D2 | container por-instância em microVM com API de container + RuntimeClass + medições | **ABERTO** | `type: microvm` baixa para VM completa (`workload.rs:15`); `runtime_handlers=vec![]` (`runtime_svc.rs:208`); só densidade de containers medida |
| D3 | `storedVersion` + migração de estado testada em CI | **ABERTO** | 0 `storedVersion`; só spike `56_P4_D6_...` |
| D4a | overlay VXLAN + WireGuard entre zonas (ADR-0084) | **FECHADO** (ADR Proposto) | `delonix-sdn/src/infra.rs:8289`, `wg.rs`; live `g4-overlay-forwarding.md` 0% perda 2 nós |
| D4b | CNI multi-nó / recusa explícita (N15) | **FECHADO** | `cni:default`→Cilium (`cluster.rs:305`); `none`→NotReady nomeado (`:159`) |
| D4c | `IPAddress` NAT 1:1 movível entre nós | **ABERTO** | Kind inexistente |
| D4d | Service type:LoadBalancer (ADR-0066) + IPv6 (0065/0082) | **PARCIAL** | LB = VIP L4 de **nó único** (`lb.rs:1`), não cross-node; IPv6 scaffolding, v6 recusado (matrix adiantada) |
| D5a | ADR-0081 ceph-native implementada | **FECHADO** (ADR Proposto; matrix desactualizada) | crate `delonix-provider-ceph` em develop; `storage.rs:13`; MicroCeph 19.2.3 live |
| D5b | StoragePool ceph-rbd/cephfs + pools ADR-0067 (N20) | **FECHADO** | `storage.rs:215-221`; dir/btrfs/zfs/lvm-thin + ceph |
| D5c | `Volume.size` cresce a quente (N16) | **FECHADO** | `volume.rs:336` |
| D5d | `Snapshot`→Bucket agendado/retenção por `Job action:CreateSnapshot` | **ABERTO** | peças ad-hoc (`vm.snapshot`, backup etcd); sem destino Bucket, sem Kind Job, sem agendamento |
| D6 | hotplug CPU/mem/disco/NIC ≤ max (imperativo) | **FECHADO** | `vm.rs:1388`; matrix 49-56; DX-8504 (`codes.rs:1532`) |
| D6 | …via manifesto (reconciliador, ADR-0068 D9) | **ABERTO** | `reconcile.rs:323,451` ainda planeia Replace |
| D6 | guest agent nos 3 backends | **PARCIAL 2/3** | libvirt/proxmox live; CH sem canal virtio-serial (`:538`, estrutural) |
| D6 | migração a frio (N12) | **FECHADO** | matrix L110; `e2e.sh:6497/6594`; proxmox live |
| D6 | arrancar de disco partilhado existente | **ABERTO** | root sempre overlay fresco (`local_ports.rs:258-310`); só discos extra referenciam volume |
| D6 | fencing RBD lock + blocklist + teste split-brain | **ABERTO** | sem código; delega a gestor externo |
| D6 | live migration fora (ADR-0031) | **CONFIRMADO fora** | CH `:509`; só Proxmox delega ao PVE |
| D7 | CRI re-medida critest (N6) | **FECHADO** 84/104 ≈ 80,8% | `docs/cri-conformance.md` (v1.36.0, 2026-10-24) |
| D7 | CRI ≥95% (N26) | **ABERTO** (~14 pp) | 5 gaps reais (2 já corrigidos em ramo não-integrado) |
| D8 | Kind `Node` capacity/allocatable/allocated/overcommit/eventos | **ABERTO** | só RPC `GetCapacity` (`node.proto:80-102`); sem allocated/IPs/GPU, sem eventos |
| D8 | `unschedulable` drena c/ disruption | **ABERTO** | 0 no contrato; só wrapper `kubectl` (`cluster.rs:2141`) |
| D9 | SLIs daemonless + observe/diagnose | **ABERTO** | `engine-slis-2026-10-12.md` «nothing wired»; sem `/metrics` no node-api |
| D10 | init gera Stack App+Svc+Gateway+HTTPRoute+Secret | **ABERTO** | gera compose + `Container`/`Stack` |
| D10 | rust + DORM (migrações+outbox) | **ABERTO** | `rust/src/memstore.rs:12` (em memória) |
| D10 | gRPC+REST buf / LRO / Idempotency-Key / JetStream | **ABERTO** | ADR-0061 D6 «gRPC not generated»; RabbitMQ, não NATS |
| D10 | templates OCI assinados (`-t oci://`) | **ABERTO / fora de contrato** | `scaffold.rs:25-27` embebido; ADR-0061 D2 «no network at generation time» |
| D11 | fuzzing manifesto/conversores/diff | **PARCIAL** | `fuzz/` 6 alvos (parsers de entrada) + CI; não cobre manifesto→diff |
| D11 | feed CVE no scanner | **PARCIAL** | OSV parser existe (`scanner lib.rs:525,632`); DB embebida = 5 placeholders, sem sync automático |
| D11 | cobertura CLI (N11) | **95,6%** (258/270) mas ratchet X no tip | `RELEASES.md:318`; `cli-execution-ratchet` vermelho em 64a5953a |
| D11 | bateria limpa no tip (N30) | **ABERTO no tip** | verde em `b5e153b3`; `64a5953a` CI FAILURE |

Lacunas de Outubro confirmadas **fechadas**: backup de etcd (`etcd_backup.rs`),
`UpdateContainerResources` (`runtime_svc.rs:333`), hotplug add+remove CH/libvirt, migração a
frio nos 3 backends — vários muito além do plano de fases da ADR-0068.

---

## Contradições com decisões registadas (o prompt é a nova vontade do dono; cada uma exige ADR)

Pela regra #3 do prompt, cada contradição abaixo tem de ficar escrita numa ADR que diga o que
substitui:

1. **Drift detection.** `AGENTS.md:1246,1691` + ADR-0069 decidem **não** fazer drift
   («trabalho de um orchestrator com controllers, deliberadamente»). O A5 pede `stack drift`
   com política report|correct. → ADR da fundação substitui essa decisão.
2. **Nome de VM/instância único no namespace.** ADR-0069 Pending #2 decidiu nome **único no
   nó** (conflito entre namespaces). O A3 pede unicidade **dentro do namespace** (dois
   namespaces podem ter cada um a sua VM `X`). → ADR do catálogo substitui ADR-0069 #2.
3. **kube-vip.** ADR-0085 **rejeitou** keepalived/VIP. O KubernetesCluster v1 quer kube-vip
   (ARP/BGP) como endpoint por omissão. → ADR do cluster substitui a decisão de endpoint da
   ADR-0085.
4. **KubernetesCluster convergente.** ADR-0069 D7 pôs `KubernetesCluster` **fora** do
   `stack apply` (`in_stack:false`, «procedimento SSH remoto»). O prompt quer um recurso
   convergente. → ADR do cluster substitui essa classificação.
5. **`ResourceService` genérico vs «um pedido por RPC».** O portão de contrato
   (`AGENTS.md:148,159-166`) impõe serviço/identidade explícitos e proíbe `ResourceMeta`
   genérico. O C1 propõe exactamente um serviço genérico sobre qualquer Kind. → ADR do
   contrato de recursos substitui a regra e **nomeia a excepção ao `buf breaking`** (os
   serviços por Kind retirados; como são servidos, há aviso de clientes, mas no nó são
   consumidos pela própria CLI/agente — a ADR documenta a janela de migração).

---

## Roteiro de ADRs da Fase 1

| ADR | Título | Substitui / baseia-se | Estado nesta entrega |
|---|---|---|---|
| **0086** | Kind catalog v1 — six groups, `v1beta1`, conversion machinery | ADR-0069 D7, ADR-0020 (nomes/grupos), ADR-0069 #2 (nome de VM) | **redigida nesta fase** |
| **0087** | Generic resource contract on the node socket | portão de contrato (um-pedido-por-RPC), serviços por Kind; nomeia excepção `buf breaking`; implementa ADR-0003 (autz por namespace) | **redigida nesta fase** |
| 0088 | Declarative foundation — schema-driven diff, status/conditions, dependency graph, lifecycle controls (drift) | AGENTS.md «no drift», amplia ADR-0007/0019/0069 D1 | esboço de decisão pronto; redigir no arranque da Fase 2 |
| 0089 | KubernetesCluster as a convergent resource | ADR-0069 D7 (`in_stack:false`), ADR-0085 (endpoint/kube-vip) | esboço de decisão pronto; redigir no arranque da Fase 10 |
| 0090 | Capability amendments — microVM containers, state versioning, inter-node IPAddress/LB, shared-storage fencing, Node Kind, SLIs | amplia ADR-0006, fecha ADR-0081; novo para Node/fencing/SLIs | esboço de decisão pronto; redigir por capítulo nas Fases 7–11 |

As 0088–0090 ficam com a decisão esboçada aqui e redigem-se no arranque da fase que as
constrói (o prompt entrega por fases), para não fixar detalhe antes da medição de
implementação de cada uma. As 0086 e 0087 — as que o Entregável #2 nomeia explicitamente e de
que todas as fases seguintes dependem — ficam redigidas já.
