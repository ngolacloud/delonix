#!/usr/bin/env bash
# release-validate.sh — run the four release-gate suites on HEAD, record each in
# reports/release-validation/<tag>.json, and — only if ALL pass — commit that
# manifest as a DOC-ONLY child that becomes the tag. It NEVER tags, pushes or
# publishes (the human gate, per the delonix-release skill).
#
# The doc-only child is deliberate: a manifest cannot record its own commit's
# SHA (a hash fixpoint), so the suites are recorded on the content commit and
# the manifest rides a child that changes nothing but the manifest — which
# `release_gate.py check` accepts as the tag's validated parent.
#
# Refuses under load (MAX_LOAD, default 4.0) so a contended host never produces
# a dirty verdict — the same doctrine as bench.sh. --force overrides.
#
# Usage:  scripts/release-validate.sh [vX.Y.Z] [--force]
# Env:    MAX_LOAD=4.0  DELONIX_SKIP=critest (comma list to skip, e.g. on a
#         rootless host where critest needs root — it is then NOT recorded, so
#         the gate stays red until it runs on a root-capable host/VM)
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"

FORCE=0
TAG=""
for a in "$@"; do
  case "$a" in
    --force) FORCE=1 ;;
    v[0-9]*) TAG="$a" ;;
    *) echo "argumento desconhecido: $a" >&2; exit 2 ;;
  esac
done
# Default tag = workspace version from Cargo.toml ([workspace.package] version).
if [[ -z "$TAG" ]]; then
  VER=$(awk '/^\[workspace.package\]/{f=1} f&&/^version *=/{gsub(/[" ]/,"",$3); print $3; exit}' Cargo.toml)
  TAG="v${VER}"
fi
MAX_LOAD="${MAX_LOAD:-4.0}"
SKIP=",${DELONIX_SKIP:-},"
COMMIT=$(git rev-parse HEAD)

echo "== release-validate $TAG @ ${COMMIT:0:12} =="

LOAD=$(awk '{print $1}' /proc/loadavg)
if [[ "$FORCE" != 1 ]] && awk "BEGIN{exit !($LOAD > $MAX_LOAD)}"; then
  echo "host ocupado (load $LOAD > $MAX_LOAD) — adio. --force para ignorar." >&2
  exit 3
fi

echo "== build (release) =="
find bins/delonix/templates -name .ruff_cache -o -name __pycache__ 2>/dev/null | xargs -r rm -rf 2>/dev/null
cargo build --release --locked \
  -p delonix-bin -p delonix-cri-bin -p delonix-mgmt-bin -p delonix-mcp-bin -p delonix-node-api-bin || exit 1
BIN="$ROOT/target/release/delonix"

run_suite() {
  local name="$1"; shift
  if [[ "$SKIP" == *",$name,"* ]]; then
    echo "== $name: SALTADO (DELONIX_SKIP) — NÃO gravado (gate fica vermelho) =="
    return 0
  fi
  echo "== $name: a correr =="
  if "$@"; then
    python3 scripts/release_gate.py record --tag "$TAG" --suite "$name" --commit "$COMMIT" --passed
  else
    python3 scripts/release_gate.py record --tag "$TAG" --suite "$name" --commit "$COMMIT" --failed
    echo "== $name: FALHOU =="
  fi
}

run_suite cargo-test cargo test --workspace --locked --no-fail-fast
run_suite e2e        bash scripts/e2e.sh "$BIN"
run_suite chaos      bash scripts/chaos.sh --max-load "$MAX_LOAD"
# critest needs root: use the privileged-container harness when not uid 0.
if [[ "$(id -u)" == 0 ]]; then
  run_suite critest bash scripts/critest.sh
else
  run_suite critest bash scripts/critest-root-cni.sh
fi

echo "== veredicto =="
if python3 scripts/release_gate.py check --tag "$TAG" --commit "$COMMIT"; then
  git add "reports/release-validation/${TAG}.json"
  git commit -q -m "release: $TAG validation evidence (4 suites green @ ${COMMIT:0:12})"
  D=$(git rev-parse HEAD)
  echo
  echo "GO. Manifesto committed como filho doc-only @ ${D:0:12}."
  echo "Passos humanos (NÃO automatizados):"
  echo "  git tag $TAG $D && git push origin $TAG"
  echo "  gh workflow run release.yml -f tag=$TAG"
  exit 0
fi
echo
echo "NO-GO — ver as suites vermelhas acima. Manifesto NÃO committed."
exit 1
