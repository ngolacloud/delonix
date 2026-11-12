#!/usr/bin/env bash
# Corre a campanha de prontidão para produção (`prod-campaign-multinode.sh`) UMA
# vez, quando o host estiver OCIOSO. Feito para ser disparado por um timer/cron
# periódico num host partilhado: não se pode saber de antemão quando a carga
# baixa, por isso o timer dispara de hora a hora e ESTE wrapper decide.
#
# Três estados, por disparada:
#   • já corrida (sentinela existe)  → sai 0, nada a fazer (não repete);
#   • host contendido (load alto)    → sai 0, adia para a próxima disparada;
#   • host ocioso                    → corre a campanha, guarda o relatório,
#                                       escreve a sentinela com o resultado.
# Re-correr de propósito: apaga a sentinela (`$STATE/done`).
#
# ───────────────────────────── instalar (systemd --user, SEM sudo) ──────────
#   REPO=/caminho/para/delonix-runtime
#   mkdir -p ~/.config/systemd/user
#   cat > ~/.config/systemd/user/delonix-prod-campaign.service <<EOF
#   [Unit]
#   Description=Delonix — campanha de produção quando o host estiver ocioso
#   [Service]
#   Type=oneshot
#   Environment=MAX_LOAD=4.0
#   ExecStart=$REPO/scripts/prod-campaign-when-idle.sh
#   EOF
#   cat > ~/.config/systemd/user/delonix-prod-campaign.timer <<EOF
#   [Unit]
#   Description=Dispara a campanha de produção de hora a hora (corre quando ociosa)
#   [Timer]
#   OnBootSec=15min
#   OnUnitActiveSec=1h
#   Persistent=true
#   [Install]
#   WantedBy=timers.target
#   EOF
#   loginctl enable-linger "$USER"        # o timer sobrevive a logout/reboot
#   systemctl --user daemon-reload
#   systemctl --user enable --now delonix-prod-campaign.timer
#   # ver:    systemctl --user list-timers | grep delonix
#   #         journalctl --user -u delonix-prod-campaign.service
#   # parar:  systemctl --user disable --now delonix-prod-campaign.timer
#
# ───────────────────────────── alternativa cron ─────────────────────────────
#   crontab -e →  7 * * * *  MAX_LOAD=4.0 /caminho/delonix-runtime/scripts/prod-campaign-when-idle.sh >> ~/.local/state/delonix-prod-campaign/cron.log 2>&1
#
# NB: NÃO é um job de CI — a campanha precisa de kind multi-nó privilegiado +
# KVM (+ Ceph p/ o C4), que os runners de GitHub Actions não têm. Corre num
# host capaz (o teu portátil, ou uma VM/servidor dedicado).
#
# Env:  MAX_LOAD=4.0  · DELONIX_BIN=<path>  · DELONIX_CAMPAIGN_STATE=<dir>
#       CAMPAIGN_ARGS="--workers 2 --control-planes 1"  (passado ao runner)
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
STATE="${DELONIX_CAMPAIGN_STATE:-${XDG_STATE_HOME:-$HOME/.local/state}/delonix-prod-campaign}"
MAX_LOAD="${MAX_LOAD:-4.0}"
RUNNER="$ROOT_DIR/scripts/prod-campaign-multinode.sh"
mkdir -p "$STATE"
SENTINEL="$STATE/done"
log() { echo "$(date -Is) $*"; }

[ -x "$RUNNER" ] || { log "sem $RUNNER (constrói/actualiza o repo)"; exit 1; }

if [ -f "$SENTINEL" ]; then
  log "já corrida ($(cat "$SENTINEL")) — nada a fazer. Apaga $SENTINEL para re-correr."
  exit 0
fi

LOAD=$(awk '{print $1}' /proc/loadavg)
if awk "BEGIN{exit !($LOAD > $MAX_LOAD)}"; then
  log "host ocupado (load $LOAD > $MAX_LOAD) — adio para a próxima disparada."
  exit 0
fi

BIN="${DELONIX_BIN:-$ROOT_DIR/target/release/delonix}"
[ -x "$BIN" ] || BIN="$ROOT_DIR/target/debug/delonix"
TS=$(date +%Y%m%d-%H%M%S)
REPORT="$STATE/campaign-$TS.log"
log "host ocioso (load $LOAD) — a correr a campanha → $REPORT"

# shellcheck disable=SC2086
if DELONIX_BIN="$BIN" MAX_LOAD="$MAX_LOAD" bash "$RUNNER" ${CAMPAIGN_ARGS:-} >"$REPORT" 2>&1; then
  printf 'PASS %s (%s)\n' "$(date -Is)" "$(basename "$REPORT")" > "$SENTINEL"
  log "campanha PASSOU — relatório: $REPORT"
else
  rc=$?
  printf 'DONE-rc%s %s (%s)\n' "$rc" "$(date -Is)" "$(basename "$REPORT")" > "$SENTINEL"
  log "campanha terminou com rc=$rc (ver FAIL/SKIP no relatório: $REPORT). Sentinela escrita (uma corrida autoritária); apaga-a para repetir."
fi
