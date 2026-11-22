#!/usr/bin/env bash
# CLI audit suite: parameter-level matrix, differential against docker/podman,
# and terminal sampling. Entry point for the four tiers.
#
# It complements `tests/cli-parity/run.sh`, which measures the PARITY of the
# frequent verbs. This one measures the CLI's OWN surface: every discovered
# leaf, every option, the declared conflicts, and the layout.
#
# ## Why it always measures the TREE binary
#
# The five binaries have to sit in the SAME directory: `delonix` does not serve
# the CRI, the node API, the MCP or the management API — it `exec`s the sibling,
# and `exec_server` prefers the sibling NEXT TO the executable before falling
# back to the PATH. On this host `~/.local/bin/delonix-cri` is from 2026-09-07
# and comes EARLIER on the PATH than `/usr/local/bin`, so a run against the
# PATH can serve a sibling from another version. Pass the target path, where
# the five were built together.
#
#   cargo build --release -p delonix-bin -p delonix-cri-bin \
#     -p delonix-mgmt-bin -p delonix-mcp-bin -p delonix-node-api-bin
#
# ## Isolation (both roots, never one)
#
# Isolating DELONIX_ROOT and leaving DELONIX_NET_RUNTIME_DIR shared puts two
# state roots fighting over /tmp/delonix-net-<uid>/*.sock — an incident this
# repo has already measured, which restarted a production container. Both are
# isolated here. The net runtime dir lives under a SHORT path on purpose: a
# unix socket's sun_path caps at 108 bytes.
#
# ## Usage
#
#   tests/cli-parity/audit/run.sh matrix       <delonix-bin>   # inventory + cases + run
#   tests/cli-parity/audit/run.sh differential <delonix-bin>   # needs docker AND podman
#   tests/cli-parity/audit/run.sh ux           <delonix-bin>   # PTY samples 80/120, en/pt
#   tests/cli-parity/audit/run.sh all          <delonix-bin>
#   tests/cli-parity/audit/run.sh case <delonix-bin> PARSE-0103 [ID...]
#
# Exit: 1 when a case FAILS. A BLOQUEADO or NAO_EXECUTADO does not fail the
# gate — a measurement that could not be made is not a negative result, and it
# is reported in its own block instead of diluted into a green.
set -euo pipefail

here=$(cd "$(dirname "$0")" && pwd)
mode=${1:?uso: run.sh <matrix|differential|ux|all|case> <delonix-bin> [ID...]}
BIN=${2:?caminho do binario delonix construido DESTA arvore}
BIN=$(readlink -f "$BIN")
[ -x "$BIN" ] || { echo "FALHA: '$BIN' nao e um binario delonix executavel" >&2; exit 2; }
shift 2 || true

OUT=${AUDIT_OUT:-$(mktemp -d /tmp/dlxaudit.XXXXXX)}
RID=$(date +%H%M%S)
ROOT="$OUT/root"
NETDIR="/tmp/dlxaudit-net-$RID"   # short: sun_path caps at 108 bytes
mkdir -p "$ROOT" "$NETDIR" "$OUT/fixtures"
export DELONIX_ROOT="$ROOT" DELONIX_NET_RUNTIME_DIR="$NETDIR"
trap 'rm -rf "$NETDIR"' EXIT

py() { python3 -I "$@"; }

build_matrix() {
    echo "== inventario: a percorrer a ajuda de todas as folhas =="
    py "$here/discover.py" "$BIN" > "$OUT/inventory.json"
    py - <<PY
import json; d=json.load(open("$OUT/inventory.json")); print(" ", json.dumps(d["totals"]))
PY
    echo "== casos =="
    py "$here/matrix.py" --inventory "$OUT/inventory.json" \
        --out "$OUT/m_parser.jsonl" --stats "$OUT/stats_parser.json" >/dev/null
    AUD_EXISTING_FILE=/dev/null py "$here/matrix2.py" \
        --inventory "$OUT/inventory.json" --rules "$here/declared_rules.json" \
        --out "$OUT/m_negpair.jsonl" --unresolved "$OUT/neg_unresolved.json" \
        --stats "$OUT/stats_negpair.json" --cap 8 >/dev/null
    py "$here/functional.py"  --out "$OUT/m_func1.jsonl" --workdir "$OUT/fixtures" --run-id "$RID" >/dev/null
    py "$here/functional2.py" --out "$OUT/m_func2.jsonl" --workdir "$OUT/fixtures" --run-id "$RID" >/dev/null
    cat "$OUT/m_parser.jsonl" "$OUT/m_negpair.jsonl" "$OUT/m_func1.jsonl" "$OUT/m_func2.jsonl" \
        > "$OUT/matrix.jsonl"
    # A stable id has to be UNIQUE: two generators once shared the PAIR-####
    # range and the export silently deduped 82 cases, so the totals stopped
    # reconciling. The gate is here, not in a comment.
    py - <<PY
import json, collections, sys
ids=[json.loads(l)["id"] for l in open("$OUT/matrix.jsonl")]
dup=[k for k,v in collections.Counter(ids).items() if v>1]
print(f"  casos={len(ids)} unicos={len(set(ids))}")
if dup:
    print("FALHA: ids duplicados:", dup[:10], file=sys.stderr); sys.exit(1)
PY
}

run_matrix() {
    build_matrix
    echo "== execucao =="
    # The runner exits 1 when a case fails. With `set -e` + `pipefail` that
    # aborted the script HERE, before the export — so a run with findings
    # produced no machine-readable matrix, which is the one artefact a run with
    # findings most needs. The verdict is decided at the end, not by this line.
    py "$here/runner.py" --matrix "$OUT/matrix.jsonl" --bin "$BIN" --out "$OUT/results" \
        --root "$ROOT" --net-runtime "$NETDIR" > "$OUT/run.log" 2>&1 || true
    tail -8 "$OUT/run.log"
    echo "== matriz legivel por maquina =="
    py "$here/export_matrix.py" --inventory "$OUT/inventory.json" \
        --matrix "$OUT/matrix.jsonl" --results "$OUT/results/results.jsonl" \
        --out "$OUT/matrix-export" | tail -14
    echo "artefactos em $OUT"
    py - <<PY
import json, sys
t=json.load(open("$OUT/results/tally.json"))
sys.exit(1 if t.get("FAIL") else 0)
PY
}

run_case() {
    build_matrix
    local ids=()
    for i in "$@"; do ids+=(--id "$i"); done
    py "$here/runner.py" --matrix "$OUT/matrix.jsonl" --bin "$BIN" --out "$OUT/results" \
        --root "$ROOT" --net-runtime "$NETDIR" "${ids[@]}"
    echo "evidencia em $OUT/results/evidence/"
}

run_differential() {
    for e in docker podman; do
        command -v "$e" >/dev/null 2>&1 || {
            echo "FALHA: '$e' nao encontrado. O diferencial precisa dos TRES motores;" >&2
            echo "       um motor em falta e um pre-requisito falhado, nunca um pass." >&2
            exit 1; }
    done
    echo "== modos (uma diferenca pode vir do MODO, nao do motor) =="
    echo "  docker: $(docker version --format '{{.Server.Version}}') $(docker info --format '{{if .SecurityOptions}}{{end}}')rootful=$(ps -o user= -p "$(pgrep -f dockerd | head -1)" 2>/dev/null | grep -q root && echo sim || echo nao)"
    echo "  podman: $(podman version --format '{{.Version}}') rootless=$(podman info --format '{{.Host.Security.Rootless}}')"
    echo "  delonix: $("$BIN" --version | head -1) rootless"
    "$BIN" image pull busybox@sha256:bdf57e528e45e4433820e045b29b4597825a1c9e38353532d90a01445013f82e >/dev/null 2>&1 || true
    py "$here/differential.py" --delonix "$BIN" --out "$OUT/dif" --run-id "$RID" --root "$ROOT"
}

run_ux() {
    py "$here/ux_sample.py" --bin "$BIN" --out "$OUT/ux" --root "$ROOT"
    echo "amostras em $OUT/ux/samples/"
}

case "$mode" in
    matrix)       run_matrix ;;
    differential) run_differential ;;
    ux)           run_ux ;;
    case)         run_case "$@" ;;
    all)          run_matrix; run_differential; run_ux ;;
    *) echo "modo desconhecido: $mode" >&2; exit 2 ;;
esac
