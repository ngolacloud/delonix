# Campanha (c) — produção crítica nível 2: multi-nó · Ceph · failover

> Próximo nível da validação de prontidão (a Ronda 1 em `findings-host.md` cobriu
> single-node). Objectivo: validar o Delonix sob as condições que **só aparecem
> em produção distribuída** — escalonamento entre nós, armazenamento persistente
> partilhado (Ceph), e recuperação de falha de **nó inteiro** (não só de
> processo). Doutrina: só o efeito verificado conta.

## Harness disponível (não reinventar)
- **Multi-nó in-engine:** `delonix cluster create --workers N --control-planes M`
  (kind-mode: nós `kindest/node` correm NO motor, bootstrap `kubeadm`, sem Docker).
- **Multi-nó em VMs reais:** `kubeadm` HA (5 VMs + 3 CP + etcd + HAProxy) — já
  provado (ver memória `delonix-kubeadm-ha-offline-findings`; CNI foi bloqueador).
- **Ceph:** infra provada — MicroCeph 19.2.3 nativo (RADOS/RBD/CephFS/RGW) +
  `ObjectStorage` backend `cephRgw` + roles Ansible microceph / Helm rook_ceph.
- **Scripts:** `e2e_overlay_multinode.sh`, `e2e_cluster_kind.sh`,
  `e2e_node_network_ops.py`, `chaos.sh`.

## Provas a correr (por ordem de achabilidade)

### C1 — Multi-nó: escalonamento entre nós ⏳ (em curso)
`cluster create prodtest --workers 2` → 1 CP + 2 workers Ready. Escalonar um
Deployment de N réplicas e verificar que **se espalham pelos nós** (anti-afinidade
implícita). **Efeito:** pods em ≥2 nós distintos, todos Running.

### C2 — Failover de nó (o teste central de produção crítica)
Com o cluster C1: `delonix vm stop`/matar um worker (simula falha de nó) →
verificar que o k8s marca o nó NotReady e **re-escalona os pods** para o nó
sobrevivente. **Efeito:** réplicas voltam a Running noutro nó dentro do
pod-eviction-timeout; nenhum pod fica preso em Terminating/Unknown.

### C3 — Rede overlay entre nós
`e2e_overlay_multinode.sh` — pod no nó A alcança pod no nó B (CNI cross-node).
**Efeito:** conectividade pod-a-pod através de nós, IP roteável.

### C4 — Armazenamento Ceph persistente (sobrevive à falha de nó)
`kind: ObjectStorage`/`Volume` com backend Ceph (RBD/CephFS). Escrever dados num
pod, matar o nó onde corre, verificar que o pod re-escalonado **relê os mesmos
dados** do Ceph. **Efeito:** persistência sobrevive à perda do nó — o critério
que distingue "stateful em produção" de "demo". (Precisa de um Ceph vivo — reusar
MicroCeph ou stand-up mínimo.)

### C5 — HA do control-plane
`cluster create --control-planes 3` → matar 1 CP → a API continua (etcd quorum).
**Efeito:** `kubectl` responde com 1 CP em baixo.

## Onde isto quebra (hipóteses a confirmar/refutar)
- Memória: **CNI foi bloqueador na HA kubeadm** (5 VMs) — C3 é o teste directo.
- Failover (C2): o pod-eviction + re-schedule depende do kubelet/kube-controller;
  o Delonix é o runtime CRI por baixo — valida que o CRI reporta o nó como morto
  a tempo e limpa os pods órfãos.
- Ceph (C4): o acoplamento CSI/driver é o ponto frágil típico.

## C1 — primeira tentativa (host sob forte contenção): achados
`cluster create --name prodtest --workers 2` (kind-mode): os **3 nós
arrancaram** (`prodtest-control-plane`+`worker`+`worker2` como containers
`kindest/node:v1.37.0`, `cluster ls` → state **up**, API em 127.0.0.1:6443,
k8s v1.37), **MAS o create falhou** com «timeout waiting for containerd on node
'prodtest-worker2' (90s)» e **não exportou o kubeconfig** → cluster inacessível.
Três achados (sob a carga do host partilhado: VM + benches + 3 nós):
1. **BUG resiliência:** o wait de containerd (90s, sem retry/backoff) falha o
   create inteiro por UM nó lento, mesmo com os nós de pé — devia esperar com
   retry ou reportar «nós up, kubeconfig pendente», não abortar.
2. **BUG limpeza de create parcial:** os nós ficaram **órfãos** (não registados
   como cluster) — `cluster <name> delete` não os encontra; foi preciso
   `container rm -f` manual. Um create falhado deve limpar ou registar.
3. **UX:** `cluster delete prodtest` erra — a sintaxe é `cluster <name> delete`
   (nome primeiro). Mensagem clara, mas invertida face a docker/podman.

> Nota: a contenção do host (laptop partilhado, muitas cargas) exagera o (1).
> **A re-correr num ambiente limpo / VM dedicada** para separar o bug de
> resiliência da carga. Os nós SOBEM — o bloqueio é o gate de prontidão + o
> kubeconfig.

## Estado
| Prova | Estado |
|---|---|
| C1 multi-nó scheduling | **PARCIAL** — nós sobem, create aborta no wait de containerd sob carga (3 achados acima); re-correr limpo |
| C2 failover de nó | a seguir a C1 |
| C3 overlay cross-node | a seguir (`e2e_overlay_multinode.sh`) |
| C4 Ceph persistente | precisa de Ceph vivo (reusar MicroCeph) |
| C5 HA control-plane | opcional (`--control-planes 3`) |

> Esta é uma campanha de **várias rondas** (cada prova é não-trivial e algumas
> exigem VMs/Ceph). O host tem KVM/qemu/virsh; o cluster kind-mode corre no motor.
