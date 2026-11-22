#!/usr/bin/env bash
# Delonix observability lab — dogfooding edition.
#
# Brings up a full Prometheus + Grafana + OpenTelemetry (collector + Jaeger)
# stack, each component running AS A DELONIX-MANAGED CONTAINER, scraping the
# Delonix runtime's own telemetry. Proves the three observability legs end to
# end: structured logs, Prometheus metrics, OTLP traces.
#
# Requires: delonix on PATH, a delonix-mgmt server started with
#   DELONIX_MGMT_METRICS_ADDR=0.0.0.0:9199  (TCP /metrics, full registry)
#   DELONIX_OTLP_ENDPOINT=http://127.0.0.1:4318  (spans -> collector)
#
# The components share the host network namespace (the rootless default), so
# every service binds a host port directly and `localhost` wiring just works.
set -euo pipefail

LAB="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DELONIX="${DELONIX:-delonix}"

PROM_IMG="${PROM_IMG:-docker.io/prom/prometheus:latest}"
GRAFANA_IMG="${GRAFANA_IMG:-docker.io/grafana/grafana:latest}"
OTELCOL_IMG="${OTELCOL_IMG:-docker.io/otel/opentelemetry-collector:latest}"
JAEGER_IMG="${JAEGER_IMG:-docker.io/jaegertracing/all-in-one:latest}"

echo "==> pulling images"
for img in "$JAEGER_IMG" "$OTELCOL_IMG" "$PROM_IMG" "$GRAFANA_IMG"; do
  "$DELONIX" pull "$img"
done

echo "==> jaeger (OTLP intake :14317, UI :16686)"
"$DELONIX" run -d --name obs-jaeger \
  -e COLLECTOR_OTLP_ENABLED=true \
  "$JAEGER_IMG" \
  --collector.otlp.grpc.host-port=0.0.0.0:14317 \
  --collector.otlp.http.host-port=0.0.0.0:14318 \
  --query.http-server.host-port=0.0.0.0:16686

echo "==> otel-collector (OTLP :4318 -> debug + jaeger)"
"$DELONIX" run -d --name obs-otelcol \
  -v "$LAB/otel-collector-config.yaml:/etc/otelcol/config.yaml:ro" \
  "$OTELCOL_IMG" --config=/etc/otelcol/config.yaml

echo "==> prometheus (:9091, scrapes delonix-mgmt :9199)"
"$DELONIX" run -d --name obs-prometheus \
  -v "$LAB/prometheus.yml:/etc/prometheus/prometheus.yml:ro" \
  "$PROM_IMG" \
  --config.file=/etc/prometheus/prometheus.yml \
  --web.listen-address=0.0.0.0:9091

echo "==> grafana (:3000, anonymous admin)"
"$DELONIX" run -d --name obs-grafana \
  -e GF_SECURITY_ADMIN_PASSWORD=admin \
  -e GF_AUTH_ANONYMOUS_ENABLED=true \
  -e GF_AUTH_ANONYMOUS_ORG_ROLE=Admin \
  -v "$LAB/grafana/provisioning:/etc/grafana/provisioning:ro" \
  -v "$LAB/grafana/dashboards:/var/lib/grafana/dashboards:ro" \
  "$GRAFANA_IMG"

cat <<EOF

==> lab up. URLs:
  Grafana     http://localhost:3000      (dashboard: Delonix / Delonix Runtime)
  Prometheus  http://localhost:9091      (targets: /targets)
  Jaeger      http://localhost:16686     (service: delonix-mgmt)

Generate traces by exercising the runtime with DELONIX_OTLP_ENDPOINT set
(the CRI/mgmt server emits cri.* spans). Tear down with ./stop.sh
EOF
