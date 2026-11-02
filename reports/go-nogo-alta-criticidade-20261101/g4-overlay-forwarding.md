# G4 — overlay inter-nó: prova em 2 nós reais + causa-raiz

> **Data:** 2026-11-02. **Âmbito:** o caminho "via prova" do G4 (decidido pelo
> dono: o Delonix quer SER HA). Pergunta: o `--driver overlay` (VXLAN+WireGuard)
> encaminha tráfego entre dois nós? **Medido**, não afirmado.

## Montagem

Duas VMs reais do golden `delonix-vm-k8s:1.36` (ubuntu 24.04), no `virbr0`
libvirt, **IPs roteáveis um para o outro** (`192.168.122.175` ↔ `.210`, ping
host-a-host 0% loss). Em cada VM corri o binário **0.1.0 da linha atual** (copiado
por scp; `delonix --version` = 0.1.0 nos dois), com `DELONIX_ROOT=/tmp/dlxr`
isolado. Dependências que o golden não trazia e foram instaladas: `wireguard-tools`
(`wg`), `slirp4netns`; e `sysctl kernel.apparmor_restrict_unprivileged_userns=0`
(o binário em `/tmp` não tem o perfil AppArmor que o `install.sh` escreve).

Fluxo: `network node init` nos dois → troca de pubkeys → `network create mesh
--driver overlay --vni 42 --wg-ip … --peer <ip>=<pub>=<wgip>` nos dois (rc0,
peer com a pubkey correta confirmada no `network inspect`) → `run -d --net mesh`
um busybox em cada → ping entre os IPs-overlay.

## Resultado: NÃO encaminha

| Sentido | Resultado |
|---|---|
| container A → container B (IP overlay) | **100% packet loss** |
| container B → container A | **100% packet loss** |

Idêntico ao que a simulação de 2 netns já tinha dado. Dois nós REAIS roteáveis,
pubkeys trocadas, slirp presente — e mesmo assim zero forwarding.

## Causa-raiz (cravada no nó e no código)

No nó, `net netns status` mostra `ingress UP — … slirp <pid>`: o holder rootless
está atrás de **slirp4netns**. E no netns PRINCIPAL da VM:

```
$ ss -ulnp | grep -E '51820|4789'
(nada)
```

**Nada escuta em UDP 51820 (WireGuard) nem 4789 (VXLAN) na interface real do nó.**
A WG e o VXLAN são configurados DENTRO do netns do holder (infra.rs: `wg-up`
porta 51820 na ~3274, VXLAN porta 4789 na ~4710), mas o `slirp_add_hostfwd`
(lib.rs ~2101) só é usado para **portas publicadas de containers** — nunca para
51820/4789. Logo o handshake WireGuard que o peer envia para `<ip-do-nó>:51820`
chega à VM e bate em **porta fechada**: o túnel nunca estabelece, e sem túnel não
há VXLAN nem forwarding.

Isto explica — e confirma com dado — o `Partial` honesto da matriz
(`provider_report.rs:113`: "inter-node forwarding needs a second node the battery
does not have"; "traffic across two nodes not measured"). Agora foi medido: **não
encaminha**, e sabe-se exatamente porquê.

## Achado secundário: `--subnet` ignorado (bug)

`network create --driver overlay --subnet 10.244.0.0/16` → o overlay ficou em
**`10.234.0.0/16`** (auto), nos dois nós, cada um a escolher independentemente.
Para um L2 que é suposto ser partilhado entre nós, (a) a opção foi engolida em
silêncio (a classe de bug que o repo persegue) e (b) nada coordena o /16 entre
nós → dois nós podem atribuir o mesmo IP. Reproduzido na sim netns E nas VMs.

## O que isto significa para o G4

O overlay rootless, como está, **não serve para HA multi-nó**: o data-plane
não atravessa os nós. Dois bloqueios a fechar antes de "via prova" poder passar:

1. **Encaminhar o overlay para o holder.** Quando existe uma rede `overlay`,
   adicionar hostfwds no slirp para **UDP 51820 (WG)** e **UDP 4789 (VXLAN)**
   (o `slirp_add_hostfwd` já existe; falta ligá-lo a estas portas), para que o
   `<ip-do-nó>:51820/4789` entre no holder. É SDN-core com implicações de
   firewall/segurança (em que endereço ligar, idempotência, teardown, interação
   com o `fwdeny`/governança) — merece desenho/ADR, não um patch às cegas.
2. **Honrar `--subnet`** no driver overlay + coordenar o /16 entre nós.

Alternativa a (1): um modo de overlay ROOTFUL onde a WG/VXLAN vivem numa
interface real do nó (sem slirp). Também decisão de desenho.

## Fix do bloqueio #1 (ADR-0084) — medido DEPOIS, nas mesmas VMs

`realize_overlay` passa a adicionar um hostfwd no slirp para UDP 51820 ligado a
`0.0.0.0` (`infra::set_overlay_wg_hostfwd`). Re-corrido nas 2 VMs com o binário
que leva o fix:

| | ANTES | DEPOIS |
|---|---|---|
| `ss -ulnp` no nó, UDP 51820 | nada | `slirp4netns … 0.0.0.0:51820` (nos dois nós) |
| handshake WireGuard | nunca | **completa** nos dois (`wg show`: `latest handshake: …`) |
| transferência no túnel | 0 | node-a 10,27 KiB rx / 7,23 KiB tx; node-b 7,99 / 10,06 |
| VXLAN no túnel | — | `10.42.0.1 ↔ 10.42.0.2:4789` (ARP do container encapsulada e enviada) |

Ou seja: o **transporte WireGuard+VXLAN inter-nó funciona** — era exatamente o que
não acontecia. O ping container-a-container end-to-end **ainda falha**: a ARP é
encapsulada e viaja no túnel, mas não resolve através do overlay. Causa provável,
o **bloqueio #2**: o `--subnet` ignorado faz os dois nós escolherem
`10.234.0.0/16` independentemente e ambos os bridges ficam em `10.234.0.1`, sem
coordenação do L2/gateway entre nós (IPAM por-nó, não por-overlay). É o próximo
fix, à parte deste.

## Reprodutível

`scripts/e2e_overlay_multinode.sh --via ssh <userA@nodeA> <userB@nodeB>` corre
este fluxo e as asserções contra quaisquer dois nós. O cabeçalho do script traz
o procedimento e estes achados.
