# ADR-0084 — O overlay rootless encaminha a sua WireGuard para o holder (hostfwd)

**Estado:** Proposto (2026-11-02) — fecha o bloqueio #1 do G4 (caminho "via prova")
do plano NO-GO→GO. Implementado em `feat/ipv6-gonogo` e **provado em 2 nós reais**
(2 VMs do golden k8s): **ANTES** do fix, nada escutava em 51820 e o handshake
WireGuard NUNCA acontecia (0 bytes, 100 % de perda); **DEPOIS**, o `ss` mostra
`slirp4netns` em `0.0.0.0:51820`, o handshake **completa** nos dois nós
(`latest handshake: …`) e o túnel **transfere dados nos dois sentidos**
(node-a 10,27 KiB rx / 7,23 KiB tx; node-b 7,99 KiB rx / 10,06 KiB tx), com o
VXLAN a ser encapsulado e enviado pela WG (`10.42.0.1 ↔ 10.42.0.2:4789`). Ou
seja: o **transporte WireGuard+VXLAN inter-nó** passa a funcionar — era o bloqueio
que este ADR fecha. O ping container-a-container end-to-end AINDA falha numa
camada ACIMA (ARP não resolve através do overlay), por causa do bloqueio #2 (o
driver overlay ignora `--subnet` → os dois nós escolhem `10.234.0.0/16`
independentemente e ambos os bridges ficam em `10.234.0.1`, sem coordenação do L2)
— tratado à parte, não é o transporte. Aguarda GO do dono.
**Relaciona:** `reports/go-nogo-alta-criticidade-20261101/g4-overlay-forwarding.md`
(a medição que motivou), ADR-0013 (rotas entre redes), ADR-0082 (dual-stack IPv6).

## Contexto

A bancada do G4 (`scripts/e2e_overlay_multinode.sh`) correu contra dois nós REAIS
(2 VMs, `virbr0`, IPs roteáveis, pubkeys WireGuard trocadas) e mediu: o
`network create --driver overlay` cria nos dois nós, os containers sobem, **mas o
forwarding inter-nó dá 100% de perda**.

Causa-raiz cravada no nó e no código. O overlay encriptado realiza-se DENTRO do
holder rootless: a interface WireGuard (`wg-up`, porta 51820) e o uplink VXLAN
(porta 4789, com o FDB a apontar ao `wg_ip` do peer — logo o VXLAN **cavalga** o
túnel WireGuard) vivem no netns do holder. Esse netns alcança o exterior só por
**slirp4netns**, que faz NAT de saída e **não encaminha inbound**. Resultado: no
netns PRINCIPAL do nó, `ss -ulnp` não mostra nada em UDP 51820 — o handshake
WireGuard que o peer envia para `<ip-do-nó>:51820` chega à máquina e bate em
**porta fechada**. Sem túnel, não há VXLAN nem forwarding.

O motor já tem a peça que falta: `slirp_add_hostfwd` encaminha `<host>:porta →
<holder>:porta`, usado para publicar portas de containers — mas nunca para a
porta da WireGuard do overlay.

## Decisão

Ao realizar um overlay encriptado (`realize_overlay`), a seguir a subir a
WireGuard e os peers, **adicionar um hostfwd no slirp para UDP 51820**, para que
`<ip-do-nó>:51820` entre no holder onde a WireGuard escuta. Só esta porta: o
VXLAN (4789) viaja DENTRO do túnel e não precisa de reachability direta.

Três escolhas, e porquê:

- **Ligado a `0.0.0.0`**, não ao loopback (ao contrário de uma porta publicada,
  que por omissão só liga a `127.0.0.1`). Um overlay é inter-nó por definição e o
  peer liga ao IP real do nó; o túnel é autenticado+encriptado pela WireGuard, por
  isso expor o UDP é o objetivo, não um risco acrescentado.
- **Idempotente e à escala do nó.** Há UMA interface WireGuard por nó (uma só
  `node.key`), partilhada por todos os overlays. O segundo overlay encontra 51820
  já ligada (o slirp recusa um duplicado no mesmo endereço) e salta — a guarda de
  porta-ocupada transforma isso em sucesso, em vez do `add_hostfwd failed` opaco do
  slirp (que não distingue duplicado de falha real).
- **Sem teardown explícito.** O hostfwd pertence ao processo slirp e morre com ele
  quando o holder é desmontado (`net netns down` / última rede removida), por isso
  nunca sobrevive aos overlays que serve. Não há porta pendurada entre reboots.

## Alternativas consideradas

- **Overlay ROOTFUL com a WireGuard numa interface real do nó** (sem slirp).
  Elimina o problema na raiz, mas é outro modelo de execução e privilégio; fica
  para um ADR sucessor se o rootful multi-nó passar a objetivo.
- **Não encaminhar e documentar o overlay como single-host.** Rejeitado: contradiz
  a decisão de âmbito do G4 (o Delonix quer SER HA) e o `--driver overlay` já
  anuncia peers inter-nó.

## Consequências

- O **transporte WireGuard+VXLAN** do overlay passa a funcionar entre nós (handshake
  completa, dados fluem, VXLAN viaja no túnel) — o bloqueio #1 do G4 fecha, provado
  em 2 VMs reais. O ping container-a-container end-to-end AINDA não passa:
  - **#2A (`--subnet` ignorado) — RESOLVIDO à parte** (`create_overlay` honra o
    subnet; os dois nós passam a aterrar no mesmo `/16` e gateway). Necessário,
    mas NÃO suficiente: com os dois nós já no mesmo `10.244.0.0/16`, a ARP continua
    sem resolver.
  - **#2B (decapsulação VXLAN no peer) — EM ABERTO.** Medido em 2 VMs: o frame
    VXLAN chega DECIFRADO ao `wgN` do peer (a ARP é visível), mas o device VXLAN
    não o decapsula (`RX=0`, `IpInDelivers` quase nulo). NÃO é firewall (policy
    accept, sem drops), NÃO é o `--subnet`, e NÃO é a amarração ao underlay:
    criar o VXLAN com `local <wg_ip> dev <wgN>` (tentado e revertido) deixa o
    `RX=0` na mesma. A causa (entrega local do pacote interno sobre a WireGuard)
    fica por identificar — ver `g4-overlay-forwarding.md`.
- 51820/udp fica exposto em todas as interfaces do nó enquanto houver um overlay.
  É o contrato de um nó de overlay (como o `:51820` de qualquer WireGuard), e o
  túnel recusa quem não tenha a chave.
- A prova é re-corrível: `scripts/e2e_overlay_multinode.sh --via ssh A B`.
