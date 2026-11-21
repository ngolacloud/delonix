#!/usr/bin/env bash
# install-vscode.sh — instala a extensão VS Code do Delonix (o `.vsix`).
#
#   curl -fsSL https://github.com/ngolacloud/delonix/releases/latest/download/install-vscode.sh | bash
#
# This installs ONLY the editor extension — the developer-convenience half that
# used to live inside `install.sh`. It is separate on purpose: installing a
# container/microVM engine must not depend on an editor being present, and a
# developer who wants the extension should not have to touch the runtime
# installer. This script needs NO root (the editor CLI installs per-user) and
# never compiles.
#
# It serves VS Code AND its forks through one CLI (Cursor, Windsurf, VSCodium,
# Antigravity share the extension CLI). The `.vsix` is covered by the same signed
# `SHA256SUMS` as every other release asset, and is verified before install.
#
# Flags:
#   --version <tag>              instala o `.vsix` dessa release (default: latest)
#   --insecure-skip-signature    salta a verificação minisign (aceita o risco)
#   -h | --help                  mostra este cabeçalho
#
# Env:
#   DELONIX_EDITOR_TIMEOUT=<segundos>   tecto de tempo por chamada a um editor
#                                       (default 120): um launcher que abre uma
#                                       janela em vez de responder na CLI não
#                                       pendura o script — desiste e segue.
set -euo pipefail

REPO="ngolacloud/delonix"
# Chave PÚBLICA minisign das releases oficiais (só a linha base64) — a mesma raiz
# de confiança do install.sh. O `.vsix` é coberto pelo SHA256SUMS assinado.
MINISIGN_PUBKEY="RWSiOqlKAnVVB+pJLQxgYHq/kdN6RbBQdlL5gOcZ6H/xkwSAPIqTo+GB"
VERSION="latest"
SKIP_SIG=0

has_cmd() { command -v "$1" >/dev/null 2>&1 || [ -x "/usr/sbin/$1" ] || [ -x "/sbin/$1" ]; }

# Mesma gramática de saída do install.sh: cores só nos estados, desligadas fora de um tty.
if [ -t 1 ]; then
  C_OK=$'\033[1;32m'; C_SKIP=$'\033[2m'; C_WARN=$'\033[1;33m'; C_ERR=$'\033[1;31m'; C_0=$'\033[0m'
else
  C_OK=""; C_SKIP=""; C_WARN=""; C_ERR=""; C_0=""
fi
CLEANUP_TMP=""
cleanup() {
  [ -n "$CLEANUP_TMP" ] && rm -rf "$CLEANUP_TMP"
  [ -t 1 ] && printf '\033[?25h'
  return 0
}
trap cleanup EXIT

msg()   { printf 'install/delonix-vscode: %s\n' "$*"; }
step()  { printf '[%s] %s: %s\n' "$1" "$2" "$3"; }
skip()  { printf '[%s] %s: %salready satisfied (SKIP)%s\n' "$1" "$2" "$C_SKIP" "$C_0"; }
stepok(){ printf '[%s] %s: %sOK%s\n' "$1" "$2" "$C_OK" "$C_0"; }
warn()  { printf '%swarning%s %s\n' "$C_WARN" "$C_0" "$*" >&2; }
die()   { printf '%serror%s %s\n' "$C_ERR" "$C_0" "$*" >&2; exit 1; }

SPIN_FRAMES=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
spin() {
  local phase="$1" name="$2" doing="$3"; shift 3
  if [ ! -t 1 ]; then
    step "$phase" "$name" "$doing"
    "$@"
    return $?
  fi
  "$@" &
  local pid=$! i=0
  printf '\033[?25l'
  while kill -0 "$pid" 2>/dev/null; do
    printf '\r\033[K[%s] %s: %s %s' "$phase" "$name" "$doing" "${SPIN_FRAMES[i % 10]}"
    i=$((i + 1))
    sleep 0.1
  done
  local rc=0
  wait "$pid" || rc=$?
  printf '\r\033[K\033[?25h'
  return $rc
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version)    shift; VERSION="${1:?--version requires an argument}" ;;
    --insecure-skip-signature) SKIP_SIG=1 ;;
    -h|--help)    grep '^#' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown argument: $1 (see --help)" ;;
  esac
  shift
done

command -v curl >/dev/null 2>&1 || die "curl is required to download the extension — install it and re-run"

if [ "$VERSION" = latest ]; then
  BASE_URL="https://github.com/$REPO/releases/latest/download"
else
  BASE_URL="https://github.com/$REPO/releases/download/$VERSION"
fi
TMP=$(mktemp -d)
CLEANUP_TMP="$TMP"

# Um asset cujo nome é o nome, sem sufixo de arquitectura: o `.vsix` é
# independente de arquitectura e chama-se `delonix-vscode` e mais nada.
fetch_named_asset() { # $1 nome EXACTO do asset
  curl -fsSL -o "$TMP/$1" "$BASE_URL/$1" || return 1
  echo "$1"
}
verify_asset() { # nunca instalar um download sem conferir contra o SHA256SUMS
  [ -s "$TMP/SHA256SUMS" ] \
    || die "could not download SHA256SUMS — check your network and re-run (this is a download failure, not a corrupted/tampered file)"
  ( cd "$TMP" && grep -E " $1\$" SHA256SUMS | sha256sum -c - >/dev/null 2>&1 ) \
    || die "SHA256 verification FAILED for $1 — corrupted or tampered download, aborting"
}
# A assinatura distingue "não corrompeu" de "veio mesmo de nós". Corre ANTES de
# qualquer verify_asset: sem SHA256SUMS autêntico, os hashes não valem nada.
# FAIL-CLOSED. Sem gestor de pacotes/root aqui, se o minisign faltar diz como o
# instalar em vez de o puxar à força.
verify_signature() {
  if [ "$SKIP_SIG" = 1 ]; then
    warn "signature verification SKIPPED (--insecure-skip-signature) — you are trusting whatever the network served"
    return 0
  fi
  command -v minisign >/dev/null 2>&1 \
    || die "minisign is needed to verify the release signature — install it (apt/dnf/zypper/pacman install minisign) and re-run, or pass --insecure-skip-signature if you accept the risk"
  curl -fsSL -o "$TMP/SHA256SUMS.minisig" "$BASE_URL/SHA256SUMS.minisig" \
    || die "this release has no signature (SHA256SUMS.minisig) — releases before signing was introduced need --insecure-skip-signature"
  printf 'untrusted comment: delonix-runtime release key\n%s\n' "$MINISIGN_PUBKEY" > "$TMP/delonix.pub"
  ( cd "$TMP" && minisign -V -p delonix.pub -m SHA256SUMS >/dev/null 2>&1 ) \
    || die "SIGNATURE verification FAILED for SHA256SUMS — this release was NOT signed by the delonix key. Do not install. Report it at https://github.com/$REPO/security"
  step extension signature "minisign verified (SHA256SUMS)"
}

msg "installing the Delonix VS Code extension ($VERSION)..."

# ---- Extensão de editor -----------------------------------------------------
# Um só caminho serve VS Code E os seus forks: o Cursor, o Windsurf, o VSCodium e
# o Antigravity partilham a CLI de extensões (`antigravity --list-extensions`
# responde tal como o `code`). O `codex` NÃO entra: é um agente de linha de
# comandos, não um editor com extensões.
#
# "best-effort" tem de querer dizer que um editor DESISTE, não que PENDURA. O
# sintoma relatado — «o editor abre e a instalação trava» — é o que acontece
# quando um launcher não reconhece um argumento de CLI: abre a JANELA com ele e
# fica em primeiro plano, e o `$(…)`/`spin` espera pelo processo até alguém
# fechar o editor. Daí `editor_cli`: TODA a invocação de um editor é limitada no
# tempo e corre com o stdin FECHADO (o launcher do VS Code faz `read -r YN` em
# WSL e, com `curl … | bash`, o stdin É o script).
EDITOR_TIMEOUT=${DELONIX_EDITOR_TIMEOUT:-120}
editor_cli() {
  local ed="$1" out="$2"; shift 2
  if command -v timeout >/dev/null 2>&1; then
    DONT_PROMPT_WSL_INSTALL=1 timeout -k 5 "$EDITOR_TIMEOUT" "$ed" "$@" </dev/null >"$out" 2>&1
  else
    DONT_PROMPT_WSL_INSTALL=1 "$ed" "$@" </dev/null >"$out" 2>&1
  fi
}

EDITORS_FOUND=""
for ed in code code-insiders codium cursor windsurf antigravity; do
  command -v "$ed" >/dev/null 2>&1 && EDITORS_FOUND="$EDITORS_FOUND $ed"
done
# Um editor só entra na lista depois de PROVAR que fala CLI, e a sonda é
# `--version`: o comando mais barato que toda a família VS Code serve sem GUI.
# Quem não responde nunca recebe um caminho de `.vsix` — é precisamente esse
# argumento que um launcher sem ramo de CLI abriria numa janela.
EDITORS_CLI=""
for ed in $EDITORS_FOUND; do
  ed_rc=0; editor_cli "$ed" "$TMP/.ed-probe" --version || ed_rc=$?
  case "$ed_rc" in
    0) EDITORS_CLI="$EDITORS_CLI $ed" ;;
    124|137)
      warn "$ed did not answer \`--version\` within ${EDITOR_TIMEOUT}s — it is not usable as a CLI from here (a window of it may have opened; close it). Skipping its extension: install the .vsix from the editor (raise the ceiling with DELONIX_EDITOR_TIMEOUT=<seconds>)" ;;
    *)
      warn "$ed does not answer \`--version\` (exit $ed_rc) — skipping its extension: $(tail -1 "$TMP/.ed-probe" 2>/dev/null)" ;;
  esac
done
if [ -z "$EDITORS_FOUND" ]; then
  die "no VS Code-family editor on this host (code/code-insiders/codium/cursor/windsurf/antigravity) — nothing to install the extension into"
elif [ -z "$EDITORS_CLI" ]; then
  die "no editor here answers on the CLI (see the warnings above) — install the .vsix from the editor by hand"
fi

# Sem `2>/dev/null`: foi silenciar este stderr que escondeu o 404 e fez o SKIP
# abaixo afirmar uma coisa falsa durante uma release inteira.
verify_signature
dl_vsix() { fetch_named_asset delonix-vscode > "$TMP/.asset-vsix"; }
if spin extension download "downloading..." dl_vsix; then
  VSIX_ASSET=$(cat "$TMP/.asset-vsix")
  curl -fsSL -o "$TMP/SHA256SUMS" "$BASE_URL/SHA256SUMS" 2>/dev/null || true
  verify_asset "$VSIX_ASSET"
  # O CLI do VS Code EXIGE que o ficheiro termine em `.vsix`: sem isso lê o
  # argumento como um ID de extensão e responde «make sure you use the full
  # extension ID, including the publisher», frase que nada tem que ver com a
  # causa. O asset chama-se `delonix-vscode` porque é esse o nome que o
  # SHA256SUMS assinado cobre — verifica-se com o nome dele e instala-se com uma
  # cópia que tem a extensão que o editor pede.
  cp "$TMP/$VSIX_ASSET" "$TMP/$VSIX_ASSET.vsix"
  for ed in $EDITORS_CLI; do
    # NUNCA sobrepor uma extensão já instalada: a release traz a versão que
    # existia quando foi construída, e essa pode ser MAIS VELHA do que a que o
    # editor tem. Quando a extensão estiver nas galerias é o editor que a mantém
    # em dia; o trabalho deste script é a PRIMEIRA instalação, e mais nenhum.
    ed_rc=0; editor_cli "$ed" "$TMP/.ed-list" --list-extensions --show-versions || ed_rc=$?
    if [ "$ed_rc" != 0 ]; then
      warn "$ed could not list its extensions (exit $ed_rc) — skipping it: $(tail -1 "$TMP/.ed-list" 2>/dev/null)"
      continue
    fi
    HAVE=$(grep -i '^ngolacloud\.delonix@' "$TMP/.ed-list" | head -1 || true)
    if [ -n "$HAVE" ]; then
      skip extension "$ed already has ${HAVE#*@} — leaving it alone (update it from the editor, or install a .vsix by hand)"
      continue
    fi
    # Com um perfil frio esta chamada mede ~20 s: sem o spinner é indistinguível
    # de travada, que é metade da queixa.
    ed_rc=0
    spin extension "$ed" "installing..." \
      editor_cli "$ed" "$TMP/.ed-install" --install-extension "$TMP/$VSIX_ASSET.vsix" --force || ed_rc=$?
    case "$ed_rc" in
      0) stepok extension "$ed" ;;
      124|137)
        warn "$ed did not finish installing the extension within ${EDITOR_TIMEOUT}s — gave up (a window of it may have opened; close it). Install it by hand: $ed --install-extension <the .vsix from the release>" ;;
      *)
        warn "could not install the extension into $ed (exit $ed_rc): $(tail -1 "$TMP/.ed-install" 2>/dev/null) — install it by hand: $ed --install-extension <the .vsix from the release>" ;;
    esac
  done
else
  die "this release ships no editor extension (delonix-vscode asset not found for $VERSION)"
fi
# ---- fim da extensão de editor ----------------------------------------------

msg "done"
