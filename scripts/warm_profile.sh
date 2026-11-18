#!/usr/bin/env bash
# Perfil do PLANO MORNO (ADR-0095): prova que um processo persistente que
# reutiliza infra pré-construída (runner pré-carregado + imagem resolvida) faz
# cair o `pre_spawn` do arranque a frio — medido com o MESMO profiler embutido
# (`DELONIX_PROFILE_SPAWN`) e a MESMA subtracção `wall − spawn_total` que o
# `scripts/spawn_profile.sh` usa, para ser comparável com
# `reports/spawn-profile-2026-11-18/`.
#
# O `delonix __poolbench` arranca containers idênticos de DUAS maneiras no MESMO
# processo persistente:
#   cold — um `__apirun` novo por corrida (o que o CRI/API fazem hoje)
#   warm — entregue a um runner estacionado do pool (binário já carregado,
#          imagem já resolvida)
# e emite, por corrida, uma linha `{"role":"bench","phase":...,"wall_us":...}`
# ao lado das linhas `{"role":"parent",...,"total_us":...}` do profiler. O
# relator emparelha-as (parent imediatamente antes do bench) e dá
# pre_spawn = wall − spawn_total para cada fase.
#
#   scripts/warm_profile.sh [--runs N] [--size M] [--image REF] [--bin PATH] [--out DIR]
set -uo pipefail

RUNS=20
SIZE=4
IMAGE=alpine
BIN="${BIN:-$(cd "$(dirname "$0")/.." && pwd)/target/release/delonix}"
OUT=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --runs) RUNS="$2"; shift 2 ;;
    --size) SIZE="$2"; shift 2 ;;
    --image) IMAGE="$2"; shift 2 ;;
    --bin) BIN="$2"; shift 2 ;;
    --out) OUT="$2"; shift 2 ;;
    *) echo "uso: $0 [--runs N] [--size M] [--image REF] [--bin PATH] [--out DIR]"; exit 2 ;;
  esac
done
[[ -x "$BIN" ]] || { echo "binário ausente: $BIN" >&2; exit 1; }

# Mesmo juízo de bancada que o spawn_profile.sh: sob carga mede-se a bancada.
NTHREADS=$(nproc); LOAD1=$(cut -d' ' -f1 /proc/loadavg)
LIMIT=$(awk "BEGIN{print $NTHREADS/2}")
if awk "BEGIN{exit !($LOAD1 > $LIMIT)}"; then
  echo "RECUSA: load1=$LOAD1 > $LIMIT (nproc/2). Sob carga mede-se a bancada, não o motor." >&2
  exit 3
fi

OUT="${OUT:-$(cd "$(dirname "$0")/.." && pwd)/reports/warm-profile-$(date +%Y-%m-%d)}"
mkdir -p "$OUT"
SANDBOX=$(mktemp -d /tmp/dlx-warm-prof.XXXXXX)
trap 'rm -rf "$SANDBOX"' EXIT
mkdir -p "$SANDBOX/root" "$SANDBOX/run"
dlx() { env DELONIX_ROOT="$SANDBOX/root" DELONIX_NET_RUNTIME_DIR="$SANDBOX/run" "$BIN" "$@"; }

echo "== bancada ==" >&2
echo "  bin: $BIN" >&2
echo "  kernel: $(uname -r) | cpu: $NTHREADS threads, load1=$LOAD1" >&2
echo "  versão: $(dlx --version 2>/dev/null | head -1) | runs: $RUNS/fase | pool: $SIZE | imagem: $IMAGE | out: $OUT" >&2

echo "== aquecimento (pull+extracção de $IMAGE no root virgem) ==" >&2
dlx container run --rm --net host "$IMAGE" true >/dev/null 2>&1

JSONL="$OUT/warm.jsonl"; : > "$JSONL"
echo "== bench (cold vs warm) ==" >&2
env DELONIX_PROFILE_SPAWN="$JSONL" DELONIX_ROOT="$SANDBOX/root" \
    DELONIX_NET_RUNTIME_DIR="$SANDBOX/run" \
    "$BIN" __poolbench "$IMAGE" "$RUNS" "$SIZE" 2> "$OUT/bench.stderr"
sed -n '1,40p' "$OUT/bench.stderr" >&2

echo >&2; echo "== relatório ==" >&2
python3 "$(dirname "$0")/warm_profile_report.py" "$JSONL" | tee "$OUT/REPORT.txt"
