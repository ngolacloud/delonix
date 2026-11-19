#!/usr/bin/env bash
# Tear down the Delonix observability lab.
set -uo pipefail
DELONIX="${DELONIX:-delonix}"
for c in obs-grafana obs-prometheus obs-otelcol obs-jaeger; do
  "$DELONIX" rm -f "$c" 2>/dev/null || true
done
echo "lab torn down."
