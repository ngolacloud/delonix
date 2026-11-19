#!/usr/bin/env bash
# Tear down the logging lab: every container it created, the demo workloads,
# and the generated enrichment table. Leaves the pulled images in place.
set -uo pipefail

LAB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DELONIX="${DELONIX:-delonix}"
VM_NAME="log-demo-vm"
CT_NAME="log-demo-web"

echo "==> removing lab containers"
for c in log-vector log-grafana log-loki log-graylog log-opensearch log-mongo \
         log-kibana log-elasticsearch "$CT_NAME"; do
  "$DELONIX" rm -f "$c" >/dev/null 2>&1 && echo "   removed $c" || true
done

echo "==> removing demo VM (if any)"
"$DELONIX" vm destroy "$VM_NAME" >/dev/null 2>&1 && echo "   removed VM $VM_NAME" || true

rm -f "$LAB/enrichment/containers.csv" 2>/dev/null || true
echo "==> done."
