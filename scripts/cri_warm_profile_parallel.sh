#!/usr/bin/env bash
# Perfil do PLANO MORNO sob CONCORRÊNCIA (ADR-0095), medido via CRI REAL.
#
# Prova que o refill assíncrono tira o spawn/warm (e o round-trip de serviço) de
# dentro do mutex do plano: DOIS `StartContainer` mornos para imagens DIFERENTES
# (sub-pools quentes), disparados EM PARALELO, não se serializam um no outro —
# (a) os seus intervalos [início,fim] de `crictl start` SOBREPÕEM-SE, e (b) ambos
# os `pre_spawn` ficam ~7 ms (nenhum inflaciona pela espera do lock nem pelo
# refill, que agora corre na thread de fundo `warm-refiller`).
#
# É o irmão concorrente de `scripts/cri_warm_profile.sh` (sequencial, 1 imagem).
# Sobe UM `delonix-cri` isolado com WARM=1 e o profiler embutido, puxa 2 imagens,
# cria 2 pods (A, B), aquece cada sub-pool (1.º membro frio cria o netns + semeia;
# espera o refiller encher) e depois, por ronda, cria c_i em A e B e dispara os
# dois `crictl start` ao MESMO tempo, medindo cada um.
#
# Pré-requisitos: `crictl` no PATH; host ROOTLESS; `DELONIX_CNI` NÃO definido (cai
# no caminho SDN nativo). Medir SÓ com o host ocioso (load1 < nproc/2) — a via
# isolada é hipersensível a carga.
#
#   scripts/cri_warm_profile_parallel.sh                 # 2 imagens, 4 rondas mornas
#   ROUNDS=6 POOL_SIZE=3 scripts/cri_warm_profile_parallel.sh
#   KEEP=1 scripts/cri_warm_profile_parallel.sh          # não derruba servidor/estado
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${PROFILE:-release}"
BIN_DIR="$ROOT_DIR/target/$PROFILE"
CRI_BIN="$BIN_DIR/delonix-cri"
CLI_BIN="$BIN_DIR/delonix"
ROUNDS="${ROUNDS:-4}"                 # rondas mornas em paralelo (por imagem)
POOL_SIZE="${POOL_SIZE:-4}"           # runners parqueados por imagem
IMG_A="${IMG_A:-docker.io/library/alpine:latest}"
IMG_B="${IMG_B:-docker.io/library/busybox:latest}"
SEED_WAIT="${SEED_WAIT:-3}"           # s para a thread de fundo encher os sub-pools

command -v crictl >/dev/null || { echo "ERRO: crictl não está no PATH"; exit 2; }

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

mkpod() { # $1=nome $2=uid
  cat > "$WORK/pod.$1.json" <<JSON
{ "metadata": { "name": "$1", "namespace": "default", "attempt": 1, "uid": "$2" },
  "log_directory": "/tmp", "linux": {} }
JSON
}
mkctr() { # $1=pod-tag $2=img $3=idx -> escreve ctr json, ecoa o caminho
  local f="$WORK/ctr.$1.$3.json"
  cat > "$f" <<JSON
{ "metadata": { "name": "$1c$3" }, "image": { "image": "$2" },
  "command": ["sleep", "3600"], "log_path": "$1c$3.log", "linux": {} }
JSON
  echo "$f"
}

cleanup() {
  [[ "${KEEP:-}" == "1" ]] && { echo "KEEP=1 → work=$WORK sock=$SOCK prof=$PROF"; return; }
  [[ -n "${POD_A:-}" ]] && crictl --runtime-endpoint "$EP" rmp -f "$POD_A" >/dev/null 2>&1
  [[ -n "${POD_B:-}" ]] && crictl --runtime-endpoint "$EP" rmp -f "$POD_B" >/dev/null 2>&1
  # Matar o GRUPO de sessão do nosso servidor (setsid → pgid=SRV_PID) reap-a o
  # servidor E os __poolrunner que semeámos; NÃO toca em holders/runners alheios.
  [[ -n "${SRV_PID:-}" ]] && kill -TERM "-$SRV_PID" 2>/dev/null
  DELONIX_ROOT="$DELONIX_ROOT" "$CLI_BIN" netns down >/dev/null 2>&1
  rm -rf "$WORK" 2>/dev/null   # bloqueado pelo sandbox nalguns hosts; ignorar
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

echo "==> pull $IMG_A ; $IMG_B"
crictl --runtime-endpoint "$EP" pull "$IMG_A" >/dev/null || { echo "ERRO pull A"; exit 3; }
crictl --runtime-endpoint "$EP" pull "$IMG_B" >/dev/null || { echo "ERRO pull B"; exit 3; }

mkpod A warmpod-A-uid-1; mkpod B warmpod-B-uid-1
POD_A="$(crictl --runtime-endpoint "$EP" runp "$WORK/pod.A.json")" || { echo "ERRO runp A"; cat "$SRVLOG"; exit 3; }
POD_B="$(crictl --runtime-endpoint "$EP" runp "$WORK/pod.B.json")" || { echo "ERRO runp B"; cat "$SRVLOG"; exit 3; }

# 1.º membro de cada pod: FRIO (cria o netns do pod + semeia o sub-pool da imagem).
echo "==> aquecer: 1.º membro frio por pod (cria netns + semeia)"
warm_pod() { # $1=tag $2=img $3=pod
  local f cid
  f="$(mkctr "$1" "$2" 1)"
  cid="$(crictl --runtime-endpoint "$EP" create "$3" "$f" "$WORK/pod.$1.json")" \
    || { echo "ERRO create ${1}c1"; cat "$SRVLOG"; exit 3; }
  crictl --runtime-endpoint "$EP" start "$cid" >/dev/null \
    || { echo "ERRO start ${1}c1"; cat "$SRVLOG"; exit 3; }
}
warm_pod A "$IMG_A" "$POD_A"
warm_pod B "$IMG_B" "$POD_B"
echo "   espera ${SEED_WAIT}s para o warm-refiller encher os 2 sub-pools…"
sleep "$SEED_WAIT"
# NB: `pgrep __poolrunner` conta TAMBÉM os supervisores por-container (herdam o
# argv do runner que os forkou), por isso cresce com os containers vivos — não é
# a contagem de runners parqueados (essa é limitada por DELONIX_CRI_WARM_CAP).
echo "   processos com argv __poolrunner (runners parqueados + supervisores vivos): $(pgrep -fc "__poolrunner" 2>/dev/null || echo 0)"

# ms epoch (bc de date +%s.%N), para medir início/fim de cada start e a sobreposição.
now_ms() { echo "$(date +%s.%N) * 1000" | bc; }

echo
echo "== rondas mornas EM PARALELO (c2..c$((ROUNDS+1))): A ‖ B =="
echo "   (chave do não-bloqueio: por ronda A≈B ⇒ nenhum espera pelo outro; se o"
echo "    mutex serializasse, um start pagaria ~+58 ms do refill do outro)"
OVERLAP_OK=0; OVERLAP_TOT=0; ASYM_SUM=0
for i in $(seq 2 $((ROUNDS+1))); do
  fa="$(mkctr A "$IMG_A" "$i")"; fb="$(mkctr B "$IMG_B" "$i")"
  CA="$(crictl --runtime-endpoint "$EP" create "$POD_A" "$fa" "$WORK/pod.A.json")" || { echo "ERRO create Ac$i"; exit 3; }
  CB="$(crictl --runtime-endpoint "$EP" create "$POD_B" "$fb" "$WORK/pod.B.json")" || { echo "ERRO create Bc$i"; exit 3; }
  # Dispara os DOIS starts ao mesmo tempo; cada subshell regista o seu [início,fim].
  ( b=$(now_ms); crictl --runtime-endpoint "$EP" start "$CA" >/dev/null 2>&1; e=$(now_ms); echo "$b $e" > "$WORK/t.A.$i" ) &
  pa=$!
  ( b=$(now_ms); crictl --runtime-endpoint "$EP" start "$CB" >/dev/null 2>&1; e=$(now_ms); echo "$b $e" > "$WORK/t.B.$i" ) &
  pb=$!
  wait "$pa" "$pb"
  read -r ba ea < "$WORK/t.A.$i"; read -r bb eb < "$WORK/t.B.$i"
  da=$(echo "$ea - $ba" | bc); db=$(echo "$eb - $bb" | bc)
  # Sobreposição = max(0, min(fim) - max(início)). >0 ⇒ correram em simultâneo.
  lo=$(echo "if ($ba > $bb) $ba else $bb" | bc); hi=$(echo "if ($ea < $eb) $ea else $eb" | bc)
  ov=$(echo "$hi - $lo" | bc)
  OVERLAP_TOT=$((OVERLAP_TOT+1)); [[ $(echo "$ov > 0" | bc) == 1 ]] && OVERLAP_OK=$((OVERLAP_OK+1))
  asym=$(echo "d=$da-$db; if (d < 0) -d else d" | bc)   # |A−B|
  ASYM_SUM=$(echo "$ASYM_SUM + $asym" | bc)
  printf "   ronda %-2s  A start=%6.0fms  B start=%6.0fms  |A−B|=%5.0fms  sobreposição=%6.0fms  %s\n" \
    "$i" "$da" "$db" "$asym" "$ov" "$([[ $(echo "$ov > 0" | bc) == 1 ]] && echo '‖ concorrente' || echo 'serializado!')"
  sleep 1   # deixar o warm-refiller repor ambos os sub-pools antes da ronda seguinte
done

echo
echo "== sobreposição: $OVERLAP_OK/$OVERLAP_TOT rondas com os 2 starts a correr em simultâneo =="
printf "== assimetria média |A−B| = %.0f ms (pequena ⇒ nenhum espera pelo outro; picos partilhados = carga do host, simétricos) ==\n" \
  "$(echo "scale=2; $ASYM_SUM / $OVERLAP_TOT" | bc)"
echo "== runners residentes agora (devem existir; __apirun deve ser 0) =="
echo "   __poolrunner: $(pgrep -fc "__poolrunner" 2>/dev/null || echo 0) | __apirun: $(pgrep -fc "__apirun" 2>/dev/null || echo 0)"
echo
echo "== pre_spawn = wall − spawn_total (via warm_profile_report.py) =="
echo "   NOTA: com 2 starts concorrentes as linhas do profiler interlaçam-se; o"
echo "   emparelhador pode perder/trocar pares, mas como AMBAS as fases são warm o"
echo "   veredicto (~7 ms) é robusto ao interlace (ver cabeçalho do script)."
python3 "$ROOT_DIR/scripts/warm_profile_report.py" "$PROF" || { echo "(report falhou; dump cru)"; cat "$PROF"; }
