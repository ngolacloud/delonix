#!/usr/bin/env bash
# Campanha de prontidão para produção crítica — nível 2 (multi-nó/Ceph/failover).
#
# Automatiza as provas C1–C5 de `reports/production-readiness-*/campaign-c-*.md`,
# de uma só vez, num ambiente LIMPO. Doutrina do harness: só o EFEITO verificado
# conta — cada prova valida o resultado funcional (nós Ready, pods distribuídos,
# reschedule após falha de nó), nunca `exit 0`.
#
# Porquê um ambiente limpo: a 1ª tentativa de C1 num laptop partilhado sob carga
# (VM + benches + 3 nós) abortou no «timeout waiting for containerd (90s)». Este
# script RECUSA-SE a correr com a máquina contendida (como `bench.sh`), porque
# uma falha de contenção lida como falha do motor envenena o relatório.
#
# Uso:
#   scripts/prod-campaign-multinode.sh [--workers N] [--control-planes M]
#                                      [--keep] [--only C1,C2,...] [--force]
# Env:
#   DELONIX_BIN=target/release/delonix   (senão procura release/debug)
#   MAX_LOAD=4.0                          (guarda de carga; --force ignora)
#   CEPH_STORAGECLASS=<nome>              (C4; sem isto, C4 faz SKIP)
set -uo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WORKERS=2
CPS=1
KEEP=0
ONLY=""
FORCE=0
MAX_LOAD="${MAX_LOAD:-4.0}"
CLUSTER="prodcamp-$$"

while [ $# -gt 0 ]; do
  case "$1" in
    --workers) WORKERS="$2"; shift 2;;
    --control-planes) CPS="$2"; shift 2;;
    --keep) KEEP=1; shift;;
    --only) ONLY="$2"; shift 2;;
    --force) FORCE=1; shift;;
    *) echo "argumento desconhecido: $1" >&2; exit 2;;
  esac
done

# --- binário ---------------------------------------------------------------
BIN="${DELONIX_BIN:-}"
if [ -z "$BIN" ]; then
  for c in "$ROOT_DIR/target/release/delonix" "$ROOT_DIR/target/debug/delonix"; do
    [ -x "$c" ] && BIN="$c" && break
  done
fi
[ -x "$BIN" ] || { echo "sem binário delonix (constrói com 'cargo build --release -p delonix-bin' ou passa DELONIX_BIN)" >&2; exit 1; }
command -v kubectl >/dev/null 2>&1 || { echo "kubectl não está no PATH — necessário para validar os efeitos" >&2; exit 1; }

# strip ANSI/INFO do output do motor
clean() { sed -E 's/\x1b\[[0-9;]*m//g' | grep -viE '^\s*(INFO|DEBUG|WARN) '; }
want()  { [ -z "$ONLY" ] || echo "$ONLY" | tr ',' '\n' | grep -qx "$1"; }

declare -A RESULT   # Cx -> PASS|FAIL|SKIP
declare -A NOTE
mark() { RESULT["$1"]="$2"; NOTE["$1"]="${3:-}"; printf '  [%s] %s — %s\n' "$2" "$1" "${3:-}"; }

KC="$(mktemp -t prodcamp-kc.XXXXXX)"
export KUBECONFIG="$KC"

# --- bancada / guarda de carga --------------------------------------------
echo "== bancada =="
LOAD=$(awk '{print $1}' /proc/loadavg)
echo "  binário:  $($BIN --version 2>/dev/null | head -1)"
echo "  kernel:   $(uname -r)   load(1m): $LOAD   (limiar: $MAX_LOAD)"
echo "  cpu:      $(nproc) threads"
if [ "$FORCE" -ne 1 ] && awk "BEGIN{exit !($LOAD > $MAX_LOAD)}"; then
  echo "RECUSADO: load $LOAD > $MAX_LOAD — a máquina está contendida; uma falha aqui" >&2
  echo "          lê-se como falha do motor e não é. Espera que baixe, ou passa --force." >&2
  exit 3
fi

teardown() {
  [ "$KEEP" -eq 1 ] && { echo "== --keep: cluster '$CLUSTER' mantido =="; rm -f "$KC"; return; }
  echo "== teardown =="
  $BIN cluster destroy "$CLUSTER" >/dev/null 2>&1 || true
  # rede-de-segurança: nós órfãos de um create parcial (BUG C1-2)
  for id in $($BIN container ls -aq 2>/dev/null | grep -viE '^\s*(INFO|DEBUG)'); do
    $BIN container inspect "$id" 2>/dev/null | grep -q "$CLUSTER" && $BIN container rm -f "$id" >/dev/null 2>&1
  done
  rm -f "$KC"
}
trap teardown EXIT

# wait helper: repete `cmd` até dar `expect` ou esgotar `timeout`s
wait_for() { # <timeout_s> <descrição> <cmd...>; o cmd deve devolver 0 quando pronto
  local t="$1" desc="$2"; shift 2
  local deadline=$(( $(date +%s) + t ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    "$@" >/dev/null 2>&1 && return 0
    sleep 3
  done
  echo "  timeout ($t s) à espera: $desc" >&2
  return 1
}

nodes_ready() { # conta nós Ready; arg1 = esperado
  local n; n=$(kubectl get nodes --no-headers 2>/dev/null | grep -cw Ready)
  [ "${n:-0}" -ge "$1" ]
}

# ===========================================================================
# C1 — multi-nó: cluster sobe e escalona um Deployment por VÁRIOS nós
# ===========================================================================
if want C1; then
  echo "== C1: multi-nó ($CPS CP + $WORKERS workers) =="
  if ! $BIN cluster create --name "$CLUSTER" --workers "$WORKERS" --control-planes "$CPS" 2>&1 | clean | tail -3; then
    mark C1 FAIL "cluster create falhou (ver log; achado C1-1: wait de containerd sem retry)"
  else
    $BIN cluster kubeconfig "$CLUSTER" > "$KC" 2>/dev/null
    TOTAL=$((CPS + WORKERS))
    if ! wait_for 180 "$TOTAL nós Ready" nodes_ready "$TOTAL"; then
      mark C1 FAIL "nem todos os nós ficaram Ready ($(kubectl get nodes --no-headers 2>/dev/null | grep -cw Ready)/$TOTAL)"
    else
      # escalonar réplicas e exigir espalhamento por >=2 nós
      kubectl create deployment spread --image=registry.k8s.io/pause:3.9 --replicas=$((TOTAL*2)) >/dev/null 2>&1
      wait_for 120 "pods Running" bash -c 'kubectl get pods -l app=spread --no-headers 2>/dev/null | grep -qw Running'
      sleep 5
      NODES_USED=$(kubectl get pods -l app=spread -o jsonpath='{range .items[*]}{.spec.nodeName}{"\n"}{end}' 2>/dev/null | sort -u | grep -c .)
      if [ "${NODES_USED:-0}" -ge 2 ]; then
        mark C1 PASS "Deployment espalhou por $NODES_USED nós (de $TOTAL)"
      else
        mark C1 FAIL "pods concentrados em $NODES_USED nó(s) — scheduler não espalhou"
      fi
    fi
  fi
fi

# ===========================================================================
# C2 — failover de nó: drenar + matar um worker → o workload RECUPERA
#
# A pergunta correcta NÃO é «sobrou algum pod imputado ao nó morto?» — ao matar
# o container-nó à força, os pods velhos ficam `Terminating`/`Unknown` com
# `nodeName=vítima` até ao eviction-timeout do k8s (~5 min); contá-los é contar
# FANTASMAS, não falha de failover. A pergunta é «as réplicas DESEJADAS voltaram
# a estar disponíveis em nós VIVOS?». Por isso C2 mede duas coisas reais:
#   (1) o `drain` ESVAZIA o nó graciosamente (0 pods `spread` a correr lá), e
#   (2) depois de matar o nó, o Deployment volta a `availableReplicas == desejado`
#       (contagem imune a fantasmas — só conta Running+Ready, em qualquer nó vivo).
# ===========================================================================
if want C2 && [ "${RESULT[C1]:-}" = PASS ]; then
  echo "== C2: failover de nó =="
  VICTIM=$(kubectl get nodes --no-headers 2>/dev/null | awk '{print $1}' | grep -i worker | head -1)
  DESIRED=$(kubectl get deploy spread -o jsonpath='{.spec.replicas}' 2>/dev/null)
  if [ -z "$VICTIM" ] || [ -z "$DESIRED" ]; then
    mark C2 SKIP "sem worker identificável ou Deployment 'spread' ausente (C1?)"
  else
    BEFORE=$(kubectl get pods -l app=spread --field-selector spec.nodeName="$VICTIM" --no-headers 2>/dev/null | grep -cw Running)
    echo "  vítima: $VICTIM ($BEFORE pods lá, de $DESIRED réplicas); 1) drain gracioso..."
    $BIN cluster drain "$VICTIM" >/dev/null 2>&1 || true
    # (1) ESPERAR o drain esvaziar o nó — o sinal real de failover gracioso.
    victim_empty() { [ "$(kubectl get pods -l app=spread --field-selector spec.nodeName="$VICTIM" --no-headers 2>/dev/null | grep -cw Running)" -eq 0 ]; }
    if ! wait_for 180 "o drain a esvaziar '$VICTIM'" victim_empty; then
      mark C2 FAIL "drain não esvaziou '$VICTIM' em 180s (pods não evictados)"
    else
      echo "  2) matar o nó (falha dura) + exigir o Deployment de volta a $DESIRED disponíveis..."
      $BIN container rm -f "$VICTIM" >/dev/null 2>&1 || $BIN container stop "$VICTIM" >/dev/null 2>&1 || true
      # limpar fantasmas Terminating no nó morto (senão lingeram até ~5 min)
      kubectl delete pods -l app=spread --field-selector spec.nodeName="$VICTIM" \
        --grace-period=0 --force --ignore-not-found >/dev/null 2>&1 || true
      # (2) availableReplicas conta só Running+Ready, em qualquer nó vivo — imune a fantasmas.
      deploy_recovered() { [ "$(kubectl get deploy spread -o jsonpath='{.status.availableReplicas}' 2>/dev/null)" = "$DESIRED" ]; }
      if wait_for 180 "o Deployment de volta a $DESIRED réplicas disponíveis" deploy_recovered; then
        mark C2 PASS "drain esvaziou '$VICTIM' + Deployment recuperou a $DESIRED réplicas em nós vivos após matar o nó"
      else
        AVAIL=$(kubectl get deploy spread -o jsonpath='{.status.availableReplicas}' 2>/dev/null)
        mark C2 FAIL "após matar '$VICTIM', o Deployment ficou em ${AVAIL:-0}/$DESIRED disponíveis (não recuperou em 180s)"
      fi
    fi
  fi
elif want C2; then
  mark C2 SKIP "C1 não passou"
fi

# ===========================================================================
# C3 — rede overlay: pod no nó A alcança pod no nó B (CNI cross-node)
# ===========================================================================
if want C3 && [ "${RESULT[C1]:-}" = PASS ]; then
  echo "== C3: overlay cross-node =="
  kubectl run c3srv --image=registry.k8s.io/e2e-test-images/agnhost:2.47 --command -- /agnhost netexec --http-port=8080 >/dev/null 2>&1
  wait_for 120 "c3srv Running" bash -c 'kubectl get pod c3srv --no-headers 2>/dev/null | grep -qw Running'
  SRV_NODE=$(kubectl get pod c3srv -o jsonpath='{.spec.nodeName}' 2>/dev/null)
  SRV_IP=$(kubectl get pod c3srv -o jsonpath='{.status.podIP}' 2>/dev/null)
  # cliente FORÇADO para outro nó (anti-afinidade por hostname)
  OTHER=$(kubectl get nodes --no-headers 2>/dev/null | awk '{print $1}' | grep -v "^$SRV_NODE$" | head -1)
  if [ -z "$OTHER" ] || [ -z "$SRV_IP" ]; then
    mark C3 SKIP "não há segundo nó ou sem IP de pod (SRV_IP=$SRV_IP OTHER=$OTHER)"
  else
    kubectl run c3cli --image=registry.k8s.io/e2e-test-images/agnhost:2.47 \
      --overrides="{\"spec\":{\"nodeName\":\"$OTHER\"}}" --command -- sleep 300 >/dev/null 2>&1
    wait_for 120 "c3cli Running" bash -c 'kubectl get pod c3cli --no-headers 2>/dev/null | grep -qw Running'
    if kubectl exec c3cli -- /agnhost connect "$SRV_IP:8080" --timeout=5s >/dev/null 2>&1 \
       || wait_for 30 "cross-node reach" kubectl exec c3cli -- wget -qO- "http://$SRV_IP:8080/hostname"; then
      mark C3 PASS "pod em '$OTHER' alcançou pod em '$SRV_NODE' ($SRV_IP) — CNI cross-node"
    else
      mark C3 FAIL "pod em '$OTHER' NÃO alcançou '$SRV_IP' no nó '$SRV_NODE' (achado da memória: CNI bloqueou na HA)"
    fi
  fi
  kubectl delete pod c3srv c3cli --ignore-not-found >/dev/null 2>&1
elif want C3; then
  mark C3 SKIP "C1 não passou"
fi

# ===========================================================================
# C4 — Ceph: PV persistente sobrevive à perda do nó (precisa de StorageClass)
# ===========================================================================
if want C4 && [ "${RESULT[C1]:-}" = PASS ]; then
  echo "== C4: armazenamento Ceph persistente =="
  SC="${CEPH_STORAGECLASS:-$(kubectl get storageclass --no-headers 2>/dev/null | awk '/ceph|rbd|rook/{print $1; exit}')}"
  if [ -z "$SC" ]; then
    mark C4 SKIP "sem StorageClass Ceph (define CEPH_STORAGECLASS ou instala rook/ceph-csi)"
  else
    kubectl apply -f - >/dev/null 2>&1 <<EOF
apiVersion: v1
kind: PersistentVolumeClaim
metadata: {name: c4pvc}
spec: {accessModes: [ReadWriteOnce], storageClassName: $SC, resources: {requests: {storage: 1Gi}}}
EOF
    kubectl run c4w --image=busybox --overrides='{"spec":{"volumes":[{"name":"d","persistentVolumeClaim":{"claimName":"c4pvc"}}],"containers":[{"name":"c4w","image":"busybox","command":["sh","-c","echo DELONIX-CEPH-OK > /d/mark; sleep 600"],"volumeMounts":[{"name":"d","mountPath":"/d"}]}]}}' >/dev/null 2>&1
    if wait_for 180 "escrita no PV Ceph" bash -c 'kubectl exec c4w -- cat /d/mark 2>/dev/null | grep -q DELONIX-CEPH-OK'; then
      W_NODE=$(kubectl get pod c4w -o jsonpath='{.spec.nodeName}' 2>/dev/null)
      $BIN container rm -f "$W_NODE" >/dev/null 2>&1 || true   # mata o nó
      kubectl delete pod c4w --grace-period=0 --force --ignore-not-found >/dev/null 2>&1
      kubectl run c4r --image=busybox --overrides='{"spec":{"volumes":[{"name":"d","persistentVolumeClaim":{"claimName":"c4pvc"}}],"containers":[{"name":"c4r","image":"busybox","command":["sh","-c","cat /d/mark; sleep 600"],"volumeMounts":[{"name":"d","mountPath":"/d"}]}]}}' >/dev/null 2>&1
      if wait_for 180 "reler do PV noutro nó" bash -c 'kubectl exec c4r -- cat /d/mark 2>/dev/null | grep -q DELONIX-CEPH-OK'; then
        mark C4 PASS "dados sobreviveram à perda do nó '$W_NODE' — PV Ceph re-anexado"
      else
        mark C4 FAIL "dados NÃO relidos após a perda do nó — PV não re-anexou (acoplamento CSI?)"
      fi
    else
      mark C4 FAIL "escrita inicial no PV Ceph falhou (provisionamento?)"
    fi
    kubectl delete pod c4r --ignore-not-found >/dev/null 2>&1; kubectl delete pvc c4pvc --ignore-not-found >/dev/null 2>&1
  fi
elif want C4; then
  mark C4 SKIP "C1 não passou"
fi

# ===========================================================================
# C5 — HA do control-plane: matar 1 CP → a API continua (etcd quorum)
# (só faz sentido com >=3 CP; corre num cluster PRÓPRIO)
# ===========================================================================
if want C5; then
  echo "== C5: HA control-plane (3 CP) =="
  HACL="$CLUSTER-ha"
  if ! $BIN cluster create --name "$HACL" --control-planes 3 --workers 0 2>&1 | clean | tail -2; then
    mark C5 FAIL "cluster HA (3 CP) não subiu"
  else
    $BIN cluster kubeconfig "$HACL" > "$KC" 2>/dev/null
    if wait_for 240 "3 CP Ready" nodes_ready 3; then
      CP=$(kubectl get nodes --no-headers 2>/dev/null | awk '/control/{print $1}' | head -1)
      $BIN container rm -f "$CP" >/dev/null 2>&1 || true   # mata 1 CP
      sleep 8
      if kubectl get nodes >/dev/null 2>&1; then
        mark C5 PASS "a API respondeu com 1 de 3 CP em baixo (etcd manteve quórum)"
      else
        mark C5 FAIL "a API caiu ao perder 1 de 3 CP (quórum etcd perdido?)"
      fi
    else
      mark C5 FAIL "os 3 CP não ficaram Ready"
    fi
    $BIN cluster destroy "$HACL" >/dev/null 2>&1 || true
  fi
fi

# --- relatório -------------------------------------------------------------
echo ""
echo "== RESULTADO DA CAMPANHA (multi-nó/Ceph/failover) =="
rc=0
for c in C1 C2 C3 C4 C5; do
  r="${RESULT[$c]:-—}"
  printf "  %-3s  %-5s  %s\n" "$c" "$r" "${NOTE[$c]:-}"
  [ "$r" = FAIL ] && rc=1
done
echo ""
echo "PASS = efeito verificado · SKIP = infra ausente (ex. Ceph) · FAIL = quebra real"
exit $rc
