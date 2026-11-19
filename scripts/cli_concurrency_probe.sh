#!/usr/bin/env bash
# Exercita CONCORRÊNCIA / alta disponibilidade da superfície CLI do delonix.
#
# Lança K processos `delonix` em paralelo a martelar um conjunto de folhas
# READ-ONLY seguras, durante DURATION segundos OU ITERATIONS iterações por
# trabalhador. Mede invocações totais, taxa de sucesso/erro, latência agregada
# (min/p50/p95/p99/max/mean em ms) e throughput (invocações/seg), e verifica que
# a concorrência não corrompe estado (ex.: `ps` + `info` em paralelo não devem
# falhar por contenção de lock — se falharem, é um achado).
#
# Degrada com SKIP: cada folha candidata é pré-validada em série; a que sair
# !=0 sozinha (comando inválido ou backend/estado ausente) fica FORA do conjunto
# activo e é registada em "skipped" — NÃO conta como falha de concorrência. Uma
# folha que passa em série mas falha sob carga É um achado real (lock contention
# / corrupção), e é isso que esta sonda procura.
#
# Emite: reports/e2e/<versão>/concurrency-results.json
#
# Variáveis de ambiente:
#   DELONIX_BIN    binário a medir (default target/release/delonix do repo)
#   CONCURRENCY    nº de trabalhadores em paralelo (default 32)
#   DURATION       segundos a martelar em modo-duração (default 20)
#   ITERATIONS     se definido (>0), usa modo-iterações: cada trabalhador faz N
#                  invocações e pára (total = CONCURRENCY*N). Tem prioridade.
#   VERSION        subpasta de reports/e2e (default 0.1.3)
#
# Sem dependências exóticas: coreutils (date, sort, mkdir, mktemp) + awk.
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
BIN=${DELONIX_BIN:-"$repo/target/release/delonix"}
[ -n "$BIN" ] && [ -x "$BIN" ] || {
  echo "sem binário executável em '$BIN' — aponta DELONIX_BIN" >&2
  exit 1
}

CONCURRENCY=${CONCURRENCY:-32}
DURATION=${DURATION:-20}
ITERATIONS=${ITERATIONS:-0}
VERSION=${VERSION:-0.1.3}
OUT="$repo/reports/e2e/$VERSION"
JSON="$OUT/concurrency-results.json"
mkdir -p "$OUT"

now_ms() { echo $(( $(date +%s%N) / 1000000 )); }

# Conjunto candidato de folhas READ-ONLY seguras (uma por linha, argv tal e qual).
CANDIDATES=(
  "version"
  "info"
  "ps"
  "image ls"
  "vm ls"
  "net ls"
  "system info"
  "events --help"
)

WORK=$(mktemp -d "${TMPDIR:-/tmp}/dlx-conc.XXXXXX")
cleanup() { rm -rf "$WORK"; }
trap cleanup EXIT

# escapa string p/ literal JSON
json_escape() { printf '%s' "$1" | sed 's/\\/\\\\/g; s/"/\\"/g; s/\t/ /g'; }

# --- pré-validação em série: filtra candidatas que falham sozinhas -------------
echo "binário: $BIN"
echo "CONCURRENCY=$CONCURRENCY  DURATION=${DURATION}s  ITERATIONS=$ITERATIONS  VERSION=$VERSION"
echo "pré-validação das folhas candidatas..."
ACTIVE=()
SKIPPED_JSON=""
for c in "${CANDIDATES[@]}"; do
  # shellcheck disable=SC2086
  if "$BIN" $c >/dev/null 2>&1; then
    ACTIVE+=("$c"); echo "  activa : $c"
  else
    rc=$?
    echo "  SKIP   : $c (rc=$rc em série — inválida ou backend ausente)"
    SKIPPED_JSON+="$([ -n "$SKIPPED_JSON" ] && echo ,){\"leaf\":\"$(json_escape "$c")\",\"serial_rc\":$rc}"
  fi
done
[ "${#ACTIVE[@]}" -gt 0 ] || { echo "nenhuma folha activa — nada a martelar" >&2; exit 1; }
printf '%s\n' "${ACTIVE[@]}" > "$WORK/active.txt"

# --- trabalhador --------------------------------------------------------------
# Cada trabalhador cicla round-robin pelas folhas activas, cronometra cada
# invocação e escreve no SEU ficheiro (um por trabalhador → sem corrupção de
# escrita concorrente). Linha de resultado: "<rc> <ns> <leaf>".
# Falhas vão para um ficheiro à parte: "<rc>\t<leaf>\t<excerto stderr>".
worker() {
  local id="$1" deadline_ms="$2" max_iters="$3"
  local res="$WORK/res.$id" fail="$WORK/fail.$id"
  mapfile -t acts < "$WORK/active.txt"
  local nacts=${#acts[@]} k=0 it=0 leaf t0 t1 rc errf
  errf="$WORK/err.$id"
  while :; do
    if [ "$max_iters" -gt 0 ]; then
      [ "$it" -ge "$max_iters" ] && break
    else
      [ "$(now_ms)" -ge "$deadline_ms" ] && break
    fi
    leaf="${acts[$((k % nacts))]}"
    k=$((k+1)); it=$((it+1))
    t0=$(date +%s%N)
    # shellcheck disable=SC2086
    "$BIN" $leaf >/dev/null 2>"$errf"; rc=$?
    t1=$(date +%s%N)
    printf '%d %d %s\n' "$rc" "$((t1 - t0))" "$leaf" >> "$res"
    if [ "$rc" -ne 0 ]; then
      printf '%d\t%s\t%s\n' "$rc" "$leaf" "$(head -c 200 "$errf" | tr '\n\t' '  ')" >> "$fail"
    fi
  done
}

# --- semáforo de trabalhadores ------------------------------------------------
if [ "$ITERATIONS" -gt 0 ]; then
  mode="iterations"; deadline_ms=0; max_iters="$ITERATIONS"
  echo "modo-iterações: $CONCURRENCY trabalhadores × $ITERATIONS iterações"
else
  mode="duration"; deadline_ms=$(( $(now_ms) + DURATION * 1000 )); max_iters=0
  echo "modo-duração: $CONCURRENCY trabalhadores durante ${DURATION}s"
fi

run_start=$(now_ms)
for ((w=0; w<CONCURRENCY; w++)); do
  worker "$w" "$deadline_ms" "$max_iters" &
done
wait
run_end=$(now_ms)
elapsed_ms=$(( run_end - run_start )); [ "$elapsed_ms" -lt 1 ] && elapsed_ms=1

# --- agregação ----------------------------------------------------------------
cat "$WORK"/res.* 2>/dev/null > "$WORK/all.txt" || true
total=$(wc -l < "$WORK/all.txt" | tr -d ' ')
total=${total:-0}
[ "$total" -gt 0 ] || { echo "nenhuma invocação registada" >&2; exit 1; }
ok=$(awk '$1==0' "$WORK/all.txt" | wc -l | tr -d ' ')
err=$(( total - ok ))

# latência agregada (coluna ns) → ms
read -r min p50 p95 p99 max mean < <(
  awk '{print $2}' "$WORK/all.txt" | sort -n | awk '
    { a[NR]=$1; sum+=$1 }
    END {
      n=NR
      i50=int((50*n+99)/100); if(i50<1)i50=1; if(i50>n)i50=n
      i95=int((95*n+99)/100); if(i95<1)i95=1; if(i95>n)i95=n
      i99=int((99*n+99)/100); if(i99<1)i99=1; if(i99>n)i99=n
      printf "%.3f %.3f %.3f %.3f %.3f %.3f\n",
        a[1]/1e6, a[i50]/1e6, a[i95]/1e6, a[i99]/1e6, a[n]/1e6, (sum/n)/1e6
    }')

# taxas (3 casas) e throughput
succ_rate=$(awk -v o="$ok" -v t="$total" 'BEGIN{printf "%.4f", t? o/t : 0}')
err_rate=$(awk -v e="$err" -v t="$total" 'BEGIN{printf "%.4f", t? e/t : 0}')
throughput=$(awk -v t="$total" -v ms="$elapsed_ms" 'BEGIN{printf "%.2f", t*1000.0/ms}')

# lista de falhas (até 50) — toda falha de folha ACTIVA é achado de concorrência
FAILS_JSON=""
nfails=0
if ls "$WORK"/fail.* >/dev/null 2>&1; then
  while IFS=$'\t' read -r frc fleaf fexc; do
    [ -n "$frc" ] || continue
    nfails=$((nfails+1))
    [ "$nfails" -gt 50 ] && continue
    FAILS_JSON+="$([ -n "$FAILS_JSON" ] && echo ,){\"leaf\":\"$(json_escape "$fleaf")\",\"rc\":$frc,\"stderr\":\"$(json_escape "$fexc")\"}"
  done < <(cat "$WORK"/fail.* 2>/dev/null)
fi
# achado: alguma folha que passou a pré-validação falhou sob carga
lock_contention="false"; [ "$err" -gt 0 ] && lock_contention="true"

ACTIVE_JSON=""
for a in "${ACTIVE[@]}"; do
  ACTIVE_JSON+="$([ -n "$ACTIVE_JSON" ] && echo ,)\"$(json_escape "$a")\""
done

cat > "$JSON" <<EOF
{
  "binary": "$(json_escape "$BIN")",
  "mode": "$mode",
  "concurrency": $CONCURRENCY,
  "duration_s": $DURATION,
  "iterations_per_worker": $ITERATIONS,
  "elapsed_ms": $elapsed_ms,
  "active_leaves": [${ACTIVE_JSON}],
  "skipped_leaves": [${SKIPPED_JSON}],
  "total_invocations": $total,
  "ok": $ok,
  "errors": $err,
  "success_rate": $succ_rate,
  "error_rate": $err_rate,
  "throughput_per_s": $throughput,
  "latency_ms": {
    "min": $min, "p50": $p50, "p95": $p95, "p99": $p99, "max": $max, "mean": $mean
  },
  "state_corruption_suspected": $lock_contention,
  "failure_count": $nfails,
  "failures": [${FAILS_JSON}]
}
EOF

echo
echo "invocações: $total  ok: $ok  erros: $err  (sucesso $succ_rate, erro $err_rate)"
echo "latência ms  min=$min p50=$p50 p95=$p95 p99=$p99 max=$max mean=$mean"
echo "throughput: $throughput inv/s  (elapsed ${elapsed_ms}ms)"
if [ "$err" -gt 0 ]; then
  echo "ACHADO: $err invocação(ões) de folhas activas falharam sob concorrência (ver failures em $JSON)"
else
  echo "sem corrupção de estado detectada: todas as folhas activas rc==0 sob carga"
fi
echo "→ $JSON"
