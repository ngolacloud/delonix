#!/usr/bin/env bash
# Perfil do PLANO MORNO ligado ao `serve cri` (ADR-0095), medido via CRI REAL.
#
# Prova que, com `DELONIX_CRI_WARM=1`, um `StartContainer` para um membro de pod
# SDN nativo (rootless) cuja imagem tem sub-pool quente é servido por um RUNNER
# RESIDENTE parqueado no holder — sem o processo frio `__apirun`/`netns run` — e
# faz cair o `pre_spawn` para ~7 ms. É o análogo CRI do `scripts/warm_profile.sh`
# (que mede o `__poolbench`): sobe um `delonix-cri` isolado com o plano morno ON e
# o profiler embutido (`DELONIX_PROFILE_SPAWN`), cria UM pod com vários containers
# alpine (o 1.º membro cria o netns do pod a frio e SEMEIA o sub-pool; os membros
# seguintes são servidos mornos) e emparelha, por start morno, a linha `bench`
# (wall que o CRI emite) com a `parent` do runner (spawn_total):
#
#     pre_spawn = wall − spawn_total        (a MESMA definição do __poolbench)
#
# Pré-requisitos: `crictl` no PATH; host ROOTLESS; `DELONIX_CNI` NÃO definido (para
# cair no caminho SDN nativo, não CNI). Medir SÓ com o host ocioso (load1 <
# nproc/2) — a via isolada é hipersensível a carga.
#
#   scripts/cri_warm_profile.sh            # 1 pod, 6 containers (1 frio + 5 mornos)
#   NCTR=8 POOL_SIZE=4 scripts/cri_warm_profile.sh
#   KEEP=1 scripts/cri_warm_profile.sh     # não derruba o servidor/estado no fim
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${PROFILE:-release}"
BIN_DIR="$ROOT_DIR/target/$PROFILE"
CRI_BIN="$BIN_DIR/delonix-cri"
CLI_BIN="$BIN_DIR/delonix"
NCTR="${NCTR:-6}"
POOL_SIZE="${POOL_SIZE:-4}"
IMAGE="${IMAGE:-docker.io/library/alpine:latest}"

command -v crictl >/dev/null || { echo "ERRO: crictl não está no PATH"; exit 2; }

# Build idempotente dos DOIS binários — o CRI delega o ciclo de vida na CLI irmã
# (`cli_bin`), e os runners são lançados a partir DELA (intercepta `__poolrunner`),
# por isso os dois binários TÊM de coabitar em target/<perfil>/.
if [[ "${NO_BUILD:-}" != "1" ]]; then
  ( cd "$ROOT_DIR" && cargo build $([[ "$PROFILE" == release ]] && echo --release) \
      -p delonix-cri-bin --bin delonix-cri -p delonix-bin --bin delonix ) || exit $?
fi
[[ -x "$CRI_BIN" && -x "$CLI_BIN" ]] || { echo "ERRO: binários em falta em $BIN_DIR"; exit 2; }

WORK="$(mktemp -d /tmp/delonix-cri-warm.XXXXXX)"
DELONIX_ROOT="$WORK/root"; mkdir -p "$DELONIX_ROOT"
SOCK="$WORK/cri.sock"; EP="unix://$SOCK"
PROF="$WORK/profile.jsonl"; : > "$PROF"
SRVLOG="$WORK/server.log"
export DELONIX_ROOT

cat > "$WORK/pod.json" <<'JSON'
{ "metadata": { "name": "warmpod", "namespace": "default", "attempt": 1, "uid": "warmpod-uid-1" },
  "log_directory": "/tmp", "linux": {} }
JSON

cleanup() {
  [[ "${KEEP:-}" == "1" ]] && { echo "KEEP=1 → work=$WORK sock=$SOCK prof=$PROF"; return; }
  [[ -n "${POD:-}" ]] && crictl --runtime-endpoint "$EP" rmp -f "$POD" >/dev/null 2>&1
  [[ -n "${SRV_PID:-}" ]] && kill "$SRV_PID" 2>/dev/null
  DELONIX_ROOT="$DELONIX_ROOT" "$CLI_BIN" netns down >/dev/null 2>&1
  rm -rf "$WORK"
}
trap cleanup EXIT

echo "== load1 agora: $(cut -d' ' -f1 /proc/loadavg) (nproc=$(nproc)); medir só com < $(( $(nproc) / 2 )) =="
echo "==> arranque delonix-cri (WARM=1 size=$POOL_SIZE) root=$DELONIX_ROOT"
DELONIX_ROOT="$DELONIX_ROOT" DELONIX_CRI_ADDR="$EP" DELONIX_CRI_CAP_CEILING=all \
  DELONIX_CRI_WARM=1 DELONIX_CRI_WARM_SIZE="$POOL_SIZE" DELONIX_CRI_WARM_CAP=16 \
  DELONIX_PROFILE_SPAWN="$PROF" \
  setsid "$CRI_BIN" >"$SRVLOG" 2>&1 < /dev/null &
SRV_PID=$!
for _ in $(seq 1 50); do [[ -S "$SOCK" ]] && break; sleep 0.2; done
[[ -S "$SOCK" ]] || { echo "ERRO: servidor não abriu o socket"; cat "$SRVLOG"; exit 3; }
crictl --runtime-endpoint "$EP" version >/dev/null || { echo "ERRO: crictl não fala"; cat "$SRVLOG"; exit 3; }

echo "==> pull $IMAGE"
crictl --runtime-endpoint "$EP" pull "$IMAGE" >/dev/null || { echo "ERRO pull"; exit 3; }
echo "==> runp pod"
POD="$(crictl --runtime-endpoint "$EP" runp "$WORK/pod.json")" || { echo "ERRO runp"; cat "$SRVLOG"; exit 3; }

for i in $(seq 1 "$NCTR"); do
  cat > "$WORK/ctr.$i.json" <<JSON
{ "metadata": { "name": "c$i" }, "image": { "image": "$IMAGE" },
  "command": ["sleep", "3600"], "log_path": "c$i.log", "linux": {} }
JSON
  CID="$(crictl --runtime-endpoint "$EP" create "$POD" "$WORK/ctr.$i.json" "$WORK/pod.json")" \
    || { echo "ERRO create c$i"; cat "$SRVLOG"; exit 3; }
  T0=$(date +%s.%N)
  crictl --runtime-endpoint "$EP" start "$CID" >/dev/null || { echo "ERRO start c$i"; cat "$SRVLOG"; exit 3; }
  T1=$(date +%s.%N)
  kind="morno (runner residente)"; [[ "$i" == "1" ]] && kind="frio (cria o netns + semeia)"
  printf "   c%-2s start=%6.0fms  (%s)\n" "$i" "$(echo "($T1-$T0)*1000"|bc)" "$kind"
done

echo
echo "== runners residentes agora parqueados (devem existir; __apirun deve ser 0) =="
echo "   __poolrunner: $(pgrep -fc "__poolrunner" 2>/dev/null || echo 0) | __apirun: $(pgrep -fc "__apirun" 2>/dev/null || echo 0)"
echo
echo "== pre_spawn = wall − spawn_total (via warm_profile_report.py) =="
python3 "$ROOT_DIR/scripts/warm_profile_report.py" "$PROF" || { echo "(report falhou; dump cru)"; cat "$PROF"; }
