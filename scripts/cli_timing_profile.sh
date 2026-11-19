#!/usr/bin/env bash
# Mede o TEMPO DE EXECUÇÃO de `delonix <folha> --help` por folha da CLI, agrupado
# pelo primeiro token da folha (container, vm, net, image, ...).
#
# Porquê `--help` e não a folha real: `--help` é SEMPRE seguro — clap
# curto-circuita antes do comando correr, não toca em estado nem backend —, logo
# o número isola o custo de arranque+parse de CADA folha sem precisar de root nem
# de daemon. É o companheiro de execução do inventário que `cli-tree.sh` mantém.
#
# Emite:
#   reports/e2e/<versão>/timing-results.jsonl   (uma linha por folha + arranque)
#   um resumo agregado por grupo para stdout
#
# Estatística por folha, sobre RUNS repetições (default 20): min/p50/p95/p99/
# max/mean em ms. `performance-slo.md` exige >=20 amostras para uma afirmação de
# latência; RUNS<20 fica como informativo (marcado no JSONL com "informational").
#
# Variáveis de ambiente:
#   DELONIX_BIN    binário a medir (default target/release/delonix do repo)
#   RUNS           repetições por folha (default 20)
#   LEAVES_LIMIT   mede só as primeiras N folhas (smoke-test do próprio arnês)
#   VERSION        subpasta de reports/e2e (default 0.1.3)
#
# Sem dependências exóticas: coreutils (date, sort, mkdir) + awk.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
BIN=${DELONIX_BIN:-"$repo/target/release/delonix"}
[ -n "$BIN" ] && [ -x "$BIN" ] || {
  echo "sem binário executável em '$BIN' — aponta DELONIX_BIN" >&2
  exit 1
}

RUNS=${RUNS:-20}
VERSION=${VERSION:-0.1.3}
LEAVES_LIMIT=${LEAVES_LIMIT:-0}
OUT="$repo/reports/e2e/$VERSION"
JSONL="$OUT/timing-results.jsonl"
mkdir -p "$OUT"
: > "$JSONL"   # idempotente: trunca a cada corrida

# Converte amostras em nanossegundos (uma por linha em stdin) para a tabela de
# percentis em MILISSEGUNDOS. Percentil por rank-mais-próximo (ceil), sem
# dependência de gawk (ordena com `sort -n` de coreutils).
stats() {
  sort -n | awk '
    { a[NR]=$1; sum+=$1 }
    END {
      n=NR
      if (n==0) { print "0 0 0 0 0 0"; exit }
      i50=int((50*n+99)/100); if(i50<1)i50=1; if(i50>n)i50=n
      i95=int((95*n+99)/100); if(i95<1)i95=1; if(i95>n)i95=n
      i99=int((99*n+99)/100); if(i99<1)i99=1; if(i99>n)i99=n
      printf "%.3f %.3f %.3f %.3f %.3f %.3f\n",
        a[1]/1e6, a[i50]/1e6, a[i95]/1e6, a[i99]/1e6, a[n]/1e6, (sum/n)/1e6
    }'
}

# escapa uma string para um literal JSON (barra invertida e aspas)
json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g'; }

# Mede RUNS invocações de `$BIN $args` (stdout+stderr descartados), recolhe os
# deltas em ns e devolve a linha JSON. $1=leaf (rótulo) $2=group $3..=args reais.
measure_one() {
  local leaf="$1" group="$2"; shift 2
  local samples="" i t0 t1
  for ((i=0; i<RUNS; i++)); do
    t0=$(date +%s%N)
    "$BIN" "$@" >/dev/null 2>&1 || true   # --help pode sair !=0 nalguns stubs; o custo mede-se na mesma
    t1=$(date +%s%N)
    samples+="$((t1 - t0))"$'\n'
  done
  local st min p50 p95 p99 max mean
  st=$(printf '%s' "$samples" | stats)
  read -r min p50 p95 p99 max mean <<<"$st"
  local info="false"; [ "$RUNS" -lt 20 ] && info="true"
  printf '{"leaf":"%s","group":"%s","runs":%d,"min_ms":%s,"p50_ms":%s,"p95_ms":%s,"p99_ms":%s,"max_ms":%s,"mean_ms":%s,"informational":%s}\n' \
    "$(json_escape "$leaf")" "$(json_escape "$group")" "$RUNS" \
    "$min" "$p50" "$p95" "$p99" "$max" "$mean" "$info" >> "$JSONL"
}

echo "binário: $BIN"
echo "RUNS=$RUNS  VERSION=$VERSION  LEAVES_LIMIT=$LEAVES_LIMIT"
[ "$RUNS" -lt 20 ] && echo "AVISO: RUNS<20 — números marcados como informational (ver performance-slo.md)"

# --- arranque base do binário: grupo __startup__ -------------------------------
echo "a medir arranque base (__startup__)..."
measure_one "--version" "__startup__" --version
measure_one "--help"    "__startup__" --help

# --- uma linha por folha -------------------------------------------------------
mapfile -t LEAVES < <(DELONIX_BIN="$BIN" bash "$repo/scripts/cli-tree.sh" --leaves 2>/dev/null)
total=${#LEAVES[@]}
[ "$total" -gt 0 ] || { echo "cli-tree.sh não devolveu folhas" >&2; exit 1; }
if [ "$LEAVES_LIMIT" -gt 0 ] && [ "$LEAVES_LIMIT" -lt "$total" ]; then
  LEAVES=("${LEAVES[@]:0:$LEAVES_LIMIT}")
fi
echo "a medir ${#LEAVES[@]} de $total folhas (×$RUNS)..."

n=0
for leaf in "${LEAVES[@]}"; do
  [ -n "$leaf" ] || continue
  group=${leaf%% *}            # primeiro token
  # shellcheck disable=SC2086  # $leaf é uma lista de argumentos deliberada
  measure_one "$leaf" "$group" $leaf --help
  n=$((n+1))
  if [ $((n % 25)) -eq 0 ]; then echo "  ... $n/${#LEAVES[@]}"; fi
done
echo "feito: $n folhas → $JSONL"

# --- resumo agregado por grupo (stdout) ---------------------------------------
echo
echo "resumo por grupo (mean dos means, p95 máximo) — ms:"
awk '
  {
    # extrai group, mean_ms, p95_ms de cada linha JSONL por rótulo
    g=""; mean=0; p95=0
    if (match($0, /"group":"[^"]*"/))  { s=substr($0,RSTART,RLENGTH); gsub(/"group":"|"/,"",s); g=s }
    if (match($0, /"p95_ms":[0-9.]+/)) { s=substr($0,RSTART,RLENGTH); sub(/"p95_ms":/,"",s); p95=s+0 }
    if (match($0, /"mean_ms":[0-9.]+/)){ s=substr($0,RSTART,RLENGTH); sub(/"mean_ms":/,"",s); mean=s+0 }
    if (g=="") next
    cnt[g]++; summean[g]+=mean; if (p95>maxp95[g]) maxp95[g]=p95
  }
  END {
    printf "%-16s %7s %12s %10s\n", "grupo", "folhas", "mean(mean)", "max(p95)"
    for (g in cnt)
      printf "%-16s %7d %12.3f %10.3f\n", g, cnt[g], summean[g]/cnt[g], maxp95[g]
  }' "$JSONL" | { IFS= read -r header; echo "$header"; sort -k4 -rn; }
