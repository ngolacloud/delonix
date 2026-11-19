#!/usr/bin/env bash
# Delonix logging lab — dogfooding edition.
#
# Brings up three log-ingestion stacks — Loki+Grafana, Graylog (MongoDB +
# OpenSearch), Elasticsearch+Kibana — each AS A DELONIX-MANAGED CONTAINER, plus
# one Vector collector that tails the runtime's own log files and fans every
# line out to all three at once. Then it creates a demo container and (if a VM
# backend is present) a demo VM with serial capture, so there is real traffic
# from more than one kind of resource.
#
# Proves the boundary in docs/ARCHITECTURE-SCOPE.md end to end: the runtime
# EMITS structured logs (files), an external collector SHIPS them. No GELF/Loki/
# ES client lives in the engine.
#
# Components share the host network namespace (the rootless default), so every
# service binds a host port and `localhost` wiring just works. Ports are chosen
# to not collide with the observability (OTel) lab (3000/9091/16686/4318).
set -euo pipefail

LAB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DELONIX="${DELONIX:-delonix}"
ROOT="${DELONIX_ROOT:-$HOME/.local/share/delonix}"

LOKI_IMG="${LOKI_IMG:-docker.io/grafana/loki:3.1.1}"
GRAFANA_IMG="${GRAFANA_IMG:-docker.io/grafana/grafana:11.2.0}"
VECTOR_IMG="${VECTOR_IMG:-docker.io/timberio/vector:0.41.1-debian}"
MONGO_IMG="${MONGO_IMG:-docker.io/library/mongo:6.0}"
OPENSEARCH_IMG="${OPENSEARCH_IMG:-docker.io/opensearchproject/opensearch:2.15.0}"
GRAYLOG_IMG="${GRAYLOG_IMG:-docker.io/graylog/graylog:6.1}"
ES_IMG="${ES_IMG:-docker.elastic.co/elasticsearch/elasticsearch:8.15.1}"
KIBANA_IMG="${KIBANA_IMG:-docker.elastic.co/kibana/kibana:8.15.1}"

VM_NAME="log-demo-vm"
CT_NAME="log-demo-web"

echo "==> cleaning any previous lab containers"
for c in log-vector log-grafana log-loki log-graylog log-opensearch log-mongo \
         log-kibana log-elasticsearch "$CT_NAME"; do
  "$DELONIX" rm -f "$c" >/dev/null 2>&1 || true
done

echo "==> pulling images (first run only)"
for img in "$LOKI_IMG" "$GRAFANA_IMG" "$VECTOR_IMG" "$MONGO_IMG" \
           "$OPENSEARCH_IMG" "$GRAYLOG_IMG" "$ES_IMG" "$KIBANA_IMG"; do
  "$DELONIX" pull "$img" || { echo "pull failed: $img"; exit 1; }
done

# ---------------------------------------------------------------------------
# 1. Loki + Grafana
# ---------------------------------------------------------------------------
echo "==> loki (push+query :3100)"
"$DELONIX" run -d --name log-loki "$LOKI_IMG" \
  -config.file=/etc/loki/local-config.yaml

echo "==> grafana (:3001, anonymous admin, Loki + ES datasources)"
"$DELONIX" run -d --name log-grafana \
  -e GF_SERVER_HTTP_PORT=3001 \
  -e GF_SECURITY_ADMIN_PASSWORD=admin \
  -e GF_AUTH_ANONYMOUS_ENABLED=true \
  -e GF_AUTH_ANONYMOUS_ORG_ROLE=Admin \
  -v "$LAB/grafana/provisioning:/etc/grafana/provisioning:ro" \
  "$GRAFANA_IMG"

# ---------------------------------------------------------------------------
# 2. Elasticsearch + Kibana
# ---------------------------------------------------------------------------
echo "==> elasticsearch (:9200, security off, 512m heap, no disk watermark)"
"$DELONIX" run -d --name log-elasticsearch \
  -e discovery.type=single-node \
  -e xpack.security.enabled=false \
  -e "ES_JAVA_OPTS=-Xms512m -Xmx512m" \
  -e cluster.routing.allocation.disk.threshold_enabled=false \
  "$ES_IMG"

echo "==> kibana (:5601)"
"$DELONIX" run -d --name log-kibana \
  -e ELASTICSEARCH_HOSTS=http://localhost:9200 \
  -e SERVER_HOST=0.0.0.0 \
  "$KIBANA_IMG"

# ---------------------------------------------------------------------------
# 3. Graylog (MongoDB + OpenSearch + Graylog)
# ---------------------------------------------------------------------------
echo "==> mongodb (:27017, graylog metadata)"
"$DELONIX" run -d --name log-mongo "$MONGO_IMG"

echo "==> opensearch (:9201, graylog message store, 512m heap)"
"$DELONIX" run -d --name log-opensearch \
  -e discovery.type=single-node \
  -e plugins.security.disabled=true \
  -e http.port=9201 \
  -e "OPENSEARCH_JAVA_OPTS=-Xms512m -Xmx512m" \
  -e cluster.routing.allocation.disk.threshold_enabled=false \
  -e "OPENSEARCH_INITIAL_ADMIN_PASSWORD=Delonix.Lab.2026" \
  "$OPENSEARCH_IMG"

# admin / admin (sha256 of "admin")
PW_SHA2="8c6976e5b5410415bde908bd4dee15dfb167a9c873fc4bb8a81f6f2ab448a918"
echo "==> graylog (web :9000, GELF-TCP :12201)"
"$DELONIX" run -d --name log-graylog \
  -e GRAYLOG_PASSWORD_SECRET=delonixlablogshipsecret01 \
  -e "GRAYLOG_ROOT_PASSWORD_SHA2=$PW_SHA2" \
  -e GRAYLOG_HTTP_EXTERNAL_URI=http://localhost:9000/ \
  -e GRAYLOG_MONGODB_URI=mongodb://localhost:27017/graylog \
  -e GRAYLOG_ELASTICSEARCH_HOSTS=http://localhost:9201 \
  "$GRAYLOG_IMG"

echo "==> waiting for Graylog API (up to 180s)"
for i in $(seq 1 60); do
  if curl -sf -u admin:admin http://localhost:9000/api/system/cluster/nodes >/dev/null 2>&1; then
    break
  fi
  sleep 3
done

echo "==> creating the GELF TCP input in Graylog (idempotent)"
curl -s -u admin:admin -H 'Content-Type: application/json' -H 'X-Requested-By: delonix-lab' \
  -X POST http://localhost:9000/api/system/inputs \
  -d '{"title":"delonix-gelf-tcp","type":"org.graylog2.inputs.gelf.tcp.GELFTCPInput","global":true,"configuration":{"bind_address":"0.0.0.0","port":12201,"recv_buffer_size":1048576}}' \
  >/dev/null 2>&1 || echo "   (input may already exist — fine)"
sleep 5

# ---------------------------------------------------------------------------
# 4. Demo workloads (real traffic from two resource kinds)
# ---------------------------------------------------------------------------
echo "==> demo CONTAINER '$CT_NAME' (--log-cri: per-line ts+stream)"
BUSYBOX_IMG="${BUSYBOX_IMG:-docker.io/library/busybox:latest}"
"$DELONIX" pull "$BUSYBOX_IMG" >/dev/null 2>&1 || true
"$DELONIX" run -d --name "$CT_NAME" --log-cri "$BUSYBOX_IMG" \
  sh -c 'i=0; while true; do echo "delonix demo container line $i $(date -Iseconds)"; i=$((i+1)); sleep 2; done'

VM_UP=0
if "$DELONIX" vm image ls 2>/dev/null | grep -q .; then
  # cloud-hypervisor runs rootless, so the serial file is owned by this user and
  # both `vm logs` and the collector can read it. A system-libvirt backend would
  # write it as root 0600 (unreadable rootless) — see `delonix vm logs` hint.
  echo "==> demo VM '$VM_NAME' (--capture-serial, rootless cloud-hypervisor)"
  if "$DELONIX" vm create "$VM_NAME" --backend cloud-hypervisor --capture-serial >/dev/null 2>&1; then
    VM_UP=1
  else
    echo "   (vm create failed — skipping the VM leg; container leg still proves the pipe)"
  fi
else
  echo "==> no VM image present — skipping the VM leg (run: delonix vm image pull)"
fi

# ---------------------------------------------------------------------------
# 5. Enrichment table (container id -> name/image) then the collector
# ---------------------------------------------------------------------------
echo "==> building enrichment table (id,name,image)"
mkdir -p "$LAB/enrichment"
{
  echo "id,name,image"
  for j in "$ROOT"/containers/*.json; do
    [ -e "$j" ] || continue
    python3 - "$j" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
print(f'{d.get("id","")},{d.get("name","")},{d.get("image","")}')
PY
  done
} > "$LAB/enrichment/containers.csv"

echo "==> vector collector (tails $ROOT, ships to all three sinks)"
# `--` separates delonix's own flags from the container's argv: without it
# `--config` is captured by `delonix run`'s own `--config` (config-object) flag.
"$DELONIX" run -d --name log-vector \
  -v "$ROOT:/delonix:ro" \
  -v "$LAB/vector.yaml:/etc/vector/vector.yaml:ro" \
  -v "$LAB/enrichment:/enrichment:ro" \
  "$VECTOR_IMG" -- --config /etc/vector/vector.yaml

cat <<EOF

================================================================================
 Delonix logging lab is up. One collector, three sinks, two resource kinds.

   Grafana (Loki)        http://localhost:3001   (Explore -> Loki -> {kind=~".+"})
   Kibana (Elasticsearch) http://localhost:5601  (data view: delonix-logs-*)
   Graylog               http://localhost:9000   (admin / admin -> Search)

 Verify from the shell (after ~30s of ingestion):
   Loki:  curl -sG localhost:3100/loki/api/v1/query_range \\
            --data-urlencode 'query={kind="container"}' | head -c 400
   ES:    curl -s 'localhost:9200/delonix-logs-*/_search?q=delonix_kind:container&size=1'
   Graylog: open the Search page; filter  delonix_kind:container  or  delonix_kind:vm

 Demo resources:  container '$CT_NAME'$( [ "$VM_UP" = 1 ] && echo ", VM '$VM_NAME'" )
 Tear down:       $LAB/stop.sh
================================================================================
EOF
