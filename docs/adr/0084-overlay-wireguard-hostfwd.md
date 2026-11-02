# ADR-0084 — O overlay rootless encaminha a sua WireGuard para o holder (hostfwd)

**Estado:** Proposto (2026-11-02) — o caminho "via prova" do G4 (overlay inter-nó),
**PROVADO end-to-end em 2 VMs reais do golden k8s**: container-a-container **0 %
de perda nos dois sentidos** (~0,7 ms), com a VXLAN a decapsular no peer
(`VXLAN RX > 0`). Tirou TRÊS fixes, cada um necessário e todos medidos: **#1** o
hostfwd da WireGuard deste ADR (sem ele nada escutava em 51820, handshake nunca
subia); **#2A** o `create_overlay` honrar `--subnet` (os dois nós no mesmo
`/16`+gateway); **#2B** o firewall do holder (`dlxinput`) aceitar a VXLAN de
entrada (`iifname "wgo*" udp dport 4789 accept`) — sem isso o frame chegava
decifrado ao `wgN` mas o `ct state new drop` terminal comia-o (`VXLAN RX=0`).
Implementado em `feat/ipv6-gonogo`. Aguarda GO do dono.
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

- O overlay inter-nó passa a **encaminhar container-a-container end-to-end** — o
  caminho "via prova" do G4 fica PROVADO em 2 VMs reais (ping 0% perda nos dois
  sentidos, ~0,7 ms). Tirou TRÊS fixes, cada um necessário:
  - **#1 — hostfwd da WireGuard (este ADR):** sem ele nada escutava em 51820 e o
    túnel nunca subia.
  - **#2A — `--subnet` honrado** (`create_overlay`, à parte): os dois nós aterram
    no mesmo `/16`+gateway. Necessário mas não suficiente.
  - **#2B — o firewall do holder aceita a VXLAN de entrada.** A causa-raiz, cravada
    por conntrack+nstat: o frame VXLAN chegava DECIFRADO ao `wgN` do peer mas o
    device não decapsulava (`RX=0`) porque a chain `dlxinput` tem um
    `ct state new drop` terminal e a VXLAN de entrada (`iifname wgo*`, udp 4789) é
    uma conexão NEW que caía lá. Fix: `dlxinput` passa a aceitar
    `iifname "wgo*" udp dport 4789 accept` (a mesma disciplina de allowlist das
    outras regras — o WireGuard já autenticou o peer). Depois do fix, `VXLAN RX`
    sobe e o ping passa. NÃO era o `--subnet` nem a amarração `local/dev` (essa
    foi tentada e revertida). Ver `g4-overlay-forwarding.md`.
- 51820/udp fica exposto em todas as interfaces do nó enquanto houver um overlay.
  É o contrato de um nó de overlay (como o `:51820` de qualquer WireGuard), e o
  túnel recusa quem não tenha a chave.
- A prova é re-corrível: `scripts/e2e_overlay_multinode.sh --via ssh A B`.
