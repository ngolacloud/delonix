#!/usr/bin/env bash
# Perfil por fase do arranque A FRIO de um container no delonix (rootless).
#
# O `docs/measured-comparison.md` mede ~91 ms de arranque a frio (host) e
# ~216–344 ms com rede isolada, mas NÃO diz onde se vão. Este arnês liga o
# profiler embutido (`DELONIX_PROFILE_SPAWN`, em `crates/adapters/delonix-linux`)
# E cronometra o WALL-CLOCK de cada corrida, para separar:
#
#   wall  = pre_spawn (arranque do processo + resolução de imagem/rootfs no CLI)
#         + spawn_total (clone + cgroup + rede-hook + mounts, medido pelo profiler)
#
# «processo» é o pre_spawn (o binário daemonless re-exec a cada run); as restantes
# fases são as do profiler. NÃO altera o motor — o profiler é um no-op sem a var.
#
#   scripts/spawn_profile.sh [--runs N] [--bin PATH] [--out DIR]
#
# Modos medidos, cada um com a 1ª corrida (FRIA) reportada à parte da mediana
# (QUENTE), porque é a 1ª que paga o overlay/cgroup frios:
#   host      — `--net host` sem -p: partilha a netns do host (linha 4a, ~91 ms)
#   isolated  — `--net <rede>` com SENTINELA a segurar o plano (linha 4b, ~216 ms)
set -uo pipefail

RUNS=20
BIN="${BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/release/delonix}"
OUT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs) RUNS="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    *) echo "uso: $0 [--runs N] [--bin PATH] [--out DIR]"; exit 2 ;;
  esac
done
[[ -x "$BIN" ]] || { echo "binário ausente: $BIN" >&2; exit 1; }

# Juízo de bancada: recusa medir sob carga (a mesma lição do bench.sh).
NTHREADS=$(nproc); LOAD1=$(cut -d' ' -f1 /proc/loadavg)
LIMIT=$(awk "BEGIN{print $NTHREADS/2}")
if awk "BEGIN{exit !($LOAD1 > $LIMIT)}"; then
  echo "RECUSA: load1=$LOAD1 > $LIMIT (nproc/2). Sob carga mede-se a bancada, não o motor." >&2
  exit 3
fi

OUT="${OUT:-$(cd "$(dirname "$0")/.." && pwd)/reports/spawn-profile-$(date +%Y-%m-%d)}"
mkdir -p "$OUT"
SANDBOX=$(mktemp -d /tmp/dlx-spawn-prof.XXXXXX)
trap 'rm -rf "$SANDBOX"' EXIT
mkdir -p "$SANDBOX/root" "$SANDBOX/run"
dlx() { env DELONIX_ROOT="$SANDBOX/root" DELONIX_NET_RUNTIME_DIR="$SANDBOX/run" "$BIN" "$@"; }

# Corre N vezes um `container run`, cronometra o wall de cada e junta-o à linha
# de perfil do pai correspondente (ordem preservada: a i-ésima linha é a i-ésima
# corrida). Escreve <modo>.jsonl (perfil+wall por corrida) e <modo>.stderr.
measure() {
  local mode="$1"; shift              # host | isolated
  local jsonl="$OUT/$mode.jsonl"; : > "$jsonl"
  local walls="$OUT/$mode.walls"; : > "$walls"
  local tmp="$SANDBOX/$mode.prof"; : > "$tmp"
  local i t0 t1
  for i in $(seq 1 "$RUNS"); do
    t0=$(date +%s%N)
    env DELONIX_PROFILE_SPAWN="$tmp" DELONIX_ROOT="$SANDBOX/root" \
        DELONIX_NET_RUNTIME_DIR="$SANDBOX/run" "$BIN" "$@" \
        >/dev/null 2>>"$OUT/$mode.stderr"
    t1=$(date +%s%N)
    echo "$(( (t1-t0)/1000 ))" >> "$walls"   # wall em microssegundos
  done
  # Junta cada wall à linha de pai da mesma corrida (i-ésima linha = i-ésima run).
  grep '"role":"parent"' "$tmp" | paste -d' ' - "$walls" | \
    awk '{w=$NF; sub(/ [0-9]+$/,""); sub(/}$/, ",\"wall_us\":"w"}"); print}' > "$jsonl"
  # Guarda também as linhas de filho (sub-fases de mounts) vindas do stderr.
  grep "CHILD mounts" "$OUT/$mode.stderr" 2>/dev/null > "$OUT/$mode.child.txt" || true
  echo "  $mode: $(wc -l < "$jsonl") corridas; sub-perfil de filho: $(wc -l < "$OUT/$mode.child.txt" 2>/dev/null || echo 0) linhas" >&2
}

echo "== bancada ==" >&2
echo "  bin: $BIN" >&2
echo "  kernel: $(uname -r) | cpu: $NTHREADS threads, load1=$LOAD1" >&2
echo "  versão: $(dlx --version 2>/dev/null | head -1) | runs: $RUNS/modo | out: $OUT" >&2

echo "== aquecimento (pull+extracção de alpine no root virgem) ==" >&2
dlx container run --rm --net host alpine true >/dev/null 2>&1

echo "== arranque puro do processo (exec+linker+parse CLI, sai sem trabalho) ==" >&2
PSTART=""
for i in 1 2 3; do
  t0=$(date +%s%N); dlx --version >/dev/null 2>&1; t1=$(date +%s%N)
  PSTART="$PSTART $(( (t1-t0)/1000 ))"
done
echo "$PSTART" | tr ' ' '\n' | grep -v '^$' > "$OUT/process-start.us"
echo "  --version (µs): $PSTART" >&2

echo "== host (--net host) ==" >&2
measure host container run --rm --net host alpine true

echo "== isolado (--net profnet, sentinela a segurar o plano) ==" >&2
if dlx network create profnet >/dev/null 2>&1; then
  SENTINEL=$(dlx container run -d --net profnet alpine sleep 600 2>/dev/null | tail -1)
  sleep 0.3
  measure isolated container run --rm --net profnet alpine true
  [[ -n "${SENTINEL:-}" ]] && dlx container rm -f "$SENTINEL" >/dev/null 2>&1
  dlx network rm profnet >/dev/null 2>&1 || true
else
  echo "  isolado SALTADO (network create falhou):" >&2
  dlx network create profnet 2>&1 | head -3 >&2
fi

echo >&2; echo "== relatório ==" >&2
python3 "$(dirname "$0")/spawn_profile_report.py" "$OUT" | tee "$OUT/REPORT.txt"
