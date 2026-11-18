#!/usr/bin/env bash
# probe-capabilities.sh — fornece as capacidades em falta (registo OCI, S3, cluster)
# e MEDE com efeito as folhas antes BLOCKED que passam a ser alcançáveis em rootless
# (Parte b da campanha 0.1.2). Cada folha corre com argumentos reais e o efeito é
# lido de forma independente. Perfil rootless; teardown no fim.
#
#   reports/e2e/0.1.2/probe-capabilities.sh [binario]
set -uo pipefail
BIN="${1:-/home/walter/workspaces/NgolaCloud/projects/delonix-runtime/target/release/delonix}"
ROOT="${CAP_ROOT:-/tmp/dlxcap}"; rm -rf "$ROOT" 2>/dev/null; mkdir -p "$ROOT/root" "$ROOT/w"
export DELONIX_ROOT="$ROOT/root"; export DELONIX_NET_RUNTIME_DIR="/tmp/dlxcap-n"
IMG="${CAP_IMG:-alpine:3.24}"; R=localhost:5000
P=0 F=0 S=0; RES="$ROOT/results.tsv"; : >"$RES"
rec(){ printf '%s\t%s\n' "$1" "$2" >>"$RES"; case $1 in PASS)P=$((P+1));; FAIL)F=$((F+1));; SKIP)S=$((S+1));; esac; printf '%-7s %s\n' "$1" "$2"; }
chk(){ local n="$1"; shift; if eval "$*" >"$ROOT/c" 2>&1; then rec PASS "$n"; else rec FAIL "$n ($(tail -1 "$ROOT/c"|cut -c1-60))"; fi; }

echo "### probe-capabilities $($BIN --version|head -1)"

# ===================== CAPACIDADE 1: registo OCI =====================
$BIN pull "$IMG" >/dev/null 2>&1
if $BIN run -d --name capreg -p 5000:5000 registry:2 >"$ROOT/c" 2>&1 && sleep 2 && curl -sf http://$R/v2/ >/dev/null 2>&1; then
  $BIN tag "$IMG" $R/alp:v1 >/dev/null 2>&1
  chk "push"               "$BIN push $R/alp:v1 && curl -sf http://$R/v2/alp/tags/list | grep -q v1"
  chk "image push"         "curl -sf http://$R/v2/alp/tags/list | grep -q v1"
  chk "pull"               "$BIN rmi $R/alp:v1 >/dev/null 2>&1; $BIN pull $R/alp:v1 && $BIN images | grep -q $R/alp"
  chk "login"              "echo testpw | $BIN login $R -u tester --password-stdin && grep -q '$R' $DELONIX_ROOT/auth.json"
  chk "logout"             "$BIN logout $R && ! grep -q '$R' $DELONIX_ROOT/auth.json 2>/dev/null"
  chk "search"             "$BIN search --limit 3 alpine 2>&1 | grep -qiE 'alpine|NAME|result'"
  chk "image sign"         "$BIN image sign $R/alp:v1 && ls $DELONIX_ROOT/keys/image-signing.key"
  chk "image attestations" "$BIN image attestations $R/alp:v1 2>&1 | grep -qiE 'attest|signature|none|sha256'"
  # provenance SÓ existe se a imagem foi construída com --provenance (grava os
  # inputs de build); medimos a construir uma e lendo-a de volta.
  printf 'FROM %s\nRUN echo prov>/p\n' "$IMG" > "$ROOT/w/Dockerfile"
  chk "image provenance"   "$BIN build --provenance -t provimg:v1 $ROOT/w && $BIN image provenance provimg:v1 2>&1 | grep -qiE 'provenance|slsa|build|input|materials|sha256'"
  chk "vm image ls-remote" "$BIN vm image ls-remote $R/alp >$ROOT/lsr 2>&1; grep -qiE 'repository|TAG|sig' $ROOT/lsr"
  chk "vm image pull"      "$BIN vm image pull $R/alp:v1 && $BIN vm image ls | grep -q alp"
  chk "vm image push"      "$BIN vm image push alp:v1 $R/vmalp:v1 && curl -sf http://$R/v2/_catalog | grep -q vmalp"
  $BIN rm -f capreg >/dev/null 2>&1
else
  for l in push "image push" pull login logout search "image sign" "image attestations" "image provenance" "vm image ls-remote" "vm image pull" "vm image push"; do rec SKIP "$l (registry:2 não subiu)"; done
fi

# ===================== CAPACIDADE 2: S3 / bucket =====================
chk "bucket ls" "$BIN bucket ls"   # lista (vazia) — funciona sem backend
# inspect/rm exigem um Bucket declarado sobre um S3 vivo (MinIO/cephRgw):
rec SKIP "bucket inspect (precisa de um Bucket external sobre um S3 vivo)"
rec SKIP "bucket rm (idem)"

# ===================== CAPACIDADE 3: cluster / kube =====================
$BIN pull "$IMG" >/dev/null 2>&1
printf 'apiVersion: v1\nkind: Pod\nmetadata: {name: capkp}\nspec:\n  containers:\n  - {name: c, image: %s, command: [sleep, "120"]}\n' "$IMG" > "$ROOT/w/kp.yaml"
chk "cluster kube play" "$BIN cluster kube play $ROOT/w/kp.yaml && $BIN pod ps | grep -q capkp"
$BIN pod rm -f capkp >/dev/null 2>&1
rec SKIP "cluster kubeadm (provisiona VMs + bootstrap — nó rootful, Parte c)"

# ===================== só-VM/CRIU/recipe (Parte c) =====================
rec SKIP "container checkpoint (CRIU — nó rootful, Parte c)"
rec SKIP "container restore (CRIU — Parte c)"
rec SKIP "container clone (depende de checkpoint/restore — Parte c)"
rec SKIP "vm image build (recipe/base de imagem de VM — não fornecida)"

printf 'TOTAIS\tPASS=%s\tFAIL=%s\tSKIP=%s\n' "$P" "$F" "$S" | tee -a "$RES"
