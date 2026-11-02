#!/usr/bin/env bash
# Bancada de DENSIDADE: como a latência de attach cresce com o número de
# containers no nó. O ponto que a bateria de latência (`bench.sh`) NÃO mede e
# diz em letra miúda que não mede (`docs/comparacao-medida.md`): "duas chamadas
# do caminho de attach (`nft list sets`, `nft -a list chain fwdeny`) são dumps
# que crescem com o número de containers e namespaces, e a bateria correu num nó
# VAZIO". Um número de attach de nó vazio é um número que NÃO se repete em
# produção — este harness mede a curva, não o ponto.
#
# ## O que mede
#
# Para cada PATAMAR de densidade (1, 10, 25, … containers vivos na rede-bancada):
# povoa o nó até ao patamar com containers de fundo, e então cronometra K
# attaches de prova (`run -d` + `rm`) à MESMA rede, publicando p50/p99. Captura
# também, quando consegue entrar no netns do holder, o TAMANHO dos dois dumps
# `nft` em cada patamar — a prova direta de que crescem (ou não) com N.
#
# A saída é a CURVA: latência e tamanho-dos-dumps vs densidade. Uma curva plana
# é a reivindicação de "igual ou superior"; uma curva que sobe linearmente é o
# estrangulamento a confessar-se — e aí o número diz onde otimizar, em vez de se
# otimizar às cegas (que é exatamente o que a cultura deste repo proíbe).
#
# ## Porque herda o juízo de bancada do `bench.sh`
#
# Mesma razão, mesmas palavras: um número de latência colhido numa máquina
# carregada mede a contenção da máquina, não o motor. Este harness SOURCEIA o
# mesmo `bancada.sh` e RECUSA-SE a correr (exit 3) acima do limiar. `--force`
# corre na mesma e marca o resultado NÃO PUBLICÁVEL. E há mais um juízo próprio
# da densidade: medir densidade num nó que JÁ tem containers alheios mistura a
# carga de base na curva, por isso avisa quando encontra containers fora do seu
# prefixo e marca o resultado não publicável sob `--force`.
#
# ## Uso
#
#   scripts/bench_density.sh [--tiers 1,10,25,50,100] [--samples N]
#                            [--image REF] [--force] [--json]
#                            [--bin PATH] [--max-load N] [--keep]
#
# `--json` emite UM objeto em stdout (contrato igual ao `bench.sh`: humano para
# stderr); `bench.density.publishable` diz sempre se o número vale. `--keep`
# deixa os containers de pé para inspeção (implica não publicável).

set -uo pipefail

# O juízo sobre a bancada é PARTILHADO com o `bench.sh`/`chaos.sh` — uma só
# opinião sobre "esta máquina serve para medir?".
# shellcheck source=scripts/bancada.sh
. "$(cd "$(dirname "$0")" && pwd)/bancada.sh"

TIERS="1,10,25,50,100"
SAMPLES=8
IMAGE="busybox:1.36"
FORCE=0
JSON=0
KEEP=0
MAXLOAD=""
BIN="${BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/release/delonix}"

# Prefixo próprio: tudo o que este harness cria começa por aqui, e tudo o que
# começa por aqui é dele para limpar. Nunca toca em nada que não tenha criado.
PFX="benchdens"
NET="${PFX}-net"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tiers) TIERS="$2"; shift 2 ;;
    --samples) SAMPLES="$2"; shift 2 ;;
    --image) IMAGE="$2"; shift 2 ;;
    --force) FORCE=1; shift ;;
    --json) JSON=1; shift ;;
    --keep) KEEP=1; shift ;;
    --bin) BIN="$2"; shift 2 ;;
    --max-load) MAXLOAD="$2"; shift 2 ;;
    *) echo "uso: $0 [--tiers 1,10,25] [--samples N] [--image REF] [--force] [--json] [--bin PATH] [--max-load N] [--keep]"; exit 2 ;;
  esac
done

# stdout real no fd 3; com --json o resto do script fala para stderr, para o
# objeto JSON sair limpo e parseável (mesmo contrato do bench.sh).
if [[ "$JSON" == "1" ]]; then
  exec 3>&1 1>&2
else
  exec 3>&1
fi

if [[ ! -x "$BIN" ]]; then
  echo "binário AUSENTE: $BIN — nada a medir (constrói com 'cargo build --release')." >&2
  exit 2
fi

PUBLISHABLE=1
NOTES=()
mark_unpublishable() { PUBLISHABLE=0; NOTES+=("$1"); }

now_ms() { date +%s%3N; }

# Nomes dos containers (um por linha). `delonix ps` não tem `--format` Go-template
# (gap conhecido), por isso lemos o `-o json` — o contrato estável (ADR-0005).
# `$1` = "-a" para incluir parados, vazio só para vivos.
ps_names() { "$BIN" ps ${1:-} -o json 2>/dev/null | python3 -c 'import json,sys
try: d=json.load(sys.stdin)
except Exception: d=[]
print("\n".join(n for n in (c.get("name","") for c in d) if n))'; }

# p50/p99 de uma lista de inteiros (um por linha), por interpolação do nearest-rank.
pctl() { # $1 = percentil (0-100); stdin = amostras
  local p="$1"
  sort -n | awk -v p="$p" '
    { a[NR]=$1 }
    END {
      if (NR==0) { print "null"; exit }
      r=(p/100.0)*(NR-1)+1; lo=int(r); hi=lo+1; f=r-lo
      if (hi>NR) hi=NR
      printf "%.0f", a[lo]+(a[hi]-a[lo])*f
    }'
}

# ---- limpeza: tudo o que tem o prefixo, e só isso -----------------------------
cleanup() {
  [[ "$KEEP" == "1" ]] && { echo "--keep: deixo $PFX-* de pé (não publicável)." >&2; return; }
  local ids
  ids=$(ps_names -a | grep -E "^${PFX}-" || true)
  if [[ -n "$ids" ]]; then
    echo "$ids" | xargs -r -n1 "$BIN" rm -f >/dev/null 2>&1 || true
  fi
  "$BIN" network rm "$NET" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Quantos containers do NOSSO prefixo estão vivos.
live_count() { ps_names | grep -cE "^${PFX}-" || true; }

# Tamanho (nº de linhas) dos dois dumps nft que a doc diz crescerem com N — o
# sinal DIRETO do estrangulamento. Só mensurável onde a tabela `inet dlxing` é
# visível do host: ROOTFUL (motor como root real). Em ROOTLESS a tabela vive no
# netns do holder, que — por desenho documentado do motor (`infra.rs`: "você
# NÃO pode `nsenter --user --net` do host") — não se entra a partir de uma shell;
# aí devolvemos `null` (NUNCA 0, que se leria como "não cresce"). Um nó dedicado
# que queira esta coluna corre o harness em rootful.
INGRESS_FAMILY="inet"; INGRESS_TABLE="dlxing"
HOST_HAS_INGRESS=0
nft list table "$INGRESS_FAMILY" "$INGRESS_TABLE" >/dev/null 2>&1 && HOST_HAS_INGRESS=1
holder_nft_lines() { # $1 = objeto nft (ex: "list sets" | "-a list chain fwdeny")
  [[ "$HOST_HAS_INGRESS" == "1" ]] || { echo "null"; return; }
  # shellcheck disable=SC2086
  nft $1 "$INGRESS_FAMILY" "$INGRESS_TABLE" 2>/dev/null | grep -c . || echo "null"
}

# ---- bancada ------------------------------------------------------------------
bancada_medir "$MAXLOAD"
echo "=== bench_density — Delonix ===" >&2
echo "bin:        $BIN" >&2
echo "versão:     $("$BIN" --version 2>/dev/null | head -1)" >&2
echo "kernel:     $(uname -sr)" >&2
echo "ncpu:       $BANCADA_NCPU   load1: $BANCADA_LOAD1   limiar: $BANCADA_THRESHOLD" >&2
echo "imagem:     $IMAGE" >&2
echo "patamares:  $TIERS   amostras/patamar: $SAMPLES" >&2

if [[ "$BANCADA_OK" != "1" ]]; then
  if [[ "$FORCE" != "1" ]]; then
    bancada_recusa "bench_density.sh" \
      "A curva latência-vs-densidade mediria a contenção da máquina, não o motor."
  fi
  mark_unpublishable "load $BANCADA_LOAD1 acima do limiar $BANCADA_THRESHOLD (--force)"
fi

# Containers alheios (fora do nosso prefixo) sujam a curva: a carga de base
# deles entra na latência medida. Avisa e marca não publicável.
foreign=$(ps_names | grep -vcE "^${PFX}-" || true); foreign=$((foreign + 0))
if [[ "$foreign" -gt 0 ]]; then
  echo "AVISO: $foreign container(s) alheios vivos — a base deles entra na curva." >&2
  mark_unpublishable "$foreign container(s) alheios no nó durante a medição"
fi

if ! "$BIN" image ls 2>/dev/null | grep -qF "$IMAGE"; then
  echo "imagem $IMAGE ausente — puxa-a primeiro ('$BIN image pull $IMAGE')." >&2
  exit 2
fi

"$BIN" network rm "$NET" >/dev/null 2>&1 || true
"$BIN" network create "$NET" >/dev/null 2>&1 || { echo "falha a criar a rede $NET" >&2; exit 1; }

IFS=',' read -r -a TIER_ARR <<< "$TIERS"
ROWS_JSON=()
bg_n=0   # quantos containers de fundo já criámos

printf '\n%-8s %-8s %-10s %-10s %-12s %-14s\n' "alvo" "vivos" "p50(ms)" "p99(ms)" "nft_sets" "nft_fwdeny" >&2
printf -- '--------------------------------------------------------------------\n' >&2

for tier in "${TIER_ARR[@]}"; do
  # Povoa até ao patamar com containers de FUNDO que ficam de pé.
  while [[ "$bg_n" -lt "$tier" ]]; do
    bg_n=$((bg_n+1))
    "$BIN" run -d --net "$NET" --name "${PFX}-bg-$(printf '%04d' "$bg_n")" \
      "$IMAGE" sleep 86400 >/dev/null 2>&1 \
      || { echo "falha a arrancar container de fundo #$bg_n — paro a rampa aqui." >&2; mark_unpublishable "rampa truncada no patamar $tier"; break 2; }
  done

  live=$(live_count)

  # K attaches de PROVA à mesma rede, cronometrados individualmente.
  samples=""
  for k in $(seq 1 "$SAMPLES"); do
    pname="${PFX}-probe-$(printf '%04d' "$k")"
    t0=$(now_ms)
    if "$BIN" run -d --net "$NET" --name "$pname" "$IMAGE" sleep 86400 >/dev/null 2>&1; then
      t1=$(now_ms)
      samples+="$((t1-t0))"$'\n'
    fi
    "$BIN" rm -f "$pname" >/dev/null 2>&1 || true
  done

  p50=$(printf '%s' "$samples" | pctl 50)
  p99=$(printf '%s' "$samples" | pctl 99)
  nft_sets=$(holder_nft_lines "list sets")
  nft_fwdeny=$(holder_nft_lines "-a list chain fwdeny")

  printf '%-8s %-8s %-10s %-10s %-12s %-14s\n' "$tier" "$live" "$p50" "$p99" "$nft_sets" "$nft_fwdeny" >&2
  ROWS_JSON+=("{\"target\":$tier,\"live\":$live,\"attach_p50_ms\":$p50,\"attach_p99_ms\":$p99,\"nft_sets_lines\":$nft_sets,\"nft_fwdeny_lines\":$nft_fwdeny,\"samples\":$(printf '%s' "$samples" | grep -c .)}")
done

# ---- saída --------------------------------------------------------------------
notes_json="[]"
if [[ ${#NOTES[@]} -gt 0 ]]; then
  notes_json=$(printf '%s\n' "${NOTES[@]}" | awk 'BEGIN{printf "["} {printf "%s\"%s\"", (NR>1?",":""), $0} END{printf "]"}')
fi
rows_joined=$(IFS=,; echo "${ROWS_JSON[*]}")

if [[ "$JSON" == "1" ]]; then
  cat >&3 <<JSON
{"bench":{"kind":"density","tool":"delonix","version":"$("$BIN" --version 2>/dev/null | head -1)","kernel":"$(uname -sr)","ncpu":$BANCADA_NCPU,"load1":$BANCADA_LOAD1,"threshold":$BANCADA_THRESHOLD,"image":"$IMAGE","samples_per_tier":$SAMPLES,"publishable":$([[ $PUBLISHABLE == 1 ]] && echo true || echo false),"notes":$notes_json},"density":{"curve":[$rows_joined]}}
JSON
fi

echo >&2
if [[ "$PUBLISHABLE" == "1" ]]; then
  echo "PUBLICÁVEL: a curva acima é uma medição válida desta máquina." >&2
else
  echo "NÃO PUBLICÁVEL: ${NOTES[*]}" >&2
  echo "Para uma curva publicável: um nó OCIOSO e DEDICADO, sem containers alheios." >&2
fi
