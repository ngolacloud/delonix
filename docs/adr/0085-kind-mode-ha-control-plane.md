# ADR-0085 — `cluster create` (kind-mode) provisiona HA multi-control-plane

**Estado:** Proposto (2026-11-03) — fecha a lacuna que a prova do G4 revelou: o
turnkey `delonix cluster create` (kind-mode, nós em containers) recusava
`--control-planes > 1` porque não provisionava o endpoint estável (LB/VIP) que o
kubeadm HA exige. **Implementado e provado ao vivo** em `feat/ipv6-gonogo`:
`cluster create --control-planes 3` levanta 3 CP + um HAProxy container + etcd
stacked + kindnet, 3/3 Ready, e **sobrevive à queda de um CP**. Aguarda GO do dono.
**Relaciona:** ADR-0084 (overlay inter-nó), `reports/go-nogo-alta-criticidade-20261101/g4-ha-controlplane.md`.

## Contexto

O G4 (via prova) pedia um control-plane k8s **HA sobre o Delonix**. Isso foi
provado pelo caminho **kubeadm-em-VMs** (`cluster kubeadm`, que já provisiona um
HAProxy numa VM). Mas o caminho **turnkey e leve** — `cluster create`, que corre
os nós como **containers** (`kindest/node`) — recusava a HA:

```
DX-1000: --control-planes 3 is not supported yet: kubeadm HA needs a stable
endpoint (load-balancer/VIP) in front of the control-planes, which delonix does
not provision yet.
```

A recusa era honesta (vários CP com o `controlPlaneEndpoint` a apontar para UM
deles não é HA, é um ponto único de falha com a aparência de HA), mas a lacuna
era de produto: faltava provisionar o LB.

## Decisão

Em kind-mode, quando `--control-planes > 1`, o Delonix provisiona ele próprio o
endpoint estável — um **container HAProxy** na rede do cluster — e faz o bootstrap
HA do kubeadm. Concretamente (`kindmode::create_ha`):

1. **N containers de control-plane** (`<nome>-control-plane`, `-control-plane2`, …)
   na rede do cluster; NENHUM publica a porta da API.
2. Um **container HAProxy** (`<nome>-external-load-balancer`, imagem
   `haproxy:lts-alpine`) com um `haproxy.cfg` que balanceia TCP `6443` pelos IPs
   dos CP (reutiliza o `lb::build_haproxy_cfg` do caminho VM); publica `api_port`
   para o host. O seu IP na rede do cluster é o `controlPlaneEndpoint`.
3. `kubeadm init --upload-certs` no líder, com `controlPlaneEndpoint = <lb>:6443`
   e o IP do LB nos `certSANs`; a chave de certificados é lida de
   `kubeadm init phase upload-certs`.
4. Os outros CP fazem `kubeadm join` com uma `JoinConfiguration` que leva o bloco
   `controlPlane` (o seu `localAPIEndpoint` + a `certificateKey`) —
   `cp_join_config_yaml`.
5. CNI (kindnet) uma vez; **autorização dos PodCIDRs por nó** (sem isto o
   anti-spoofing do bridge dropa o tráfego de pods cross-node — a mesma classe do
   bloqueio #2B do overlay); kubeconfig exportado (host → LB → apiservers).

É o que o `kind` upstream faz com `kindest/haproxy`; aqui o LB é um container
HAProxy comum, do mesmo `build_haproxy_cfg` que o caminho VM.

## Prova (ao vivo, containers, sem VMs)

`delonix cluster create --name g4k --control-planes 3 --workers 0 --cni default`
→ `cluster "g4k" ready (HA)`. 4 containers (`g4k-external-load-balancer` + 3 CP),
`kubectl` via `127.0.0.1:6443` → o LB → 3/3 Ready, pod `Running` cross-node.
**Failover:** parar o CP líder → a API sobrevive via o LB (routed para cp2/cp3),
continua a **escrever** (configmap, quórum 2/3) e a **agendar** (pod novo em cp2).

## Alternativas consideradas

- **keepalived/VIP partilhado** em vez de um LB dedicado. Rejeitado para kind: os
  nós são containers numa bridge; um container LB é mais simples e espelha o kind.
- **Manter a recusa e remeter para o caminho VM.** Rejeitado: o `cluster kubeadm`
  (VMs) já fazia HA; a lacuna era precisamente o turnkey leve não a fazer.

## Consequências

- `cluster create --control-planes N` (e o manifesto `mode: kind` com
  `controlPlane.count > 1`) passam a dar HA turnkey **sem VMs**. O caminho
  single-CP fica inalterado.
- Dependência nova: a imagem `haproxy:lts-alpine` (puxada on-demand quando há HA).
- A segurança de fecho de rede (PodCIDRs autorizados por porta) é a mesma do
  caminho multi-worker; o LB não filtra (TCP passthrough), a governança fica nos
  nós.
