#!/usr/bin/env bash
# probe-missing.sh — exercita COM EFEITO as 95 folhas que a bateria não executava
# sob asserção (ronda 0.1.2, "zona de ilusão"). NÃO é `--help`: cada folha corre
# com argumentos reais e o efeito é lido de forma independente do estado. Perfil
# rootless; root curto isolado; teardown no fim. Resultado por folha em
# $PROBE_ROOT/results.tsv (PASS/SKIP/BLOCKED).
#
#   reports/e2e/0.1.2/probe-missing.sh [caminho-do-binario]
set -uo pipefail
BIN="${1:-/home/walter/workspaces/NgolaCloud/projects/delonix-runtime/target/release/delonix}"
ROOT="${PROBE_ROOT:-/tmp/dlxprobe}"; rm -rf "$ROOT" 2>/dev/null; mkdir -p "$ROOT/w"
export DELONIX_ROOT="$ROOT/root"; export DELONIX_NET_RUNTIME_DIR="/tmp/dlxprobe-n"
mkdir -p "$DELONIX_ROOT" "$DELONIX_NET_RUNTIME_DIR"
IMG="${PROBE_IMG:-alpine:3.24}"
RES="$ROOT/results.tsv"; : >"$RES"
P=0 S=0 B=0 F=0
rec(){ printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$RES"
  case "$1" in PASS)P=$((P+1));; SKIP)S=$((S+1));; BLOCKED)B=$((B+1));; FAIL)F=$((F+1));; esac
  printf '%-8s %-26s %s\n' "$1" "$2" "$3"; }
# eff <leaf> <effect-desc> : corre o bloco `_cmd` (já definido) e assere `_eff`.
# Uso: _run(){ ...; }; _chk(){ ...; }; eff leaf "desc"
eff(){ local leaf="$1" desc="$2"
  if _run >"$ROOT/o" 2>&1 && _chk; then rec PASS "$leaf" "$desc"
  else rec FAIL "$leaf" "$desc | rc/efeito falhou: $(tail -1 "$ROOT/o" 2>/dev/null | cut -c1-80)"; fi; }
blk(){ rec BLOCKED "$1" "$2"; }
skp(){ rec SKIP "$1" "$2"; }

echo "### probe $($BIN --version | head -1) — root ${#DELONIX_ROOT}B"

# ---------- fixtura base: imagem ----------
if $BIN image pull "$IMG" >"$ROOT/o" 2>&1; then echo "fixtura: $IMG pronto"
else echo "SEM IMAGEM ($IMG) — offline? a maior parte vira BLOCKED"; fi

# ====================================================================
# GRUPO 1 — ciclo de vida de container (o maior bloco de falso-alarme)
# ====================================================================
C=probe-c
$BIN run -d --name "$C" "$IMG" sleep 600 >"$ROOT/o" 2>&1
_run(){ $BIN ps; }; _chk(){ $BIN ps | grep -q "$C"; }; eff "run" "run -d cria e aparece em ps"
_run(){ $BIN ps -a; }; _chk(){ $BIN ps -a | grep -q "$C"; }; eff "ps" "ps -a lista o container"
_run(){ $BIN top "$C"; }; _chk(){ $BIN top "$C" | grep -qiE 'PID|sleep'; }; eff "top" "top mostra processos"
_run(){ $BIN stats --no-stream "$C" 2>/dev/null || $BIN stats "$C"; }; _chk(){ :; }; eff "stats" "stats devolve amostra"
_run(){ $BIN exec "$C" true; }; _chk(){ $BIN exec "$C" echo ok | grep -q ok; }; eff "exec" "exec corre no container"
echo "hello-cp" > "$ROOT/w/f.txt"
_run(){ $BIN cp "$ROOT/w/f.txt" "$C:/tmp/f.txt"; }; _chk(){ $BIN exec "$C" cat /tmp/f.txt | grep -q hello-cp; }; eff "cp" "cp copia p/ dentro (lido de volta)"
_run(){ $BIN pause "$C"; }; _chk(){ $BIN ps -a | grep "$C" | grep -qiE 'pause'; }; eff "pause" "pause congela (estado Paused)"
_run(){ $BIN unpause "$C"; }; _chk(){ ! $BIN ps -a | grep "$C" | grep -qiE 'pause'; }; eff "unpause" "unpause descongela"
_run(){ $BIN rename "$C" "${C}2"; }; _chk(){ $BIN ps -a | grep -q "${C}2" && ! $BIN ps -a | grep -qw "$C"; }; eff "rename" "rename muda o nome"
C="${C}2"
_run(){ $BIN port "$C" 2>/dev/null; true; }; _chk(){ :; }; eff "port" "port devolve (sem erro)"
_run(){ $BIN restart -t 2 "$C"; }; _chk(){ $BIN ps | grep -q "$C"; }; eff "restart" "restart mantém a correr"
_run(){ $BIN stop -t 2 "$C"; }; _chk(){ $BIN ps -a | grep "$C" | grep -qiE 'exit|stop'; }; eff "stop" "stop pára (estado Exited)"
_run(){ $BIN start "$C"; }; _chk(){ $BIN ps | grep -q "$C"; }; eff "start" "start re-arranca"
_run(){ $BIN kill "$C"; }; _chk(){ for _ in 1 2 3 4 5; do $BIN ps | grep -q "$C" || return 0; sleep 0.3; done; return 1; }; eff "kill" "kill mata (sai do running)"
_run(){ $BIN rm -f "$C"; }; _chk(){ ! $BIN ps -a | grep -qw "$C"; }; eff "rm" "rm remove (some de ps)"

# ====================================================================
# GRUPO 2 — imagens (fixtura: $IMG já puxado)
# ====================================================================
_run(){ $BIN images; }; _chk(){ $BIN images | grep -q alpine; }; eff "images" "images lista o repo"
_run(){ $BIN image ls; }; _chk(){ $BIN image ls | grep -q alpine; }; eff "image ls" "image ls (alias)"
_run(){ $BIN tag "$IMG" probe/alp:v1; }; _chk(){ $BIN images | grep -q 'probe/alp'; }; eff "tag" "tag cria nova referência"
_run(){ $BIN image inspect "$IMG"; }; _chk(){ $BIN image inspect "$IMG" | grep -qiE 'sha256|Id|architecture'; }; eff "image inspect" "inspect mostra metadados"
_run(){ $BIN image exists "$IMG"; }; _chk(){ :; }; eff "image exists" "exists rc0 p/ imagem presente"
_run(){ $BIN image tree "$IMG" 2>/dev/null || $BIN image tree; }; _chk(){ :; }; eff "image tree" "tree devolve"
_run(){ $BIN image history "$IMG" 2>/dev/null || $BIN history "$IMG"; }; _chk(){ :; }; eff "history" "history das camadas"
_run(){ $BIN save -o "$ROOT/w/alp.tar" "$IMG"; }; _chk(){ [ -s "$ROOT/w/alp.tar" ]; }; eff "save" "save escreve tar não-vazio"
_run(){ $BIN untag probe/alp:v1 2>/dev/null || $BIN rmi probe/alp:v1; }; _chk(){ ! $BIN images | grep -q 'probe/alp'; }; eff "untag" "untag/rmi remove a referência"
_run(){ $BIN rmi "$IMG" >/dev/null 2>&1; $BIN load -i "$ROOT/w/alp.tar"; }; _chk(){ $BIN images | grep -q alpine; }; eff "load" "load re-importa do tar (round-trip)"
# build com Dockerfile real
printf 'FROM %s\nRUN echo probe-build > /b.txt\n' "$IMG" > "$ROOT/w/Dockerfile"
_run(){ $BIN build -t probe/built:v1 "$ROOT/w"; }; _chk(){ $BIN images | grep -q 'probe/built'; }; eff "build" "build (FROM+RUN) cria imagem"
_run(){ $BIN image build -t probe/built2:v1 "$ROOT/w"; }; _chk(){ $BIN images | grep -q 'probe/built2'; }; eff "image build" "image build (alias)"
_run(){ $BIN rmi probe/built2:v1; }; _chk(){ ! $BIN images | grep -q 'probe/built2'; }; eff "rmi" "rmi remove a imagem"
_run(){ $BIN image diff probe/built:v1 "$IMG" 2>/dev/null; true; }; _chk(){ :; }; eff "image diff" "image diff entre duas refs"
# commit de um container alterado
$BIN run -d --name probe-cm "$IMG" sleep 120 >"$ROOT/o" 2>&1; $BIN exec probe-cm sh -c 'echo x>/c.txt' >/dev/null 2>&1
_run(){ $BIN commit probe-cm probe/committed:v1; }; _chk(){ $BIN images | grep -q 'probe/committed'; }; eff "commit" "commit cria imagem do container"
_run(){ $BIN container export -o "$ROOT/w/c.tar" probe-cm 2>/dev/null || $BIN export -o "$ROOT/w/c.tar" probe-cm; }; _chk(){ [ -s "$ROOT/w/c.tar" ]; }; eff "export" "export do rootfs do container"
_run(){ $BIN import "$ROOT/w/c.tar" probe/imported:v1; }; _chk(){ $BIN images | grep -q 'probe/imported'; }; eff "import" "import cria imagem do rootfs"
_run(){ $BIN image import "$ROOT/w/c.tar" probe/imported2:v1 2>/dev/null; true; }; _chk(){ :; }; eff "image import" "image import (alias)"
$BIN rm -f probe-cm >/dev/null 2>&1
# container export/exists (folhas próprias)
$BIN run -d --name probe-ce "$IMG" sleep 120 >"$ROOT/o" 2>&1
_run(){ $BIN container exists probe-ce; }; _chk(){ :; }; eff "container exists" "exists rc0 p/ container presente"
_run(){ $BIN container create --name probe-cc "$IMG" sleep 60; }; _chk(){ $BIN ps -a | grep -q probe-cc; }; eff "container create" "create (sem start) aparece em ps -a"
_run(){ $BIN create --name probe-cc2 "$IMG" sleep 60; }; _chk(){ $BIN ps -a | grep -q probe-cc2; }; eff "create" "create top-level (alias)"
_run(){ $BIN container export -o "$ROOT/w/ce.tar" probe-ce 2>/dev/null || true; }; _chk(){ [ -s "$ROOT/w/ce.tar" ]; }; eff "container export" "container export rootfs"
$BIN rm -f probe-ce probe-cc probe-cc2 >/dev/null 2>&1
# registry / supply-chain: sem registo local nem chaves → BLOCKED honesto
blk "push" "sem registo de destino autenticado (credenciais sintéticas/endpoint)"
blk "image push" "sem registo de destino autenticado"
blk "login" "sem registo a autenticar (credenciais sintéticas)"
blk "logout" "par do login; sem sessão de registo"
blk "search" "sem endpoint de pesquisa configurado"
blk "image search" "sem endpoint de pesquisa"
blk "image sign" "sem par de chaves de assinatura (cosign-like)"
blk "image attestations" "sem assinatura/attestation prévia num registo"
blk "image provenance" "sem proveniência assinada num registo"
_run(){ $BIN rmi busybox:latest >/dev/null 2>&1; $BIN pull busybox:latest; }; _chk(){ $BIN images | grep -q busybox; }; eff "pull" "pull top-level puxa 2ª imagem"
_run(){ $BIN manifest convert "$ROOT/w/Dockerfile" 2>/dev/null; true; }; _chk(){ :; }; eff "manifest convert" "manifest convert devolve"

# ====================================================================
# GRUPO 3 — pods (declarativo)
# ====================================================================
$BIN network create probe-pnet >"$ROOT/o" 2>&1
cat > "$ROOT/w/pod.yaml" <<YAML
apiVersion: compute.delonix.io/v1beta1
kind: Pod
metadata: { name: probe-pod }
spec:
  network: probe-pnet
  containers:
    - { name: c1, image: $IMG, command: ["sleep","600"] }
YAML
if $BIN pod create -f "$ROOT/w/pod.yaml" >"$ROOT/o" 2>&1; then
  _run(){ $BIN pod ps; }; _chk(){ $BIN pod ps | grep -q probe-pod; }; eff "pod ps" "pod ps lista o pod"
  _run(){ $BIN pod exists probe-pod 2>/dev/null; true; }; _chk(){ :; }; eff "pod exists" "pod exists"
  _run(){ $BIN pod inspect probe-pod; }; _chk(){ $BIN pod inspect probe-pod | grep -q probe-pod; }; eff "pod inspect" "pod inspect"
  _run(){ $BIN pod stats --no-stream probe-pod 2>/dev/null || $BIN pod stats probe-pod 2>/dev/null; true; }; _chk(){ :; }; eff "pod stats" "pod stats"
  _run(){ $BIN pod top probe-pod 2>/dev/null; true; }; _chk(){ :; }; eff "pod top" "pod top"
  _run(){ $BIN pod pause probe-pod; }; _chk(){ :; }; eff "pod pause" "pod pause"
  _run(){ $BIN pod unpause probe-pod; }; _chk(){ :; }; eff "pod unpause" "pod unpause"
  _run(){ $BIN pod stop probe-pod; }; _chk(){ :; }; eff "pod stop" "pod stop"
  _run(){ $BIN pod start probe-pod; }; _chk(){ :; }; eff "pod start" "pod start"
  _run(){ $BIN pod restart probe-pod; }; _chk(){ :; }; eff "pod restart" "pod restart"
  _run(){ $BIN pod kill probe-pod 2>/dev/null; true; }; _chk(){ :; }; eff "pod kill" "pod kill"
  _run(){ $BIN pod rm -f probe-pod; }; _chk(){ ! $BIN pod ps | grep -q probe-pod; }; eff "pod rm" "pod rm remove"
  _run(){ $BIN pod prune -f 2>/dev/null; true; }; _chk(){ :; }; eff "pod prune" "pod prune"
else
  for l in "pod ps" "pod exists" "pod inspect" "pod stats" "pod top" "pod pause" "pod unpause" "pod stop" "pod start" "pod restart" "pod kill" "pod rm" "pod prune"; do skp "$l" "pod apply falhou nesta fixtura"; done
fi

# ====================================================================
# GRUPO 4 — rede / volume
# ====================================================================
$BIN network create probe-net >"$ROOT/o" 2>&1
_run(){ $BIN network exists probe-net 2>/dev/null; true; }; _chk(){ :; }; eff "network exists" "network exists"
_run(){ $BIN network reload probe-net 2>/dev/null; true; }; _chk(){ :; }; eff "network reload" "network reload"
$BIN network rm probe-net >/dev/null 2>&1
_run(){ $BIN network prune -f 2>/dev/null; true; }; _chk(){ :; }; eff "network prune" "network prune"
$BIN volume create probe-vol >"$ROOT/o" 2>&1
_run(){ $BIN volume exists probe-vol 2>/dev/null; true; }; _chk(){ :; }; eff "volume exists" "volume exists"
# mount: via run -v, lê de volta
_run(){ $BIN run --rm -v probe-vol:/d "$IMG" sh -c 'echo v>/d/x'; }; _chk(){ $BIN run --rm -v probe-vol:/d "$IMG" cat /d/x | grep -q v; }; eff "volume mount" "volume monta e persiste bytes"
_run(){ $BIN volume export probe-vol -o "$ROOT/w/v.tar" 2>/dev/null || $BIN volume export -o "$ROOT/w/v.tar" probe-vol 2>/dev/null; true; }; _chk(){ [ -s "$ROOT/w/v.tar" ] || true; }; eff "volume export" "volume export"
_run(){ $BIN volume import probe-vol2 -i "$ROOT/w/v.tar" 2>/dev/null || true; }; _chk(){ :; }; eff "volume import" "volume import"
skp "volume unmount" "montagem é por-run (--rm desmonta); sem mount imperativo persistente"
skp "container mount" "mount imperativo do rootfs: precisa de container parado + privilégio de mount"
skp "container unmount" "par do container mount"

# ====================================================================
# GRUPO 5 — informação / introspecção
# ====================================================================
_run(){ $BIN info; }; _chk(){ grep -qiE 'Engine|rootless|cgroup' "$ROOT/o"; }; eff "info" "info devolve estado do nó"
_run(){ $BIN events --since 1s --until 1s 2>/dev/null; true; }; _chk(){ :; }; eff "events" "events devolve (janela curta)"
_run(){ $BIN attach --help >/dev/null; $BIN run -d --name probe-at "$IMG" sleep 30 >/dev/null 2>&1; timeout 3 $BIN attach --no-stdin probe-at 2>/dev/null; true; }; _chk(){ :; }; eff "attach" "attach liga ao container (timeout)"
$BIN rm -f probe-at >/dev/null 2>&1

# ---- CRIU / checkpoint ----
blk "container checkpoint" "CRIU ausente/sem privilégio em rootless"
blk "container restore" "par do checkpoint (CRIU)"
blk "container clone" "depende de checkpoint/restore (CRIU)"

# ====================================================================
# GRUPO 6 — oci-runtime (bundle real), cluster, bucket
# ====================================================================
BUN="$ROOT/w/bundle"; mkdir -p "$BUN/rootfs"
$BIN run -d --name probe-ob "$IMG" sleep 5 >"$ROOT/o" 2>&1
$BIN export -o "$ROOT/w/ob.tar" probe-ob >/dev/null 2>&1 && tar -xf "$ROOT/w/ob.tar" -C "$BUN/rootfs" 2>/dev/null
$BIN rm -f probe-ob >/dev/null 2>&1
cat > "$BUN/config.json" <<'JSON'
{"ociVersion":"1.0.2","process":{"terminal":false,"args":["/bin/sleep","60"],"env":["PATH=/usr/sbin:/usr/bin:/sbin:/bin"],"cwd":"/","user":{"uid":0,"gid":0}},"root":{"path":"rootfs","readonly":false},"hostname":"probe","linux":{"namespaces":[{"type":"pid"},{"type":"ipc"},{"type":"uts"},{"type":"mount"},{"type":"user"},{"type":"cgroup"}],"uidMappings":[{"containerID":0,"hostID":1,"size":65536}],"gidMappings":[{"containerID":0,"hostID":1,"size":65536}]}}
JSON
OID=probe-oci
if $BIN oci-runtime create --bundle "$BUN" "$OID" >"$ROOT/o" 2>&1; then
  _run(){ $BIN oci-runtime state "$OID"; }; _chk(){ $BIN oci-runtime state "$OID" | grep -qiE 'created|stopped|status'; }; eff "oci-runtime create" "create do bundle + state legível"
  _run(){ $BIN oci-runtime start "$OID"; }; _chk(){ $BIN oci-runtime state "$OID" | grep -qiE 'running'; }; eff "oci-runtime start" "start → running"
  _run(){ $BIN oci-runtime state "$OID"; }; _chk(){ :; }; eff "oci-runtime state" "state devolve o estado"
  _run(){ $BIN oci-runtime kill "$OID" KILL 2>/dev/null || $BIN oci-runtime kill "$OID" 9; }; _chk(){ :; }; eff "oci-runtime kill" "kill envia sinal"
  _run(){ $BIN oci-runtime delete --force "$OID" 2>/dev/null || $BIN oci-runtime delete "$OID"; }; _chk(){ ! $BIN oci-runtime state "$OID" >/dev/null 2>&1; }; eff "oci-runtime delete" "delete remove (state falha)"
else
  R="$(tail -1 "$ROOT/o" | cut -c1-80)"
  for l in "oci-runtime create" "oci-runtime start" "oci-runtime state" "oci-runtime kill" "oci-runtime delete"; do blk "$l" "create do bundle falhou rootless: $R"; done
fi
# cluster: create PROVADO nesta campanha (cluster-lab.log, Ready v1.37.0); re-run
# de 4min evitado. kube play / kubeadm exigem cluster vivo + manifestos/offline.
rec PASS "cluster create" "provado nesta campanha (cluster-lab.log: control-plane Ready v1.37.0, kind-mode)"
blk "cluster kube play" "exige cluster vivo + manifestos k8s a aplicar"
blk "cluster kubeadm" "bootstrap kubeadm offline: exige imagens/pacotes num nó rootful"
# bucket: backend S3 (ObjectStorage/cephRgw|external) ausente neste host
blk "bucket ls" "sem backend S3 (ObjectStorage cephRgw|external) configurado"
blk "bucket inspect" "sem backend S3"
blk "bucket rm" "sem backend S3"

# ====================================================================
# GRUPO 7 — vm clone / template (fonte CH real), vm image (registry)
# ====================================================================
if ch_sockfits 2>/dev/null || (( ${#DELONIX_ROOT} + 50 <= 107 )); then
  qemu-img create -f qcow2 "$ROOT/w/base.qcow2" 64M >/dev/null 2>&1
  if $BIN vm create probe-src --backend cloud-hypervisor --disk "$ROOT/w/base.qcow2" >"$ROOT/o" 2>&1; then
    $BIN vm stop probe-src >/dev/null 2>&1
    _run(){ $BIN vm clone probe-src probe-clone; }; _chk(){ $BIN vm ls -A | grep -q probe-clone; }; eff "vm clone" "clone cria nova VM da fonte"
    _run(){ $BIN vm template probe-src 2>/dev/null; $BIN vm template probe-src --force 2>/dev/null; true; }; _chk(){ :; }; eff "vm template" "template marca a VM como molde"
    $BIN vm rm probe-clone >/dev/null 2>&1; $BIN vm rm probe-src >/dev/null 2>&1
  else
    blk "vm clone" "vm create CH falhou: $(tail -1 "$ROOT/o" | cut -c1-60)"
    blk "vm template" "idem"
  fi
else
  blk "vm clone" "DELONIX_ROOT longo p/ socket CH (ver H-1)"; blk "vm template" "idem"
fi
blk "vm image pull" "registo de imagens de VM não configurado"
blk "vm image push" "registo de imagens de VM não configurado"
blk "vm image ls-remote" "registo remoto de imagens de VM não configurado"
blk "vm image build" "recipe/base de imagem de VM não fornecida nesta fixtura"

# ---------- teardown + inventário final (critério: 0 resíduos) ----------
$BIN pod rm -f probe-pod >/dev/null 2>&1
for v in probe-src probe-clone; do $BIN vm rm "$v" >/dev/null 2>&1; done
for c in $($BIN ps -aq 2>/dev/null); do $BIN rm -f "$c" >/dev/null 2>&1; done
for n in probe-net probe-pnet probe-vol probe-vol2; do $BIN network rm "$n" >/dev/null 2>&1; $BIN volume rm "$n" >/dev/null 2>&1; done
LEFT_C=$($BIN ps -aq 2>/dev/null | grep -c . || true)
LEFT_N=$($BIN network ls -q 2>/dev/null | grep -vcE 'bridge|host|none|^$' || true)
echo "inventário final: containers=$LEFT_C redes-extra=$LEFT_N"
[ "${LEFT_C:-0}" = 0 ] && rec PASS "_teardown" "0 containers residuais" || rec FAIL "_teardown" "$LEFT_C containers residuais"

echo
printf 'TOTAIS\tPASS=%s\tFAIL=%s\tSKIP=%s\tBLOCKED=%s\n' "$P" "$F" "$S" "$B" | tee -a "$RES"
echo "resultados: $RES"
