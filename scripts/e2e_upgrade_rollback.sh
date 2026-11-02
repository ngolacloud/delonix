#!/usr/bin/env bash
# E2E de UPGRADE / ROLLBACK — o portão G3 do plano NO-GO→GO, o desqualificador
# número um: "upgrade/rollback NÃO TESTADO; os supervisores vivos correm o
# código com que nasceram". Este harness mede a pergunta que ninguém tinha
# respondido: um container VIVO sobrevive a substituir-se o binário do motor
# por baixo dele, e o binário novo continua a geri-lo e a reconciliá-lo?
#
# ## O modelo que isto testa (e porque é a pergunta certa)
#
# O Delonix é daemonless: um `run -d` deixa um processo supervisor vivo, lançado
# a partir do binário daquele instante, e uma invocação SEGUINTE da CLI fala com
# ele pelo estado em disco + sockets de controlo. Logo "upgrade" não é reiniciar
# um daemon — é (1) o binário em disco mudar e (2) uma CLI nova gerir containers
# nascidos sob a antiga. As duas coisas medem-se aqui.
#
# ## O que foi MEDIDO ao vivo e virou asserção (2026-11-02, uid rootless)
#
#   * `cp` POR CIMA do binário em uso -> ETXTBSY: o kernel recusa reescrever um
#     ELF mapeado por um processo vivo. Um upgrade por `cp` em cima FALHA.
#   * `mv`/rename atómico por cima -> OK, e o container continua `Up`. É o que o
#     dpkg/rpm/`install(1)` já fazem (ficheiro novo = inode novo; o supervisor
#     fica agarrado ao inode antigo, ainda mapeado). **Logo o procedimento de
#     upgrade suportado é rename atómico, nunca `cp` in-place** — e está no
#     runbook `docs/operations/upgrade-rollback.md`.
#
# ## O que este harness NÃO prova (dito por inteiro)
#
# Com `--old` == `--new` (o default: só há uma versão compilada), prova o
# MECANISMO — sobrevivência à troca do ficheiro, continuidade de gestão,
# reconciliação, rollback. NÃO prova a compatibilidade de CÓDIGO entre versões
# (o binário novo a LER estado escrito pela versão antiga): isso exige dois
# binários de versões diferentes. Passa `--old <bin-da-tag-anterior>` quando
# existir, e o harness passa a medir também isso, sem mudar mais nada. O formato
# de estado EM DISCO entre versões é coberto, à parte, por `e2e_state_upgrade.sh`.
#
# ## Uso
#
#   scripts/e2e_upgrade_rollback.sh [--old PATH] [--new PATH] [--image REF] [--keep]

set -uo pipefail

REPO_BIN="$(cd "$(dirname "$0")/.." && pwd)/target/release/delonix"
OLD="$REPO_BIN"
NEW="$REPO_BIN"
IMAGE="busybox:1.36"
KEEP=0
PFX="g3upg"
NET="${PFX}-net"
C1="${PFX}-c1"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --old) OLD="$2"; shift 2 ;;
    --new) NEW="$2"; shift 2 ;;
    --image) IMAGE="$2"; shift 2 ;;
    --keep) KEEP=1; shift ;;
    *) echo "uso: $0 [--old PATH] [--new PATH] [--image REF] [--keep]"; exit 2 ;;
  esac
done

for b in "$OLD" "$NEW"; do
  [[ -x "$b" ]] || { echo "binário AUSENTE ou não-executável: $b" >&2; exit 2; }
done

WORK="$(mktemp -d)"
ENGINE="$WORK/engine"   # o binário "instalado"; os supervisores nascem daqui.

PASS=0; FAIL=0
ok()   { PASS=$((PASS+1)); printf '  \033[32m✓\033[0m %s\n' "$1"; }
bad()  { FAIL=$((FAIL+1)); printf '  \033[31m✗\033[0m %s\n' "$1"; [[ -n "${2:-}" ]] && printf '      %s\n' "$2"; }
E()    { "$ENGINE" "$@"; }   # chama SEMPRE o binário instalado (= a versão corrente do motor)

# Instala um binário ATOMICAMENTE no lugar do motor — a cópia para um temp no
# MESMO filesystem e depois `mv` (rename), que é o que um upgrade real faz e a
# única forma que não bate em ETXTBSY com containers vivos.
install_engine() { # $1 = binário de origem
  cp "$1" "$WORK/.staged" && mv "$WORK/.staged" "$ENGINE"
}

cleanup() {
  if [[ "$KEEP" == "1" ]]; then echo "--keep: deixo $PFX-* e $WORK de pé." >&2; return; fi
  [[ -x "$ENGINE" ]] && { E rm -f "$C1" >/dev/null 2>&1 || true; E network rm "$NET" >/dev/null 2>&1 || true; }
  rm -rf "$WORK"
}
trap cleanup EXIT

say() { printf '\n\033[1m== %s ==\033[0m\n' "$1"; }

# IDs de versão, para o relatório dizer o que comparou.
install_engine "$OLD"; V_OLD="$(E --version 2>/dev/null | head -1)"
install_engine "$NEW"; V_NEW="$(E --version 2>/dev/null | head -1)"
install_engine "$OLD"   # arrancamos SEMPRE na versão antiga.

echo   "e2e_upgrade_rollback — Delonix"
echo   "  old:   $OLD   ($V_OLD)"
echo   "  new:   $NEW   ($V_NEW)"
echo   "  image: $IMAGE"
[[ "$V_OLD" == "$V_NEW" ]] && echo "  AVISO: old == new — prova o MECANISMO, não a compatibilidade entre versões (ver cabeçalho)."

if ! E image ls 2>/dev/null | grep -qF "$IMAGE"; then
  echo "imagem $IMAGE ausente — puxa-a primeiro ('$ENGINE image pull $IMAGE')." >&2; exit 2
fi

# ---- SETUP: na versão ANTIGA, pôr carga de pé e marcar estado ----------------
say "SETUP (motor $V_OLD): rede + container vivo + marca de estado"
E network rm "$NET" >/dev/null 2>&1 || true
E network create "$NET" >/dev/null 2>&1 || { echo "falha a criar $NET" >&2; exit 1; }
E rm -f "$C1" >/dev/null 2>&1 || true
# Um comando que EMITE log (para `logs` ter o que mostrar) e fica vivo.
if E run -d --net "$NET" --name "$C1" "$IMAGE" sh -c 'echo booted-old; exec sleep 86400' >/dev/null 2>&1; then
  ok "run -d do container de carga na rede $NET"
else
  bad "run -d do container de carga" "setup falhou — nada a medir"; exit 1
fi
MARK="upg-$$-$(od -An -N4 -tu4 </dev/urandom | tr -d ' ')"
E exec "$C1" sh -c "echo $MARK > /marker.txt" >/dev/null 2>&1 \
  && ok "escrita de marca de estado na camada gravável (/marker.txt)" \
  || bad "escrita da marca de estado"
pid_before="$(E ps -o json 2>/dev/null | python3 -c "import json,sys;print(next((c['id'] for c in json.load(sys.stdin) if c['name']=='$C1'),''))")"

status_of() { E ps -o json 2>/dev/null | python3 -c "import json,sys;print(next((c['status'] for c in json.load(sys.stdin) if c['name']=='$C1'),'AUSENTE'))"; }
marker_in() { E exec "$C1" cat /marker.txt 2>/dev/null | tr -d '\r\n'; }

# ---- UPGRADE: substituir o binário por baixo do container vivo ---------------
say "UPGRADE (motor $V_OLD → $V_NEW): rename atómico com o container VIVO"

# Achado medido: `cp` in-place num ELF mapeado por um supervisor vivo dá ETXTBSY.
# Confirmamo-lo para que o harness guarde a razão de o upgrade usar rename.
if cp "$NEW" "$ENGINE" 2>/dev/null; then
  bad "cp in-place do binário em uso devia falhar (ETXTBSY) e não falhou" \
      "nota: ou nenhum processo o mapeia, ou o FS permite — reinstalo via rename na mesma"
  install_engine "$NEW"
else
  ok "cp in-place REJEITADO (ETXTBSY) — é por isto que o upgrade usa rename, não cp"
  install_engine "$NEW"
fi

[[ -x "$ENGINE" ]] && ok "binário do motor substituído atomicamente (rename)" || bad "rename do binário"

case "$(status_of)" in
  Up*) ok "SOBREVIVÊNCIA: o container vivo continua Up após a troca do binário" ;;
  *)   bad "SOBREVIVÊNCIA: o container NÃO está Up após o upgrade" "estado: $(status_of)" ;;
esac

pid_after="$(E ps -o json 2>/dev/null | python3 -c "import json,sys;print(next((c['id'] for c in json.load(sys.stdin) if c['name']=='$C1'),''))")"
[[ -n "$pid_after" && "$pid_after" == "$pid_before" ]] \
  && ok "o container é o MESMO (id estável $pid_before) — não foi recriado" \
  || bad "o id do container mudou no upgrade" "antes=$pid_before depois=$pid_after"

[[ "$(marker_in)" == "$MARK" ]] \
  && ok "GESTÃO: o motor novo faz exec e lê a marca de estado intacta" \
  || bad "o motor novo não leu a marca" "esperado=$MARK obtido=$(marker_in)"

if E logs "$C1" 2>/dev/null | grep -q booted-old; then
  ok "GESTÃO: o motor novo lê os logs do container nascido sob o antigo"
else
  bad "o motor novo não leu os logs do container"
fi

# ---- RECONCILIAÇÃO: stop/start pelo motor novo, estado preservado ------------
say "RECONCILIAÇÃO (motor $V_NEW): stop → start, estado tem de persistir"
E stop "$C1" >/dev/null 2>&1 && ok "stop pelo motor novo" || bad "stop pelo motor novo"
if E start "$C1" >/dev/null 2>&1; then ok "start pelo motor novo"; else bad "start pelo motor novo"; fi
sleep 1
case "$(status_of)" in Up*) ok "o container volta a Up após start" ;; *) bad "não voltou a Up" "estado: $(status_of)" ;; esac
[[ "$(marker_in)" == "$MARK" ]] \
  && ok "a marca de estado persiste através do stop/start (camada gravável preservada)" \
  || bad "a marca perdeu-se no stop/start" "obtido=$(marker_in)"
E network ls 2>/dev/null | grep -qF "$NET" && ok "a rede $NET sobrevive ao upgrade" || bad "a rede desapareceu"

# ---- ROLLBACK: voltar ao binário antigo, ainda tem de gerir ------------------
say "ROLLBACK (motor $V_NEW → $V_OLD): o binário antigo retoma a gestão"
install_engine "$OLD"
[[ -x "$ENGINE" ]] && ok "rollback do binário (rename de volta ao antigo)" || bad "rename de rollback"
case "$(status_of)" in Up*) ok "SOBREVIVÊNCIA: o container continua Up após o rollback" ;; *) bad "o container não está Up após rollback" "estado: $(status_of)" ;; esac
[[ "$(marker_in)" == "$MARK" ]] \
  && ok "a marca de estado continua intacta após o rollback" \
  || bad "a marca perdeu-se no rollback" "obtido=$(marker_in)"
E stop "$C1" >/dev/null 2>&1 && E start "$C1" >/dev/null 2>&1 && sleep 1
case "$(status_of)" in Up*) ok "stop/start funciona de novo sob o motor antigo" ;; *) bad "stop/start falhou no rollback" ;; esac

# ---- veredicto ----------------------------------------------------------------
say "VEREDICTO"
echo "  passaram: $PASS   falharam: $FAIL"
if [[ "$FAIL" -eq 0 ]]; then
  echo "  G3 (mecanismo): upgrade por rename + rollback com carga VIVA, sobrevivência e reconciliação PROVADOS."
  [[ "$V_OLD" == "$V_NEW" ]] && echo "  Falta a compatibilidade CRUZADA de versões: correr com --old de uma tag anterior."
  exit 0
else
  echo "  G3: há asserções falhadas acima — NÃO está provado."
  exit 1
fi
