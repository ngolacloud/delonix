#!/usr/bin/env bash
# Conformância CRI REAL: corre o `critest` (cri-tools) contra um `delonix-cri`
# ISOLADO. NÃO é um teste unitário — sobe um servidor CRI a sério e um cliente
# gRPC (o `critest`) exercita pods/containers/imagens de ponta a ponta.
#
# Uso:   ./scripts/critest.sh [-- <args extra do critest>]
# Env:   FOCUS=<regex>   → -ginkgo.focus
#        SKIP=<regex>    → -ginkgo.skip
#        KEEP=1          → não derruba o servidor/estado no fim (para depurar)
#        GINKGO_TIMEOUT  → default 20m
#
# Pré-requisitos: `critest` e `crictl` no PATH (v1.x). NÃO precisa de Go.
#
# Isolamento (crítico): usa um `DELONIX_ROOT` e um socket PRÓPRIOS por execução
# (mktemp + PID), para NÃO colidir com outra sessão que partilhe
# `~/.local/share/delonix`. Regra do repo: NUNCA o `delonix` do PATH — o CRI
# resolve a CLI irmã ao seu executável (`cli_bin`), por isso os dois binários
# TÊM de estar no mesmo dir (`target/<perfil>/`).
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${PROFILE:-debug}"
BIN_DIR="$ROOT_DIR/target/$PROFILE"
CRI_BIN="$BIN_DIR/delonix-cri"
CLI_BIN="$BIN_DIR/delonix"
GINKGO_TIMEOUT="${GINKGO_TIMEOUT:-20m}"

command -v critest >/dev/null || { echo "ERRO: critest não está no PATH"; exit 2; }
command -v crictl  >/dev/null || { echo "ERRO: crictl não está no PATH"; exit 2; }

# Build (idempotente) dos DOIS binários — o CRI delega o ciclo de vida na CLI.
if [[ "${NO_BUILD:-}" != "1" ]]; then
  echo "==> cargo build -p delonix-cri-bin -p delonix-bin ($PROFILE)"
  ( cd "$ROOT_DIR" && cargo build ${PROFILE:+$([[ $PROFILE == release ]] && echo --release)} \
      -p delonix-cri-bin --bin delonix-cri -p delonix-bin --bin delonix ) || exit $?
fi
[[ -x "$CRI_BIN" && -x "$CLI_BIN" ]] || { echo "ERRO: binários em falta em $BIN_DIR"; exit 2; }

DELONIX_ROOT="$(mktemp -d "/tmp/delonix-cri-critest-root.XXXXXX")"
SOCK="/tmp/delonix-cri-critest-$$.sock"
LOG="${LOG:-/tmp/delonix-critest-$$.log}"
export DELONIX_ROOT
rm -f "$SOCK"

cleanup() {
  [[ "${KEEP:-}" == "1" ]] && { echo "KEEP=1 → estado em $DELONIX_ROOT (socket $SOCK)"; return; }
  [[ -n "${SRV_PID:-}" ]] && kill "$SRV_PID" 2>/dev/null
  # holder/slirp rootless deste root (isolado): derruba pela infra
  DELONIX_ROOT="$DELONIX_ROOT" "$CLI_BIN" netns down >/dev/null 2>&1 || true
  rm -f "$SOCK"
  rm -rf "$DELONIX_ROOT"
}
trap cleanup EXIT

echo "==> a arrancar delonix-cri (root=$DELONIX_ROOT socket=$SOCK)"
# A conformance run is a throwaway server: the suite creates privileged containers
# on purpose, which the default capability ceiling refuses, so the ceiling is lifted
# explicitly here (override with DELONIX_CRI_CAP_CEILING to measure a bounded node).
DELONIX_ROOT="$DELONIX_ROOT" DELONIX_CRI_ADDR="unix://$SOCK" \
  DELONIX_CRI_CAP_CEILING="${DELONIX_CRI_CAP_CEILING:-all}" \
  setsid "$CRI_BIN" >"$LOG.server" 2>&1 < /dev/null &
SRV_PID=$!
# espera o socket ficar de pé (o servidor imprime a escutar)
for _ in $(seq 1 50); do [[ -S "$SOCK" ]] && break; sleep 0.2; done
[[ -S "$SOCK" ]] || { echo "ERRO: o servidor não abriu o socket"; cat "$LOG.server"; exit 3; }

echo "==> smoke: crictl version"
crictl --runtime-endpoint "unix://$SOCK" version || { echo "ERRO: crictl não fala com o servidor"; exit 3; }

# AppArmor conformance: critest's apparmor specs reference two profiles that MUST be
# loaded on the node (the runtime fail-closes a reference to an unloaded profile — the
# correct behaviour). The kubelet/operator loads them out-of-band in a real cluster;
# here the harness does it, but ONLY where it can: root + apparmor_parser + AppArmor
# enabled. A no-op elsewhere (e.g. a rootless host), so the specs stay honestly failed
# there instead of being silently skipped.
if [[ "$(id -u)" == "0" ]] && command -v apparmor_parser >/dev/null \
   && [[ -d /sys/kernel/security/apparmor ]]; then
  echo "==> loading critest AppArmor profiles (apparmor_parser)"
  AA_PROFILES="$(mktemp)"
  cat >"$AA_PROFILES" <<'AAEOF'
#include <tunables/global>
profile cri-validate-apparmor-test-deny-write flags=(attach_disconnected) {
  #include <abstractions/base>
  file,
  # Deny all file writes.
  deny /** w,
}
profile cri-validate-apparmor-test-audit-write flags=(attach_disconnected) {
  #include <abstractions/base>
  file,
  # Only audit file writes.
  audit /** w,
}
AAEOF
  apparmor_parser -r -W "$AA_PROFILES" \
    && echo "   loaded: $(aa-status --json 2>/dev/null | grep -o 'cri-validate-apparmor-test-[a-z-]*' | sort -u | tr '\n' ' ')" \
    || echo "   WARNING: apparmor_parser failed — the AppArmor specs will fail (profiles absent)"
  rm -f "$AA_PROFILES"
else
  echo "==> skipping AppArmor profile load (not root / no apparmor_parser) — apparmor specs will fail honestly"
fi

echo "==> critest (timeout ginkgo=$GINKGO_TIMEOUT)"
set -x
critest \
  -runtime-endpoint "unix://$SOCK" \
  -image-endpoint "unix://$SOCK" \
  -ginkgo.timeout="$GINKGO_TIMEOUT" \
  -ginkgo.no-color \
  ${FOCUS:+-ginkgo.focus="$FOCUS"} \
  ${SKIP:+-ginkgo.skip="$SKIP"} \
  "$@" 2>&1 | tee "$LOG"
rc=${PIPESTATUS[0]}
set +x
echo "==> critest terminou com rc=$rc (log completo em $LOG; servidor em $LOG.server)"
exit "$rc"
