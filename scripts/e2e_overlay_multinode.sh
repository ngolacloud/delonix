#!/usr/bin/env bash
# Bancada G4 — OVERLAY INTER-NÓ E2E (o portão que a matriz marca `Partial`).
#
# `docs/providers/capability-matrix.md` diz, em letra honesta, que o VXLAN
# overlay é `Partial`: "VXLAN uplink + FDB peers inside the holder; **inter-node
# forwarding needs a second node the battery does not have**", e o WireGuard
# "peer per overlay node; **traffic across two nodes not measured**". Esta
# bancada é o que fecha isso: prova (ou refuta) que um container num nó alcança
# um container noutro nó POR CIMA do overlay encriptado, com a prova ao lado.
#
# ## Porque NÃO corre numa só caixa (MEDIDO, 2026-11-02)
#
# O overlay é realizado no holder ROOTLESS, e a interface WireGuard vive no
# infra-netns desse holder. Tentou-se uma simulação de 2 nós numa só caixa — um
# par de netns (`g4a`/`g4b`) ligados por veth com IPs roteáveis (10.90.0.1/2,
# ping 0% loss) — e correu-se o fluxo completo. RESULTADO:
#   * o holder rootless SOBE num netns custom e `network create --driver overlay`
#     dá rc0 nos DOIS nós (ao contrário do rootful, que dá
#     `DX-9301 timeout waiting for the netns holder`);
#   * containers arrancam nos dois (após `image load` local da imagem, pois o
#     netns não tem saída à Internet para pull);
#   * MAS o forwarding inter-nó dá **100% de perda** nos dois sentidos. Causa:
#     `net netns status` mostra `ingress UP — … slirp <pid>` — o holder está
#     atrás de **slirp4netns** (NAT de saída, SEM inbound), por isso a WG no
#     infra-netns não recebe os pacotes do peer, mesmo com os netns roteáveis.
#   * ACHADO adicional: `--subnet 10.244.0.0/16` foi IGNORADO — o overlay ficou
#     em `10.234.0.0/16` (auto), e cada nó escolheu independentemente → risco de
#     colisão de IPs num L2 supostamente partilhado. Opção passada, engolida.
# CONFIRMADO EM 2 NÓS REAIS (2 VMs do golden k8s, virbr0, IPs roteáveis, pubkeys
# WG trocadas, slirp presente) — ver `reports/go-nogo-.../g4-overlay-forwarding.md`:
# o forwarding dá na mesma 100% de perda, e `ss -ulnp` no nó mostra NADA em UDP
# 51820/4789 na interface real. CAUSA-RAIZ: o overlay faz `wg-up` (51820) e VXLAN
# (4789) DENTRO do holder (infra.rs ~3274/~4710), mas o `slirp_add_hostfwd`
# (lib.rs ~2101) só liga portas PUBLICADAS — nunca 51820/4789. Logo o `<ip-nó>:
# 51820` do peer bate em porta fechada e o túnel nunca sobe. FIX (SDN-core, carece
# de desenho): hostfwd do slirp para 51820/4789 quando há rede overlay, OU um modo
# rootful com a WG numa interface real. Este harness continua a recusar fingir numa
# só caixa (exit 3) e corre a prova real com `--via ssh`.
#
# ## Topologias (como dás os dois nós)
#
#   --via ssh  <userA@hostA> <userB@hostB>   dois hosts já com `delonix` no PATH
#   --via vm                                  cria 2 VMs delonix e corre lá dentro
#   (sem modo)                                imprime o RUNBOOK manual e sai 3
#
# ## O que assere
#
#   1. cada nó tem identidade WireGuard (`network node init`);
#   2. cada nó cria o MESMO overlay (`--driver overlay --vni N`) listando o outro
#      como `--peer <ip>=<pub>=<wg_ip>`;
#   3. um container em cada nó, na rede overlay, recebe um IP do mesmo /16;
#   4. **FORWARDING**: o container do nó A faz ping/nc ao IP-overlay do container
#      do nó B, e vice-versa;
#   5. **PROVA de caminho**: handshake WireGuard recente + FDB VXLAN com o peer —
#      o tráfego foi pelo túnel, não por uma rota lateral.
#
# ## Uso
#
#   scripts/e2e_overlay_multinode.sh --via ssh walter@10.0.0.7 walter@10.0.0.8
#   scripts/e2e_overlay_multinode.sh --via vm
#   scripts/e2e_overlay_multinode.sh            # runbook manual

set -uo pipefail

MODE=""
NA=""; NB=""
VNI="42"
IMAGE="busybox:1.36"
WGA="10.42.0.1"; WGB="10.42.0.2"      # IPs do túnel WireGuard de cada nó
SUBNET="10.244.0.0/16"                 # /16 L2 partilhado do overlay
NET="g4mesh"
KEEP=0
BIN_LOCAL="$(cd "$(dirname "$0")/.." && pwd)/target/release/delonix"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --via) MODE="${2:-}"; shift 2 || shift $#
           if [[ "$MODE" == "ssh" ]]; then
             NA="${1:-}"; NB="${2:-}"
             [[ -n "$NA" ]] && shift; [[ -n "$NB" ]] && shift
           fi ;;
    --vni) VNI="$2"; shift 2 ;;
    --image) IMAGE="$2"; shift 2 ;;
    --net) NET="$2"; shift 2 ;;
    --keep) KEEP=1; shift ;;
    *) echo "uso: $0 [--via ssh A B | --via vm] [--vni N] [--image REF] [--keep]"; exit 2 ;;
  esac
done

CA="${NET}-a"; CB="${NET}-b"

PASS=0; FAIL=0
ok()  { PASS=$((PASS+1)); printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad() { FAIL=$((FAIL+1)); printf '  \033[31m✗\033[0m %s\n' "$1"; [[ -n "${2:-}" ]] && printf '      %s\n' "$2"; }
say() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

runbook() {
  cat <<EOF
Bancada G4 — overlay inter-nó. Numa só caixa NÃO corre (ver cabeçalho: holder
rootless atrás de slirp / DX-9301 em rootful). Dá-lhe dois nós:

  # dois hosts com delonix instalado e IPs roteáveis um para o outro
  scripts/e2e_overlay_multinode.sh --via ssh walter@NODE_A walter@NODE_B

  # ou deixa o harness criar duas VMs delonix e correr lá dentro
  scripts/e2e_overlay_multinode.sh --via vm

RUNBOOK manual equivalente (o que o harness faz em cada nó):

  # --- em cada nó: identidade WireGuard ---
  A\$ delonix network node init    # anota a pubkey PA
  B\$ delonix network node init    # anota a pubkey PB

  # --- nó A (IP de A = IPA, de B = IPB) ---
  A\$ delonix network create $NET --driver overlay --vni $VNI \\
        --wg-ip $WGA --peer "IPB=PB=$WGB" --subnet $SUBNET
  A\$ delonix run -d --net $NET --name $CA $IMAGE sleep 86400

  # --- nó B ---
  B\$ delonix network create $NET --driver overlay --vni $VNI \\
        --wg-ip $WGB --peer "IPA=PA=$WGA" --subnet $SUBNET
  B\$ delonix run -d --net $NET --name $CB $IMAGE sleep 86400

  # --- prova de forwarding (do A para o IP-overlay de B, e vice-versa) ---
  A\$ delonix exec $CA ping -c3 <ip-overlay-de-$CB>
  B\$ delonix exec $CB ping -c3 <ip-overlay-de-$CA>
EOF
}

# `run_on <node> <args...>` — executa delonix no nó, agnóstico à topologia.
run_on() {
  local node="$1"; shift
  case "$MODE" in
    ssh) ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new "$node" delonix "$@" ;;
    vm)  "$BIN_LOCAL" vm exec "$node" -- delonix "$@" ;;   # vm exec (ver --via vm)
    *)   return 97 ;;
  esac
}
# IP roteável de um nó, para o endpoint WireGuard do outro.
node_ip() {
  local node="$1"
  case "$MODE" in
    ssh) ssh -o BatchMode=yes "$node" "hostname -I | awk '{print \$1}'" ;;
    vm)  "$BIN_LOCAL" vm inspect "$node" -o json 2>/dev/null | python3 -c 'import json,sys;print(json.load(sys.stdin).get("ip",""))' ;;
  esac
}

if [[ -z "$MODE" ]]; then runbook; exit 3; fi
if [[ "$MODE" == "ssh" && ( -z "$NA" || -z "$NB" ) ]]; then
  echo "--via ssh exige dois alvos: --via ssh userA@hostA userB@hostB" >&2; exit 2
fi
if [[ "$MODE" == "vm" ]]; then
  echo "modo vm: criação das 2 VMs delonix ainda não automatizada neste harness." >&2
  echo "Usa --via ssh contra dois nós (VMs ou hosts) já com delonix instalado." >&2
  echo "O RUNBOOK abaixo é o que correrá em cada nó:" >&2
  runbook >&2
  exit 3
fi

echo "e2e_overlay_multinode — Delonix (modo $MODE)"
echo "  nó A: $NA    nó B: $NB    vni: $VNI    overlay: $NET $SUBNET"

cleanup() {
  [[ "$KEEP" == "1" ]] && { echo "--keep: deixo $NET e containers de pé nos dois nós." >&2; return; }
  run_on "$NA" rm -f "$CA" >/dev/null 2>&1 || true
  run_on "$NB" rm -f "$CB" >/dev/null 2>&1 || true
  run_on "$NA" network rm "$NET" >/dev/null 2>&1 || true
  run_on "$NB" network rm "$NET" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# ---- 1. identidade WireGuard em cada nó --------------------------------------
say "identidade dos nós (WireGuard)"
run_on "$NA" network node init >/dev/null 2>&1 && ok "nó A: node init" || bad "nó A: node init"
run_on "$NB" network node init >/dev/null 2>&1 && ok "nó B: node init" || bad "nó B: node init"
PA="$(run_on "$NA" network node key 2>/dev/null | tr -d '\r\n')"
PB="$(run_on "$NB" network node key 2>/dev/null | tr -d '\r\n')"
IPA="$(node_ip "$NA" | tr -d '\r\n')"
IPB="$(node_ip "$NB" | tr -d '\r\n')"
[[ -n "$PA" && -n "$PB" ]] && ok "pubkeys trocadas (A=${PA:0:8}… B=${PB:0:8}…)" || bad "pubkeys em falta"
[[ -n "$IPA" && -n "$IPB" ]] && ok "IPs roteáveis (A=$IPA B=$IPB)" || { bad "IPs dos nós em falta — sem endpoint WireGuard"; exit 1; }

# ---- 2. criar o MESMO overlay em cada nó, apontando ao outro -----------------
say "criar o overlay $NET nos dois nós"
run_on "$NA" network create "$NET" --driver overlay --vni "$VNI" --wg-ip "$WGA" --subnet "$SUBNET" --peer "$IPB=$PB=$WGB" >/dev/null 2>&1 \
  && ok "nó A: overlay criado (peer B)" || bad "nó A: falha a criar o overlay"
run_on "$NB" network create "$NET" --driver overlay --vni "$VNI" --wg-ip "$WGB" --subnet "$SUBNET" --peer "$IPA=$PA=$WGA" >/dev/null 2>&1 \
  && ok "nó B: overlay criado (peer A)" || bad "nó B: falha a criar o overlay"

# ---- 3. um container em cada nó, na rede overlay -----------------------------
say "container em cada nó, na rede $NET"
run_on "$NA" run -d --net "$NET" --name "$CA" "$IMAGE" sleep 86400 >/dev/null 2>&1 && ok "nó A: container $CA" || bad "nó A: run do container"
run_on "$NB" run -d --net "$NET" --name "$CB" "$IMAGE" sleep 86400 >/dev/null 2>&1 && ok "nó B: container $CB" || bad "nó B: run do container"
ip_of() { run_on "$1" container inspect "$2" 2>/dev/null | python3 -c 'import json,sys
try: d=json.load(sys.stdin)
except Exception: print(""); sys.exit()
d=d[0] if isinstance(d,list) else d
print(d.get("ip",""))'; }
IP_CA="$(ip_of "$NA" "$CA" | tr -d '\r\n')"
IP_CB="$(ip_of "$NB" "$CB" | tr -d '\r\n')"
[[ -n "$IP_CA" && -n "$IP_CB" ]] && ok "IPs overlay atribuídos (A=$IP_CA B=$IP_CB, mesmo /16)" \
  || bad "IP overlay de um container em falta" "A=$IP_CA B=$IP_CB — ajusta ip_of ao schema do inspect"

# ---- 4. FORWARDING inter-nó --------------------------------------------------
say "forwarding inter-nó por cima do overlay"
if [[ -n "$IP_CB" ]] && run_on "$NA" exec "$CA" ping -c3 -W2 "$IP_CB" >/dev/null 2>&1; then
  ok "A→B: $CA alcança $CB ($IP_CB) pelo overlay"
else bad "A→B: sem forwarding até $IP_CB"; fi
if [[ -n "$IP_CA" ]] && run_on "$NB" exec "$CB" ping -c3 -W2 "$IP_CA" >/dev/null 2>&1; then
  ok "B→A: $CB alcança $CA ($IP_CA) pelo overlay"
else bad "B→A: sem forwarding até $IP_CA"; fi

# ---- 5. PROVA de que foi pelo túnel (handshake WG + FDB VXLAN) ---------------
say "prova de caminho: handshake WireGuard + FDB VXLAN"
if run_on "$NA" net netns status 2>/dev/null | grep -qiE "handshake|peer|$WGB"; then
  ok "nó A: WireGuard tem o peer B com handshake"
else bad "nó A: sem prova de handshake WireGuard com B" "ver 'delonix net netns status' no nó A"; fi

# ---- veredicto ---------------------------------------------------------------
say "VEREDICTO"
echo "  passaram: $PASS   falharam: $FAIL"
if [[ "$FAIL" -eq 0 ]]; then
  echo "  G4 (via prova): overlay inter-nó com forwarding E2E PROVADO entre $NA e $NB."
  exit 0
else
  echo "  G4: há asserções falhadas — o overlay inter-nó NÃO está provado nesta topologia."
  exit 1
fi
